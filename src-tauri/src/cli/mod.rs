//! Terminal frontend. The supervisor is the only owner of the service manager.
mod supervisor;
mod tui;

use crate::{
    config::{Layout, Ports, ProjectConfig, Result},
    manager::Snapshot,
};
use serde::{Deserialize, Serialize};
use std::{env, io::IsTerminal, path::PathBuf, process::Command};

#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "command", rename_all = "kebab-case", deny_unknown_fields)]
pub(super) enum Request {
    Status,
    Diagnostics,
    Start {
        service: String,
    },
    Stop {
        service: String,
    },
    Restart {
        service: String,
    },
    Log {
        service: String,
    },
    Ports {
        ports: Ports,
    },
    PhpVersion {
        version: String,
    },
    Backup,
    SaveProject {
        previous: Option<String>,
        project: ProjectConfig,
    },
    RemoveProject {
        domain: String,
    },
    ProjectHttps {
        domain: String,
        enabled: bool,
    },
    InstallCli,
    RemoveCli,
    ShellTools,
    Shutdown,
    PrepareUpdate,
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Envelope {
    pub runtime: PathBuf,
    pub request: Request,
}

#[derive(Serialize, Deserialize)]
pub(super) struct Response {
    pub data: serde_json::Value,
    pub error: Option<String>,
    pub runtime: PathBuf,
}

pub fn run() -> Result<()> {
    let args: Vec<String> = env::args().skip(1).collect();
    if args.first().is_some_and(|value| value == "__sync-hosts") {
        return crate::hosts::apply(&args[1..]);
    }
    #[cfg(target_os = "linux")]
    if args.first().is_some_and(|value| value == "__trust-ca") {
        return crate::tls::apply_linux_trust(
            std::path::Path::new(args.get(1).ok_or("Missing certificate path.")?),
            false,
        );
    }
    #[cfg(target_os = "linux")]
    if args.first().is_some_and(|value| value == "__remove-ca") {
        return crate::tls::apply_linux_trust(
            std::path::Path::new(args.get(1).ok_or("Missing certificate path.")?),
            true,
        );
    }
    if args == ["--build-info"] {
        println!("{}", crate::diagnostics::build_identity());
        return Ok(());
    }
    if matches!(
        args.first().map(String::as_str),
        Some("help" | "--help" | "-h")
    ) {
        println!("Fairway XMP terminal edition\n\nfairway [tui]                 Open the terminal dashboard\nfairway status [--json]       Service status and paths\nfairway start|stop|restart [all|nginx|mysql]\nfairway project add DOMAIN FOLDER [--name NAME] [--index]\n                             Serve FOLDER at DOMAIN.test (updates the hosts file with sudo)\nfairway project remove DOMAIN Remove a project; its files are kept\nfairway project https DOMAIN on|off\n                             Toggle HTTPS for one project (:8443)\nfairway project hosts        Retry the hosts-file update for registered projects\nfairway logs nginx|mysql|php  Show recent log entries\nfairway ports                Show configured ports\nfairway ports WEB PMA DB PHP  Change ports while all services are stopped\nfairway php-version          Show selected PHP version\nfairway php-version 8.0–8.5  Switch PHP after stopping all services (8.0/8.1 are legacy)\nfairway diagnostics          Print a shareable diagnostic report\nfairway backup               Back up student databases (MySQL must be running)\nfairway php|composer|mysql|mysqldump [arguments...]\nfairway shell                Open a shell with bundled PHP/Composer on PATH\nfairway install-cli          Enable PHP/Composer/Fairway in shell profiles\nfairway remove-cli           Remove managed shell integration\nfairway shutdown             Gracefully stop services and the supervisor\nfairway uninstall            Remove the installation; preserve projects and data\n\nClosing the dashboard leaves services running. PHP-FPM follows Nginx.\nTools and services run as your user; do not run Fairway with sudo.");
        return Ok(());
    }
    if matches!(args.first().map(String::as_str), Some("--version" | "-V")) {
        println!("Fairway XMP {}", env!("CARGO_PKG_VERSION"));
        return Ok(());
    }
    if unsafe { libc::geteuid() } == 0 {
        return Err("Run Fairway as your normal user, without sudo.".into());
    }
    let executable = env::current_exe().map_err(|e| e.to_string())?;
    let root = executable
        .parent()
        .and_then(|p| p.parent())
        .ok_or("Cannot locate the installation.")?;
    let layout = Layout::new(root.join("runtime"))?;
    if args.first().is_some_and(|s| s == "__serve") {
        return supervisor::serve(&layout);
    }
    if args.first().is_some_and(|s| s == "clear-domains") {
        return crate::hosts::ensure(&[], "__sync-hosts");
    }
    if args.first().is_some_and(|s| s == "clear-https-trust") {
        return crate::tls::remove_trust(&layout);
    }
    if args.first().is_some_and(|s| s == "uninstall") {
        let status = Command::new("sh")
            .arg(root.join("uninstall.sh"))
            .status()
            .map_err(|e| e.to_string())?;
        return if status.success() {
            Ok(())
        } else {
            Err("Uninstall did not complete; your data has been preserved.".into())
        };
    }
    let command = args.first().map(String::as_str).unwrap_or("tui");
    if command == "shutdown" || command == "__prepare-update" {
        if !supervisor::available(&layout) {
            // Starting a controller also recovers any verified service left after a crash.
            supervisor::ensure(&layout)?;
        }
        supervisor::call(
            &layout,
            &if command == "shutdown" {
                Request::Shutdown
            } else {
                Request::PrepareUpdate
            },
        )?;
        supervisor::wait_shutdown(&layout)?;
        println!("Services and supervisor stopped.");
        return Ok(());
    }
    let request = parse(&args)?;
    supervisor::ensure(&layout)?;
    if let Request::ProjectHttps {
        domain,
        enabled: true,
    } = &request
    {
        if !layout
            .registered_projects()?
            .iter()
            .any(|p| p.domain == *domain)
        {
            return Err(format!("No project uses {domain}.test."));
        }
        crate::tls::ensure_trust(&layout)?;
    }
    if command == "tui" {
        if !std::io::stdin().is_terminal() || !std::io::stdout().is_terminal() {
            return Err("The dashboard needs an interactive terminal. Use fairway status --json for scripts.".into());
        }
        return tui::run(&layout);
    }
    if ["php", "composer", "mysql", "mysqldump", "shell"].contains(&command) {
        supervisor::call(&layout, &Request::Status)?; // Ensure generated configuration exists.
        let mut tool = if command == "shell" {
            Command::new(env::var_os("SHELL").unwrap_or_else(|| "/bin/sh".into()))
        } else {
            Command::new(if ["mysql", "mysqldump"].contains(&command) {
                layout.exe("mysql", command)
            } else {
                layout.php_exe("php")?
            })
        };
        tool.env("PHP_INI_SCAN_DIR", "")
            .env("MIBS", "")
            .env("PHPRC", layout.data.join("config/php.ini"))
            .env("COMPOSER_HOME", layout.data.join("composer"));
        if command == "composer" {
            tool.arg(layout.runtime.join("composer.phar"));
        }
        if ["mysql", "mysqldump"].contains(&command) {
            tool.arg("--no-defaults")
                .args([
                    "--user=fairway",
                    "--password=fairway",
                    "--host=127.0.0.1",
                    "--protocol=TCP",
                ])
                .arg(format!("--port={}", layout.ports()?.mysql));
        }
        if command == "shell" {
            supervisor::call(&layout, &Request::ShellTools)?;
            // Create the tools without editing profiles unless explicitly requested.
            let mut paths = vec![layout.bin.clone()];
            paths.extend(env::split_paths(&env::var_os("PATH").unwrap_or_default()));
            tool.env("PATH", env::join_paths(paths).map_err(|e| e.to_string())?);
        } else {
            tool.args(&args[1..]);
        }
        let status = tool.status().map_err(|e| e.to_string())?;
        std::process::exit(status.code().unwrap_or(1));
    }
    let mut data = supervisor::call(&layout, &request)?;
    if command == "project" && args[1] != "https" {
        let projects = layout.registered_projects()?;
        let domains = projects
            .iter()
            .map(|p| p.domain.clone())
            .collect::<Vec<_>>();
        crate::hosts::ensure(&domains, "__sync-hosts").map_err(|e| {
            format!("The hosts file was not updated: {e}. Retry with fairway project hosts.")
        })?;
        if args[1] == "remove" && crate::tls::trusted(&layout) && !projects.iter().any(|p| p.https)
        {
            crate::tls::remove_trust(&layout)?;
        }
        println!("Projects and the hosts file are up to date.");
        return Ok(());
    }
    if command == "ports" && args.len() == 1 {
        data = data["ports"].clone();
    }
    if command == "php-version" && args.len() == 1 {
        data = data["phpVersion"].clone();
    }
    if command == "status" && args.get(1).is_some_and(|s| s == "--json") {
        println!(
            "{}",
            serde_json::to_string_pretty(&data).map_err(|e| e.to_string())?
        );
    } else if command == "status" {
        let snapshot: Snapshot = serde_json::from_value(data).map_err(|e| e.to_string())?;
        println!("Fairway XMP {}", snapshot.version);
        for service in snapshot.services {
            println!(
                "{:<12} {:<10} port {}{}",
                service.id,
                if service.ready {
                    "ready"
                } else if service.running {
                    "starting"
                } else {
                    "stopped"
                },
                service.port,
                service
                    .message
                    .map(|m| format!(" — {m}"))
                    .unwrap_or_default()
            );
        }
        println!(
            "Projects: {}\nData: {}",
            snapshot.document_root, snapshot.data_dir
        );
        for project in snapshot.projects {
            println!(
                "{}: {} -> {}{}",
                project.config.name,
                crate::integration::project_url(&project.config, snapshot.ports.website),
                project.config.root.display(),
                if !project.available {
                    " (web root missing)"
                } else if !project.domain_ready {
                    " (hosts file not updated; run fairway project hosts)"
                } else {
                    ""
                }
            );
        }
    } else if let Some(text) = data.as_str() {
        println!("{text}");
    } else {
        println!(
            "{}",
            serde_json::to_string_pretty(&data).map_err(|e| e.to_string())?
        );
    }
    Ok(())
}

fn parse(args: &[String]) -> Result<Request> {
    let command = args.first().map(String::as_str).unwrap_or("tui");
    let service = || -> Result<String> {
        let value = args.get(1).map(String::as_str).unwrap_or("all");
        if args.len() > 2 || !["all", "nginx", "mysql"].contains(&value) {
            return Err("Choose all, nginx or mysql. PHP-FPM is managed by Nginx.".into());
        }
        Ok(value.into())
    };
    match command {
        "start" => Ok(Request::Start {
            service: service()?,
        }),
        "stop" => Ok(Request::Stop {
            service: service()?,
        }),
        "restart" => Ok(Request::Restart {
            service: service()?,
        }),
        "logs" if args.len() == 2 && ["nginx", "mysql", "php"].contains(&args[1].as_str()) => {
            Ok(Request::Log {
                service: args[1].clone(),
            })
        }
        "ports" if args.len() == 5 => {
            let values = args[1..]
                .iter()
                .map(|s| {
                    s.parse::<u16>()
                        .map_err(|_| "Ports must be numbers from 1024 to 65535.".to_string())
                })
                .collect::<Result<Vec<_>>>()?;
            let ports = Ports {
                website: values[0],
                phpmyadmin: values[1],
                mysql: values[2],
                php: values[3],
            };
            ports.validate()?;
            Ok(Request::Ports { ports })
        }
        "backup" if args.len() == 1 => Ok(Request::Backup),
        "project" => project_request(&args[1..]),
        "diagnostics" if args.len() == 1 => Ok(Request::Diagnostics),
        "php-version" if args.len() == 2 => Ok(Request::PhpVersion {
            version: args[1].clone(),
        }),
        "install-cli" if args.len() == 1 => Ok(Request::InstallCli),
        "remove-cli" if args.len() == 1 => Ok(Request::RemoveCli),
        "status" if args.len() == 1 || (args.len() == 2 && args[1] == "--json") => {
            Ok(Request::Status)
        }
        "ports" | "php-version" | "tui" if args.len() <= 1 => Ok(Request::Status),
        "php" | "composer" | "mysql" | "mysqldump" | "shell" => Ok(Request::Status),
        _ => Err("Unknown command or arguments. Run fairway --help.".into()),
    }
}

fn project_request(args: &[String]) -> Result<Request> {
    let words = args.iter().map(String::as_str).collect::<Vec<_>>();
    match words.as_slice() {
        ["add", domain, folder, options @ ..] => {
            let mut name = domain.to_string();
            let mut index_fallback = false;
            let mut options = options.iter();
            while let Some(option) = options.next() {
                match *option {
                    "--index" => index_fallback = true,
                    "--name" => name = options.next().ok_or("--name needs a value.")?.to_string(),
                    _ => return Err(format!("Unknown option {option}.")),
                }
            }
            let root = std::fs::canonicalize(folder)
                .map_err(|e| format!("Cannot use {folder} as the web root: {e}"))?;
            Ok(Request::SaveProject {
                previous: None,
                project: ProjectConfig {
                    name,
                    domain: domain.to_string(),
                    root,
                    https: false,
                    index_fallback,
                },
            })
        }
        ["remove", domain] => Ok(Request::RemoveProject {
            domain: domain.to_string(),
        }),
        ["https", domain, state @ ("on" | "off")] => Ok(Request::ProjectHttps {
            domain: domain.to_string(),
            enabled: *state == "on",
        }),
        ["hosts"] => Ok(Request::Status),
        _ => Err("Unknown project command. Run fairway --help.".into()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn args(s: &str) -> Vec<String> {
        s.split_whitespace().map(String::from).collect()
    }
    #[test]
    fn rejects_unsupported_services_and_bad_ports_before_contacting_supervisor() {
        assert!(parse(&args("start php")).is_err());
        assert!(parse(&args("stop unrelated")).is_err());
        assert!(parse(&args("ports 80 8801 13306 19000")).is_err());
        assert!(parse(&args("ports 8800 8800 13306 19000")).is_err());
        assert!(parse(&args("ports 8800 8801 13306 19000")).is_ok());
        assert!(parse(&args("project add shop /definitely/missing/folder")).is_err());
        assert!(parse(&args("project add shop / --unknown")).is_err());
        assert!(parse(&args("project https shop maybe")).is_err());
        assert!(matches!(
            parse(&args("project add shop / --name Shop --index")),
            Ok(Request::SaveProject { project, .. }) if project.name == "Shop" && project.index_fallback
        ));
    }
}
