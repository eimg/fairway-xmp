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

impl Layout {
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
        let nginx = nginx_config(self, ports);
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

pub fn ini(path: &Path) -> String {
    format!(
        "\"{}\"",
        path.to_string_lossy()
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

fn nginx_config(layout: &Layout, ports: &Ports) -> String {
    let fastcgi = format!(
        "fastcgi_pass 127.0.0.1:{};\nfastcgi_param QUERY_STRING $query_string;\nfastcgi_param REQUEST_METHOD $request_method;\nfastcgi_param CONTENT_TYPE $content_type;\nfastcgi_param CONTENT_LENGTH $content_length;\nfastcgi_param SCRIPT_FILENAME $document_root$fastcgi_script_name;\nfastcgi_param SCRIPT_NAME $fastcgi_script_name;\nfastcgi_param REQUEST_URI $request_uri;\nfastcgi_param DOCUMENT_URI $document_uri;\nfastcgi_param DOCUMENT_ROOT $document_root;\nfastcgi_param SERVER_PROTOCOL $server_protocol;\nfastcgi_param REQUEST_SCHEME $scheme;\nfastcgi_param GATEWAY_INTERFACE CGI/1.1;\nfastcgi_param SERVER_SOFTWARE nginx;\nfastcgi_param REMOTE_ADDR $remote_addr;\nfastcgi_param REMOTE_PORT $remote_port;\nfastcgi_param SERVER_ADDR $server_addr;\nfastcgi_param SERVER_PORT $server_port;\nfastcgi_param SERVER_NAME $server_name;\nfastcgi_param REDIRECT_STATUS 200;\nfastcgi_param HTTP_PROXY \"\";\n", ports.php);
    format!(
        "worker_processes 1;\npid {};\nerror_log {};\nevents {{ worker_connections 256; }}\nhttp {{\ninclude {};\ndefault_type application/octet-stream;\naccess_log {};\nclient_max_body_size 64m;\nclient_body_temp_path {};\nfastcgi_temp_path {};\nproxy_temp_path {};\nuwsgi_temp_path {};\nscgi_temp_path {};\nserver {{ listen 127.0.0.1:{}; server_name localhost; root {}; index index.php index.html;\nlocation = /__fairway_health {{ access_log off; default_type text/plain; return 200 'fairway'; }}\nlocation ~ /\\. {{ deny all; }}\nlocation / {{ try_files $uri $uri/ =404; }}\nlocation ~ \\.php$ {{ try_files $uri =404; {} }}\n}}\nserver {{ listen 127.0.0.1:{}; server_name localhost; root {}; index index.php;\nlocation = /__fairway_health {{ access_log off; default_type text/plain; return 200 'fairway'; }}\nlocation ~ /\\. {{ deny all; }}\nlocation = /config.inc.php {{ deny all; }}\nlocation ~ ^/(libraries|templates|vendor|setup)/ {{ deny all; }}\nlocation / {{ try_files $uri $uri/ =404; }}\nlocation ~ \\.php$ {{ try_files $uri =404; {} }}\n}}\n}}\n",
        nginx_path(&layout.data.join("nginx.pid")), nginx_path(&layout.data.join("logs/nginx.log")),
        nginx_path(&layout.runtime.join("nginx/conf/mime.types")), nginx_path(&layout.data.join("logs/access.log")),
        nginx_path(&layout.data.join("tmp/client")), nginx_path(&layout.data.join("tmp/fastcgi")),
        nginx_path(&layout.data.join("tmp/proxy")), nginx_path(&layout.data.join("tmp/uwsgi")), nginx_path(&layout.data.join("tmp/scgi")),
        ports.website, nginx_path(&layout.www), fastcgi, ports.phpmyadmin,
        nginx_path(&layout.data.join("phpmyadmin")), fastcgi)
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
        let nginx = nginx_config(&layout, &Ports::default());
        assert!(nginx.contains("/Users/A \\$name/fairway/www"));
        assert!(nginx.contains("listen 127.0.0.1:8801"));
        assert!(nginx.contains("/tmp/data/phpmyadmin"));
        assert!(!nginx.contains("listen 0.0.0.0"));
        assert_eq!(php_string("a'b\\c"), "'a\\'b\\\\c'");
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
