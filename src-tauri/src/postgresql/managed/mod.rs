//! macOS arm64 managed service. No external-cluster adoption or arbitrary paths/SQL.
use super::{Error, ErrorCode};
use crate::config::Layout;
use serde::{Deserialize, Serialize};
use std::{
    fs::File,
    path::PathBuf,
    process::{Child, Stdio},
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};
pub(crate) mod files;
mod health;
pub use health::worker_entrypoint;
mod logs;
pub(crate) mod runtime;
mod secrets;
#[cfg(test)]
mod tests;
type Result<T> = std::result::Result<T, Error>;
fn failure(message: &str) -> Error {
    Error::new(ErrorCode::Unavailable, message)
}
fn json<T: Serialize>(v: &T) -> Result<Vec<u8>> {
    serde_json::to_vec(v).map_err(|_| failure("Cannot encode managed PostgreSQL state"))
}
fn decode<T: serde::de::DeserializeOwned>(b: &[u8]) -> Result<T> {
    serde_json::from_slice(b)
        .map_err(|_| failure("Invalid managed PostgreSQL metadata; data was retained"))
}
use files::{Dir, Identity};
use secrets::{Secret, Vault};

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Cluster {
    schema_version: u32,
    id: String,
    identity: Identity,
    runtime_sha256: String,
    port: u16,
    config_sha256: String,
    hba_sha256: String,
    phase: String,
}
pub use super::managed_contract::{Confirmation, Initialize, Logs, Snapshot};

struct Run {
    child: Child,
    _runtime_lease: File,
    _pump: logs::Pump,
}
struct Controller {
    base: Dir,
    path: PathBuf,
    _service_lease: File,
    source: PathBuf,
    settings: PathBuf,
    vault: Arc<dyn Vault>,
    cluster: Option<Cluster>,
    run: Option<Run>,
    last_log: Option<Arc<Mutex<logs::Capture>>>,
    last_log_name: Option<String>,
}
pub struct Managed {
    layout: Option<Layout>,
    controller: Mutex<Option<Controller>>,
    vault: Arc<dyn Vault>,
}

impl Managed {
    pub fn new(layout: Option<Layout>) -> Self {
        Self {
            layout,
            controller: Mutex::new(None),
            vault: Arc::new(secrets::Keychain),
        }
    }
    fn with<T>(&self, action: impl FnOnce(&mut Controller) -> Result<T>) -> Result<T> {
        let mut guard = self
            .controller
            .try_lock()
            .map_err(|_| Error::new(ErrorCode::Busy, "PostgreSQL operation is in progress"))?;
        if guard.is_none() {
            let layout = self
                .layout
                .as_ref()
                .ok_or_else(|| failure("Application data is unavailable"))?;
            *guard = Some(Controller::open(layout, self.vault.clone())?);
        }
        action(guard.as_mut().unwrap())
    }
    pub fn provision(&self, r: Confirmation) -> Result<Snapshot> {
        consent(r.confirmed)?;
        self.with(|c| {
            runtime::provision(&c.base, &c.path, &c.source)?;
            c.snapshot(false)
        })
    }
    pub fn initialize(&self, r: Initialize) -> Result<Snapshot> {
        consent(r.confirmed)?;
        self.with(|c| {
            c.initialize(r.port)?;
            c.snapshot(false)
        })
    }
    pub fn start(&self, r: Confirmation) -> Result<Snapshot> {
        consent(r.confirmed)?;
        self.with(|c| {
            c.start()?;
            c.snapshot(true)
        })
    }
    pub fn stop(&self, r: Confirmation) -> Result<Snapshot> {
        consent(r.confirmed)?;
        self.with(|c| {
            c.stop()?;
            c.snapshot(false)
        })
    }
    pub fn restart(&self, r: Confirmation) -> Result<Snapshot> {
        consent(r.confirmed)?;
        self.with(|c| {
            c.stop()?;
            c.start()?;
            c.snapshot(true)
        })
    }
    fn read<T>(
        &self,
        empty: impl FnOnce() -> T,
        action: impl FnOnce(&mut Controller) -> Result<T>,
    ) -> Result<T> {
        let mut guard = self
            .controller
            .try_lock()
            .map_err(|_| Error::new(ErrorCode::Busy, "PostgreSQL operation is in progress"))?;
        if guard.is_none() {
            let Some(layout) = &self.layout else {
                return Ok(empty());
            };
            let data = Dir::absolute(&layout.data)?;
            if !data.exists("postgresql")? {
                return Ok(empty());
            }
            *guard = Some(Controller::open(layout, self.vault.clone())?);
        }
        action(guard.as_mut().unwrap())
    }
    pub fn health(&self) -> Result<Snapshot> {
        self.read(empty, |c| c.snapshot(true))
    }
    pub fn snapshot(&self) -> Result<Snapshot> {
        self.read(empty, |c| c.snapshot(false))
    }
    pub fn logs(&self) -> Result<Logs> {
        self.read(
            || Logs {
                text: String::new(),
                truncated: false,
            },
            |c| c.logs(),
        )
    }
    pub(crate) fn explorer_lease(&self) -> crate::postgresql::explorer_contract::Result<File> {
        use crate::postgresql::explorer_contract::*;
        self.read(
            || Err(fail(Code::Unavailable, "Runtime not provisioned")),
            |c| Ok(Ok(c.base.shared_lock("clients.lease")?)),
        )
        .map_err(|error| {
            if error.code == ErrorCode::Busy {
                fail(Code::Busy, "PostgreSQL metadata resolver is busy")
            } else {
                fail(Code::Unavailable, "Runtime client lease unavailable")
            }
        })?
    }
    pub(crate) fn explorer_connection(
        &self,
        cluster_id: Option<&str>,
        database: String,
    ) -> crate::postgresql::explorer_contract::Result<crate::postgresql::explorer::wire::Envelope>
    {
        use crate::postgresql::{
            explorer::wire::{Envelope, ManagedIdentity},
            explorer_contract::*,
        };
        self.read(
            || {
                Err(fail(
                    Code::Unavailable,
                    "Explicit runtime provisioning is required",
                ))
            },
            |c| {
                let rt = c.runtime()?;
                let envelope = if let Some(id) = cluster_id {
                    c.reap()?;
                    let (_, cluster, path) = c.validate()?;
                    if cluster.id != id || c.run.is_none() {
                        return Err(failure("Requested owned cluster is not running"));
                    }
                    let account = format!(
                        "{}-{}",
                        runtime::hash(
                            c.path
                                .to_str()
                                .ok_or_else(|| failure("Invalid managed path"))?
                                .as_bytes()
                        ),
                        cluster.id
                    );
                    let secret = c.vault.read(&account)?;
                    Envelope {
                        runtime: rt.path,
                        host: "127.0.0.1".into(),
                        port: cluster.port,
                        user: "fairway".into(),
                        database,
                        password: Password::from_bytes(&secret.0)
                            .map_err(|_| failure("Credential unavailable"))?,
                        managed: Some(ManagedIdentity {
                            cluster_name: format!("fairway-{}", cluster.id),
                            data_directory: path,
                        }),
                        operation: Operation::Schemas,
                    }
                } else {
                    Envelope {
                        runtime: rt.path,
                        host: String::new(),
                        port: 0,
                        user: String::new(),
                        database,
                        password: Password::from_bytes(b"")
                            .map_err(|_| failure("Credential unavailable"))?,
                        managed: None,
                        operation: Operation::Schemas,
                    }
                };
                Ok(Ok(envelope))
            },
        )
        .map_err(|error| {
            if error.code == ErrorCode::Busy {
                fail(Code::Busy, "PostgreSQL metadata resolver is busy")
            } else {
                fail(
                    Code::Unavailable,
                    "Verified PostgreSQL runtime or owned credentials unavailable",
                )
            }
        })?
    }
    pub fn stop_for_shutdown(&self) -> Result<()> {
        let mut guard = self
            .controller
            .lock()
            .map_err(|_| failure("PostgreSQL state unavailable"))?;
        if let Some(c) = guard.as_mut() {
            c.stop()?;
        }
        Ok(())
    }
}
fn empty() -> Snapshot {
    Snapshot {
        supported: true,
        runtime_version: runtime::VERSION,
        cluster_id: None,
        port: None,
        running: false,
        responsiveness: "notChecked".into(),
        authentication: "notChecked".into(),
        message: None,
    }
}
fn consent(confirmed: bool) -> Result<()> {
    if !confirmed {
        Err(Error::new(
            ErrorCode::ConfirmationRequired,
            "Explicit managed PostgreSQL authorization is required",
        ))
    } else {
        Ok(())
    }
}
fn valid_id(id: &str) -> bool {
    id.len() == 32 && id.bytes().all(|b| b.is_ascii_hexdigit())
}
fn clean_command(path: impl AsRef<std::ffi::OsStr>) -> std::process::Command {
    let mut cmd = crate::process::command(path);
    cmd.env_clear()
        .env("PATH", "/usr/bin:/bin:/usr/sbin:/sbin")
        .env("LC_ALL", "C")
        .stdin(Stdio::null());
    use std::os::unix::process::CommandExt;
    unsafe {
        cmd.pre_exec(|| {
            let limit = libc::rlimit {
                rlim_cur: 0,
                rlim_max: 0,
            };
            if libc::setrlimit(libc::RLIMIT_CORE, &limit) != 0 {
                return Err(std::io::Error::last_os_error());
            }
            Ok(())
        });
    }
    cmd
}
fn port_available(port: u16) -> Result<()> {
    if port < 1024 {
        return Err(failure("Choose a PostgreSQL port between 1024 and 65535"));
    }
    let listener = std::net::TcpListener::bind(("127.0.0.1", port))
        .map_err(|_| failure("PostgreSQL port is already occupied; no process was stopped"))?;
    drop(listener);
    if crate::process::connected(port) {
        return Err(failure(
            "PostgreSQL port is already occupied; no process was stopped",
        ));
    }
    Ok(())
}
impl Controller {
    fn open(layout: &Layout, vault: Arc<dyn Vault>) -> Result<Self> {
        let data = Dir::absolute(&layout.data)?;
        let base = if data.exists("postgresql")? {
            let dir = data.child("postgresql")?;
            dir.private()?;
            let identity: Identity = decode(&dir.read(".fairway-managed-root", 4096)?)?;
            if identity != dir.identity()? {
                return Err(failure(
                    "Unowned PostgreSQL root; no existing data was modified",
                ));
            }
            dir
        } else {
            let dir = data.make("postgresql")?;
            dir.create(".fairway-managed-root", &json(&dir.identity()?)?, false)?;
            dir
        };
        let path = layout.data.join("postgresql");
        base.assert_path(&path)?;
        let lease = base.lock("service.lock")?;
        let cluster = if base.exists("active.json")? {
            Some(decode::<Cluster>(&base.read("active.json", 64 * 1024)?)?)
        } else {
            None
        };
        let last_log_name = if base.exists("last-log.json")? {
            let n: String = decode(&base.read("last-log.json", 4096)?)?;
            if !n.starts_with("log-") || !n.ends_with(".txt") {
                return Err(failure("Invalid managed log reference"));
            }
            // Descriptor-relative read rejects links, traversal and foreign files.
            base.read(&n, 1024 * 1024)?;
            Some(n)
        } else {
            None
        };
        Ok(Self {
            base,
            path,
            _service_lease: lease,
            source: layout.runtime.join("postgresql"),
            settings: layout.data.clone(),
            vault,
            cluster,
            run: None,
            last_log: None,
            last_log_name,
        })
    }
    fn check_port(&self, port: u16) -> Result<()> {
        let data = Dir::absolute(&self.settings)?;
        // Do not call Layout::ports: discovery must not migrate shared settings.
        let ports = if data.exists("settings.json")? {
            decode::<crate::config::Ports>(&data.read("settings.json", 65536)?)?
        } else {
            crate::config::Ports::default()
        };
        if ports.values().contains(&port) || port == crate::tls::https_port() {
            return Err(failure(
                "PostgreSQL port is reserved by another Fairway service",
            ));
        }
        port_available(port)
    }
    fn runtime(&self) -> Result<runtime::Runtime> {
        runtime::installed(&self.base, &self.path)
    }
    fn cluster_path(&self, c: &Cluster) -> Result<PathBuf> {
        if !valid_id(&c.id) || c.schema_version != 1 || c.phase != "completed" {
            return Err(failure(
                "Unknown or incomplete PostgreSQL cluster; no adoption or repair is permitted",
            ));
        }
        Ok(self.path.join(format!("cluster-{}", c.id)))
    }
    fn validate(&self) -> Result<(runtime::Runtime, Cluster, PathBuf)> {
        self.base.assert_path(&self.path)?;
        let c = self
            .cluster
            .as_ref()
            .ok_or_else(|| failure("Initialize a new managed PostgreSQL cluster explicitly first"))?
            .clone();
        let rt = self.runtime()?;
        if rt.manifest_sha256 != c.runtime_sha256 {
            return Err(failure(
                "PostgreSQL cluster/runtime receipt mismatch; no upgrade permitted",
            ));
        }
        let path = self.cluster_path(&c)?;
        let dir = Dir::absolute(&path)?;
        dir.private()?;
        if dir.identity()? != c.identity
            || dir.read(".fairway-cluster", 128)? != c.id.as_bytes()
            || dir.read("PG_VERSION", 128)? != b"18\n"
        {
            return Err(failure(
                "PostgreSQL cluster ownership/version changed; no adoption permitted",
            ));
        }
        if runtime::hash(&dir.read("postgresql.conf", 64 * 1024)?) != c.config_sha256
            || runtime::hash(&dir.read("pg_hba.conf", 64 * 1024)?) != c.hba_sha256
            || !dir
                .read("postgresql.auto.conf", 64 * 1024)?
                .split(|b| *b == b'\n')
                .all(|line| line.is_empty() || line.starts_with(b"#"))
        {
            return Err(failure("Managed PostgreSQL configuration changed; restore the reviewed configuration explicitly"));
        }
        validate_tree(&dir, 0, &mut 0)?;
        dir.assert_path(&path)?;
        Ok((rt, c, path))
    }
    fn initialize(&mut self, port: u16) -> Result<()> {
        if self.cluster.is_some() || self.base.exists("active.json")? {
            return Err(failure(
                "A managed cluster is already registered; it will never be overwritten or upgraded",
            ));
        }
        if self.base.entries()?.iter().any(|(n, _)| {
            n.starts_with("job-") || n.starts_with("cluster-") || n.starts_with(".cluster-")
        }) {
            return Err(failure("A prior PostgreSQL initialization is retained. Review its journal; no existing cluster will be adopted, deleted or overwritten"));
        }
        self.check_port(port)?;
        let rt = self.runtime()?;
        let _lease = self.base.lock("runtime.lease")?;
        let id = format!("{:032x}", rand::random::<u128>());
        let account = format!(
            "{}-{id}",
            runtime::hash(
                self.path
                    .to_str()
                    .ok_or_else(|| failure("Invalid managed data path"))?
                    .as_bytes()
            )
        );
        // Journal nonsecret intent before the vault write: an interruption must
        // not lose the opaque reference to an already-created credential item.
        self.base.create(&format!("job-{id}.json"),&json(&serde_json::json!({"id":id,"phase":"securingCredential","port":port,"runtime":rt.manifest_sha256}))?,false)?;
        let secret = Secret::random();
        self.vault.create(&account, &secret)?;
        let checked = self.vault.read(&account)?;
        if checked.0 != secret.0 {
            return Err(failure(
                "Keychain verification failed; no cluster was initialized",
            ));
        }
        let stage = format!(".cluster-{id}");
        let root = self.base.make(&stage)?;
        self.base.atomic(&format!("job-{id}.json"),&json(&serde_json::json!({"id":id,"identity":root.identity()?,"phase":"initializing","port":port,"runtime":rt.manifest_sha256}))?)?;
        let stage_path = self.path.join(&stage);
        let mut command = clean_command(rt.path.join("bin/initdb"));
        command
            .current_dir(&stage_path)
            .args([
                "--no-clean",
                "--auth-local=scram-sha-256",
                "--auth-host=scram-sha-256",
                "--username=fairway",
                "--encoding=UTF8",
                "--locale=C",
                "--pwfile=/dev/fd/0",
                "-D",
            ])
            .arg(&stage_path);
        let log_name = format!("log-initialize-{id}.txt");
        let init_log = files::log_file(&self.path.join(&log_name))?;
        self.base.atomic("last-log.json", &json(&log_name)?)?;
        self.last_log_name = Some(log_name);
        let capture = Arc::new(Mutex::new(logs::Capture::new(init_log)));
        self.last_log = Some(capture.clone());
        finite_with_secret(command, &secret, capture)?;
        root.assert_path(&stage_path)?;
        let target = self.path.join(format!("cluster-{id}"));
        let config = configuration(port, &id);
        let hba=b"local all all reject\nhost all fairway 127.0.0.1/32 scram-sha-256\nhost all all 0.0.0.0/0 reject\nhost all all ::0/0 reject\n";
        root.atomic("postgresql.conf", config.as_bytes())?;
        root.atomic("pg_hba.conf", hba)?;
        root.create(".fairway-cluster", id.as_bytes(), false)?;
        if root.exists("postmaster.pid")? {
            return Err(failure(
                "Unexpected process marker in initialization stage; data retained",
            ));
        }
        let cluster = Cluster {
            schema_version: 1,
            id: id.clone(),
            identity: root.identity()?,
            runtime_sha256: rt.manifest_sha256,
            port,
            config_sha256: runtime::hash(config.as_bytes()),
            hba_sha256: runtime::hash(hba),
            phase: "completed".into(),
        };
        self.base
            .atomic(&format!("job-{id}.json"), &json(&cluster)?)?;
        self.base.assert_path(&self.path)?;
        root.assert_path(&stage_path)?;
        self.base.publish(&stage, &format!("cluster-{id}"))?;
        root.assert_path(&target)?;
        self.base.create("active.json", &json(&cluster)?, false)?;
        self.cluster = Some(cluster);
        Ok(())
    }
    fn start(&mut self) -> Result<()> {
        self.reap()?;
        if self.run.is_some() {
            return Err(failure("Managed PostgreSQL is already running"));
        }
        let (rt, c, path) = self.validate()?;
        let dir = Dir::absolute(&path)?;
        if dir.exists("postmaster.pid")? {
            return Err(failure("An existing postmaster marker is present. No process was adopted, stopped or marker removed"));
        }
        self.check_port(c.port)?;
        let lease = self.base.lock("runtime.lease")?;
        if self
            .base
            .entries()?
            .iter()
            .filter(|(n, _)| n.starts_with("log-"))
            .count()
            >= 32
        {
            return Err(failure(
                "Managed PostgreSQL log history is full; retained files were not pruned",
            ));
        }
        let log_name = format!("log-{:032x}.txt", rand::random::<u128>());
        let log = files::log_file(&self.path.join(&log_name))?;
        self.base.atomic("last-log.json", &json(&log_name)?)?;
        self.last_log_name = Some(log_name);
        let capture = Arc::new(Mutex::new(logs::Capture::new(log)));
        let mut cmd = clean_command(rt.path.join("bin/postgres"));
        cmd.arg("-D")
            .arg(&path)
            .current_dir(&path)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        let mut child = cmd
            .spawn()
            .map_err(|_| failure("Could not launch the verified managed PostgreSQL runtime"))?;
        let pump = logs::Pump::new(
            child
                .stdout
                .take()
                .ok_or_else(|| failure("Missing PostgreSQL output"))?,
            child
                .stderr
                .take()
                .ok_or_else(|| failure("Missing PostgreSQL errors"))?,
            capture.clone(),
        );
        let pump = match pump {
            Ok(p) => p,
            Err(e) => {
                unsafe { libc::kill(child.id() as i32, libc::SIGTERM) };
                let _ = child.wait();
                return Err(e);
            }
        };
        self.run = Some(Run {
            child,
            _runtime_lease: lease,
            _pump: pump,
        });
        self.last_log = Some(capture);
        let deadline = Instant::now() + Duration::from_secs(15);
        loop {
            self.reap()?;
            if self.run.is_none() {
                return Err(failure(
                    "Managed PostgreSQL exited during startup; inspect its managed logs",
                ));
            }
            if health::accepting(&rt, c.port)? {
                return Ok(());
            }
            if Instant::now() >= deadline {
                return Err(failure("PostgreSQL has not become responsive; its owned run is retained for Stop/log inspection"));
            }
            std::thread::sleep(Duration::from_millis(50));
        }
    }
    fn reap(&mut self) -> Result<()> {
        if let Some(run) = self.run.as_mut() {
            if run
                .child
                .try_wait()
                .map_err(|_| failure("Cannot inspect owned PostgreSQL process"))?
                .is_some()
            {
                self.run.take();
            }
        }
        Ok(())
    }
    fn stop(&mut self) -> Result<()> {
        self.reap()?;
        let Some(run) = self.run.as_mut() else {
            return Ok(());
        };
        // An unreaped Child is authority. Never consult a PID file or another service.
        if unsafe { libc::kill(run.child.id() as i32, libc::SIGTERM) } != 0 {
            return Err(failure("Could not request PostgreSQL smart shutdown"));
        }
        let deadline = Instant::now() + Duration::from_secs(10);
        loop {
            if run
                .child
                .try_wait()
                .map_err(|_| failure("Cannot wait for PostgreSQL shutdown"))?
                .is_some()
            {
                self.run.take();
                return Ok(());
            }
            if Instant::now() >= deadline {
                return Err(failure("PostgreSQL smart shutdown is waiting for clients. Close its connections and retry Stop; no immediate shutdown or forced kill was used"));
            }
            std::thread::sleep(Duration::from_millis(25));
        }
    }
    fn snapshot(&mut self, check: bool) -> Result<Snapshot> {
        self.reap()?;
        let mut s = empty();
        if let Some(c) = &self.cluster {
            s.cluster_id = Some(c.id.clone());
            s.port = Some(c.port);
        }
        if self.cluster.is_none()
            && self.base.entries()?.iter().any(|(n, _)| {
                n.starts_with("job-") || n.starts_with("cluster-") || n.starts_with(".cluster-")
            })
        {
            s.message=Some("Prior initialization retained for journal review; no cluster was adopted or overwritten".into());
        }
        s.running = self.run.is_some();
        if self.cluster.is_some() && !s.running {
            let (_, _, path) = self.validate()?;
            if Dir::absolute(&path)?.exists("postmaster.pid")? {
                s.message=Some("Existing postmaster marker: no process was adopted or stopped; manual ownership review is required".into());
            }
        }
        if check && s.running {
            let (rt, c, path) = self.validate()?;
            s.responsiveness = health::responsiveness(&rt, c.port)?.into();
            let account = format!(
                "{}-{}",
                runtime::hash(
                    self.path
                        .to_str()
                        .ok_or_else(|| failure("Invalid managed data path"))?
                        .as_bytes()
                ),
                c.id
            );
            match self.vault.read(&account){Ok(secret)=>{s.authentication=if health::authenticate(&rt,&c,&path,&secret)?{"authenticated"}else{"rejected"}.into();},Err(_)=>s.message=Some("Secure credential unavailable; server responsiveness does not establish authenticated access".into())}
        }
        Ok(s)
    }
    fn logs(&self) -> Result<Logs> {
        if let Some(log) = &self.last_log {
            Ok(log
                .lock()
                .map_err(|_| failure("Managed logs unavailable"))?
                .read())
        } else if let Some(name) = &self.last_log_name {
            let b = self.base.read(name, 1024 * 1024)?;
            let start = b.len().saturating_sub(64 * 1024);
            Ok(Logs {
                text: String::from_utf8_lossy(&b[start..]).into_owned(),
                truncated: start > 0 || b.len() == 1024 * 1024,
            })
        } else {
            Ok(Logs {
                text: String::new(),
                truncated: false,
            })
        }
    }
}
impl Drop for Controller {
    fn drop(&mut self) {
        let _ = self.stop();
    }
}
fn configuration(port: u16, id: &str) -> String {
    format!("listen_addresses = '127.0.0.1'\nport = {port}\nunix_socket_directories = ''\nssl = off\npassword_encryption = 'scram-sha-256'\ncluster_name = 'fairway-{id}'\nshared_buffers = '16MB'\nmax_connections = 20\nlogging_collector = off\nlog_statement = 'none'\nlog_min_error_statement = 'panic'\nlog_parameter_max_length = 0\nlog_parameter_max_length_on_error = 0\nlog_connections = off\n")
}
fn validate_tree(dir: &Dir, depth: u32, count: &mut usize) -> Result<()> {
    if depth > 32 {
        return Err(failure("Managed cluster tree exceeds its bounds"));
    }
    dir.private()?;
    for (name, mode) in dir.entries()? {
        *count += 1;
        if *count > 100_000 {
            return Err(failure("Managed cluster tree exceeds its bounds"));
        }
        match mode & libc::S_IFMT as u32 {
            m if m == libc::S_IFDIR as u32 => validate_tree(&dir.child(&name)?, depth + 1, count)?,
            m if m == libc::S_IFREG as u32 => {
                if mode & 0o077 != 0 {
                    return Err(failure(
                        "Managed cluster contains files accessible to another account",
                    ));
                }
            }
            _ => return Err(failure(
                "Managed cluster contains a symbolic link or special file; no adoption permitted",
            )),
        }
    }
    Ok(())
}
fn finite_with_secret(
    mut command: std::process::Command,
    secret: &Secret,
    capture: Arc<Mutex<logs::Capture>>,
) -> Result<()> {
    use std::io::Write;
    // Feed initdb through an inherited pipe, never an argv/environment/password file.
    let (read, mut write) = std::os::unix::net::UnixStream::pair()
        .map_err(|_| failure("Cannot create private initdb password pipe"))?;
    let input: std::os::fd::OwnedFd = read.into();
    command.stdin(Stdio::from(input));
    write
        .write_all(&secret.0)
        .and_then(|_| write.write_all(b"\n"))
        .map_err(|_| failure("Cannot send private initialization credential"))?;
    write
        .shutdown(std::net::Shutdown::Write)
        .map_err(|_| failure("Cannot close private initialization credential"))?;
    // command_output always replaces stdin, so use its bounded capture through a
    // dedicated implementation rather than putting credentials in captured output.
    let mut child = command
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|_| failure("Could not execute initdb"))?;
    drop(command);
    drop(write);
    let _pump = logs::Pump::new(
        child
            .stdout
            .take()
            .ok_or_else(|| failure("Missing initdb output"))?,
        child
            .stderr
            .take()
            .ok_or_else(|| failure("Missing initdb output"))?,
        capture,
    )?;
    let deadline = Instant::now() + Duration::from_secs(45);
    loop {
        if let Some(status) = child
            .try_wait()
            .map_err(|_| failure("Could not wait for initdb"))?
        {
            return if status.success() {
                Ok(())
            } else {
                Err(failure(
                    "initdb failed; its private stage and journal were retained",
                ))
            };
        }
        if Instant::now() >= deadline {
            unsafe { libc::kill(-(child.id() as i32), libc::SIGTERM) };
            let cleanup = Instant::now() + Duration::from_secs(2);
            while child
                .try_wait()
                .map_err(|_| failure("Cannot inspect initdb cleanup"))?
                .is_none()
                && Instant::now() < cleanup
            {
                std::thread::sleep(Duration::from_millis(25));
            }
            if child
                .try_wait()
                .map_err(|_| failure("Cannot inspect initdb cleanup"))?
                .is_none()
            {
                // Unpublished bootstrap only; never a managed/external live cluster.
                unsafe {
                    libc::kill(-(child.id() as i32), libc::SIGKILL);
                }
            }
            let _ = child.wait();
            return Err(failure("initdb timed out; private stage retained"));
        }
        std::thread::sleep(Duration::from_millis(25));
    }
}
