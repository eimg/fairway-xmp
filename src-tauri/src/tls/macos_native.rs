//! Opt-in native trust experiment. Keep the login keychain and user/SSL scope.
use crate::config::Result;
use core_foundation::{
    base::{CFType, TCFType},
    dictionary::CFDictionary,
    number::CFNumber,
    string::CFString,
};
use security_framework::{
    base::Error,
    certificate::SecCertificate,
    item::{AddRef, ItemAddOptions, ItemAddValue, Location},
    os::macos::keychain::SecKeychain,
    policy::SecPolicy,
    secure_transport::SslProtocolSide,
};
use security_framework_sys::{
    base::{errSecDuplicateItem, errSecSuccess},
    trust_settings::{
        kSecTrustSettingsDomainUser, kSecTrustSettingsResultTrustRoot,
        SecTrustSettingsSetTrustSettings,
    },
};
use std::{fs, path::Path};

const USER_CANCELLED: i32 = -128;
const AUTHORIZATION_CANCELLED: i32 = -60006;

fn trust_setting_keys() -> (CFString, CFString) {
    // SecTrustSettings.h defines these keys as CFSTR macros, not exported symbols.
    (
        CFString::from_static_string("kSecTrustSettingsPolicy"),
        CFString::from_static_string("kSecTrustSettingsResult"),
    )
}

fn certificate_from_pem(bytes: &[u8]) -> Result<SecCertificate> {
    let pem =
        pem::parse(bytes).map_err(|e| format!("Cannot read the local CA certificate: {e}"))?;
    if pem.tag() != "CERTIFICATE" {
        return Err("The local CA file is not a certificate. Its contents were preserved.".into());
    }
    let certificate = SecCertificate::from_der(pem.contents())
        .map_err(|e| format!("Cannot decode the local CA certificate: {e}"))?;
    if certificate.subject_summary() != super::CA_NAME {
        return Err(
            "The local CA certificate has an unexpected name. Its contents were preserved.".into(),
        );
    }
    Ok(certificate)
}

fn ssl_root_settings() -> CFDictionary<CFString, CFType> {
    let policy = SecPolicy::create_ssl(SslProtocolSide::SERVER, None);
    let (policy_key, result_key) = trust_setting_keys();
    CFDictionary::from_CFType_pairs(&[
        (policy_key, policy.as_CFType()),
        (
            result_key,
            CFNumber::from(kSecTrustSettingsResultTrustRoot as i32).as_CFType(),
        ),
    ])
}

fn native_error(stage: &str, error: Error) -> String {
    if error.code() == USER_CANCELLED || error.code() == AUTHORIZATION_CANCELLED {
        format!("Certificate setup was cancelled while {stage}. You can retry HTTPS setup.")
    } else {
        format!("Could not {stage}: {error} (macOS error {}).", error.code())
    }
}

pub(super) fn install(root: &Path, keychain: &Path) -> Result<()> {
    // Decode the actual PEM used by verification, not a potentially stale DER sidecar.
    let certificate = certificate_from_pem(&fs::read(root).map_err(|e| e.to_string())?)?;
    let keychain =
        SecKeychain::open(keychain).map_err(|e| native_error("open your login keychain", e))?;
    let mut add = ItemAddOptions::new(ItemAddValue::Ref(AddRef::Certificate(certificate.clone())));
    add.set_location(Location::FileKeychain(keychain));
    if let Err(error) = add.add() {
        // A prior cancelled trust approval may have already inserted this public certificate.
        if error.code() != errSecDuplicateItem {
            return Err(native_error(
                "add the local CA to your login keychain",
                error,
            ));
        }
    }
    let settings = ssl_root_settings();
    // SAFETY: the certificate and retained CF dictionary remain alive for the call.
    // Only SSL server trust in the current user's domain is requested. This may
    // display Apple's authorization UI; the caller already runs off the UI thread.
    let status = unsafe {
        SecTrustSettingsSetTrustSettings(
            certificate.as_concrete_TypeRef(),
            kSecTrustSettingsDomainUser,
            settings.as_CFTypeRef(),
        )
    };
    if status != errSecSuccess {
        return Err(native_error(
            "approve SSL trust for the local CA",
            Error::from_code(status),
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use rcgen::{CertifiedIssuer, KeyPair};

    #[test]
    fn native_trust_uses_the_supplied_ca_and_rejects_other_material() {
        let key = KeyPair::generate().unwrap();
        let ca = CertifiedIssuer::self_signed(super::super::ca_params(), &key).unwrap();
        let parsed = certificate_from_pem(ca.pem().as_bytes()).unwrap();
        assert_eq!(parsed.to_der(), ca.der().as_ref());
        assert!(certificate_from_pem(key.serialize_pem().as_bytes()).is_err());
        assert!(certificate_from_pem(b"not a certificate").is_err());
        let other =
            CertifiedIssuer::self_signed(rcgen::CertificateParams::default(), &key).unwrap();
        assert!(certificate_from_pem(other.pem().as_bytes()).is_err());
    }

    #[test]
    fn native_trust_has_explicit_ssl_policy_and_root_result() {
        let settings = ssl_root_settings();
        let (policy_key, result_key) = trust_setting_keys();
        assert_eq!(settings.len(), 2);
        assert!(settings.get(policy_key).downcast::<SecPolicy>().is_some());
        let result = settings.get(result_key).downcast::<CFNumber>().unwrap();
        assert_eq!(
            result.to_i32(),
            Some(kSecTrustSettingsResultTrustRoot as i32)
        );
    }

    #[test]
    fn native_errors_preserve_cancellation_and_failure_context() {
        assert!(
            native_error("approving trust", Error::from_code(USER_CANCELLED)).contains("cancelled")
        );
        assert!(
            native_error("approving trust", Error::from_code(AUTHORIZATION_CANCELLED))
                .contains("cancelled")
        );
        let error = native_error("open your login keychain", Error::from_code(-25293));
        assert!(error.contains("open your login keychain") && error.contains("-25293"));
    }
}
