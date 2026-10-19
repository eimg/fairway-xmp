use crate::{
    config::{atomic_write, Layout, Ports, Project, ProjectConfig, Result},
    process::{self, OwnedProcess},
};
use serde::Serialize;
use std::{
    collections::BTreeMap,
    fs,
    path::Path,
    process::Stdio,
    thread,
    time::{Duration, Instant},
};

#[derive(Clone, Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ServiceStatus {
    pub id: String,
    pub running: bool,
    pub ready: bool,
    pub port: u16,
    pub pid: Option<u32>,
    pub message: Option<String>,
}

#[derive(Clone, Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Snapshot {
    pub ports: Ports,
    pub services: Vec<ServiceStatus>,
    pub document_root: String,
    pub projects: Vec<Project>,
    #[serde(default)]
    pub sites_error: Option<String>,
    /// The hosts file does not match the registered domains yet.
    pub domains_pending: bool,
    pub https_trusted: bool,
    pub os: String,
    pub data_dir: String,
    pub runtime_ready: bool,
    pub runtime_versions: serde_json::Value,
    #[serde(default)]
    pub mail_available: bool,
    pub php_version: String,
    pub php_versions: BTreeMap<String, String>,
    pub path_installed: bool,
    pub cli_php: Option<String>,
    pub cli_composer: Option<String>,
    pub path_php: Option<String>,
    pub path_composer: Option<String>,
    pub version: String,
}

pub struct Manager {
    pub layout: Layout,
    pub ports: Ports,
    processes: BTreeMap<String, OwnedProcess>,
    errors: BTreeMap<String, String>,
}

impl Manager {
    pub fn new(layout: Layout) -> Result<Self> {
        layout.initialize()?;
        let ports = layout.ports()?;
        layout.write_configs(&ports)?;
        let mut manager = Self {
            layout,
            ports,
            processes: BTreeMap::new(),
            errors: BTreeMap::new(),
        };
        #[cfg(unix)]
        for id in ["php", "nginx"] {
            if let Ok(pid) = fs::read_to_string(manager.layout.data.join(format!("{id}.pid"))) {
                if let Ok(pid) = pid.trim().parse::<u32>() {
                    if process::matches_web(pid, id, &manager.layout) {
                        manager
                            .processes
                            .insert(id.into(), OwnedProcess::adopt_database(pid));
                    }
                }
            }
        }
        #[cfg(unix)]
        if let Ok(pid) = fs::read_to_string(manager.layout.data.join("mail.pid")) {
            if let Ok(pid) = pid.trim().parse::<u32>() {
                let exe = manager.layout.exe("mailpit", "mailpit");
                if process::matches_command(pid, &exe, &manager.mail_args()) {
                    manager
                        .processes
                        .insert("mail".into(), OwnedProcess::adopt_database(pid));
                } else if !process::pid_alive(pid) {
                    let _ = fs::remove_file(manager.layout.data.join("mail.pid"));
                }
            }
        }
        let pid_file = manager.layout.data.join("mysql.pid");
        if let Ok(text) = fs::read_to_string(&pid_file) {
            if let Ok(pid) = text.trim().parse::<u32>() {
                if process::matches_database(
                    pid,
                    &manager.layout.exe("mysql", "mysqld"),
                    &crate::config::windows_ansi_path(
                        &manager.layout.data.join("config/mysql.cnf"),
                    ),
                ) {
                    manager
                        .processes
                        .insert("mysql".into(), OwnedProcess::adopt_database(pid));
                } else if !process::pid_alive(pid) {
                    let _ = fs::remove_file(pid_file);
                }
            }
        }
        Ok(manager)
    }

    pub fn runtime_ready(&self) -> bool {
        let Ok(php) = self.layout.php_exe("php") else {
            return false;
        };
        let Ok(worker) = self
            .layout
            .php_exe(if cfg!(windows) { "php-cgi" } else { "php-fpm" })
        else {
            return false;
        };
        [
            php,
            worker,
            self.layout.exe("nginx", "nginx"),
            self.layout.exe("mysql", "mysqld"),
            self.layout.exe("mysql", "mysql"),
            self.layout.exe("mysql", "mysqladmin"),
            self.layout.runtime.join("composer.phar"),
            self.layout.runtime.join("phpmyadmin/index.php"),
            self.layout.runtime.join("cacert.pem"),
            self.layout.runtime.join("nginx/conf/mime.types"),
            self.layout.runtime.join("manifest.json"),
        ]
        .iter()
        .all(|p| p.is_file())
    }

    fn mail_args(&self) -> Vec<String> {
        vec![
            "--listen".into(),
            format!("127.0.0.1:{}", self.ports.mail),
            "--smtp".into(),
            format!("127.0.0.1:{}", self.ports.smtp),
            "--database".into(),
            self.layout
                .data
                .join("mail/mailpit.db")
                .to_string_lossy()
                .into_owned(),
            // Local apps may send any SMTP login; nothing leaves this computer.
            "--smtp-auth-accept-any".into(),
            "--smtp-auth-allow-insecure".into(),
            "--smtp-disable-rdns".into(),
            "--disable-version-check".into(),
        ]
    }

    /// The mail catcher is optional, so a runtime without it keeps every other service usable.
    pub fn mail_available(&self) -> bool {
        self.layout.exe("mailpit", "mailpit").is_file()
    }

    pub fn snapshot(&mut self) -> Snapshot {
        let ids: Vec<String> = self.processes.keys().cloned().collect();
        for id in ids {
            if !self.processes.get_mut(&id).is_some_and(OwnedProcess::alive) {
                self.processes.remove(&id);
                self.errors
                    .insert(id, "The service exited. Open its log for details.".into());
            }
        }
        let php = self.processes.contains_key("php") && process::connected(self.ports.php);
        let mysql = self.processes.contains_key("mysql") && process::connected(self.ports.mysql);
        let nginx = self.processes.contains_key("nginx") && process::health(self.ports.website);
        let admin = nginx && php && mysql && process::health(self.ports.phpmyadmin);
        let mail = self.processes.contains_key("mail")
            && process::connected(self.ports.mail)
            && process::connected(self.ports.smtp);
        let services = [
            ("php", self.ports.php, php),
            ("nginx", self.ports.website, nginx && php),
            ("mysql", self.ports.mysql, mysql),
            ("phpmyadmin", self.ports.phpmyadmin, admin),
            ("mail", self.ports.mail, mail),
        ]
        .into_iter()
        .map(|(id, port, ready)| {
            let owner = if id == "phpmyadmin" { "nginx" } else { id };
            ServiceStatus {
                id: id.into(),
                running: self.processes.contains_key(owner),
                ready,
                port,
                pid: self.processes.get(owner).map(|p| p.pid()),
                message: self.errors.get(id).cloned(),
            }
        })
        .collect();
        let (registered, sites_error) = match self.layout.registered_projects() {
            Ok(sites) => (sites, None),
            Err(error) => (Vec::new(), Some(error)),
        };
        let managed = crate::hosts::managed_names().unwrap_or_default();
        let domains = registered
            .iter()
            .map(|project| project.domain.clone())
            .collect::<Vec<_>>();
        let projects = registered
            .into_iter()
            .map(|config| Project {
                available: config.root.is_dir(),
                domain_ready: managed.contains(&config.domain),
                config,
            })
            .collect();
        Snapshot {
            ports: self.ports.clone(),
            services,
            document_root: self.layout.www.to_string_lossy().into(),
            projects,
            sites_error: sites_error.clone(),
            domains_pending: sites_error.is_none()
                && !crate::hosts::current(&domains).unwrap_or(false),
            https_trusted: crate::tls::trusted(&self.layout),
            os: std::env::consts::OS.into(),
            data_dir: self.layout.data.to_string_lossy().into(),
            runtime_ready: self.runtime_ready(),
            mail_available: self.mail_available(),
            runtime_versions: fs::read(self.layout.runtime.join("manifest.json"))
                .ok()
                .and_then(|s| serde_json::from_slice(&s).ok())
                .unwrap_or(serde_json::Value::Null),
            php_version: self.layout.php_version().unwrap_or_default(),
            php_versions: self
                .layout
                .php_versions()
                .unwrap_or_default()
                .into_iter()
                .filter_map(|(minor, full)| full.as_str().map(|version| (minor, version.into())))
                .collect(),
            path_installed: crate::integration::installed(&self.layout),
            cli_php: crate::integration::launcher(&self.layout, "php"),
            cli_composer: crate::integration::launcher(&self.layout, "composer"),
            path_php: crate::integration::resolve("php"),
            path_composer: crate::integration::resolve("composer"),
            version: env!("CARGO_PKG_VERSION").into(),
        }
    }

    pub fn save_ports(&mut self, ports: Ports) -> Result<()> {
        if !self.processes.is_empty() {
            return Err("Stop all services before changing ports.".into());
        }
        ports.validate()?;
        self.layout.write_configs(&ports)?;
        self.layout.save_ports(&ports)?;
        self.ports = ports;
        self.errors.clear();
        Ok(())
    }

    /// Save a new project list and apply it to Nginx, restoring the previous list on failure.
    fn apply_projects(&mut self, next: Vec<ProjectConfig>) -> Result<()> {
        let previous = self.layout.registered_projects()?;
        if next.iter().any(|project| project.https)
            && self.ports.values().contains(&crate::tls::https_port())
        {
            return Err(format!(
                "Port {} is reserved for site HTTPS. Choose another service port.",
                crate::tls::https_port()
            ));
        }
        if next.iter().any(|project| project.https)
            && !previous.iter().any(|project| project.https)
            && self.processes.contains_key("nginx")
        {
            process::ensure_port(crate::tls::https_port(), "Project HTTPS", &[])?;
        }
        self.layout.save_projects(&next)?;
        if let Err(error) = self.reload_sites() {
            self.layout.save_projects(&previous)?;
            return Err(error);
        }
        Ok(())
    }

    fn reload_sites(&mut self) -> Result<()> {
        let _ = self.snapshot();
        if !self.runtime_ready() {
            return Err("The bundled web runtime is missing. Reinstall Fairway XMP.".into());
        }
        let config = self.layout.data.join("config/nginx.conf");
        let previous = fs::read(&config).map_err(|e| e.to_string())?;
        if let Err(error) = self.layout.write_configs(&self.ports) {
            atomic_write(&config, &previous)?;
            return Err(error);
        }
        let control = |args: &[&str]| {
            let mut cmd = self.nginx_command();
            cmd.args(args);
            process::output(cmd)
        };
        let result = control(&["-t"]).and_then(|_| {
            if self.processes.contains_key("nginx") {
                control(&["-s", "reload"])?;
            }
            Ok(())
        });
        if let Err(error) = result {
            atomic_write(&config, &previous)?;
            return Err(format!("Could not apply site changes: {error}"));
        }
        Ok(())
    }

    /// Add a project, or replace the one registered as `previous`.
    pub fn save_project(&mut self, previous: Option<&str>, project: ProjectConfig) -> Result<()> {
        if !project.root.is_dir() {
            return Err("The web root folder does not exist.".into());
        }
        let mut projects = self.layout.registered_projects()?;
        match previous {
            Some(domain) => {
                let slot = projects
                    .iter_mut()
                    .find(|p| p.domain == domain)
                    .ok_or("This site is no longer registered.")?;
                *slot = project;
            }
            None => projects.push(project),
        }
        self.apply_projects(projects)
    }

    pub fn remove_project(&mut self, domain: &str) -> Result<()> {
        let mut projects = self.layout.registered_projects()?;
        let count = projects.len();
        projects.retain(|p| p.domain != domain);
        if projects.len() == count {
            return Err("This site is no longer registered.".into());
        }
        self.apply_projects(projects)
    }

    pub fn set_project_https(&mut self, domain: &str, enabled: bool) -> Result<()> {
        let mut projects = self.layout.registered_projects()?;
        let project = projects
            .iter_mut()
            .find(|p| p.domain == domain)
            .ok_or("This site is no longer registered.")?;
        if project.https == enabled {
            return Ok(());
        }
        project.https = enabled;
        self.apply_projects(projects)
    }

    /// Shared certificate trust that no remaining HTTPS site needs any more.
    pub fn unused_trust(&self) -> Result<bool> {
        Ok(crate::tls::trusted(&self.layout)
            && !self.layout.registered_projects()?.iter().any(|p| p.https))
    }

    pub fn domains(&self) -> Result<Vec<String>> {
        Ok(self
            .layout
            .registered_projects()?
            .into_iter()
            .map(|p| p.domain)
            .collect())
    }

    pub fn save_settings(&mut self, ports: Ports, version: String) -> Result<()> {
        if !self.processes.is_empty() {
            return Err("Stop all services before changing settings.".into());
        }
        ports.validate()?;
        if !self.layout.php_versions()?.contains_key(&version) {
            return Err(format!(
                "PHP {version} is not bundled with this installation."
            ));
        }
        let previous = self.layout.php_version()?;
        let version_changed = version != previous;
        self.save_php_version(version)?;
        if let Err(error) = self.save_ports(ports) {
            let restore: Result<()> = (|| {
                if version_changed {
                    atomic_write(&self.layout.data.join("php-version"), previous.as_bytes())?;
                }
                self.layout.write_configs(&self.ports)?;
                if version_changed && crate::integration::installed(&self.layout) {
                    crate::integration::install(&self.layout)?;
                }
                Ok(())
            })();
            return match restore {
                Ok(()) => Err(error),
                Err(restore_error) => Err(format!(
                    "{error} Previous settings could not be fully restored: {restore_error}"
                )),
            };
        }
        Ok(())
    }

    pub fn save_php_version(&mut self, version: String) -> Result<()> {
        if !self.processes.is_empty() {
            return Err("Stop all services before changing PHP versions.".into());
        }
        if !self.layout.php_versions()?.contains_key(&version) {
            return Err(format!(
                "PHP {version} is not bundled with this installation."
            ));
        }
        let previous = self.layout.php_version()?;
        if version == previous {
            return Ok(());
        }
        let marker = self.layout.data.join("php-version");
        atomic_write(&marker, version.as_bytes())?;
        let result = (|| {
            if !self.layout.php_exe("php")?.is_file()
                || !self
                    .layout
                    .php_exe(if cfg!(windows) { "php-cgi" } else { "php-fpm" })?
                    .is_file()
            {
                return Err(format!("PHP {version} runtime files are missing."));
            }
            self.layout.write_configs(&self.ports)?;
            if crate::integration::installed(&self.layout) {
                crate::integration::install(&self.layout)?;
            }
            Ok(())
        })();
        if result.is_err() {
            let _ = atomic_write(&marker, previous.as_bytes());
            let _ = self.layout.write_configs(&self.ports);
            if crate::integration::installed(&self.layout) {
                let _ = crate::integration::install(&self.layout);
            }
        }
        result
    }

    pub fn start_all(&mut self) -> Result<()> {
        let ids: &[&str] = if self.mail_available() {
            &["mysql", "php", "nginx", "mail"]
        } else {
            &["mysql", "php", "nginx"]
        };
        self.preflight(ids)?;
        let before: Vec<String> = self.processes.keys().cloned().collect();
        for id in ids {
            if let Err(error) = self.start(id) {
                let mut cleanup = Vec::new();
                for started in ["mail", "nginx", "php", "mysql"] {
                    if !before.iter().any(|i| i == started) {
                        if let Err(e) = self.stop(started) {
                            cleanup.push(e);
                        }
                    }
                }
                return Err(format!(
                    "{error}{}",
                    if cleanup.is_empty() {
                        String::new()
                    } else {
                        format!(" Cleanup: {}", cleanup.join("; "))
                    }
                ));
            }
        }
        Ok(())
    }

    pub fn preflight(&self, ids: &[&str]) -> Result<()> {
        if !self.runtime_ready() {
            return Err("Fairway XMP’s PHP or database files are missing or incomplete. Reinstall Fairway XMP to restore them; your projects and database data will be kept.".into());
        }
        let mut excluded = self.ports.values().to_vec();
        excluded.push(crate::tls::https_port());
        for id in ids {
            if self.processes.contains_key(*id) {
                continue;
            }
            match *id {
                "nginx" => {
                    process::ensure_port(self.ports.website, "Website", &excluded)?;
                    process::ensure_port(self.ports.phpmyadmin, "phpMyAdmin", &excluded)?;
                    if self.layout.registered_projects()?.iter().any(|p| p.https) {
                        process::ensure_port(crate::tls::https_port(), "Project HTTPS", &excluded)?;
                    }
                }
                "mysql" => process::ensure_port(self.ports.mysql, "MySQL", &excluded)?,
                "php" => process::ensure_port(self.ports.php, "PHP", &excluded)?,
                "mail" => {
                    process::ensure_port(self.ports.mail, "Mail inbox", &excluded)?;
                    process::ensure_port(self.ports.smtp, "Mail SMTP", &excluded)?;
                }
                _ => return Err("Unknown service.".into()),
            }
        }
        Ok(())
    }

    pub fn start(&mut self, id: &str) -> Result<()> {
        if !["php", "mysql", "nginx", "mail"].contains(&id) {
            return Err("Unknown service.".into());
        }
        let _ = self.snapshot();
        if self.processes.contains_key(id) {
            return Ok(());
        }
        self.errors.remove(id);
        let result = self.start_inner(id);
        if let Err(e) = &result {
            self.errors.insert(id.into(), e.clone());
        }
        result
    }

    fn start_inner(&mut self, id: &str) -> Result<()> {
        if id == "mail" && !self.mail_available() {
            return Err("The mail catcher files are missing. Reinstall Fairway XMP to restore them; your projects and data will be kept.".into());
        }
        self.preflight(&[id])?;
        self.layout.write_configs(&self.ports)?;
        if id == "nginx" {
            let php_was_running = self.processes.contains_key("php");
            self.start("php")?;
            if let Err(e) = self
                .prepare_phpmyadmin()
                .and_then(|_| self.spawn_service(id))
            {
                if !php_was_running {
                    let _ = self.stop("php");
                }
                return Err(e);
            }
            return Ok(());
        }
        if id == "mysql" {
            crate::database::prepare_upgrade(&self.layout)?;
            self.initialize_database()?;
        }
        self.spawn_service(id)?;
        if id == "mysql" {
            if let Err(e) = self.provision_database() {
                // Keep the owned process tracked if shutdown fails, never orphan/kill MySQL.
                let shutdown = self.stop("mysql");
                return Err(format!(
                    "Database setup failed: {e}. {}",
                    shutdown.err().unwrap_or_default()
                ));
            }
        }
        Ok(())
    }

    fn php_environment(&self, cmd: &mut std::process::Command) {
        cmd.env("MIBS", "")
            .env("PHP_INI_SCAN_DIR", "")
            .env(
                "PHPRC",
                crate::config::windows_ansi_path(&self.layout.data.join("config/php.ini")),
            )
            .env("COMPOSER_HOME", self.layout.data.join("composer"));
    }

    fn spawn_service(&mut self, id: &str) -> Result<()> {
        crate::logs::trim_stopped(&self.layout.data.join("logs"), id)?;
        let mut cmd = match id {
            "php" => {
                let mut cmd = process::command(self.layout.php_exe(if cfg!(windows) {
                    "php-cgi"
                } else {
                    "php-fpm"
                })?);
                cmd.arg("-c").arg(crate::config::windows_ansi_path(
                    &self.layout.data.join("config/php.ini"),
                ));
                #[cfg(windows)]
                cmd.args(["-b", &format!("127.0.0.1:{}", self.ports.php)])
                    .env("PHP_FCGI_MAX_REQUESTS", "0");
                #[cfg(not(windows))]
                cmd.arg("--nodaemonize")
                    .arg("--fpm-config")
                    .arg(self.layout.data.join("config/php-fpm.conf"));
                self.php_environment(&mut cmd);
                cmd
            }
            "nginx" => {
                let mut check = self.nginx_command();
                check.arg("-t");
                process::output(check)?;
                let cmd = self.nginx_command();
                #[cfg(not(windows))]
                let cmd = {
                    let mut cmd = cmd;
                    cmd.args(["-g", "daemon off;"]);
                    cmd
                };
                cmd
            }
            "mysql" => {
                let mut cmd = process::command(crate::config::windows_ansi_path(
                    &self.layout.exe("mysql", "mysqld"),
                ));
                cmd.arg(format!(
                    "--defaults-file={}",
                    crate::config::windows_ansi_path(&self.layout.data.join("config/mysql.cnf"))
                        .display()
                ));
                #[cfg(windows)]
                cmd.arg("--console");
                cmd
            }
            "mail" => {
                let mut cmd = process::command(self.layout.exe("mailpit", "mailpit"));
                cmd.args(self.mail_args());
                cmd
            }
            _ => return Err("Unknown service.".into()),
        };
        // MySQL's Windows runtime also resolves files relative to its working
        // directory through ANSI APIs, even when all configured paths are aliases.
        cmd.current_dir(if id == "mysql" {
            crate::config::windows_ansi_path(&self.layout.data)
        } else {
            self.layout.data.clone()
        });
        let owned = OwnedProcess::spawn(
            cmd,
            &self.layout.data.join(format!("logs/{id}-process.log")),
            id == "mysql",
        )?;
        // Mailpit writes no PID file; record one so a restarted Fairway can adopt it.
        #[cfg(unix)]
        if id == "mail" {
            atomic_write(
                &self.layout.data.join("mail.pid"),
                owned.pid().to_string().as_bytes(),
            )?;
        }
        self.processes.insert(id.into(), owned);
        let deadline = Instant::now() + Duration::from_secs(if id == "mysql" { 60 } else { 15 });
        loop {
            if !self.processes.get_mut(id).is_some_and(OwnedProcess::alive) {
                self.processes.remove(id);
                return Err(format!(
                    "{id} exited during startup. Open its log for details."
                ));
            }
            let ready = match id {
                "nginx" => {
                    process::health(self.ports.website) && process::health(self.ports.phpmyadmin)
                }
                "mysql" => process::connected(self.ports.mysql),
                "mail" => {
                    process::connected(self.ports.mail) && process::connected(self.ports.smtp)
                }
                _ => process::connected(self.ports.php),
            };
            if ready {
                return Ok(());
            }
            if Instant::now() > deadline {
                if id != "mysql" {
                    let _ = self.stop(id);
                }
                return Err(format!(
                    "{id} did not become ready. Open its log, then retry or stop it."
                ));
            }
            thread::sleep(Duration::from_millis(150));
        }
    }

    fn initialize_database(&self) -> Result<()> {
        let data = self.layout.data.join("mysql");
        #[cfg(windows)]
        if [&data, &self.layout.runtime.join("mysql")]
            .iter()
            .any(|path| !crate::config::mysql_path_supported(path))
        {
            return Err("MySQL cannot use this Windows path: it is not representable in the Windows ANSI code page and no compatible short-name alias is available. Existing data has been kept. See the Windows path guidance in the data browser documentation.".into());
        }
        if data.join("mysql").exists() {
            return Ok(());
        }
        if fs::read_dir(&data)
            .map_err(|e| e.to_string())?
            .next()
            .is_some()
        {
            return Err("The database folder contains an incomplete initialization. It has been preserved; inspect the MySQL log before retrying.".into());
        }
        let mut cmd = process::command(crate::config::windows_ansi_path(
            &self.layout.exe("mysql", "mysqld"),
        ));
        cmd.arg(format!(
            "--defaults-file={}",
            crate::config::windows_ansi_path(&self.layout.data.join("config/mysql.cnf")).display()
        ))
        .arg("--initialize-insecure");
        process::output(cmd)?;
        atomic_write(
            &self.layout.data.join("database-version"),
            crate::database::runtime_family(&self.layout)?.as_bytes(),
        )
    }

    fn nginx_command(&self) -> std::process::Command {
        let mut cmd = process::command(self.layout.exe("nginx", "nginx"));
        cmd.current_dir(&self.layout.data);
        // Nginx's Windows main() receives ANSI argv, while its configuration
        // filenames use UTF-8. Keep argv ASCII and let Windows set the Unicode
        // working directory, without requiring NTFS short-name aliases.
        #[cfg(windows)]
        cmd.args([
            "-e",
            "logs/nginx.log",
            "-p",
            "./",
            "-c",
            "config/nginx.conf",
        ]);
        #[cfg(not(windows))]
        cmd.arg("-e")
            .arg(self.layout.data.join("logs/nginx.log"))
            .arg("-p")
            .arg(&self.layout.data)
            .arg("-c")
            .arg(self.layout.data.join("config/nginx.conf"));
        cmd
    }

    fn database_command(&self, executable: &str, secure: bool) -> std::process::Command {
        let mut cmd = process::command(crate::config::windows_ansi_path(
            &self.layout.exe("mysql", executable),
        ));
        if secure {
            cmd.arg(format!(
                "--defaults-file={}",
                crate::config::windows_ansi_path(&self.layout.data.join("config/admin.cnf"))
                    .display()
            ));
        } else {
            cmd.arg("--no-defaults").args([
                "--user=root",
                "--host=127.0.0.1",
                "--protocol=TCP",
                &format!("--port={}", self.ports.mysql),
            ]);
        }
        if executable != "mysqldump" {
            cmd.arg("--connect-timeout=3");
        }
        cmd
    }

    fn provision_database(&self) -> Result<()> {
        let marker = self.layout.data.join("database-ready");
        let secret = self.layout.secret()?;
        // Retry authenticated access first, allowing recovery after interrupted account setup.
        let secure = process::sql(self.database_command("mysql", true), "SELECT 1;").is_ok();
        if !secure {
            if marker.exists() {
                return Err("Cannot authenticate with the existing database. Credentials and data have been preserved.".into());
            }
            process::sql(
                self.database_command("mysql", false),
                &format!("ALTER USER 'root'@'localhost' IDENTIFIED BY '{secret}';"),
            )?;
        }
        if !marker.exists() {
            process::sql(self.database_command("mysql", true),
                "CREATE DATABASE IF NOT EXISTS fairway CHARACTER SET utf8mb4 COLLATE utf8mb4_unicode_ci;\nCREATE USER IF NOT EXISTS 'fairway'@'127.0.0.1' IDENTIFIED BY 'fairway';\nCREATE USER IF NOT EXISTS 'fairway'@'localhost' IDENTIFIED BY 'fairway';\nGRANT ALL PRIVILEGES ON *.* TO 'fairway'@'127.0.0.1';\nGRANT ALL PRIVILEGES ON *.* TO 'fairway'@'localhost';\n")?;
        }
        atomic_write(
            &marker,
            crate::database::runtime_family(&self.layout)?.as_bytes(),
        )?;
        atomic_write(
            &self.layout.data.join("database-version"),
            crate::database::runtime_family(&self.layout)?.as_bytes(),
        )?;
        Ok(())
    }

    fn prepare_phpmyadmin(&self) -> Result<()> {
        let source = self.layout.runtime.join("phpmyadmin");
        let target = self.layout.data.join("phpmyadmin");
        let manifest =
            fs::read(self.layout.runtime.join("manifest.json")).map_err(|e| e.to_string())?;
        let stamp = self.layout.data.join("phpmyadmin-manifest.json");
        if fs::read(&stamp).ok().as_deref() != Some(manifest.as_slice())
            || !target.join("index.php").is_file()
        {
            if target.exists() {
                fs::remove_dir_all(&target).map_err(|e| e.to_string())?;
            }
            copy_tree(&source, &target)?;
            atomic_write(&stamp, &manifest)?;
        }
        fs::copy(
            self.layout.data.join("config/config.inc.php"),
            target.join("config.inc.php"),
        )
        .map_err(|e| e.to_string())?;
        Ok(())
    }

    pub fn stop(&mut self, id: &str) -> Result<()> {
        if !["php", "mysql", "nginx", "mail"].contains(&id) {
            return Err("Unknown service.".into());
        }
        if id == "php" {
            self.stop("nginx")?;
        }
        if !self.processes.contains_key(id) {
            return Ok(());
        }
        if self.processes.get_mut(id).is_some_and(OwnedProcess::alive) {
            match id {
                "mysql" => {
                    let mut cmd = self.database_command("mysqladmin", true);
                    cmd.arg("shutdown");
                    process::output(cmd)?;
                }
                "nginx" => {
                    let mut cmd = self.nginx_command();
                    cmd.args(["-s", "quit"]);
                    process::output(cmd)?;
                }
                // Mailpit shuts down cleanly on SIGTERM; SIGQUIT would dump and abort.
                "mail" => self.processes.get_mut(id).unwrap().terminate_web()?,
                _ => {
                    #[cfg(unix)]
                    self.processes.get(id).unwrap().graceful_signal()?;
                    #[cfg(windows)]
                    self.processes.get_mut(id).unwrap().terminate_web()?;
                }
            }
            let process = self.processes.get_mut(id).unwrap();
            if !process.wait(if id == "mysql" { 30 } else { 10 }) {
                if id == "mysql" {
                    return Err("MySQL is still shutting down. Data has been preserved; wait a moment and retry Stop.".into());
                }
                process.terminate_web()?;
            }
        }
        self.processes.remove(id);
        self.errors.remove(id);
        if id == "mail" {
            let _ = fs::remove_file(self.layout.data.join("mail.pid"));
        }
        Ok(())
    }

    pub fn stop_all(&mut self) -> Result<()> {
        let mut errors = Vec::new();
        for id in ["mail", "nginx", "php", "mysql"] {
            if let Err(e) = self.stop(id) {
                errors.push(format!("{id}: {e}"));
            }
        }
        if errors.is_empty() {
            Ok(())
        } else {
            Err(errors.join("\n"))
        }
    }

    pub fn log(&self, id: &str) -> Result<String> {
        let files: &[&str] = match id {
            "php" => &["php.log", "php-fpm.log", "php-process.log"],
            "nginx" | "phpmyadmin" => &["nginx.log", "nginx-process.log"],
            "mysql" => &["mysql.log", "mysql-process.log"],
            "mail" => &["mail-process.log"],
            _ => return Err("Unknown log.".into()),
        };
        files
            .iter()
            .map(|name| {
                process::tail(&self.layout.data.join("logs").join(name))
                    .map(|text| format!("{name}\n{text}"))
            })
            .collect::<Result<Vec<_>>>()
            .map(|v| v.join("\n\n"))
    }

    pub fn backup(&self) -> Result<String> {
        if !self.processes.contains_key("mysql") {
            return Err("Start MySQL before making a backup.".into());
        }
        let (path, partial, mut output) = reserve_backup(&self.layout.data.join("backups"))?;
        let result = (|| {
            let mut cmd = self.database_command("mysqldump", true);
            // Back up student databases, not MySQL's version-dependent system schema.
            let mut list = self.database_command("mysql", true);
            // JSON preserves whitespace, Unicode and control characters in names.
            // Batch-mode escaping or trimming individual names can change them.
            list.args(["--batch", "--raw", "--skip-column-names", "--default-character-set=utf8mb4", "-e",
                "SELECT COALESCE(JSON_ARRAYAGG(SCHEMA_NAME), JSON_ARRAY()) FROM information_schema.SCHEMATA WHERE SCHEMA_NAME NOT IN ('mysql','sys','information_schema','performance_schema')"]);
            let names: Option<Vec<String>> = serde_json::from_str(&process::output(list)?)
                .map_err(|e| format!("Could not read database names for backup: {e}"))?;
            let mut names = names.unwrap_or_default();
            names.sort();
            if names
                .iter()
                .any(|name| !crate::config::mysql_cli_identifier_supported(name))
            {
                return Err("The Windows MySQL client cannot back up non-ASCII database names with the current system code page. Use ASCII database names before retrying. Your databases have been kept.".into());
            }
            if names.is_empty() {
                use std::io::Write;
                output
                    .write_all(b"-- Fairway XMP backup: no student databases were present.\n")
                    .map_err(|e| e.to_string())?;
                output.sync_all().map_err(|e| e.to_string())?;
                drop(output);
                fs::rename(&partial, &path).map_err(|e| e.to_string())?;
                return Ok(path.to_string_lossy().into());
            }
            cmd.args([
                "--single-transaction",
                "--routines",
                "--events",
                "--triggers",
                "--set-gtid-purged=OFF",
                "--databases",
                "--",
            ])
            .args(names)
            .stdout(Stdio::from(output))
            .stderr(Stdio::piped());
            let result = cmd.output().map_err(|e| e.to_string())?;
            if !result.status.success() {
                return Err(String::from_utf8_lossy(&result.stderr).into());
            }
            fs::rename(&partial, &path).map_err(|e| e.to_string())?;
            Ok(path.to_string_lossy().into())
        })();
        if result.is_err() {
            let _ = fs::remove_file(&partial);
        }
        result
    }
}

fn reserve_backup(directory: &Path) -> Result<(std::path::PathBuf, std::path::PathBuf, fs::File)> {
    let stamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_err(|e| e.to_string())?
        .as_nanos();
    for _ in 0..16 {
        let path = directory.join(format!(
            "fairway-{stamp}-{:032x}.sql",
            rand::random::<u128>()
        ));
        let partial = path.with_extension("sql.partial");
        if path.exists() {
            continue;
        }
        match fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&partial)
        {
            Ok(file) => return Ok((path, partial, file)),
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(error) => return Err(error.to_string()),
        }
    }
    Err("Could not reserve a unique backup file. Try again.".into())
}

fn copy_tree(source: &Path, target: &Path) -> Result<()> {
    fs::create_dir_all(target).map_err(|e| e.to_string())?;
    for entry in fs::read_dir(source).map_err(|e| e.to_string())? {
        let entry = entry.map_err(|e| e.to_string())?;
        let kind = entry.file_type().map_err(|e| e.to_string())?;
        if kind.is_symlink() {
            return Err("Unexpected link in the phpMyAdmin runtime.".into());
        }
        if kind.is_dir() {
            copy_tree(&entry.path(), &target.join(entry.file_name()))?;
        } else {
            fs::copy(entry.path(), target.join(entry.file_name())).map_err(|e| e.to_string())?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn snapshot_reports_mail_binary_availability_independently_of_its_manifest() {
        let root =
            std::env::temp_dir().join(format!("fairway-mail-available-{}", rand::random::<u64>()));
        let layout = Layout::at(root.join("home"), root.join("data"), root.join("runtime"));
        let mut manager = Manager::new(layout).unwrap();
        assert!(!manager.snapshot().mail_available);
        let executable = manager.layout.exe("mailpit", "mailpit");
        fs::create_dir_all(executable.parent().unwrap()).unwrap();
        fs::write(&executable, b"test binary").unwrap();
        assert!(manager.snapshot().mail_available);
        fs::remove_file(executable).unwrap();
        assert!(!manager.snapshot().mail_available);
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn unreadable_sites_are_reported_and_preserved_until_repaired() {
        let root =
            std::env::temp_dir().join(format!("fairway-sites-error-{}", rand::random::<u64>()));
        let layout = Layout::at(root.join("home"), root.join("data"), root.join("runtime"));
        let mut manager = Manager::new(layout).unwrap();
        let saved = manager.layout.data.join("sites.json");
        fs::write(&saved, b"invalid json").unwrap();
        let snapshot = manager.snapshot();
        assert!(snapshot
            .sites_error
            .unwrap()
            .contains("saved site list is invalid"));
        assert!(!snapshot.domains_pending);
        assert_eq!(fs::read(&saved).unwrap(), b"invalid json");
        manager.layout.save_projects(&[]).unwrap();
        assert!(manager.snapshot().sites_error.is_none());
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn trust_cleanup_requires_no_remaining_https_sites_and_a_readable_site_list() {
        let root =
            std::env::temp_dir().join(format!("fairway-trust-cleanup-{}", rand::random::<u64>()));
        let layout = Layout::at(root.join("home"), root.join("data"), root.join("runtime"));
        let manager = Manager::new(layout).unwrap();
        fs::create_dir_all(manager.layout.data.join("tls")).unwrap();
        fs::write(manager.layout.data.join("tls/trusted"), b"test marker only").unwrap();
        let mut projects = ["one", "two"].map(|name| ProjectConfig {
            name: name.into(),
            domain: name.into(),
            root: root.clone(),
            https: true,
            index_fallback: false,
        });
        manager.layout.save_projects(&projects).unwrap();
        assert!(!manager.unused_trust().unwrap());
        projects[0].https = false;
        manager.layout.save_projects(&projects).unwrap();
        assert!(!manager.unused_trust().unwrap());
        projects[1].https = false;
        manager.layout.save_projects(&projects).unwrap();
        assert!(manager.unused_trust().unwrap());
        fs::write(manager.layout.data.join("sites.json"), b"invalid json").unwrap();
        assert!(manager.unused_trust().is_err());
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn settings_save_validates_both_fields_before_changing_either() {
        let root = std::env::temp_dir().join(format!(
            "fairway-settings-{}-{}",
            std::process::id(),
            rand::random::<u64>()
        ));
        let layout = Layout::at(root.join("home"), root.join("data"), root.join("runtime"));
        fs::create_dir_all(&layout.runtime).unwrap();
        fs::write(
            layout.runtime.join("manifest.json"),
            br#"{"php":"8.5.11","phpVersions":{"8.4":"8.4.26","8.5":"8.5.11"}}"#,
        )
        .unwrap();
        let mut manager = Manager::new(layout).unwrap();
        let ports = Ports {
            website: 8810,
            ..Ports::default()
        };
        let invalid = Ports {
            mysql: 8810,
            ..ports.clone()
        };
        assert!(manager.save_settings(invalid, "8.4".into()).is_err());
        assert_eq!(manager.layout.php_version().unwrap(), "8.5");
        assert_eq!(manager.ports, Ports::default());
        assert!(manager.save_settings(ports.clone(), "8.4".into()).is_err());
        assert_eq!(manager.layout.php_version().unwrap(), "8.5");
        assert_eq!(manager.ports, Ports::default());
        let bin = manager.layout.runtime.join("php-8.4/bin");
        fs::create_dir_all(&bin).unwrap();
        for executable in ["php", if cfg!(windows) { "php-cgi" } else { "php-fpm" }] {
            let name = if cfg!(windows) {
                format!("{executable}.exe")
            } else {
                executable.into()
            };
            fs::write(bin.join(name), b"").unwrap();
        }
        let settings = manager.layout.data.join("settings.json");
        fs::create_dir(&settings).unwrap();
        assert!(manager.save_settings(ports.clone(), "8.4".into()).is_err());
        assert_eq!(manager.layout.php_version().unwrap(), "8.5");
        assert_eq!(manager.ports, Ports::default());
        fs::remove_dir(settings).unwrap();
        manager.save_settings(ports.clone(), "8.4".into()).unwrap();
        assert_eq!(manager.layout.php_version().unwrap(), "8.4");
        assert_eq!(manager.layout.ports().unwrap(), ports);
        fs::remove_dir_all(root).unwrap();
    }
}
