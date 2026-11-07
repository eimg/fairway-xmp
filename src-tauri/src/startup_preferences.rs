//! Desktop service launch preferences. Only the shared supervisor writes them.
use crate::{
    config::{atomic_write, Layout, Result},
    manager::Manager,
};
use serde::{Deserialize, Serialize};
use std::fs;

#[derive(Clone, Debug, Default, Deserialize, Serialize, PartialEq)]
#[serde(rename_all = "camelCase", default, deny_unknown_fields)]
pub struct Services {
    pub nginx: bool,
    pub mysql: bool,
    pub mail: bool,
}

impl Services {
    pub fn load(layout: &Layout) -> Result<Self> {
        let path = layout.data.join("startup-services.json");
        match fs::read(&path) {
            Ok(bytes) => serde_json::from_slice(&bytes)
                .map_err(|error| format!("Cannot read Fairway XMP startup services: {error}. Your saved file has been kept.")),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(Self::default()),
            Err(error) => Err(format!("Cannot read Fairway XMP startup services: {error}")),
        }
    }

    pub fn set(layout: &Layout, service: &str, enabled: bool) -> Result<Self> {
        let mut preferences = Self::load(layout)?;
        match service {
            "nginx" => preferences.nginx = enabled,
            "mysql" => preferences.mysql = enabled,
            "mail" => preferences.mail = enabled,
            _ => {
                return Err(
                    "Choose Nginx, MySQL or Mail. PHP and phpMyAdmin start with Nginx.".into(),
                )
            }
        }
        atomic_write(
            &layout.data.join("startup-services.json"),
            &serde_json::to_vec_pretty(&preferences).map_err(|error| error.to_string())?,
        )?;
        Ok(preferences)
    }

    pub fn selected(&self) -> Vec<&'static str> {
        [
            ("mysql", self.mysql),
            ("nginx", self.nginx),
            ("mail", self.mail),
        ]
        .into_iter()
        .filter_map(|(id, enabled)| enabled.then_some(id))
        .collect()
    }
}

/// Called explicitly by a desktop launch, never by a status probe or CLI client.
pub fn start(manager: &mut Manager) -> Result<Vec<String>> {
    let preferences = Services::load(&manager.layout)?;
    let mut errors = Vec::new();
    for service in preferences.selected() {
        if let Err(error) = manager.start(service) {
            // One unavailable service must not prevent the others from starting.
            errors.push(format!("{service}: {error}"));
        }
    }
    Ok(errors)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture() -> (Layout, std::path::PathBuf) {
        let root = std::env::temp_dir().join(format!(
            "fairway-startup-preferences-{}",
            rand::random::<u64>()
        ));
        let layout = Layout::at(root.join("home"), root.join("data"), root.join("runtime"));
        layout.initialize().unwrap();
        (layout, root)
    }

    #[test]
    fn defaults_are_off_and_changes_preserve_other_services() {
        let (layout, root) = fixture();
        assert_eq!(Services::load(&layout).unwrap(), Services::default());
        assert!(!layout.data.join("startup-services.json").exists());
        Services::set(&layout, "nginx", true).unwrap();
        Services::set(&layout, "mysql", true).unwrap();
        let saved = Services::set(&layout, "mail", true).unwrap();
        assert_eq!(saved.selected(), ["mysql", "nginx", "mail"]);
        Services::set(&layout, "mysql", false).unwrap();
        assert_eq!(
            Services::load(&layout).unwrap().selected(),
            ["nginx", "mail"]
        );
        assert!(Services::set(&layout, "php", true).is_err());
        assert!(Services::set(&layout, "phpmyadmin", true).is_err());
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn malformed_preferences_are_preserved_and_not_overwritten() {
        let (layout, root) = fixture();
        let path = layout.data.join("startup-services.json");
        for invalid in ["not json", r#"{"nginx":"yes"}"#, r#"{"unknown":true}"#] {
            fs::write(&path, invalid).unwrap();
            assert!(Services::load(&layout).is_err());
            assert!(Services::set(&layout, "mysql", true).is_err());
            assert_eq!(fs::read_to_string(&path).unwrap(), invalid);
        }
        fs::remove_dir_all(root).unwrap();
    }
}
