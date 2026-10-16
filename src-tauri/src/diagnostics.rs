//! Shareable support information built from an allowlist, never raw logs or configs.
use crate::manager::Snapshot;
use serde_json::{json, Value};

pub fn build_identity() -> Value {
    json!({ "version": env!("CARGO_PKG_VERSION"), "commit": env!("FAIRWAY_BUILD_COMMIT"),
        "modifiedSources": env!("FAIRWAY_BUILD_DIRTY") == "true",
        "os": std::env::consts::OS, "architecture": std::env::consts::ARCH })
}

fn os_version() -> Option<String> {
    if cfg!(target_os = "linux") {
        let text = std::fs::read_to_string("/etc/os-release").ok()?;
        return text
            .lines()
            .find_map(|line| line.strip_prefix("PRETTY_NAME="))
            .map(|value| value.trim_matches('"').chars().take(160).collect());
    }
    let mut command = if cfg!(windows) {
        let mut command = crate::process::command("cmd.exe");
        command.args(["/D", "/C", "ver"]);
        command
    } else {
        let mut command = crate::process::command("/usr/bin/sw_vers");
        command.arg("-productVersion");
        command
    };
    let output = command.output().ok()?;
    if !output.status.success() {
        return None;
    }
    String::from_utf8_lossy(&output.stdout)
        .lines()
        .find(|line| !line.trim().is_empty())
        .map(|line| line.trim().chars().take(160).collect())
}

pub fn report(snapshot: &Snapshot) -> Value {
    fn detected(path: &Option<String>, own: &Option<String>) -> &'static str {
        match path {
            None => "not-detected",
            Some(path)
                if own.as_ref().is_some_and(|own| {
                    if cfg!(windows) {
                        own.eq_ignore_ascii_case(path)
                    } else {
                        own == path
                    }
                }) =>
            {
                "fairway"
            }
            Some(_) => "other-installation",
        }
    }
    let services: Vec<_> = snapshot.services.iter().map(|service| json!({
        "id": service.id, "port": service.port, "running": service.running, "ready": service.ready,
    })).collect();
    // Select known manifest fields instead of copying arbitrary JSON into a report.
    let mut versions = serde_json::Map::new();
    for name in ["php", "nginx", "mysql", "composer", "phpmyadmin"] {
        let value = snapshot.runtime_versions[name].as_str().filter(|value| {
            value.len() <= 64
                && value
                    .bytes()
                    .all(|c| c.is_ascii_alphanumeric() || b".-+_".contains(&c))
        });
        versions.insert(name.into(), json!(value));
    }
    let selected_legacy = ["8.0", "8.1"].contains(&snapshot.php_version.as_str());
    json!({
        "schema": 1, "product": "Fairway XMP", "version": snapshot.version,
        "build": { "commit": env!("FAIRWAY_BUILD_COMMIT"), "modifiedSources": env!("FAIRWAY_BUILD_DIRTY") == "true", "debug": cfg!(debug_assertions) },
        "system": { "os": std::env::consts::OS, "version": os_version(), "architecture": std::env::consts::ARCH },
        "runtimeReady": snapshot.runtime_ready, "runtimeVersions": versions,
        "selectedPhp": snapshot.php_version, "selectedPhpVersion": snapshot.php_versions.get(&snapshot.php_version),
        "selectedPhpLegacy": selected_legacy, "services": services,
        "commandLine": { "installed": snapshot.path_installed,
            "phpDetectedByApp": detected(&snapshot.path_php, &snapshot.cli_php),
            "composerDetectedByApp": detected(&snapshot.path_composer, &snapshot.cli_composer) },
        "excluded": ["credentials", "database contents", "project files", "logs", "filesystem paths"],
    })
}

#[cfg(test)]
mod tests {
    #[test]
    fn private_snapshot_fields_are_not_exported() {
        let snapshot: crate::manager::Snapshot = serde_json::from_value(serde_json::json!({
            "ports": {"website":8800,"phpmyadmin":8801,"mysql":13306,"php":19000},
            "services":[{"id":"mysql","running":false,"ready":false,"port":13306,"pid":123,"message":"password=private-secret"}],
            "documentRoot":"/Users/private-student/coursework", "dataDir":"private-data",
            "projects":[{"name":"private-project","domain":"private-project","https":false,
                "indexFallback":false,"available":true,"domainReady":true,
                "root":"/Users/private-student/coursework/private-project"}],
            "domainsPending":false,"httpsTrusted":false,"os":"macos",
            "runtimeReady":true,"runtimeVersions":{"php":"8.5.11","credentials":"private-secret"},
            "phpVersion":"8.5","phpVersions":{"8.5":"8.5.11"},"pathInstalled":true,
            "cliPhp":"private-launcher", "cliComposer":"private-composer", "pathPhp":null,
            "pathComposer":"/Users/private-student/other/composer","version":"0.1.0-beta.1"
        })).unwrap();
        let report = super::report(&snapshot);
        let text = report.to_string();
        for private in [
            "private-secret",
            "private-student",
            "private-launcher",
            "private-composer",
            "private-data",
            "private-project",
        ] {
            assert!(!text.contains(private));
        }
        assert_eq!(report["commandLine"]["phpDetectedByApp"], "not-detected");
        assert_eq!(
            report["commandLine"]["composerDetectedByApp"],
            "other-installation"
        );
        assert_eq!(report["runtimeVersions"]["php"], "8.5.11");
        assert!(report["build"]["commit"].is_string());
    }
}
