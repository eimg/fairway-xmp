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
}

impl Default for Ports {
    fn default() -> Self {
        Self {
            website: 8800,
            phpmyadmin: 8801,
            mysql: 13306,
            php: 19000,
        }
    }
}

impl Ports {
    pub fn validate(&self) -> Result<()> {
        let ports = [self.website, self.phpmyadmin, self.mysql, self.php];
        if ports.iter().any(|p| *p < 1024) {
            return Err("Choose ports between 1024 and 65535.".into());
        }
        if ports.into_iter().collect::<HashSet<_>>().len() != 4 {
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

/// A project the user registered. Saved registrations are the only source of project sites.
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
        if name.is_empty() || name.len() > 80 || name != project.name {
            return Err("Enter a project name up to 80 characters.".into());
        }
        if !valid_project_name(&project.domain) {
            return Err(format!(
                "{}.test is not available. Use lowercase letters, numbers, and hyphens.",
                project.domain
            ));
        }
        if !domains.insert(&project.domain) {
            return Err(format!(
                "{}.test is already used by another project.",
                project.domain
            ));
        }
        if !project.root.is_absolute() {
            return Err("Choose the project's web root folder.".into());
        }
    }
    Ok(())
}

impl Layout {
    fn projects_path(&self) -> PathBuf {
        self.data.join("projects.json")
    }

    pub fn registered_projects(&self) -> Result<Vec<ProjectConfig>> {
        let projects: Vec<ProjectConfig> = match fs::read(self.projects_path()) {
            Ok(bytes) => serde_json::from_slice(&bytes)
                .map_err(|e| format!("The saved project list is invalid: {e}"))?,
            Err(error) if error.kind() == io::ErrorKind::NotFound => Vec::new(),
            Err(error) => return Err(error.to_string()),
        };
        validate_projects(&projects)?;
        Ok(projects)
    }

    pub fn save_projects(&self, projects: &[ProjectConfig]) -> Result<()> {
        validate_projects(projects)?;
        atomic_write(
            &self.projects_path(),
            &serde_json::to_vec_pretty(projects).map_err(|e| e.to_string())?,
        )
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

    pub fn php_version(&self) -> Result<String> {
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
        if !self.php_versions()?.contains_key(&version) {
            return Err(format!(
                "PHP {version} is not bundled with this installation."
            ));
        }
        Ok(version)
    }

    pub fn php_root(&self) -> Result<PathBuf> {
        let selected = self.php_version()?;
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
                let ports: Ports = serde_json::from_slice(&bytes).map_err(|e| {
                    format!(
                        "Settings could not be read: {e}. Your original file has been preserved."
                    )
                })?;
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
        let conf = self.data.join("config");
        let php = format!(
            "[PHP]\nengine=On\nshort_open_tag=Off\nerror_reporting=E_ALL\ndisplay_errors=On\nlog_errors=On\nerror_log={}\ndate.timezone=UTC\nmemory_limit=256M\npost_max_size=64M\nupload_max_filesize=64M\nmax_execution_time=120\nsession.save_path={}\nupload_tmp_dir={}\nextension_dir={}\ncurl.cainfo={}\nopenssl.cafile={}\n{}",
            ini(&self.data.join("logs/php.log")), ini(&self.data.join("sessions")),
            ini(&self.data.join("tmp")), ini(&self.php_root()?.join("ext")),
            ini(&self.runtime.join("cacert.pem")), ini(&self.runtime.join("cacert.pem")),
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
            if [ports.website, ports.phpmyadmin, ports.mysql, ports.php]
                .contains(&crate::tls::https_port())
            {
                return Err(format!(
                    "Port {} is reserved for project HTTPS. Choose another service port.",
                    crate::tls::https_port()
                ));
            }
            crate::tls::ensure_certificate(self)?;
        }
        let nginx = nginx_config(self, ports, &projects);
        atomic_write(&conf.join("nginx.conf"), nginx.as_bytes())?;
        let mysql = format!(
            "[mysqld]\nbasedir={}\ndatadir={}\nport={}\nbind-address=127.0.0.1\nlower_case_table_names=1\nmysqlx=OFF\nlocal-infile=OFF\ncharacter-set-server=utf8mb4\ncollation-server=utf8mb4_unicode_ci\nmax_connections=40\ninnodb_buffer_pool_size=64M\nlog-error={}\npid-file={}\nsocket={}\nsecure-file-priv={}\n",
            ini(&self.runtime.join("mysql")), ini(&self.data.join("mysql")), ports.mysql,
            ini(&self.data.join("logs/mysql.log")), ini(&self.data.join("mysql.pid")),
            ini(&self.data.join("mysql.sock")), ini(&self.data.join("tmp")));
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
    format!(
        "\"{}\"",
        path.to_string_lossy()
            .replace('\\', "/")
            .replace('$', "\\$")
            .replace('"', "\\\"")
    )
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

fn nginx_config(layout: &Layout, ports: &Ports, projects: &[ProjectConfig]) -> String {
    let fastcgi = format!(
        "fastcgi_pass 127.0.0.1:{};\nfastcgi_param QUERY_STRING $query_string;\nfastcgi_param REQUEST_METHOD $request_method;\nfastcgi_param CONTENT_TYPE $content_type;\nfastcgi_param CONTENT_LENGTH $content_length;\nfastcgi_param SCRIPT_FILENAME $document_root$fastcgi_script_name;\nfastcgi_param SCRIPT_NAME $fastcgi_script_name;\nfastcgi_param REQUEST_URI $request_uri;\nfastcgi_param DOCUMENT_URI $document_uri;\nfastcgi_param DOCUMENT_ROOT $document_root;\nfastcgi_param SERVER_PROTOCOL $server_protocol;\nfastcgi_param REQUEST_SCHEME $scheme;\nfastcgi_param GATEWAY_INTERFACE CGI/1.1;\nfastcgi_param SERVER_SOFTWARE nginx;\nfastcgi_param REMOTE_ADDR $remote_addr;\nfastcgi_param REMOTE_PORT $remote_port;\nfastcgi_param SERVER_ADDR $server_addr;\nfastcgi_param SERVER_PORT $server_port;\nfastcgi_param SERVER_NAME $server_name;\nfastcgi_param REDIRECT_STATUS 200;\nfastcgi_param HTTP_PROXY \"\";\n", ports.php);
    // A missing web root stays configured (Nginx answers 404), so restoring the folder
    // brings the site back without another change.
    let project_servers = projects.iter().map(|project| {
        let fallback = if project.index_fallback { "/index.php?$query_string" } else { "=404" };
        let locations = format!("location ~ /\\. {{ deny all; }}\nlocation / {{ try_files $uri $uri/ {fallback}; }}\nlocation ~ \\.php$ {{ try_files $uri =404; {fastcgi} }}");
        let server = |listen: String, tls: String| format!("server {{ listen 127.0.0.1:{listen}; server_name {}.test; root {}; index index.php index.html;\n{tls}{locations}\n}}\n",
            project.domain, nginx_path(&project.root));
        let http = server(ports.website.to_string(), String::new());
        if !project.https {
            return http;
        }
        let https = server(format!("{} ssl", crate::tls::https_port()), format!("ssl_certificate {};\nssl_certificate_key {};\n",
            nginx_path(&crate::tls::certificate(layout)), nginx_path(&crate::tls::private_key(layout))));
        format!("{http}{https}")
    }).collect::<String>();
    format!(
        "worker_processes 1;\npid {};\nerror_log {};\nevents {{ worker_connections 256; }}\nhttp {{\ninclude {};\ndefault_type application/octet-stream;\naccess_log {};\nclient_max_body_size 64m;\nclient_body_temp_path {};\nfastcgi_temp_path {};\nproxy_temp_path {};\nuwsgi_temp_path {};\nscgi_temp_path {};\nserver {{ listen 127.0.0.1:{}; server_name localhost; root {}; index index.php index.html;\nlocation = /__fairway_health {{ access_log off; default_type text/plain; return 200 'fairway'; }}\nlocation ~ /\\. {{ deny all; }}\nlocation / {{ try_files $uri $uri/ =404; }}\nlocation ~ \\.php$ {{ try_files $uri =404; {} }}\n}}\nserver {{ listen 127.0.0.1:{}; server_name localhost; root {}; index index.php;\nlocation = /__fairway_health {{ access_log off; default_type text/plain; return 200 'fairway'; }}\nlocation ~ /\\. {{ deny all; }}\nlocation = /config.inc.php {{ deny all; }}\nlocation ~ ^/(libraries|templates|vendor|setup)/ {{ deny all; }}\nlocation / {{ try_files $uri $uri/ =404; }}\nlocation ~ \\.php$ {{ try_files $uri =404; {} }}\n}}\n{}\n}}\n",
        nginx_path(&layout.data.join("nginx.pid")), nginx_path(&layout.data.join("logs/nginx.log")),
        nginx_path(&layout.runtime.join("nginx/conf/mime.types")), nginx_path(&layout.data.join("logs/access.log")),
        nginx_path(&layout.data.join("tmp/client")), nginx_path(&layout.data.join("tmp/fastcgi")),
        nginx_path(&layout.data.join("tmp/proxy")), nginx_path(&layout.data.join("tmp/uwsgi")), nginx_path(&layout.data.join("tmp/scgi")),
        ports.website, nginx_path(&layout.www), fastcgi, ports.phpmyadmin,
        nginx_path(&layout.data.join("phpmyadmin")), fastcgi, project_servers)
}

fn windows_php_extensions(php_root: &Path) -> String {
    let mut extensions = String::from("extension=curl\nextension=fileinfo\nextension=gd\nextension=intl\nextension=mbstring\nextension=mysqli\nextension=openssl\nextension=pdo_mysql\nextension=pdo_sqlite\nextension=sodium\nextension=sqlite3\n");
    // PHP 8.0/8.1 ship ZIP in the binary; later Windows releases ship php_zip.dll.
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
    fn configurations_escape_paths_and_keep_roots_separate() {
        let layout = Layout::at(
            PathBuf::from("/Users/A $name"),
            PathBuf::from("/tmp/data"),
            PathBuf::from("/tmp/runtime"),
        );
        assert_eq!(layout.www, PathBuf::from("/Users/A $name/fairway/www"));
        let nginx = nginx_config(&layout, &Ports::default(), &[]);
        assert!(nginx.contains("/Users/A \\$name/fairway/www"));
        assert!(nginx.contains("listen 127.0.0.1:8801"));
        assert!(nginx.contains("/tmp/data/phpmyadmin"));
        assert!(!nginx.contains("listen 0.0.0.0"));
        assert_eq!(php_string("a'b\\c"), "'a\\'b\\\\c'");
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
        let nginx = nginx_config(&layout, &Ports::default(), &projects);
        let localhost = format!("server_name localhost; root {}", nginx_path(&layout.www));
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
        fs::write(layout.data.join("php-version"), "8.3").unwrap();
        assert!(layout.php_version().is_err());
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
        assert!(!windows_php_extensions(&root).contains("extension=zip"));
        fs::write(root.join("ext/php_zip.dll"), b"").unwrap();
        assert!(windows_php_extensions(&root).contains("extension=zip\n"));
        fs::remove_dir_all(root).unwrap();
    }
}
