//! Native Keychain only. No CLI, password files, fallback or password serialization.
use super::{failure, Result};
pub struct Secret(pub(super) Vec<u8>);
impl Secret {
    pub fn random() -> Self {
        use rand::RngCore;
        let mut bytes = [0; 32];
        rand::rng().fill_bytes(&mut bytes);
        let s: Vec<u8> = bytes
            .iter()
            .flat_map(|b| format!("{b:02x}").into_bytes())
            .collect();
        Self(s)
    }
}
impl Drop for Secret {
    fn drop(&mut self) {
        for byte in &mut self.0 {
            unsafe { std::ptr::write_volatile(byte, 0) };
        }
        std::sync::atomic::compiler_fence(std::sync::atomic::Ordering::SeqCst);
    }
}
pub trait Vault: Send + Sync {
    fn create(&self, id: &str, secret: &Secret) -> Result<()>;
    fn read(&self, id: &str) -> Result<Secret>;
}
pub struct Keychain;
impl Keychain {
    fn query(
        id: &str,
    ) -> core_foundation::dictionary::CFMutableDictionary<
        core_foundation::base::CFType,
        core_foundation::base::CFType,
    > {
        use core_foundation::{base::TCFType, string::CFString};
        use security_framework_sys::item::*;
        unsafe extern "C" {
            static kSecUseAuthenticationUIFail: core_foundation::string::CFStringRef;
        }
        unsafe {
            core_foundation::dictionary::CFMutableDictionary::from_CFType_pairs(&[
                (
                    CFString::wrap_under_get_rule(kSecClass).as_CFType(),
                    CFString::wrap_under_get_rule(kSecClassGenericPassword).as_CFType(),
                ),
                (
                    CFString::wrap_under_get_rule(kSecAttrService).as_CFType(),
                    CFString::new("dev.fairway.managed-postgresql.v1").as_CFType(),
                ),
                (
                    CFString::wrap_under_get_rule(kSecAttrAccount).as_CFType(),
                    CFString::new(id).as_CFType(),
                ),
                (
                    CFString::wrap_under_get_rule(kSecUseAuthenticationUI).as_CFType(),
                    CFString::wrap_under_get_rule(kSecUseAuthenticationUIFail).as_CFType(),
                ),
            ])
        }
    }
}
impl Vault for Keychain {
    fn create(&self, id: &str, secret: &Secret) -> Result<()> {
        use core_foundation::{base::TCFType, data::CFData, string::CFString};
        use security_framework_sys::{item::kSecValueData, keychain_item::SecItemAdd};
        let mut query = Self::query(id);
        unsafe {
            query.set(
                CFString::wrap_under_get_rule(kSecValueData).as_CFType(),
                CFData::from_buffer(&secret.0).as_CFType(),
            );
        }
        let code = unsafe { SecItemAdd(query.as_concrete_TypeRef(), std::ptr::null_mut()) };
        if code != 0 {
            return Err(failure("Secure Keychain credential creation unavailable; initialization was not authorized to continue"));
        }
        Ok(())
    }
    fn read(&self, id: &str) -> Result<Secret> {
        use core_foundation::{base::TCFType, boolean::CFBoolean, data::CFData, string::CFString};
        use security_framework_sys::{item::kSecReturnData, keychain_item::SecItemCopyMatching};
        let mut query = Self::query(id);
        unsafe {
            query.set(
                CFString::wrap_under_get_rule(kSecReturnData).as_CFType(),
                CFBoolean::true_value().as_CFType(),
            );
        }
        let mut result = std::ptr::null();
        let code = unsafe { SecItemCopyMatching(query.as_concrete_TypeRef(), &mut result) };
        if code != 0 || result.is_null() {
            return Err(failure(
                "Secure Keychain credential unavailable; no plaintext fallback is permitted",
            ));
        }
        let data = unsafe { CFData::wrap_under_create_rule(result.cast()) };
        Ok(Secret(data.bytes().to_vec()))
    }
}
