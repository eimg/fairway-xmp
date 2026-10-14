use crate::{
    config::{atomic_write, Layout, Ports, Result},
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
    pub data_dir: String,
    pub runtime_ready: bool,
    pub runtime_versions: serde_json::Value,
    pub php_version: String,
    pub php_versions: BTreeMap<String, String>,
    pub path_installed: bool,
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
        #[cfg(target_os = "linux")]
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
        let pid_file = manager.layout.data.join("mysql.pid");
        if let Ok(text) = fs::read_to_string(&pid_file) {
            if let Ok(pid) = text.trim().parse::<u32>() {
                if process::matches_database(
                    pid,
                    &manager.layout.exe("mysql", "mysqld"),
                    &manager.layout.data.join("config/mysql.cnf"),
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
        let services = [
            ("php", self.ports.php, php),
            ("nginx", self.ports.website, nginx && php),
            ("mysql", self.ports.mysql, mysql),
            ("phpmyadmin", self.ports.phpmyadmin, admin),
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
        Snapshot {
            ports: self.ports.clone(),
            services,
            document_root: self.layout.www.to_string_lossy().into(),
            data_dir: self.layout.data.to_string_lossy().into(),
            runtime_ready: self.runtime_ready(),
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
        self.preflight(&["php", "nginx", "mysql"])?;
        let before: Vec<String> = self.processes.keys().cloned().collect();
        for id in ["mysql", "php", "nginx"] {
            if let Err(error) = self.start(id) {
                let mut cleanup = Vec::new();
                for started in ["nginx", "php", "mysql"] {
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
        let excluded = [
            self.ports.website,
            self.ports.phpmyadmin,
            self.ports.mysql,
            self.ports.php,
        ];
        for id in ids {
            if self.processes.contains_key(*id) {
                continue;
            }
            match *id {
                "nginx" => {
                    process::ensure_port(self.ports.website, "Website", &excluded)?;
                    process::ensure_port(self.ports.phpmyadmin, "phpMyAdmin", &excluded)?;
                }
                "mysql" => process::ensure_port(self.ports.mysql, "MySQL", &excluded)?,
                "php" => process::ensure_port(self.ports.php, "PHP", &excluded)?,
                _ => return Err("Unknown service.".into()),
            }
        }
        Ok(())
    }

    pub fn start(&mut self, id: &str) -> Result<()> {
        if !["php", "mysql", "nginx"].contains(&id) {
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
            .env("PHPRC", self.layout.data.join("config/php.ini"))
            .env("COMPOSER_HOME", self.layout.data.join("composer"));
    }

    fn spawn_service(&mut self, id: &str) -> Result<()> {
        let mut cmd = match id {
            "php" => {
                let mut cmd = process::command(self.layout.php_exe(if cfg!(windows) {
                    "php-cgi"
                } else {
                    "php-fpm"
                })?);
                cmd.arg("-c").arg(self.layout.data.join("config/php.ini"));
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
                let mut check = process::command(self.layout.exe("nginx", "nginx"));
                check
                    .arg("-e")
                    .arg(self.layout.data.join("logs/nginx.log"))
                    .arg("-t")
                    .arg("-p")
                    .arg(&self.layout.data)
                    .arg("-c")
                    .arg(self.layout.data.join("config/nginx.conf"));
                process::output(check)?;
                let mut cmd = process::command(self.layout.exe("nginx", "nginx"));
                cmd.arg("-e")
                    .arg(self.layout.data.join("logs/nginx.log"))
                    .arg("-p")
                    .arg(&self.layout.data)
                    .arg("-c")
                    .arg(self.layout.data.join("config/nginx.conf"));
                #[cfg(not(windows))]
                cmd.args(["-g", "daemon off;"]);
                cmd
            }
            "mysql" => {
                let mut cmd = process::command(self.layout.exe("mysql", "mysqld"));
                cmd.arg(format!(
                    "--defaults-file={}",
                    self.layout.data.join("config/mysql.cnf").display()
                ));
                #[cfg(windows)]
                cmd.arg("--console");
                cmd
            }
            _ => return Err("Unknown service.".into()),
        };
        cmd.current_dir(&self.layout.data);
        let owned = OwnedProcess::spawn(
            cmd,
            &self.layout.data.join(format!("logs/{id}-process.log")),
            id == "mysql",
        )?;
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
        let mut cmd = process::command(self.layout.exe("mysql", "mysqld"));
        cmd.arg(format!(
            "--defaults-file={}",
            self.layout.data.join("config/mysql.cnf").display()
        ))
        .arg("--initialize-insecure");
        process::output(cmd)?;
        atomic_write(
            &self.layout.data.join("database-version"),
            crate::database::runtime_family(&self.layout)?.as_bytes(),
        )
    }

    fn database_command(&self, executable: &str, secure: bool) -> std::process::Command {
        let mut cmd = process::command(self.layout.exe("mysql", executable));
        if secure {
            cmd.arg(format!(
                "--defaults-file={}",
                self.layout.data.join("config/admin.cnf").display()
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
        if !["php", "mysql", "nginx"].contains(&id) {
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
                    let mut cmd = process::command(self.layout.exe("nginx", "nginx"));
                    cmd.arg("-e")
                        .arg(self.layout.data.join("logs/nginx.log"))
                        .arg("-p")
                        .arg(&self.layout.data)
                        .arg("-c")
                        .arg(self.layout.data.join("config/nginx.conf"))
                        .args(["-s", "quit"]);
                    process::output(cmd)?;
                }
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
        Ok(())
    }

    pub fn stop_all(&mut self) -> Result<()> {
        let mut errors = Vec::new();
        for id in ["nginx", "php", "mysql"] {
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
        let stamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_err(|e| e.to_string())?
            .as_secs();
        let path = self
            .layout
            .data
            .join(format!("backups/fairway-{stamp}.sql"));
        let partial = path.with_extension("sql.partial");
        let output = fs::File::create(&partial).map_err(|e| e.to_string())?;
        let mut cmd = self.database_command("mysqldump", true);
        // Back up student databases, not MySQL's version-dependent system schema.
        let mut list = self.database_command("mysql", true);
        list.args(["--batch", "--skip-column-names", "-e", "SHOW DATABASES"]);
        let names: Vec<_> = process::output(list)?
            .lines()
            .filter(|name| {
                !["mysql", "sys", "information_schema", "performance_schema"].contains(name)
            })
            .map(String::from)
            .collect();
        if names.is_empty() {
            return Err("There are no student databases to back up.".into());
        }
        cmd.args([
            "--single-transaction",
            "--routines",
            "--events",
            "--triggers",
            "--set-gtid-purged=OFF",
            "--databases",
        ])
        .args(names)
        .stdout(Stdio::from(output))
        .stderr(Stdio::piped());
        let result = cmd.output().map_err(|e| e.to_string())?;
        if !result.status.success() {
            let _ = fs::remove_file(partial);
            return Err(String::from_utf8_lossy(&result.stderr).into());
        }
        fs::rename(partial, &path).map_err(|e| e.to_string())?;
        Ok(path.to_string_lossy().into())
    }
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
