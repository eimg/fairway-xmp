//! Local HTTPS material for the optional project sites.
use crate::config::{atomic_write, private_file, valid_project_name, Layout, Result};
use rcgen::{
    BasicConstraints, CertificateParams, CertifiedIssuer, DistinguishedName, DnType,
    ExtendedKeyUsagePurpose, GeneralSubtree, IsCa, Issuer, KeyPair, KeyUsagePurpose,
    NameConstraints,
};
use std::{collections::BTreeSet, fs, path::PathBuf, process::Command, time::Duration};
use time::OffsetDateTime;

pub const HTTPS_PORT: u16 = 8443;

pub fn https_port() -> u16 {
    #[cfg(debug_assertions)]
    if let Ok(port) = std::env::var("FAIRWAY_DEV_HTTPS_PORT") {
        if let Ok(port) = port.parse::<u16>() {
            if port >= 1024 {
                return port;
            }
        }
    }
    HTTPS_PORT
}

fn directory(layout: &Layout) -> PathBuf {
    layout.data.join("tls")
}

pub fn certificate(layout: &Layout) -> PathBuf {
    directory(layout).join("projects.pem")
}

pub fn private_key(layout: &Layout) -> PathBuf {
    directory(layout).join("projects-key.pem")
}

fn root_certificate(layout: &Layout) -> PathBuf {
    directory(layout).join("root-ca.pem")
}

fn root_key(layout: &Layout) -> PathBuf {
    directory(layout).join("root-ca-key.pem")
}

fn trust_marker(layout: &Layout) -> PathBuf {
    directory(layout).join("trusted")
}

fn state_path(layout: &Layout) -> PathBuf {
    layout.data.join("https-projects.json")
}

pub fn enabled_names(layout: &Layout) -> Result<BTreeSet<String>> {
    let names: BTreeSet<String> = match fs::read(state_path(layout)) {
        Ok(bytes) => serde_json::from_slice(&bytes).map_err(|e| e.to_string())?,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => BTreeSet::new(),
        Err(error) => return Err(error.to_string()),
    };
    if names.iter().any(|name| !valid_project_name(name)) {
        return Err("An HTTPS project name is invalid. Check https-projects.json.".into());
    }
    Ok(names)
}

pub fn save_enabled_names(layout: &Layout, names: &BTreeSet<String>) -> Result<()> {
    atomic_write(
        &state_path(layout),
        &serde_json::to_vec_pretty(names).map_err(|e| e.to_string())?,
    )
}

fn ca_params() -> CertificateParams {
    let mut params = CertificateParams::default();
    params.distinguished_name = DistinguishedName::new();
    params
        .distinguished_name
        .push(DnType::CommonName, "Fairway XMP Local CA");
    params.is_ca = IsCa::Ca(BasicConstraints::Unconstrained);
    params.key_usages = vec![KeyUsagePurpose::KeyCertSign, KeyUsagePurpose::CrlSign];
    params.name_constraints = Some(NameConstraints {
        permitted_subtrees: vec![GeneralSubtree::DnsName(".test".into())],
        excluded_subtrees: Vec::new(),
    });
    params.not_before = OffsetDateTime::now_utc() - time::Duration::days(1);
    params.not_after = OffsetDateTime::now_utc() + time::Duration::days(3650);
    params
}

pub fn ensure_certificate(layout: &Layout) -> Result<()> {
    let dir = directory(layout);
    fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(&dir, fs::Permissions::from_mode(0o700)).map_err(|e| e.to_string())?;
    }
    let ca_cert = root_certificate(layout);
    let ca_key = root_key(layout);
    if ca_cert.exists() != ca_key.exists() {
        return Err(
            "The local HTTPS certificate authority is incomplete. Its files were preserved.".into(),
        );
    }
    if !ca_cert.exists() {
        let key = KeyPair::generate().map_err(|e| e.to_string())?;
        let ca = CertifiedIssuer::self_signed(ca_params(), &key).map_err(|e| e.to_string())?;
        atomic_write(&ca_key, key.serialize_pem().as_bytes())?;
        private_file(&ca_key)?;
        atomic_write(&ca_cert, ca.pem().as_bytes())?;
        atomic_write(&dir.join("root-ca.der"), ca.der().as_ref())?;
    }
    let mut domains = enabled_names(layout)?
        .into_iter()
        .map(|name| format!("{name}.test"))
        .collect::<Vec<_>>();
    domains.push("fairway.test".into());
    domains.sort();
    domains.dedup();
    let domains_path = dir.join("certificate-domains.json");
    let matching_domains = fs::read(&domains_path)
        .ok()
        .and_then(|bytes| serde_json::from_slice::<Vec<String>>(&bytes).ok())
        .is_some_and(|saved| saved == domains);
    let recent = matching_domains
        && [certificate(layout), private_key(layout)]
            .iter()
            .all(|path| {
                fs::metadata(path)
                    .and_then(|metadata| metadata.modified())
                    .and_then(|modified| modified.elapsed().map_err(std::io::Error::other))
                    .is_ok_and(|age| age < Duration::from_secs(300 * 24 * 3600))
            });
    if recent {
        return Ok(());
    }
    let key = KeyPair::from_pem(&fs::read_to_string(&ca_key).map_err(|e| e.to_string())?)
        .map_err(|e| e.to_string())?;
    let issuer = Issuer::from_ca_cert_pem(
        &fs::read_to_string(&ca_cert).map_err(|e| e.to_string())?,
        &key,
    )
    .map_err(|e| e.to_string())?;
    let server_key = KeyPair::generate().map_err(|e| e.to_string())?;
    let mut params = CertificateParams::new(domains.clone()).map_err(|e| e.to_string())?;
    params.distinguished_name = DistinguishedName::new();
    params
        .distinguished_name
        .push(DnType::CommonName, "Fairway XMP project sites");
    params.key_usages = vec![KeyUsagePurpose::DigitalSignature];
    params.extended_key_usages = vec![ExtendedKeyUsagePurpose::ServerAuth];
    params.not_before = OffsetDateTime::now_utc() - time::Duration::days(1);
    params.not_after = OffsetDateTime::now_utc() + time::Duration::days(365);
    let server = params
        .signed_by(&server_key, &issuer)
        .map_err(|e| e.to_string())?;
    atomic_write(&private_key(layout), server_key.serialize_pem().as_bytes())?;
    private_file(&private_key(layout))?;
    atomic_write(&certificate(layout), server.pem().as_bytes())?;
    atomic_write(
        &domains_path,
        &serde_json::to_vec(&domains).map_err(|e| e.to_string())?,
    )?;
    Ok(())
}

fn run(mut command: Command) -> Result<()> {
    let result = command.output().map_err(|e| e.to_string())?;
    if result.status.success() {
        Ok(())
    } else {
        let detail = String::from_utf8_lossy(&result.stderr).trim().to_string();
        Err(if detail.is_empty() {
            format!(
                "Command exited with {}: {}",
                result.status,
                String::from_utf8_lossy(&result.stdout).trim()
            )
        } else {
            detail
        })
    }
}

#[cfg(target_os = "macos")]
fn macos_keychain(layout: &Layout) -> PathBuf {
    layout.home.join("Library/Keychains/login.keychain-db")
}

#[cfg(target_os = "macos")]
fn verify_macos_trust(layout: &Layout) -> Result<()> {
    let mut verify = Command::new("/usr/bin/security");
    verify
        .args(["verify-cert", "-c"])
        .arg(certificate(layout))
        .args(["-p", "ssl", "-n", "fairway.test", "-k"])
        .arg(macos_keychain(layout));
    run(verify)
}

pub fn ensure_trust(layout: &Layout) -> Result<()> {
    ensure_certificate(layout)?;
    let root = root_certificate(layout);
    #[cfg(target_os = "macos")]
    {
        if verify_macos_trust(layout).is_ok() {
            atomic_write(&trust_marker(layout), b"trusted")?;
            return Ok(());
        }
        let keychain = macos_keychain(layout);
        if !keychain.is_file() {
            return Err(
                "Your macOS login keychain is unavailable. Open Keychain Access and try again."
                    .into(),
            );
        }
        let mut install = Command::new("/usr/bin/security");
        install
            .args(["add-trusted-cert", "-r", "trustRoot", "-p", "ssl", "-k"])
            .arg(keychain)
            .arg(root);
        run(install).map_err(|e| format!("Could not trust Fairway's local certificate: {e}"))?;
        verify_macos_trust(layout).map_err(|e| {
            format!("macOS did not trust Fairway's local certificate after installation: {e}")
        })?;
    }
    #[cfg(windows)]
    {
        let mut install = Command::new("certutil.exe");
        install.args(["-user", "-addstore", "Root"]).arg(root);
        run(install).map_err(|e| format!("Could not trust Fairway's local certificate: {e}"))?;
    }
    #[cfg(target_os = "linux")]
    {
        use std::io::IsTerminal;
        let program = if std::io::stdin().is_terminal() {
            "sudo"
        } else {
            "pkexec"
        };
        let executable = std::env::current_exe().map_err(|e| e.to_string())?;
        let mut install = Command::new(program);
        install.arg(executable).arg("__trust-ca").arg(root);
        run(install).map_err(|e| format!("Could not trust Fairway's local certificate: {e}"))?;
    }
    atomic_write(&trust_marker(layout), b"trusted")?;
    Ok(())
}

pub fn remove_trust(layout: &Layout) -> Result<()> {
    let root = root_certificate(layout);
    if !root.exists() || !trust_marker(layout).exists() {
        return Ok(());
    }
    #[cfg(target_os = "macos")]
    {
        let mut remove = Command::new("/usr/bin/security");
        remove
            .arg("remove-trusted-cert")
            .arg("-k")
            .arg(macos_keychain(layout))
            .arg(root);
        run(remove)
            .map_err(|e| format!("Could not remove Fairway's local certificate trust: {e}"))?;
    }
    #[cfg(windows)]
    {
        use ring::digest::{digest, SHA1_FOR_LEGACY_USE_ONLY};
        let der = fs::read(directory(layout).join("root-ca.der")).map_err(|e| e.to_string())?;
        let thumbprint = digest(&SHA1_FOR_LEGACY_USE_ONLY, &der)
            .as_ref()
            .iter()
            .map(|byte| format!("{byte:02X}"))
            .collect::<String>();
        let mut remove = Command::new("certutil.exe");
        remove.args(["-user", "-delstore", "Root", &thumbprint]);
        run(remove)
            .map_err(|e| format!("Could not remove Fairway's local certificate trust: {e}"))?;
    }
    #[cfg(target_os = "linux")]
    {
        use std::io::IsTerminal;
        let program = if std::io::stdin().is_terminal() {
            "sudo"
        } else {
            "pkexec"
        };
        let executable = std::env::current_exe().map_err(|e| e.to_string())?;
        let mut remove = Command::new(program);
        remove.arg(executable).arg("__remove-ca").arg(root);
        run(remove)
            .map_err(|e| format!("Could not remove Fairway's local certificate trust: {e}"))?;
    }
    fs::remove_file(trust_marker(layout)).map_err(|e| e.to_string())?;
    Ok(())
}

#[cfg(target_os = "linux")]
pub fn apply_linux_trust(root: &std::path::Path, remove: bool) -> Result<()> {
    if unsafe { libc::geteuid() } != 0 {
        return Err("Administrator permission is required to trust the local certificate.".into());
    }
    if !root.is_file() || root.file_name().is_none_or(|name| name != "root-ca.pem") {
        return Err("Invalid Fairway certificate path.".into());
    }
    let destination =
        std::path::Path::new("/usr/local/share/ca-certificates/fairway-xmp-local.crt");
    if destination.parent().is_some_and(std::path::Path::is_dir) {
        if remove {
            if destination.exists()
                && fs::read(destination).map_err(|e| e.to_string())?
                    == fs::read(root).map_err(|e| e.to_string())?
            {
                fs::remove_file(destination).map_err(|e| e.to_string())?;
            }
        } else {
            fs::copy(root, destination).map_err(|e| e.to_string())?;
        }
        return run(Command::new("update-ca-certificates"));
    }
    let mut command = Command::new("trust");
    command
        .arg("anchor")
        .arg(if remove { "--remove" } else { "--store" })
        .arg(root);
    run(command)
}
