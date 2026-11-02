//! Curated PHP overrides live in user data, independently of bundled patch releases.
use crate::config::{atomic_write, Layout, Result};
use serde::{Deserialize, Serialize};
use std::{collections::BTreeMap, fs};

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase", deny_unknown_fields)]
pub struct PhpSettings {
    pub memory_limit_mb: i32,
    pub max_execution_time: u32,
    pub upload_max_filesize_mb: u32,
    pub timezone: String,
    pub display_errors: bool,
    pub session_upload_progress: bool,
}

impl Default for PhpSettings {
    fn default() -> Self {
        Self {
            memory_limit_mb: 256,
            max_execution_time: 120,
            upload_max_filesize_mb: 64,
            timezone: "UTC".into(),
            display_errors: true,
            session_upload_progress: true,
        }
    }
}

impl PhpSettings {
    pub fn validate(&self) -> Result<()> {
        if self.memory_limit_mb != -1 && !(16..=1_048_576).contains(&self.memory_limit_mb) {
            return Err("Memory limit must be -1 (unlimited) or 16–1048576 MiB.".into());
        }
        if self.max_execution_time > 86_400 {
            return Err("Execution time must be 0 (unlimited) or up to 86400 seconds.".into());
        }
        if !(1..=16_384).contains(&self.upload_max_filesize_mb) {
            return Err("Upload size must be between 1 and 16384 MiB.".into());
        }
        if self.timezone.is_empty()
            || self.timezone.len() > 100
            || !self
                .timezone
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b"/_+-".contains(&b))
        {
            return Err("Choose a valid PHP timezone, such as UTC or Asia/Yangon.".into());
        }
        Ok(())
    }

    /// Leave room for multipart fields and headers above the per-file limit.
    pub fn request_limit_mb(&self) -> u32 {
        self.upload_max_filesize_mb + 8
    }

    /// Allow PHP to finish or report its execution timeout before Nginx gives up.
    /// Nginx measures response inactivity, not PHP execution time; zero would
    /// expire immediately, so unlimited PHP gets a generous 24-hour idle window.
    pub fn fastcgi_read_timeout_seconds(&self) -> u32 {
        match self.max_execution_time {
            0 => 86_400,
            seconds => seconds + 30,
        }
    }

    pub fn ini(&self) -> String {
        let memory = if self.memory_limit_mb == -1 {
            "-1".into()
        } else {
            format!("{}M", self.memory_limit_mb)
        };
        format!("display_errors={}\ndate.timezone={}\nmemory_limit={memory}\npost_max_size={}M\nupload_max_filesize={}M\nmax_execution_time={}\nsession.upload_progress.enabled={}\n",
            if self.display_errors { "On" } else { "Off" }, self.timezone,
            self.request_limit_mb(), self.upload_max_filesize_mb, self.max_execution_time,
            if self.session_upload_progress { "On" } else { "Off" })
    }
}

#[derive(Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Overrides {
    pub versions: BTreeMap<String, serde_json::Value>,
}

impl Overrides {
    pub fn load(layout: &Layout) -> Result<Self> {
        match fs::read(layout.data.join("php-settings.json")) {
            Ok(bytes) => serde_json::from_slice(&bytes).map_err(|e| {
                format!("PHP settings could not be read: {e}. The file has been preserved.")
            }),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(Self::default()),
            Err(e) => Err(e.to_string()),
        }
    }

    pub fn settings(&self, version: &str) -> Result<PhpSettings> {
        let settings: PhpSettings = self
            .versions
            .get(version)
            .map(|value| serde_json::from_value(value.clone()))
            .transpose()
            .map_err(|e| format!("PHP {version} settings are invalid: {e}"))?
            .unwrap_or_default();
        settings.validate()?;
        Ok(settings)
    }

    pub fn set(&mut self, version: &str, settings: &PhpSettings) -> Result<()> {
        settings.validate()?;
        let defaults = serde_json::to_value(PhpSettings::default()).map_err(|e| e.to_string())?;
        let mut value = serde_json::to_value(settings).map_err(|e| e.to_string())?;
        value
            .as_object_mut()
            .ok_or("Invalid PHP settings")?
            .retain(|key, value| defaults.get(key) != Some(value));
        if value.as_object().is_some_and(|value| value.is_empty()) {
            self.versions.remove(version);
        } else {
            self.versions.insert(version.into(), value);
        }
        Ok(())
    }

    pub fn save(&self, layout: &Layout) -> Result<()> {
        atomic_write(
            &layout.data.join("php-settings.json"),
            &serde_json::to_vec_pretty(self).map_err(|e| e.to_string())?,
        )
    }
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PhpSettingsView {
    pub version: String,
    pub settings: PhpSettings,
    pub customized: bool,
    pub timezones: Vec<String>,
}

pub fn timezones(layout: &Layout, version: &str) -> Result<Vec<String>> {
    let executable = layout
        .php_root_for_version(version)?
        .join("bin")
        .join(if cfg!(windows) { "php.exe" } else { "php" });
    let output = crate::process::command(executable)
        .args([
            "-n",
            "-r",
            "echo json_encode(DateTimeZone::listIdentifiers(DateTimeZone::ALL_WITH_BC));",
        ])
        .env("PHP_INI_SCAN_DIR", "")
        .env("MIBS", "")
        .output()
        .map_err(|e| format!("Could not read PHP {version} timezones: {e}"))?;
    if !output.status.success() {
        return Err(format!(
            "Could not read PHP {version} timezones: {}",
            String::from_utf8_lossy(&output.stderr)
        ));
    }
    serde_json::from_slice(&output.stdout)
        .map_err(|e| format!("Could not read PHP {version} timezones: {e}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn regeneration_after_patch_update_keeps_minor_overrides_and_invalid_files() {
        let root =
            std::env::temp_dir().join(format!("fairway-php-settings-{}", rand::random::<u64>()));
        let layout = Layout::at(root.join("home"), root.join("data"), root.join("runtime"));
        layout.initialize().unwrap();
        fs::create_dir_all(&layout.runtime).unwrap();
        let manifest = |patch: &str| serde_json::json!({"php":patch,"phpVersions":{"8.5":patch,"8.4":"8.4.27"}});
        fs::write(
            layout.runtime.join("manifest.json"),
            manifest("8.5.12").to_string(),
        )
        .unwrap();
        let mut saved = Overrides::default();
        saved
            .set(
                "8.5",
                &PhpSettings {
                    session_upload_progress: false,
                    ..PhpSettings::default()
                },
            )
            .unwrap();
        saved
            .set(
                "8.4",
                &PhpSettings {
                    memory_limit_mb: 512,
                    ..PhpSettings::default()
                },
            )
            .unwrap();
        saved.save(&layout).unwrap();
        layout
            .write_configs(&crate::config::Ports::default())
            .unwrap();
        fs::write(
            layout.runtime.join("manifest.json"),
            manifest("8.5.13").to_string(),
        )
        .unwrap();
        // App/update regeneration reads saved minor-version overrides again.
        let manager = crate::manager::Manager::new(layout.clone()).unwrap();
        let ini = fs::read_to_string(layout.data.join("config/php.ini")).unwrap();
        assert!(ini.contains("session.upload_progress.enabled=Off\n"));
        assert!(ini.contains("memory_limit=256M\n"));
        assert_eq!(
            Overrides::load(&layout)
                .unwrap()
                .settings("8.4")
                .unwrap()
                .memory_limit_mb,
            512
        );
        drop(manager);
        fs::write(layout.data.join("php-settings.json"), b"broken file").unwrap();
        assert!(Overrides::load(&layout).is_err());
        assert_eq!(
            fs::read(layout.data.join("php-settings.json")).unwrap(),
            b"broken file"
        );
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn overrides_are_sparse_and_independent_per_minor_version() {
        let mut overrides = Overrides::default();
        let changed = PhpSettings {
            session_upload_progress: false,
            upload_max_filesize_mb: 128,
            ..PhpSettings::default()
        };
        overrides.set("8.5", &changed).unwrap();
        assert_eq!(
            overrides.versions["8.5"],
            serde_json::json!({"sessionUploadProgress":false,"uploadMaxFilesizeMb":128})
        );
        assert_eq!(overrides.settings("8.5").unwrap(), changed);
        assert_eq!(overrides.settings("8.4").unwrap(), PhpSettings::default());
        overrides
            .set(
                "8.4",
                &PhpSettings {
                    timezone: "Asia/Yangon".into(),
                    ..PhpSettings::default()
                },
            )
            .unwrap();
        overrides.set("8.5", &PhpSettings::default()).unwrap();
        assert!(!overrides.versions.contains_key("8.5"));
        assert_eq!(overrides.settings("8.4").unwrap().timezone, "Asia/Yangon");
    }

    #[test]
    fn reject_invalid_limits_and_ini_injection() {
        for settings in [
            PhpSettings {
                memory_limit_mb: 0,
                ..PhpSettings::default()
            },
            PhpSettings {
                upload_max_filesize_mb: u32::MAX,
                ..PhpSettings::default()
            },
            PhpSettings {
                max_execution_time: 86_401,
                ..PhpSettings::default()
            },
            PhpSettings {
                timezone: "UTC\ndisplay_errors=Off".into(),
                ..PhpSettings::default()
            },
        ] {
            assert!(settings.validate().is_err());
        }
        let settings = PhpSettings {
            memory_limit_mb: -1,
            max_execution_time: 0,
            session_upload_progress: false,
            ..PhpSettings::default()
        };
        settings.validate().unwrap();
        assert!(settings.ini().contains("memory_limit=-1\n"));
        assert!(settings.ini().contains("post_max_size=72M\n"));
        assert!(settings
            .ini()
            .contains("session.upload_progress.enabled=Off\n"));
        assert!(PhpSettings::default().session_upload_progress);
    }
}
