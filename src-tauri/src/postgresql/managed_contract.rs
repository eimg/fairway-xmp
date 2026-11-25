use serde::{Deserialize, Serialize};
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Snapshot {
    pub supported: bool,
    pub runtime_version: &'static str,
    pub cluster_id: Option<String>,
    pub port: Option<u16>,
    pub running: bool,
    pub responsiveness: String,
    pub authentication: String,
    pub message: Option<String>,
}
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Confirmation {
    pub confirmed: bool,
}
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Initialize {
    pub confirmed: bool,
    pub port: u16,
}
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Logs {
    pub text: String,
    pub truncated: bool,
}

#[cfg(not(all(target_os = "macos", target_arch = "aarch64")))]
pub struct Managed;
#[cfg(not(all(target_os = "macos", target_arch = "aarch64")))]
impl Managed {
    pub fn new(_: Option<crate::config::Layout>) -> Self {
        Self
    }
    fn unavailable<T>() -> super::Result<T> {
        Err(super::Error::new(
            super::ErrorCode::Unavailable,
            "Managed PostgreSQL currently requires macOS arm64",
        ))
    }
    pub fn provision(&self, _: Confirmation) -> super::Result<Snapshot> {
        Self::unavailable()
    }
    pub fn initialize(&self, _: Initialize) -> super::Result<Snapshot> {
        Self::unavailable()
    }
    pub fn start(&self, _: Confirmation) -> super::Result<Snapshot> {
        Self::unavailable()
    }
    pub fn stop(&self, _: Confirmation) -> super::Result<Snapshot> {
        Self::unavailable()
    }
    pub fn restart(&self, _: Confirmation) -> super::Result<Snapshot> {
        Self::unavailable()
    }
    pub fn health(&self) -> super::Result<Snapshot> {
        Self::unavailable()
    }
    pub fn snapshot(&self) -> super::Result<Snapshot> {
        Self::unavailable()
    }
    pub fn logs(&self) -> super::Result<Logs> {
        Self::unavailable()
    }
    pub(crate) fn explorer_connection(
        &self,
        _: Option<&str>,
        _: String,
    ) -> crate::postgresql::explorer_contract::Result<crate::postgresql::explorer::wire::Envelope>
    {
        Err(crate::postgresql::explorer_contract::fail(
            crate::postgresql::explorer_contract::Code::Unavailable,
            "PostgreSQL client requires qualified macOS arm64 runtime",
        ))
    }
    pub fn stop_for_shutdown(&self) -> super::Result<()> {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn lifecycle_requests_refuse_paths_credentials_unknown_fields_and_invalid_ports() {
        for bad in [
            r#"{"confirmed":true,"path":"/existing/cluster"}"#,
            r#"{"confirmed":true,"password":"secret"}"#,
            r#"{"confirmed":true,"confirmed":false}"#,
        ] {
            assert!(serde_json::from_str::<Confirmation>(bad).is_err());
        }
        for bad in [
            r#"{"confirmed":true,"port":65536}"#,
            r#"{"confirmed":true,"port":-1}"#,
            r#"{"confirmed":true,"port":15432,"external":true}"#,
        ] {
            assert!(serde_json::from_str::<Initialize>(bad).is_err());
        }
        let request: Initialize =
            serde_json::from_str(r#"{"confirmed":true,"port":15432}"#).unwrap();
        assert!(request.confirmed);
        assert_eq!(request.port, 15432);
    }
}
