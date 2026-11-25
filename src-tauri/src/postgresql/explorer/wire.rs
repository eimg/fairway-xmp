//! A bounded private stdin protocol, never a file or public IPC response.
use super::super::explorer_contract::*;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
#[derive(Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct ManagedIdentity {
    pub cluster_name: String,
    pub data_directory: PathBuf,
}
#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct Envelope {
    pub runtime: PathBuf,
    pub host: String,
    pub port: u16,
    pub user: String,
    pub database: String,
    #[serde(serialize_with = "password")]
    pub password: Password,
    pub managed: Option<ManagedIdentity>,
    pub operation: Operation,
}
fn password<S: serde::Serializer>(p: &Password, s: S) -> std::result::Result<S::Ok, S::Error> {
    s.serialize_str(std::str::from_utf8(p.bytes()).map_err(serde::ser::Error::custom)?)
}
#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
pub(crate) struct PrivateBytes(pub Vec<u8>);
#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
impl Drop for PrivateBytes {
    fn drop(&mut self) {
        for b in &mut self.0 {
            unsafe { std::ptr::write_volatile(b, 0) }
        }
    }
}
#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
pub fn entrypoint() -> bool {
    if std::env::args_os().nth(1).as_deref()
        != Some(std::ffi::OsStr::new("--fairway-postgres-query-worker"))
    {
        return false;
    }
    use std::{
        io::{BufRead, Read, Write},
        sync::{
            atomic::{AtomicBool, Ordering},
            Arc,
        },
    };
    let mut input = std::io::BufReader::new(std::io::stdin());
    let mut bytes = PrivateBytes(Vec::new());
    let read = (&mut input)
        .take(1024 * 1024 + 1)
        .read_until(b'\n', &mut bytes.0);
    let result = (|| -> Result<Output> {
        if read.is_err() || bytes.0.len() > 1024 * 1024 || bytes.0.last() != Some(&b'\n') {
            return Err(fail(Code::InvalidRequest, "Invalid private worker input"));
        }
        let e: Envelope = serde_json::from_slice(&bytes.0)
            .map_err(|_| fail(Code::InvalidRequest, "Invalid private worker input"))?;
        super::validate_connection(&e)?;
        let base = e
            .runtime
            .parent()
            .ok_or_else(|| fail(Code::Unavailable, "Runtime unavailable"))?;
        let rt = crate::postgresql::managed::runtime::installed(
            &crate::postgresql::managed::files::Dir::absolute(base)
                .map_err(|_| fail(Code::Unavailable, "Runtime unavailable"))?,
            base,
        )
        .map_err(|_| fail(Code::Unavailable, "Runtime integrity check failed"))?;
        if rt.path != e.runtime {
            return Err(fail(Code::Unavailable, "Runtime integrity check failed"));
        }
        let cancel = Arc::new(AtomicBool::new(false));
        let flag = cancel.clone();
        std::thread::spawn(move || {
            let mut b = [0];
            let _ = input.read(&mut b);
            flag.store(true, Ordering::SeqCst);
        });
        super::native::execute(&e, cancel)
    })();
    drop(bytes);
    // No raw stderr, SQL, parameter, server diagnostic or credential logging.
    if let Ok(bytes) = serde_json::to_vec(&result) {
        let mut out = std::io::stdout().lock();
        let _ = out.write_all(&bytes).and_then(|_| out.flush());
    }
    std::process::exit(0);
}
