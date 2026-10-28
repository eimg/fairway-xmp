use rand::Rng;
use serde::{Deserialize, Serialize};
use std::{
    collections::HashSet,
    fs, io,
    io::Write,
    path::{Path, PathBuf},
};

pub type Result<T> = std::result::Result<T, String>;

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Ports {
    pub website: u16,
    pub phpmyadmin: u16,
    pub mysql: u16,
    pub php: u16,
    /// Mail inbox (web UI). Layout::ports migrates missing mail ports without collisions.
    #[serde(default = "default_mail_port")]
    pub mail: u16,
    #[serde(default = "default_smtp_port")]
    pub smtp: u16,
}

fn default_mail_port() -> u16 {
    8802
}

fn default_smtp_port() -> u16 {
    11025
}

impl Default for Ports {
    fn default() -> Self {
        Self {
            website: 8800,
            phpmyadmin: 8801,
            mysql: 13306,
            php: 19000,
            mail: default_mail_port(),
            smtp: default_smtp_port(),
        }
    }
}

impl Ports {
    pub fn values(&self) -> [u16; 6] {
        [
            self.website,
            self.phpmyadmin,
            self.mysql,
            self.php,
            self.mail,
            self.smtp,
        ]
    }

    pub fn validate(&self) -> Result<()> {
        let ports = self.values();
        if ports.iter().any(|p| *p < 1024) {
            return Err("Choose ports between 1024 and 65535.".into());
        }
        if ports.into_iter().collect::<HashSet<_>>().len() != ports.len() {
            return Err("Each service needs a different port.".into());
        }
        Ok(())
    }
}

#[derive(Clone, Debug)]
pub struct Layout {
    pub home: PathBuf,
    pub data: PathBuf,
    pub runtime: PathBuf,
    pub www: PathBuf,
    pub bin: PathBuf,
}

/// A site the user registered. Saved registrations are the only source of sites.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ProjectConfig {
    pub name: String,
    /// DNS label served as `<domain>.test`.
    pub domain: String,
    pub root: PathBuf,
    pub https: bool,
    pub index_fallback: bool,
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Project {
    #[serde(flatten)]
    pub config: ProjectConfig,
    pub available: bool,
    pub domain_ready: bool,
}

pub fn valid_project_name(name: &str) -> bool {
    name.len() <= 63
        && name
            .bytes()
            .next()
            .is_some_and(|b| b.is_ascii_lowercase() || b.is_ascii_digit())
        && name
            .bytes()
            .last()
            .is_some_and(|b| b.is_ascii_lowercase() || b.is_ascii_digit())
        && name
            .bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-')
}

pub fn validate_projects(projects: &[ProjectConfig]) -> Result<()> {
    let mut domains = HashSet::new();
    for project in projects {
        let name = project.name.trim();
        if name.is_empty() || name.chars().count() > 80 || name != project.name {
            return Err("Enter a site name up to 80 characters.".into());
        }
        if !valid_project_name(&project.domain) {
            return Err(format!(
                "{}.test is not available. Use lowercase letters, numbers, and hyphens.",
                project.domain
            ));
        }
        if !domains.insert(&project.domain) {
            return Err(format!(
                "{}.test is already used by another site.",
                project.domain
            ));
        }
        if !project.root.is_absolute() {
            return Err("Choose the site's web root folder.".into());
        }
    }
    Ok(())
}

impl Layout {
    fn sites_path(&self) -> PathBuf {
        self.data.join("sites.json")
    }

    fn legacy_sites_path(&self) -> PathBuf {
        self.data.join("projects.json")
    }

    pub fn registered_projects(&self) -> Result<Vec<ProjectConfig>> {
        let (bytes, migrate) = match fs::read(self.sites_path()) {
            Ok(bytes) => (bytes, false),
            Err(error) if error.kind() == io::ErrorKind::NotFound => {
                match fs::read(self.legacy_sites_path()) {
                    Ok(bytes) => (bytes, true),
                    Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(Vec::new()),
                    Err(error) => return Err(error.to_string()),
                }
            }
            Err(error) => return Err(error.to_string()),
        };
        let projects: Vec<ProjectConfig> = serde_json::from_slice(&bytes)
            .map_err(|e| format!("The saved site list is invalid: {e}"))?;
        validate_projects(&projects)?;
        if migrate {
            self.save_projects(&projects)?;
        } else if self.legacy_sites_path().exists() {
            fs::remove_file(self.legacy_sites_path()).map_err(|e| e.to_string())?;
        }
        Ok(projects)
    }

    pub fn save_projects(&self, projects: &[ProjectConfig]) -> Result<()> {
        validate_projects(projects)?;
        atomic_write(
            &self.sites_path(),
            &serde_json::to_vec_pretty(projects).map_err(|e| e.to_string())?,
        )?;
        if self.legacy_sites_path().exists() {
            fs::remove_file(self.legacy_sites_path()).map_err(|e| e.to_string())?;
        }
        Ok(())
    }

    pub fn new(runtime: PathBuf) -> Result<Self> {
        #[cfg(debug_assertions)]
        if let (Some(home), Some(data)) = (
            std::env::var_os("FAIRWAY_DEV_HOME"),
            std::env::var_os("FAIRWAY_DEV_DATA"),
        ) {
            return Ok(Self::at(home.into(), data.into(), runtime));
        }
        let home = dirs::home_dir().ok_or("Cannot find your home folder.")?;
        let data = dirs::data_local_dir()
            .ok_or("Cannot find your application data folder.")?
            .join("dev.fairway.environment");
        Ok(Self::at(home, data, runtime))
    }

    pub fn at(home: PathBuf, data: PathBuf, runtime: PathBuf) -> Self {
        Self {
            www: home.join("fairway/www"),
            bin: data.join("bin"),
            home,
            data,
            runtime,
        }
    }

    pub fn initialize(&self) -> Result<()> {
        for path in [
            self.www.clone(),
            self.bin.clone(),
            self.data.join("config"),
            self.data.join("logs"),
            self.data.join("mysql"),
            self.data.join("tmp"),
            self.data.join("sessions"),
            self.data.join("composer"),
            self.data.join("backups"),
            self.data.join("mail"),
        ] {
            fs::create_dir_all(path).map_err(|e| e.to_string())?;
        }
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            fs::set_permissions(&self.data, fs::Permissions::from_mode(0o700))
                .map_err(|e| e.to_string())?;
            fs::set_permissions(self.data.join("tmp"), fs::Permissions::from_mode(0o700))
                .map_err(|e| e.to_string())?;
        }
        if !self.www.join("index.php").exists() {
            fs::write(
                self.www.join("index.php"),
                include_str!("../resources/welcome.php"),
            )
            .map_err(|e| e.to_string())?;
        }
        Ok(())
    }

    pub fn exe(&self, service: &str, executable: &str) -> PathBuf {
        let name = if cfg!(windows) {
            format!("{executable}.exe")
        } else {
            executable.into()
        };
        self.runtime.join(service).join("bin").join(name)
    }

    pub fn php_versions(&self) -> Result<serde_json::Map<String, serde_json::Value>> {
        let manifest = self.runtime_manifest()?;
        if let Some(versions) = manifest["phpVersions"].as_object() {
            return Ok(versions.clone());
        }
        let version = manifest["php"]
            .as_str()
            .ok_or("The bundled PHP version is missing.")?;
        let minor = version.split('.').take(2).collect::<Vec<_>>().join(".");
        Ok(serde_json::Map::from_iter([(
            minor,
            serde_json::Value::String(version.into()),
        )]))
    }

    pub(crate) fn php_selection(&self) -> Result<String> {
        let version = match fs::read_to_string(self.data.join("php-version")) {
            Ok(version) => version.trim().to_string(),
            Err(e) if e.kind() == io::ErrorKind::NotFound => {
                let manifest = self.runtime_manifest()?;
                manifest["php"]
                    .as_str()
                    .ok_or("The bundled PHP version is missing.")?
                    .split('.')
                    .take(2)
                    .collect::<Vec<_>>()
                    .join(".")
            }
            Err(e) => return Err(e.to_string()),
        };
        Ok(version)
    }

    pub(crate) fn removed_php_selected(&self) -> Result<bool> {
        let version = self.php_selection()?;
        Ok(matches!(version.as_str(), "8.0" | "8.1")
            && !self.php_versions()?.contains_key(&version))
    }

    pub fn php_version(&self) -> Result<String> {
        let version = self.php_selection()?;
        if !self.php_versions()?.contains_key(&version) {
            return Err(unavailable_php_message(&version));
        }
        Ok(version)
    }

    pub fn php_root(&self) -> Result<PathBuf> {
        self.php_root_for_version(&self.php_version()?)
    }

    pub fn php_root_for_version(&self, selected: &str) -> Result<PathBuf> {
        if !self.php_versions()?.contains_key(selected) {
            return Err(unavailable_php_message(selected));
        }
        let manifest = self.runtime_manifest()?;
        let default = manifest["php"]
            .as_str()
            .ok_or("The bundled PHP version is missing.")?;
        if default.starts_with(&format!("{selected}.")) {
            Ok(self.runtime.join("php"))
        } else {
            Ok(self.runtime.join(format!("php-{selected}")))
        }
    }

    pub fn php_exe(&self, executable: &str) -> Result<PathBuf> {
        let name = if cfg!(windows) {
            format!("{executable}.exe")
        } else {
            executable.into()
        };
        Ok(self.php_root()?.join("bin").join(name))
    }

    fn runtime_manifest(&self) -> Result<serde_json::Value> {
        let bytes = match fs::read(self.runtime.join("manifest.json")) {
            Ok(bytes) => bytes,
            Err(e) if e.kind() == io::ErrorKind::NotFound => {
                include_bytes!("../../runtime-manifest.json").to_vec()
            }
            Err(e) => return Err(e.to_string()),
        };
        serde_json::from_slice(&bytes).map_err(|e| e.to_string())
    }

    pub fn ports(&self) -> Result<Ports> {
        let path = self.data.join("settings.json");
        match fs::read(&path) {
            Ok(bytes) => {
                let mut ports: Ports = serde_json::from_slice(&bytes).map_err(|e| {
                    format!(
                        "Settings could not be read: {e}. Your original file has been preserved."
                    )
                })?;
                // Inspect presence after typed parsing, which also rejects duplicate,
                // unknown and invalid fields. Only missing fields may be reassigned.
                let saved: serde_json::Value =
                    serde_json::from_slice(&bytes).map_err(|e| e.to_string())?;
                let missing_mail = saved.get("mail").is_none();
                let missing_smtp = saved.get("smtp").is_none();
                if missing_mail || missing_smtp {
                    let mut reserved = HashSet::from([
                        ports.website,
                        ports.phpmyadmin,
                        ports.mysql,
                        ports.php,
                        crate::tls::https_port(),
                    ]);
                    if !missing_mail {
                        reserved.insert(ports.mail);
                    }
                    if !missing_smtp {
                        reserved.insert(ports.smtp);
                    }
                    for (port, missing) in [
                        (&mut ports.mail, missing_mail),
                        (&mut ports.smtp, missing_smtp),
                    ] {
                        if missing {
                            *port = (*port..=u16::MAX)
                                .chain(1024..*port)
                                .find(|candidate| reserved.insert(*candidate))
                                .ok_or("No distinct port is available for mail.")?;
                        }
                    }
                    // Validate before writing so invalid existing settings stay intact.
                    self.save_ports(&ports)?;
                }
                ports.validate()?;
                Ok(ports)
            }
            Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(Ports::default()),
            Err(e) => Err(e.to_string()),
        }
    }

    pub fn save_ports(&self, ports: &Ports) -> Result<()> {
        ports.validate()?;
        atomic_write(
            &self.data.join("settings.json"),
            &serde_json::to_vec_pretty(ports).map_err(|e| e.to_string())?,
        )
    }

    pub fn write_configs(&self, ports: &Ports) -> Result<()> {
        ports.validate()?;
        let settings =
            crate::php_settings::Overrides::load(self)?.settings(&self.php_version()?)?;
        let conf = self.data.join("config");
        // Match PHP's development INI: keep host variables in getenv()/$_SERVER,
        // outside $_ENV, which Laravel filters when spawning artisan serve.
        // Otherwise its child loses PHPRC, PHP_INI_SCAN_DIR and MIBS.
        let php = format!(
            "[PHP]\nengine=On\nshort_open_tag=Off\nvariables_order=GPCS\nerror_reporting=E_ALL\nlog_errors=On\nerror_log={}\n{}session.save_path={}\nupload_tmp_dir={}\nextension_dir={}\ncurl.cainfo={}\nopenssl.cafile={}\n{}{}",
            ini(&self.data.join("logs/php.log")), settings.ini(), ini(&self.data.join("sessions")),
            ini(&self.data.join("tmp")), ini(&self.php_root()?.join("ext")),
            ini(&self.runtime.join("cacert.pem")), ini(&self.runtime.join("cacert.pem")),
            php_mail(self, ports),
            if cfg!(windows) { windows_php_extensions(&self.php_root()?) } else { String::new() }
        );
        atomic_write(&conf.join("php.ini"), php.as_bytes())?;
        let fpm = format!(
            "[global]\npid={}\nerror_log={}\ndaemonize=no\n[www]\nlisten=127.0.0.1:{}\nlisten.allowed_clients=127.0.0.1\npm=ondemand\npm.max_children=4\npm.process_idle_timeout=10s\ncatch_workers_output=yes\nclear_env=no\n",
            ini(&self.data.join("php.pid")), ini(&self.data.join("logs/php-fpm.log")), ports.php);
        atomic_write(&conf.join("php-fpm.conf"), fpm.as_bytes())?;
        // ponytail: an unreadable project list must not stop the basic localhost server;
        // edits still read it strictly, so it is never overwritten silently.
        let projects = self.registered_projects().unwrap_or_default();
        if projects.iter().any(|project| project.https) {
            if ports.values().contains(&crate::tls::https_port()) {
                return Err(format!(
                    "Port {} is reserved for site HTTPS. Choose another service port.",
                    crate::tls::https_port()
                ));
            }
            crate::tls::ensure_certificate(self)?;
        }
        let nginx = nginx_config(self, ports, &projects, &settings);
        atomic_write(&conf.join("nginx.conf"), nginx.as_bytes())?;
        let mysql = format!(
            "[mysqld]\nbasedir={}\ndatadir={}\nport={}\nbind-address=127.0.0.1\nlower_case_table_names=1\nmysqlx=OFF\nlocal-infile=OFF\ncharacter-set-server=utf8mb4\ncollation-server=utf8mb4_unicode_ci\nmax_connections=40\ninnodb_buffer_pool_size=64M\nlog-error={}\npid-file={}\nsocket={}\nsecure-file-priv={}\n",
            ini(&self.runtime.join("mysql")), ini(&self.data.join("mysql")), ports.mysql,
            ini(&self.data.join("logs/mysql.log")), ini(&self.data.join("mysql.pid")),
            ini(Path::new("mysql.sock")), ini(&self.data.join("tmp")));
        atomic_write(&conf.join("mysql.cnf"), mysql.as_bytes())?;
        let secret = self.secret()?;
        let admin = format!(
            "[client]\nuser=root\npassword={secret}\nhost=127.0.0.1\nport={}\nprotocol=TCP\n",
            ports.mysql
        );
        atomic_write(&conf.join("admin.cnf"), admin.as_bytes())?;
        private_file(&conf.join("admin.cnf"))?;
        let phpmyadmin_config = format!(
            "<?php\n$cfg['blowfish_secret'] = '{}';\n$i = 1;\n$cfg['Servers'][$i]['auth_type'] = 'cookie';\n$cfg['Servers'][$i]['host'] = '127.0.0.1';\n$cfg['Servers'][$i]['port'] = '{}';\n$cfg['Servers'][$i]['AllowNoPassword'] = false;\n$cfg['Servers'][$i]['AllowRoot'] = false;\n$cfg['TempDir'] = {};\n$cfg['PmaAbsoluteUri'] = 'http://127.0.0.1:{}/';\n$cfg['CheckConfigurationPermissions'] = true;\n$cfg['VersionCheck'] = false;\n", &secret[..32], ports.mysql,
            php_string(&self.data.join("tmp").to_string_lossy()), ports.phpmyadmin);
        // A writable overlay keeps generated configuration outside the signed app bundle.
        atomic_write(&conf.join("config.inc.php"), phpmyadmin_config.as_bytes())?;
        #[cfg(feature = "terminal")]
        if self.data.join("terminal-bin").is_dir() {
            crate::terminal::prepare_tools(self)?;
        }
        Ok(())
    }

    pub fn secret(&self) -> Result<String> {
        let path = self.data.join("credentials");
        match fs::read_to_string(&path) {
            Ok(s) if s.len() == 48 && s.chars().all(|c| c.is_ascii_hexdigit()) => Ok(s),
            Ok(_) => Err("Database credentials file is invalid. It has been preserved.".into()),
            Err(e) if e.kind() == io::ErrorKind::NotFound => {
                let bytes: [u8; 24] = rand::rng().random();
                let s: String = bytes.iter().map(|b| format!("{b:02x}")).collect();
                atomic_write(&path, s.as_bytes())?;
                private_file(&path)?;
                Ok(s)
            }
            Err(e) => Err(e.to_string()),
        }
    }
}

pub fn php_string(s: &str) -> String {
    format!("'{}'", s.replace('\\', "\\\\").replace('\'', "\\'"))
}

// MySQL paths and PHP startup configuration on Windows use ANSI APIs.
// Existing NTFS short-name aliases let it open the same files without relocating
// student data. Resolve the existing parent for files not created yet.
pub fn windows_ansi_path(path: &Path) -> PathBuf {
    #[cfg(windows)]
    if !path.as_os_str().as_encoded_bytes().is_ascii() {
        use std::os::windows::ffi::{OsStrExt, OsStringExt};
        unsafe extern "system" {
            fn GetShortPathNameW(long: *const u16, short: *mut u16, size: u32) -> u32;
        }
        let mut parent = path;
        let mut tail = Vec::new();
        while !parent.exists() {
            let (Some(name), Some(next)) = (parent.file_name(), parent.parent()) else {
                break;
            };
            tail.push(name.to_owned());
            parent = next;
        }
        let wide: Vec<_> = parent.as_os_str().encode_wide().chain(Some(0)).collect();
        let size = unsafe { GetShortPathNameW(wide.as_ptr(), std::ptr::null_mut(), 0) };
        if size > 0 {
            let mut buffer = vec![0u16; size as usize];
            let count = unsafe { GetShortPathNameW(wide.as_ptr(), buffer.as_mut_ptr(), size) };
            if count > 0 && count < size {
                let mut short =
                    PathBuf::from(std::ffi::OsString::from_wide(&buffer[..count as usize]));
                for name in tail.into_iter().rev() {
                    short.push(name);
                }
                return short;
            }
        }
    }
    path.to_path_buf()
}

pub fn mysql_path_supported(path: &Path) -> bool {
    #[cfg(windows)]
    {
        use std::os::windows::ffi::OsStrExt;
        unsafe extern "system" {
            fn GetACP() -> u32;
            fn WideCharToMultiByte(
                page: u32,
                flags: u32,
                wide: *const u16,
                count: i32,
                output: *mut u8,
                size: i32,
                default: *const u8,
                used_default: *mut i32,
            ) -> i32;
        }
        if unsafe { GetACP() } == 65001 {
            return true;
        }
        let path = windows_ansi_path(path);
        let wide: Vec<_> = path.as_os_str().encode_wide().collect();
        let mut lossy = 0;
        let size = unsafe {
            WideCharToMultiByte(
                0,
                0x400,
                wide.as_ptr(),
                wide.len() as i32,
                std::ptr::null_mut(),
                0,
                std::ptr::null(),
                &mut lossy,
            )
        };
        size > 0 && lossy == 0
    }
    #[cfg(not(windows))]
    {
        let _ = path;
        true
    }
}

pub fn mysql_cli_identifier_supported(name: &str) -> bool {
    #[cfg(windows)]
    {
        unsafe extern "system" {
            fn GetACP() -> u32;
        }
        // The vendor CLI receives narrow argv. Even representable ANSI bytes
        // differ from UTF-8, which is the connection charset used for dumps.
        name.is_ascii() || unsafe { GetACP() } == 65001
    }
    #[cfg(not(windows))]
    {
        let _ = name;
        true
    }
}

pub fn ini(path: &Path) -> String {
    format!(
        "\"{}\"",
        windows_ansi_path(path)
            .to_string_lossy()
            .replace('\\', "/")
            .replace('"', "\\\"")
    )
}

pub fn nginx_path(path: &Path) -> String {
    #[cfg(windows)]
    let resolved = nginx_long_path(path);
    #[cfg(windows)]
    let path = resolved.as_path();
    format!(
        "\"{}\"",
        path.to_string_lossy()
            .replace('\\', "/")
            .replace('"', "\\\"")
    )
}

#[cfg(windows)]
fn nginx_long_path(path: &Path) -> PathBuf {
    // Unlike MySQL, Nginx reads UTF-8 paths and rejects NTFS short-name aliases
    // during its filename safety checks. Windows TEMP can contain RUNNER~1 (or
    // another alias). Expand existing parents even for not-yet-created logs and
    // temporary directories, without moving any files.
    if !path.is_absolute() {
        return path.to_path_buf();
    }
    let mut parent = path;
    let mut tail = Vec::new();
    while !parent.exists() {
        let (Some(name), Some(next)) = (parent.file_name(), parent.parent()) else {
            return path.to_path_buf();
        };
        tail.push(name.to_owned());
        parent = next;
    }
    let Ok(resolved) = fs::canonicalize(parent) else {
        return path.to_path_buf();
    };
    let text = resolved.to_string_lossy();
    let mut long = if let Some(unc) = text.strip_prefix(r"\\?\UNC\") {
        PathBuf::from(format!(r"\\{unc}"))
    } else {
        PathBuf::from(text.strip_prefix(r"\\?\").unwrap_or(&text))
    };
    for name in tail.into_iter().rev() {
        long.push(name);
    }
    long
}

// Directives such as root, access_log and ssl_certificate compile variables even
// inside quotes. A geo value is literal; referencing it preserves a real '$'.
fn nginx_variable_path(path: &Path) -> String {
    nginx_path(path).replace('$', "${fairway_dollar}")
}

pub fn atomic_write(path: &Path, bytes: &[u8]) -> Result<()> {
    let tmp = path.with_extension("fairway-tmp");
    let mut options = fs::OpenOptions::new();
    options.write(true).create(true).truncate(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let mut file = options.open(&tmp).map_err(|e| e.to_string())?;
    file.write_all(bytes).map_err(|e| e.to_string())?;
    file.sync_all().map_err(|e| e.to_string())?;
    if let Ok(metadata) = fs::metadata(path) {
        fs::set_permissions(&tmp, metadata.permissions()).map_err(|e| e.to_string())?;
    }
    drop(file);
    // Windows rename cannot replace a destination; use a platform replacement API.
    #[cfg(windows)]
    {
        use std::os::windows::ffi::OsStrExt;
        unsafe extern "system" {
            fn MoveFileExW(a: *const u16, b: *const u16, flags: u32) -> i32;
        }
        let a: Vec<u16> = tmp.as_os_str().encode_wide().chain(Some(0)).collect();
        let b: Vec<u16> = path.as_os_str().encode_wide().chain(Some(0)).collect();
        if unsafe { MoveFileExW(a.as_ptr(), b.as_ptr(), 0x1 | 0x8) } == 0 {
            return Err(io::Error::last_os_error().to_string());
        }
    }
    #[cfg(not(windows))]
    fs::rename(tmp, path).map_err(|e| e.to_string())?;
    Ok(())
}

pub fn private_file(path: &Path) -> Result<()> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(path, fs::Permissions::from_mode(0o600)).map_err(|e| e.to_string())?;
    }
    #[cfg(not(unix))]
    let _ = path;
    Ok(())
}

fn nginx_config(
    layout: &Layout,
    ports: &Ports,
    projects: &[ProjectConfig],
    settings: &crate::php_settings::PhpSettings,
) -> String {
    let request_limit_mb = settings.request_limit_mb();
    let read_timeout_seconds = settings.fastcgi_read_timeout_seconds();
    let fastcgi = format!(
        "fastcgi_pass 127.0.0.1:{};\nfastcgi_param QUERY_STRING $query_string;\nfastcgi_param REQUEST_METHOD $request_method;\nfastcgi_param CONTENT_TYPE $content_type;\nfastcgi_param CONTENT_LENGTH $content_length;\nfastcgi_param SCRIPT_FILENAME $document_root$fastcgi_script_name;\nfastcgi_param SCRIPT_NAME $fastcgi_script_name;\nfastcgi_param REQUEST_URI $request_uri;\nfastcgi_param DOCUMENT_URI $document_uri;\nfastcgi_param DOCUMENT_ROOT $document_root;\nfastcgi_param SERVER_PROTOCOL $server_protocol;\nfastcgi_param REQUEST_SCHEME $scheme;\nfastcgi_param GATEWAY_INTERFACE CGI/1.1;\nfastcgi_param SERVER_SOFTWARE nginx;\nfastcgi_param REMOTE_ADDR $remote_addr;\nfastcgi_param REMOTE_PORT $remote_port;\nfastcgi_param SERVER_ADDR $server_addr;\nfastcgi_param SERVER_PORT $server_port;\nfastcgi_param SERVER_NAME $server_name;\nfastcgi_param REDIRECT_STATUS 200;\nfastcgi_param HTTP_PROXY \"\";\n", ports.php);
    // A missing web root stays configured (Nginx answers 404), so restoring the folder
    // brings the site back without another change.
    let project_servers = projects.iter().map(|project| {
        let fallback = if project.index_fallback { "/index.php?$query_string" } else { "=404" };
        let locations = format!("location ~ /\\. {{ deny all; }}\nlocation / {{ try_files $uri $uri/ {fallback}; }}\nlocation ~ \\.php$ {{ try_files $uri =404; {fastcgi} }}");
        let server = |listen: String, tls: String| format!("server {{ listen 127.0.0.1:{listen}; server_name {}.test; root {}; index index.php index.html;\n{tls}{locations}\n}}\n",
            project.domain, nginx_variable_path(&project.root));
        let http = server(ports.website.to_string(), String::new());
        if !project.https {
            return http;
        }
        let https = server(format!("{} ssl", crate::tls::https_port()), format!("ssl_certificate {};\nssl_certificate_key {};\n",
            nginx_variable_path(&crate::tls::certificate(layout)), nginx_variable_path(&crate::tls::private_key(layout))));
        format!("{http}{https}")
    }).collect::<String>();
    let mut rejected = format!("server {{ listen 127.0.0.1:{} default_server; server_name _; return 444; }}\nserver {{ listen 127.0.0.1:{} default_server; server_name _; return 444; }}\n", ports.website, ports.phpmyadmin);
    if projects.iter().any(|project| project.https) {
        rejected.push_str(&format!("server {{ listen 127.0.0.1:{} ssl default_server; ssl_reject_handshake on; server_name _; return 444; }}\n", crate::tls::https_port()));
    }
    format!(
        "worker_processes 1;\npid {};\nerror_log {};\nevents {{ worker_connections 256; }}\nhttp {{\ninclude {};\ndefault_type application/octet-stream;\ngeo $fairway_dollar {{ default \"$\"; }}\nmap $request_uri $fairway_request_path {{ ~^([^?]*) $1; default $uri; }}\nlog_format fairway escape=json '{{\"time\":\"$time_iso8601\",\"method\":\"$request_method\",\"uri\":\"$fairway_request_path\",\"status\":\"$status\",\"host\":\"$host\",\"port\":\"$server_port\",\"scheme\":\"$scheme\",\"duration\":\"$request_time\"}}';\naccess_log {} fairway;\nclient_max_body_size {request_limit_mb}m;\nfastcgi_read_timeout {read_timeout_seconds}s;\nclient_body_temp_path {};\nfastcgi_temp_path {};\nproxy_temp_path {};\nuwsgi_temp_path {};\nscgi_temp_path {};\n{rejected}server {{ listen 127.0.0.1:{}; server_name localhost 127.0.0.1; root {}; index index.php index.html;\nlocation = /__fairway_health {{ access_log off; default_type text/plain; return 200 'fairway'; }}\nlocation ~ /\\. {{ deny all; }}\nlocation / {{ try_files $uri $uri/ =404; }}\nlocation ~ \\.php$ {{ try_files $uri =404; {} }}\n}}\nserver {{ listen 127.0.0.1:{}; server_name localhost 127.0.0.1; root {}; index index.php;\nlocation = /__fairway_health {{ access_log off; default_type text/plain; return 200 'fairway'; }}\nlocation ~ /\\. {{ deny all; }}\nlocation = /config.inc.php {{ deny all; }}\nlocation ~ ^/(libraries|templates|vendor|setup)/ {{ deny all; }}\nlocation / {{ try_files $uri $uri/ =404; }}\nlocation ~ \\.php$ {{ try_files $uri =404; {} }}\n}}\n{}\n}}\n",
        nginx_path(&layout.data.join("nginx.pid")), nginx_path(&layout.data.join("logs/nginx.log")),
        nginx_path(&layout.runtime.join("nginx/conf/mime.types")), nginx_variable_path(&layout.data.join("logs/access.log")),
        nginx_path(&layout.data.join("tmp/client")), nginx_path(&layout.data.join("tmp/fastcgi")),
        nginx_path(&layout.data.join("tmp/proxy")), nginx_path(&layout.data.join("tmp/uwsgi")), nginx_path(&layout.data.join("tmp/scgi")),
        ports.website, nginx_variable_path(&layout.www), fastcgi, ports.phpmyadmin,
        nginx_variable_path(&layout.data.join("phpmyadmin")), fastcgi, project_servers)
}

/// Send PHP's mail() to the bundled mail catcher, so nothing reaches real inboxes.
fn php_mail(layout: &Layout, ports: &Ports) -> String {
    if cfg!(windows) {
        return format!("SMTP=127.0.0.1\nsmtp_port={}\n", ports.smtp);
    }
    // PHP runs sendmail_path through /bin/sh; quote the program for the shell.
    let program = layout
        .exe("mailpit", "mailpit")
        .to_string_lossy()
        .replace('\'', "'\\''");
    format!(
        "sendmail_path=\"'{}' sendmail -S 127.0.0.1:{}\"\n",
        program.replace('"', "\\\""),
        ports.smtp
    )
}

pub(crate) fn unavailable_php_message(version: &str) -> String {
    if version == "8.0" || version == "8.1" {
        format!(
            "PHP {version} is no longer included. Your selection was kept. Choose PHP 8.2 or later."
        )
    } else {
        format!("PHP {version} is not bundled with this installation.")
    }
}

fn windows_php_extensions(php_root: &Path) -> String {
    let mut extensions = String::from("extension=curl\nextension=fileinfo\nextension=gd\nextension=intl\nextension=ldap\nextension=mbstring\nextension=mysqli\nextension=openssl\nextension=pdo_mysql\nextension=pdo_pgsql\nextension=pdo_sqlite\nextension=pgsql\nextension=soap\nextension=sockets\nextension=sodium\nextension=sqlite3\n");
    // Load ZIP only when this PHP build ships php_zip.dll.
    if php_root.join("ext/php_zip.dll").is_file() {
        extensions.push_str("extension=zip\n");
    }
    extensions
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn ports_reject_duplicates_and_privileged_ports() {
        assert!(Ports::default().validate().is_ok());
        assert!(Ports {
            mysql: 8800,
            ..Ports::default()
        }
        .validate()
        .is_err());
        assert!(Ports {
            website: 80,
            ..Ports::default()
        }
        .validate()
        .is_err());
    }
    #[test]
    fn nginx_timeout_follows_php_and_is_inherited_by_every_site() {
        let layout = Layout::at(
            PathBuf::from("/tmp/home"),
            PathBuf::from("/tmp/data"),
            PathBuf::from("/tmp/runtime"),
        );
        let projects = [ProjectConfig {
            name: "Shop".into(),
            domain: "shop".into(),
            root: layout.www.clone(),
            https: true,
            index_fallback: true,
        }];
        for (execution, expected_timeout) in [(120, 150), (1, 31), (86_400, 86_430), (0, 86_400)] {
            let settings = crate::php_settings::PhpSettings {
                max_execution_time: execution,
                ..Default::default()
            };
            let config = nginx_config(&layout, &Ports::default(), &projects, &settings);
            assert_eq!(config.matches("fastcgi_read_timeout ").count(), 1);
            let directive = format!("fastcgi_read_timeout {expected_timeout}s;");
            let position = config.find(&directive).expect("the aligned read timeout");
            assert!(config.find("http {\n").unwrap() < position);
            assert!(position < config.find("server { listen").unwrap());
            // Each PHP location inherits the shared directive: localhost,
            // phpMyAdmin and the registered site's HTTP and HTTPS servers.
            assert_eq!(config.matches("fastcgi_pass ").count(), 4);
            assert!(config.contains("listen 127.0.0.1:8801;"));
            assert!(config.contains("server_name shop.test;"));
            assert!(config.contains(" ssl;"));
        }
    }

    #[test]
    fn configurations_escape_paths_and_keep_roots_separate() {
        let layout = Layout::at(
            PathBuf::from("/Users/A $name"),
            PathBuf::from("/tmp/data"),
            PathBuf::from("/tmp/runtime"),
        );
        assert_eq!(layout.www, PathBuf::from("/Users/A $name/fairway/www"));
        let nginx = nginx_config(&layout, &Ports::default(), &[], &Default::default());
        assert!(nginx.contains("/Users/A ${fairway_dollar}name/fairway/www"));
        assert_eq!(nginx_path(Path::new("/tmp/A $name")), "\"/tmp/A $name\"");
        assert!(nginx.contains("listen 127.0.0.1:8801"));
        assert!(nginx.contains("/tmp/data/phpmyadmin"));
        assert!(!nginx.contains("listen 0.0.0.0"));
        assert_eq!(php_string("a'b\\c"), "'a\\'b\\\\c'");
    }

    #[cfg(windows)]
    #[test]
    fn nginx_expands_short_aliases_and_keeps_unicode_for_future_files() {
        let root =
            std::env::temp_dir().join(format!("fairway-Nginx 日本-{}", rand::random::<u64>()));
        fs::create_dir_all(&root).unwrap();
        let alias = windows_ansi_path(&root);
        assert_eq!(
            nginx_path(&alias.join("pending/logfile")),
            nginx_path(&root.join("pending/logfile"))
        );
        let rendered = nginx_path(&root);
        assert!(rendered.contains("Nginx 日本"));
        assert!(!rendered.contains("//?/"));
        fs::remove_dir_all(root).unwrap();
    }

    #[cfg(unix)]
    #[test]
    fn php_mail_goes_to_the_bundled_catcher_with_a_shell_quoted_path() {
        let layout = Layout::at(
            PathBuf::from("/tmp/home"),
            PathBuf::from("/tmp/data"),
            PathBuf::from("/Apps/Fair way's/runtime"),
        );
        let ports = Ports {
            smtp: 12025,
            ..Ports::default()
        };
        assert_eq!(
            php_mail(&layout, &ports),
            "sendmail_path=\"'/Apps/Fair way'\\''s/runtime/mailpit/bin/mailpit' sendmail -S 127.0.0.1:12025\"\n"
        );
    }

    #[test]
    fn settings_saved_before_the_mail_catcher_use_default_mail_ports() {
        let ports: Ports =
            serde_json::from_str(r#"{"website":8810,"phpmyadmin":8811,"mysql":13307,"php":19001}"#)
                .unwrap();
        assert_eq!((ports.mail, ports.smtp), (8802, 11025));
        assert!(Ports {
            smtp: 8800,
            ..Ports::default()
        }
        .validate()
        .is_err());
    }

    #[test]
    fn startup_migrates_only_missing_mail_ports_and_persists_them() {
        let root =
            std::env::temp_dir().join(format!("fairway-mail-migration-{}", rand::random::<u64>()));
        let cases = [
            (8800, 8801, 13306, 19000, None, None, (8802, 11025)),
            (8802, 8801, 13306, 19000, None, None, (8803, 11025)),
            (8800, 8801, 11025, 19000, None, None, (8802, 11026)),
            (8802, 8803, 11025, 11026, None, None, (8804, 11027)),
            (8800, 8801, 13306, 19000, Some(11025), None, (11025, 11026)),
            (8800, 8801, 13306, 19000, None, Some(8802), (8803, 8802)),
        ];
        for (index, (website, phpmyadmin, mysql, php, mail, smtp, expected)) in
            cases.into_iter().enumerate()
        {
            let directory = root.join(index.to_string());
            let layout = Layout::at(
                directory.join("home"),
                directory.join("data"),
                root.join("runtime"),
            );
            layout.initialize().unwrap();
            let mut saved = serde_json::json!({ "website": website, "phpmyadmin": phpmyadmin, "mysql": mysql, "php": php });
            if let Some(mail) = mail {
                saved["mail"] = mail.into();
            }
            if let Some(smtp) = smtp {
                saved["smtp"] = smtp.into();
            }
            let path = layout.data.join("settings.json");
            fs::write(&path, serde_json::to_vec(&saved).unwrap()).unwrap();
            let manager = crate::manager::Manager::new(layout.clone()).unwrap();
            assert_eq!((manager.ports.mail, manager.ports.smtp), expected);
            assert_eq!(
                (
                    manager.ports.website,
                    manager.ports.phpmyadmin,
                    manager.ports.mysql,
                    manager.ports.php
                ),
                (website, phpmyadmin, mysql, php)
            );
            let persisted: Ports = serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
            assert_eq!(persisted, manager.ports);
            assert_eq!(
                crate::manager::Manager::new(layout).unwrap().ports,
                persisted
            );
        }
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn port_migration_preserves_explicit_settings_and_invalid_files() {
        let root =
            std::env::temp_dir().join(format!("fairway-mail-settings-{}", rand::random::<u64>()));
        let layout = Layout::at(root.join("home"), root.join("data"), root.join("runtime"));
        layout.initialize().unwrap();
        let path = layout.data.join("settings.json");
        let explicit = br#"{ "website":8800,"phpmyadmin":8801,"mysql":13306,"php":19000,"mail":9900,"smtp":9901 }"#;
        fs::write(&path, explicit).unwrap();
        assert_eq!(
            (layout.ports().unwrap().mail, layout.ports().unwrap().smtp),
            (9900, 9901)
        );
        assert_eq!(fs::read(&path).unwrap(), explicit);
        for invalid in [
            r#"{"website":8800,"website":8802,"phpmyadmin":8801,"mysql":13306,"php":19000}"#,
            r#"{"website":8800,"phpmyadmin":8801,"mysql":13306,"php":19000,"unknown":1}"#,
            r#"{"website":8800,"phpmyadmin":8800,"mysql":13306,"php":19000}"#,
            r#"{"website":8800,"phpmyadmin":8801,"mysql":13306,"php":19000,"mail":8800}"#,
            r#"{"website":8800,"phpmyadmin":8801,"mysql":13306,"php":19000,"smtp":80}"#,
            r#"{"website":8800,"phpmyadmin":8801,"mysql":13306,"php":19000,"mail":null}"#,
        ] {
            fs::write(&path, invalid).unwrap();
            assert!(layout.ports().is_err());
            assert_eq!(fs::read(&path).unwrap(), invalid.as_bytes());
        }
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn registered_projects_serve_their_chosen_root_including_missing_folders() {
        let root = std::env::temp_dir().join(format!(
            "fairway-sites-{}-{}",
            std::process::id(),
            rand::random::<u64>()
        ));
        let layout = Layout::at(root.join("home"), root.join("data"), root.join("runtime"));
        layout.initialize().unwrap();
        assert!(layout.registered_projects().unwrap().is_empty());
        let shop = root.join("code/shop/public");
        fs::create_dir_all(&shop).unwrap();
        let project =
            |name: &str, domain: &str, root: PathBuf, index_fallback: bool| ProjectConfig {
                name: name.into(),
                domain: domain.into(),
                root,
                https: false,
                index_fallback,
            };
        let projects = vec![
            project("My Shop", "shop", shop.clone(), true),
            project("Plain", "plain", layout.www.clone(), false),
            project("Gone", "gone", root.join("missing"), false),
        ];
        layout.save_projects(&projects).unwrap();
        assert_eq!(layout.registered_projects().unwrap(), projects);
        let nginx = nginx_config(&layout, &Ports::default(), &projects, &Default::default());
        let localhost = format!(
            "server_name localhost 127.0.0.1; root {}",
            nginx_path(&layout.www)
        );
        assert!(nginx.find(&localhost) < nginx.find("server_name shop.test"));
        assert!(nginx.contains(&format!(
            "server_name shop.test; root {}",
            nginx_path(&shop)
        )));
        assert_eq!(nginx.matches("/index.php?$query_string").count(), 1);
        assert!(nginx.contains("server_name plain.test"));
        assert!(nginx.contains("server_name gone.test"));
        for invalid in [
            vec![project("", "shop", shop.clone(), false)],
            vec![project("Shop", "Shop", shop.clone(), false)],
            vec![project("Shop", "shop", "relative".into(), false)],
            vec![
                projects[0].clone(),
                project("Again", "shop", shop.clone(), false),
            ],
        ] {
            assert!(layout.save_projects(&invalid).is_err());
        }
        assert!(layout.data.join("sites.json").is_file());
        fs::write(
            layout.data.join("projects.json"),
            fs::read(layout.data.join("sites.json")).unwrap(),
        )
        .unwrap();
        fs::remove_file(layout.data.join("sites.json")).unwrap();
        assert_eq!(layout.registered_projects().unwrap(), projects);
        assert!(layout.data.join("sites.json").is_file());
        assert!(!layout.data.join("projects.json").exists());
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn php_selection_uses_the_matching_runtime_and_rejects_unknown_versions() {
        let root = std::env::temp_dir().join(format!(
            "fairway-php-{}-{}",
            std::process::id(),
            rand::random::<u64>()
        ));
        let layout = Layout::at(root.join("home"), root.join("data"), root.join("runtime"));
        fs::create_dir_all(&layout.runtime).unwrap();
        fs::create_dir_all(&layout.data).unwrap();
        fs::write(
            layout.runtime.join("manifest.json"),
            br#"{"php":"8.5.11","phpVersions":{"8.4":"8.4.26","8.5":"8.5.11"}}"#,
        )
        .unwrap();
        assert_eq!(layout.php_root().unwrap(), layout.runtime.join("php"));
        fs::write(layout.data.join("php-version"), "8.4").unwrap();
        assert_eq!(layout.php_root().unwrap(), layout.runtime.join("php-8.4"));
        fs::write(layout.data.join("php-version"), "8.0").unwrap();
        let removed = layout.php_version().unwrap_err();
        assert!(removed.contains("no longer included"), "{removed}");
        assert_eq!(
            fs::read_to_string(layout.data.join("php-version")).unwrap(),
            "8.0"
        );
        fs::write(layout.data.join("php-version"), "8.3").unwrap();
        let unknown = layout.php_version().unwrap_err();
        assert!(unknown.contains("not bundled"), "{unknown}");
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn windows_zip_configuration_follows_the_selected_runtime() {
        let root = std::env::temp_dir().join(format!(
            "fairway-zip-{}-{}",
            std::process::id(),
            rand::random::<u64>()
        ));
        fs::create_dir_all(root.join("ext")).unwrap();
        assert!(windows_php_extensions(&root).contains("extension=pdo_pgsql\n"));
        assert!(windows_php_extensions(&root).contains("extension=pgsql\n"));
        for extension in ["sockets", "soap", "ldap"] {
            assert!(windows_php_extensions(&root).contains(&format!("extension={extension}\n")));
        }
        for extension in ["pcntl", "posix"] {
            assert!(!windows_php_extensions(&root).contains(&format!("extension={extension}\n")));
        }
        assert!(!windows_php_extensions(&root).contains("extension=zip"));
        fs::write(root.join("ext/php_zip.dll"), b"").unwrap();
        assert!(windows_php_extensions(&root).contains("extension=zip\n"));
        fs::remove_dir_all(root).unwrap();
    }
}
