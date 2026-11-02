//! Desktop proxy. This type never owns or adopts service processes.
#![cfg_attr(not(feature = "desktop"), allow(dead_code))]
use super::{supervisor, transport::Stream, Request};
use crate::{
    config::{Layout, Ports, ProjectConfig, Result},
    manager::Snapshot,
    php_settings::{PhpSettings, PhpSettingsView},
};
use serde::de::DeserializeOwned;
use std::sync::Mutex;

pub(crate) struct Client {
    pub layout: Layout,
    pub ports: Ports,
    pub domains_approval_pending: bool,
    session: Mutex<Option<Stream>>,
}
impl Client {
    pub fn new(mut layout: Layout) -> Result<Self> {
        layout.runtime = super::canonical_runtime(layout.runtime);
        supervisor::ensure(&layout)?;
        // A connected socket alone does not establish installation identity.
        let identity = supervisor::call(&layout, &Request::Hello)?;
        if identity["protocolVersion"] != 1 || identity["version"] != env!("CARGO_PKG_VERSION") {
            return Err("The running Fairway supervisor uses another version. Shut it down before switching versions.".into());
        }
        let ports = layout.ports()?;
        let domains_approval_pending = layout.data.join("address-approval-pending").is_file();
        Ok(Self {
            layout,
            ports,
            domains_approval_pending,
            session: Mutex::new(None),
        })
    }

    pub fn refresh(&mut self) -> Result<()> {
        self.ports = self.layout.ports()?;
        self.domains_approval_pending = self.layout.data.join("address-approval-pending").is_file();
        Ok(())
    }
    pub fn reserved(&self) -> bool {
        self.session.lock().is_ok_and(|session| session.is_some())
    }
    pub fn clear_session(&self) {
        if let Ok(mut session) = self.session.lock() {
            session.take();
        }
    }
    pub fn call<T: DeserializeOwned>(&self, request: Request) -> Result<T> {
        let mut session = self
            .session
            .lock()
            .map_err(|_| "Supervisor connection failed.")?;
        let value = if let Some(stream) = session.as_mut() {
            stream
                .set_read_timeout(supervisor::response_timeout(&request))
                .map_err(|e| e.to_string())?;
            supervisor::write_request(&self.layout, stream, &request)?;
            supervisor::receive_response(&self.layout, stream)?
        } else {
            supervisor::ensure(&self.layout)?;
            supervisor::call(&self.layout, &request)?
        };
        serde_json::from_value(value).map_err(|e| e.to_string())
    }
    fn action(&self, request: Request) -> Result<()> {
        self.call::<serde_json::Value>(request).map(|_| ())
    }
    pub fn begin(&self) -> Result<()> {
        let mut session = self
            .session
            .lock()
            .map_err(|_| "Supervisor connection failed.")?;
        if session.is_some() {
            return Err("An operation is already in progress.".into());
        }
        supervisor::ensure(&self.layout)?;
        let stream = supervisor::send_request(&self.layout, &Request::Acquire)?;
        supervisor::receive_response(&self.layout, &stream)?;
        *session = Some(stream);
        Ok(())
    }
    pub fn end(&self) {
        if let Ok(mut session) = self.session.lock() {
            if let Some(stream) = session.take() {
                let _ = stream.set_read_timeout(Some(std::time::Duration::from_secs(2)));
                let _ = stream.set_write_timeout(Some(std::time::Duration::from_secs(2)));
                let _ = supervisor::write_request(&self.layout, &stream, &Request::Release)
                    .and_then(|_| supervisor::receive_response(&self.layout, &stream));
            }
        }
    }
    pub fn snapshot(&mut self) -> Result<Snapshot> {
        let mut status: Snapshot = self.call(Request::Status)?;
        // Detection describes the calling desktop, even if a terminal launched
        // the supervisor with a different inherited PATH.
        status.path_php = crate::integration::resolve("php");
        status.path_composer = crate::integration::resolve("composer");
        self.ports = status.ports.clone();
        self.domains_approval_pending = status.domains_approval_pending;
        Ok(status)
    }
    pub fn start(&mut self, service: &str) -> Result<()> {
        self.action(Request::Start {
            service: service.into(),
        })
    }
    pub fn stop(&mut self, service: &str) -> Result<()> {
        self.action(Request::Stop {
            service: service.into(),
        })
    }
    pub fn start_all(&mut self) -> Result<()> {
        self.start("all")
    }
    pub fn stop_all(&mut self) -> Result<()> {
        self.stop("all")
    }
    pub fn save_settings(&mut self, ports: Ports, version: String) -> Result<()> {
        self.action(Request::Settings { ports, version })?;
        self.refresh()
    }
    pub fn php_settings(&self, version: &str) -> Result<PhpSettingsView> {
        self.call(Request::PhpSettings {
            version: version.into(),
        })
    }
    pub fn save_php_settings(
        &mut self,
        version: &str,
        settings: Option<PhpSettings>,
    ) -> Result<PhpSettingsView> {
        self.call(Request::SavePhpSettings {
            version: version.into(),
            settings,
        })
    }
    pub fn save_project(&mut self, previous: Option<&str>, project: ProjectConfig) -> Result<()> {
        self.action(Request::SaveProject {
            previous: previous.map(str::to_owned),
            project,
        })
    }
    pub fn remove_project(&mut self, domain: &str) -> Result<()> {
        self.action(Request::RemoveProject {
            domain: domain.into(),
        })
    }
    pub fn set_project_https(&mut self, domain: &str, enabled: bool) -> Result<()> {
        self.action(Request::ProjectHttps {
            domain: domain.into(),
            enabled,
        })
    }
    pub fn domains(&self) -> Result<Vec<String>> {
        Ok(self
            .layout
            .registered_projects()?
            .into_iter()
            .map(|p| p.domain)
            .collect())
    }
    pub fn unused_trust(&self) -> Result<bool> {
        Ok(crate::tls::trusted(&self.layout)
            && !self.layout.registered_projects()?.iter().any(|p| p.https))
    }
    pub fn remember_address_approval(&mut self, pending: bool) -> Result<()> {
        self.action(Request::RememberAddressApproval { pending })?;
        self.domains_approval_pending = pending;
        Ok(())
    }
    pub fn backup(&self) -> Result<String> {
        self.call(Request::Backup)
    }
    pub fn reserve_installation(&self) -> Result<std::fs::File> {
        let deadline = std::time::Instant::now() + std::time::Duration::from_millis(250);
        loop {
            if let Some(lock) =
                super::transport::lock_path(&self.layout.data.join("installation.lock"))?
            {
                return Ok(lock);
            }
            // A short-lived shared status probe is not another installation.
            if std::time::Instant::now() >= deadline {
                return Err("Fairway is already being updated.".into());
            }
            std::thread::sleep(std::time::Duration::from_millis(5));
        }
    }
    pub fn prepare_update(&self) -> Result<()> {
        self.action(Request::PrepareDesktopUpdate)
    }
    pub fn shutdown(&self) -> Result<()> {
        let pid = supervisor::pid(&self.layout);
        self.action(Request::Shutdown)?;
        self.session
            .lock()
            .map_err(|_| "Supervisor connection failed.")?
            .take();
        supervisor::wait_shutdown_pid(&self.layout, pid)
    }
    /// The supervisor continues owning the services if the desktop leaves them running.
    pub fn leave_services_running(&mut self) -> Result<()> {
        self.end();
        Ok(())
    }
}
impl Drop for Client {
    fn drop(&mut self) {
        self.end();
        #[cfg(test)]
        {
            let _ = supervisor::call(&self.layout, &Request::Shutdown);
            let _ = supervisor::wait_shutdown(&self.layout);
        }
    }
}
