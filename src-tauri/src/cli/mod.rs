//! Terminal frontend. The supervisor is the only owner of the service manager.
pub(crate) mod client;
pub(crate) mod supervisor;
#[cfg(test)]
mod tests;
mod transport;
#[cfg(unix)]
mod tui;

use crate::{
    config::{Layout, Ports, ProjectConfig, Result},
    manager::Snapshot,
};
use serde::{Deserialize, Serialize};
#[cfg(unix)]
use std::io::IsTerminal;
use std::{env, path::PathBuf, process::Command};

#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "command", rename_all = "kebab-case", deny_unknown_fields)]
pub(crate) enum Request {
    Status,
    Hello,
    Acquire,
    Release,
    Settings {
        ports: Ports,
        version: String,
    },
    PhpSettings {
        version: String,
    },
    SavePhpSettings {
        version: String,
        settings: Option<crate::php_settings::PhpSettings>,
    },
    RememberAddressApproval {
        pending: bool,
    },
    PrepareDesktopUpdate,
    Database {
        args: Vec<String>,
    },
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

/// Both the desktop executable and the Windows console entry point use this parser.
pub fn main() {
    if let Err(error) = run() {
        report_error(&error);
        std::process::exit(1);
    }
}

pub(crate) fn report_error(error: &str) {
    let (code, message) = error
        .strip_prefix("approval_required: ")
        .map_or(("operation_failed", error), |message| {
            ("approval_required", message)
        });
    if env::args().any(|arg| arg == "--json") {
        println!(
            "{}",
            serde_json::json!({"ok":false,"schemaVersion":1,"error":{"code":code,"message":message}})
        );
    } else {
        eprintln!("Fairway XMP: {error}");
    }
}

pub fn layout() -> Result<Layout> {
    #[cfg(debug_assertions)]
    if let Some(runtime) = env::var_os("FAIRWAY_RUNTIME_DIR") {
        return normalized_layout(PathBuf::from(runtime));
    }
    let executable = env::current_exe().map_err(|e| e.to_string())?;
    let directory = executable
        .parent()
        .ok_or("Cannot locate the installation.")?;
    #[cfg(target_os = "macos")]
    let runtime = directory.join("../Resources/runtime");
    #[cfg(target_os = "linux")]
    let runtime = directory
        .parent()
        .ok_or("Cannot locate the installation.")?
        .join("runtime");
    #[cfg(windows)]
    let runtime = if directory.file_name().is_some_and(|n| n == "cli") {
        directory
            .parent()
            .ok_or("Cannot locate the installation.")?
            .join("runtime")
    } else {
        directory.join("runtime")
    };
    normalized_layout(runtime)
}
fn normalized_layout(runtime: PathBuf) -> Result<Layout> {
    let runtime = canonical_runtime(runtime);
    Layout::new(runtime)
}

pub(super) fn canonical_runtime(runtime: PathBuf) -> PathBuf {
    let path = std::fs::canonicalize(&runtime).unwrap_or(runtime);
    #[cfg(windows)]
    {
        let text = path.to_string_lossy();
        if let Some(unc) = text.strip_prefix(r"\\?\UNC\") {
            return PathBuf::from(format!(r"\\{unc}"));
        }
        if let Some(drive) = text.strip_prefix(r"\\?\") {
            return PathBuf::from(drive);
        }
    }
    path
}

const HELP: &str = "Fairway XMP command line

fairway status [--json]                 Services, sites, ports and paths
fairway capabilities [--json]           Supported commands and platform
fairway start|stop|restart [all|nginx|mysql|mail]
fairway site list
fairway site add DOMAIN FOLDER [--name NAME] [--index]
fairway site edit DOMAIN FOLDER [--name NAME] [--index]
fairway site remove DOMAIN              Keep the site's files
fairway site https DOMAIN on|off
fairway site hosts                      Retry address setup
fairway logs nginx|mysql|php|mail
fairway ports [WEB PMA DB PHP [MAIL SMTP]]
fairway php-version [8.2|8.3|8.4|8.5]    Change with services stopped
fairway php-settings [VERSION]
fairway php-settings VERSION --file FILE
fairway php-settings VERSION --reset
fairway diagnostics
fairway backup                          Requires running MySQL
fairway db --help                       MySQL administration; results are JSON
fairway php|composer|mysql|mysqldump [arguments...]
fairway shell                          Open a shell with bundled tools
fairway install-cli|remove-cli          Manage user PATH integration
fairway shutdown                       Stop services and their supervisor

Add --json to management commands for machine-readable results and errors.
Commands complete before returning; an operation in progress returns an error.
Use --non-interactive to return approval_required instead of opening OS prompts.
Sites can require native OS approval. Never run Fairway with sudo.
Linux: fairway [tui] opens the dashboard; fairway uninstall preserves user data.";

pub fn run() -> Result<()> {
    let mut args: Vec<String> = env::args().skip(1).collect();
    if args.first().is_some_and(|value| value == "__sync-hosts") {
        return crate::hosts::apply(&args[1..]);
    }
    #[cfg(target_os = "linux")]
    if matches!(
        args.first().map(String::as_str),
        Some("__trust-ca" | "__remove-ca")
    ) {
        return crate::tls::apply_linux_trust(
            std::path::Path::new(args.get(1).ok_or("Missing certificate path.")?),
            args[0] == "__remove-ca",
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
        println!("{HELP}");
        return Ok(());
    }
    if matches!(args.first().map(String::as_str), Some("--version" | "-V")) {
        println!("Fairway XMP {}", env!("CARGO_PKG_VERSION"));
        return Ok(());
    }
    #[cfg(unix)]
    if unsafe { libc::geteuid() } == 0 {
        return Err("Run Fairway as your normal user, without sudo.".into());
    }
    let first = args.first().map(String::as_str).unwrap_or("").to_owned();
    let raw_args =
        ["php", "composer", "mysql", "mysqldump", "shell", "db"].contains(&first.as_str());
    let json = (!raw_args && args.iter().any(|s| s == "--json"))
        || (first == "db" && args.last().is_some_and(|s| s == "--json"));
    if json && !raw_args {
        args.retain(|s| s != "--json");
    } else if json {
        args.pop();
    }
    let non_interactive = !raw_args && args.iter().any(|s| s == "--non-interactive");
    if non_interactive {
        args.retain(|s| s != "--non-interactive");
    }
    let command = args.first().cloned().unwrap_or_else(|| {
        if cfg!(target_os = "linux") {
            "tui".into()
        } else {
            "help".into()
        }
    });
    let passthrough =
        ["php", "composer", "mysql", "mysqldump", "shell"].contains(&command.as_str());
    if command == "capabilities" {
        print_value(
            serde_json::json!({"schemaVersion":1,"version":env!("CARGO_PKG_VERSION"),"platform":env::consts::OS,"controller":"shared-supervisor","commands":["status","start","stop","restart","site","logs","ports","php-version","php-settings","diagnostics","backup","db","php","composer","mysql","mysqldump","shell","install-cli","remove-cli","shutdown"],"tui":cfg!(unix),"api":false,"mcp":false,"nativeApprovalMayBeRequired":true}),
            true,
        )?;
        return Ok(());
    }
    if command == "help" {
        println!("{HELP}");
        return Ok(());
    }
    let layout = layout()?;
    if command == "__serve" {
        return supervisor::serve(&layout);
    }
    #[cfg(target_os = "linux")]
    {
        if command == "clear-domains" {
            return crate::hosts::ensure(&[], "__sync-hosts");
        }
        if command == "clear-https-trust" {
            return crate::tls::remove_trust(&layout);
        }
        if command == "uninstall" {
            let root = env::current_exe()
                .map_err(|e| e.to_string())?
                .parent()
                .and_then(|p| p.parent())
                .ok_or("Cannot locate the installation.")?
                .to_owned();
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
    }
    if command == "db" && (args.len() == 1 || args.get(1).is_some_and(|v| v == "--help")) {
        return crate::data_browser::cli(&layout, &args[1..]);
    }
    // Parse before starting a supervisor, so invalid commands have no side effects.
    let mut request = if command == "shutdown" || command == "__prepare-update" {
        if args.len() != 1 {
            return Err("Unexpected arguments.".into());
        }
        if command == "shutdown" {
            Request::Shutdown
        } else {
            Request::PrepareUpdate
        }
    } else if command == "db" {
        Request::Database {
            args: args[1..].to_vec(),
        }
    } else {
        parse(&args)?
    };
    supervisor::ensure(&layout)?;
    if matches!(request, Request::Shutdown | Request::PrepareUpdate) {
        let pid = supervisor::pid(&layout);
        let result = supervisor::call(&layout, &request)?;
        supervisor::wait_shutdown_pid(&layout, pid)?;
        return print_value(result, json);
    }
    if command == "tui" {
        #[cfg(unix)]
        {
            if !std::io::stdin().is_terminal() || !std::io::stdout().is_terminal() {
                return Err("The dashboard needs an interactive terminal. Use fairway status --json for scripts.".into());
            }
            return tui::run(&layout);
        }
        #[cfg(windows)]
        return Err(
            "The terminal dashboard is available on Unix. Use fairway status --json.".into(),
        );
    }
    if passthrough {
        return tool(&layout, &command, &args[1..]);
    }
    let mut client = client::Client::new(layout)?;
    let read_only = matches!(
        request,
        Request::Status | Request::Diagnostics | Request::Log { .. } | Request::PhpSettings { .. }
    );
    if !read_only {
        client.begin()?;
    }
    let result = (|| {
        if let Request::SaveProject {
            previous: Some(previous),
            project,
        } = &mut request
        {
            let existing = client
                .layout
                .registered_projects()?
                .into_iter()
                .find(|p| p.domain == *previous)
                .ok_or("The site is no longer registered.")?;
            project.https = existing.https;
        }
        if let Request::ProjectHttps {
            domain,
            enabled: true,
        } = &request
        {
            if !client.domains()?.contains(domain) {
                return Err(format!("No site uses {domain}.test."));
            }
            if non_interactive {
                if !crate::tls::trusted(&client.layout)
                    || crate::tls::trust_update_needed(&client.layout)
                {
                    return Err("approval_required: Certificate trust requires your approval. Enable HTTPS in Fairway XMP or rerun this command interactively.".into());
                }
            } else {
                crate::tls::ensure_trust(&client.layout)?;
            }
        }
        let https_enable = matches!(request, Request::ProjectHttps { enabled: true, .. });
        let https_disable = matches!(request, Request::ProjectHttps { enabled: false, .. });
        let site_change = command == "site"
            && matches!(
                args.get(1).map(String::as_str),
                Some("add" | "edit" | "remove" | "hosts")
            );
        let mut data: serde_json::Value = client.call(request)?;
        if https_enable && !non_interactive {
            crate::tls::finish_trust_update(&client.layout)?;
        }
        if site_change {
            let domains = client.domains()?;
            if non_interactive && !crate::hosts::current(&domains)? {
                return Err("approval_required: The site change was saved. Address setup needs OS approval; open Fairway XMP and choose Set up addresses, or run fairway site hosts interactively.".into());
            }
            #[cfg(feature = "desktop")]
            let approval = !non_interactive
                && crate::hosts::ensure_for_desktop(&domains)?
                    == crate::hosts::SetupStatus::ApprovalRequired;
            #[cfg(not(feature = "desktop"))]
            let approval = {
                if !non_interactive {
                    crate::hosts::ensure(&domains, "__sync-hosts")?;
                }
                false
            };
            client.remember_address_approval(approval)?;
            if approval {
                return Err("approval_required: The site change was saved. Address setup requires macOS approval: allow Fairway XMP in System Settings → General → Login Items & Extensions, then run fairway site hosts.".into());
            }
        }
        if (https_disable || (site_change && args[1] == "remove")) && client.unused_trust()? {
            if non_interactive {
                return Err("approval_required: The site change was saved. Remove unused certificate trust in Fairway XMP or rerun interactively.".into());
            }
            crate::tls::remove_trust(&client.layout)?;
        }
        if command == "site" && args.get(1).is_some_and(|s| s == "list") {
            data = serde_json::to_value(client.layout.registered_projects()?)
                .map_err(|e| e.to_string())?;
        }
        if command == "ports" && args.len() == 1 {
            data = data["ports"].clone();
        }
        if command == "php-version" && args.len() == 1 {
            data = data["phpVersion"].clone();
        }
        if command == "status" {
            data["pathPhp"] = serde_json::json!(crate::integration::resolve("php"));
            data["pathComposer"] = serde_json::json!(crate::integration::resolve("composer"));
        }
        if command == "status" && !json {
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
                "Document root: {}\nData: {}",
                snapshot.document_root, snapshot.data_dir
            );
            if let Some(error) = snapshot.sites_error {
                println!("Saved sites could not be loaded: {error}. Site files were kept.");
            }
            for project in snapshot.projects {
                println!(
                    "{}: {} -> {}{}",
                    project.config.name,
                    crate::integration::project_url(&project.config, snapshot.ports.website),
                    project.config.root.display(),
                    if !project.available {
                        " (web root missing)"
                    } else if !project.domain_ready {
                        " (hosts file not updated; run fairway site hosts)"
                    } else {
                        ""
                    }
                );
            }
            return Ok(());
        }
        print_value(data, json || command == "db")
    })();
    client.end();
    result
}

fn print_value(data: serde_json::Value, json: bool) -> Result<()> {
    if !json {
        if let Some(text) = data.as_str() {
            println!("{text}");
            return Ok(());
        }
    }
    println!(
        "{}",
        serde_json::to_string_pretty(&data).map_err(|e| e.to_string())?
    );
    Ok(())
}

fn tool(layout: &Layout, command: &str, args: &[String]) -> Result<()> {
    supervisor::call(layout, &Request::Status)?;
    let mut tool = if command == "shell" {
        #[cfg(unix)]
        let shell = env::var_os("SHELL").unwrap_or_else(|| "/bin/sh".into());
        #[cfg(windows)]
        let shell = env::var_os("COMSPEC").unwrap_or_else(|| "cmd.exe".into());
        Command::new(shell)
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
        supervisor::call(layout, &Request::ShellTools)?;
        let mut paths = vec![layout.bin.clone()];
        paths.extend(env::split_paths(&env::var_os("PATH").unwrap_or_default()));
        tool.env("PATH", env::join_paths(paths).map_err(|e| e.to_string())?);
    }
    tool.args(args);
    let status = tool.status().map_err(|e| e.to_string())?;
    std::process::exit(status.code().unwrap_or(1));
}

fn parse(args: &[String]) -> Result<Request> {
    let command = args.first().map(String::as_str).unwrap_or("tui");
    let service = || -> Result<String> {
        let value = args.get(1).map(String::as_str).unwrap_or("all");
        if (args.len() > 2 && !(args.len() == 3 && args[2] == "--wait"))
            || !["all", "nginx", "mysql", "mail"].contains(&value)
        {
            return Err("Choose all, nginx, mysql or mail. PHP-FPM is managed by Nginx.".into());
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
        "logs"
            if args.len() == 2 && ["nginx", "mysql", "php", "mail"].contains(&args[1].as_str()) =>
        {
            Ok(Request::Log {
                service: args[1].clone(),
            })
        }
        "ports" if args.len() == 5 || args.len() == 7 => {
            let values = args[1..]
                .iter()
                .map(|s| {
                    s.parse::<u16>()
                        .map_err(|_| "Ports must be numbers from 1024 to 65535.".to_string())
                })
                .collect::<Result<Vec<_>>>()?;
            let defaults = Ports::default();
            let ports = Ports {
                website: values[0],
                phpmyadmin: values[1],
                mysql: values[2],
                php: values[3],
                mail: values.get(4).copied().unwrap_or(defaults.mail),
                smtp: values.get(5).copied().unwrap_or(defaults.smtp),
            };
            ports.validate()?;
            Ok(Request::Ports { ports })
        }
        "backup" if args.len() == 1 => Ok(Request::Backup),
        "site" => project_request(&args[1..]),
        "php-settings" => {
            let version = args.get(1).cloned().unwrap_or_else(|| "active".into());
            let version = if version == "active" {
                layout()?.php_version()?
            } else {
                version
            };
            match args.len() {
                1 | 2 => Ok(Request::PhpSettings { version }),
                3 if args[2] == "--reset" => Ok(Request::SavePhpSettings {
                    version,
                    settings: None,
                }),
                4 if args[2] == "--file" => {
                    let bytes = std::fs::read(&args[3]).map_err(|e| e.to_string())?;
                    let settings: crate::php_settings::PhpSettings =
                        serde_json::from_slice(&bytes).map_err(|e| e.to_string())?;
                    settings.validate()?;
                    Ok(Request::SavePhpSettings {
                        version,
                        settings: Some(settings),
                    })
                }
                _ => Err("Use fairway php-settings [VERSION] [--file FILE|--reset].".into()),
            }
        }
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
        ["list"] => Ok(Request::Status),
        [action @ ("add" | "edit"), domain, folder, options @ ..] => {
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
                previous: (*action == "edit").then(|| domain.to_string()),
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
        _ => Err("Unknown site command. Run fairway --help.".into()),
    }
}

#[cfg(test)]
mod parser_tests {
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
        assert!(parse(&args("site add shop /definitely/missing/folder")).is_err());
        assert!(parse(&args("site add shop / --unknown")).is_err());
        assert!(parse(&args("site https shop maybe")).is_err());
        assert!(matches!(
            parse(&args("site add shop / --name Shop --index")),
            Ok(Request::SaveProject { project, .. }) if project.name == "Shop" && project.index_fallback
        ));
    }
}
