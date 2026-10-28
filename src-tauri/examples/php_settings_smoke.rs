//! Exercise persistent overrides, CLI/web configuration, and actual multipart uploads.
use fairway_core::{
    config::{Layout, Ports},
    manager::Manager,
    php_settings::PhpSettings,
    process,
};
use serde_json::Value;
use std::{
    fs,
    io::{Read, Write},
    net::{TcpListener, TcpStream},
    path::{Path, PathBuf},
    time::Duration,
};

fn request(manager: &Manager, body: Option<&[u8]>) -> Result<(u16, String), String> {
    request_with_wait(manager, body, 0)
}

fn request_with_wait(
    manager: &Manager,
    body: Option<&[u8]>,
    wait: u64,
) -> Result<(u16, String), String> {
    let mut stream =
        TcpStream::connect(("127.0.0.1", manager.ports.website)).map_err(|e| e.to_string())?;
    stream
        .set_read_timeout(Some(Duration::from_secs(wait + 20)))
        .map_err(|e| e.to_string())?;
    stream
        .set_write_timeout(Some(Duration::from_secs(20)))
        .map_err(|e| e.to_string())?;
    let header = match body {
        Some(body) => format!("POST /settings.php?wait={wait} HTTP/1.0\r\nHost: localhost\r\nContent-Type: multipart/form-data; boundary=fairway\r\nContent-Length: {}\r\n\r\n", body.len()),
        None => format!("GET /settings.php?wait={wait} HTTP/1.0\r\nHost: localhost\r\n\r\n"),
    };
    stream
        .write_all(header.as_bytes())
        .map_err(|e| e.to_string())?;
    if let Some(body) = body {
        stream.write_all(body).map_err(|e| e.to_string())?;
    }
    let mut response = String::new();
    stream
        .read_to_string(&mut response)
        .map_err(|e| e.to_string())?;
    let (head, body) = response
        .split_once("\r\n\r\n")
        .ok_or("Missing HTTP response body")?;
    let status = head
        .split_whitespace()
        .nth(1)
        .ok_or("Missing HTTP status")?
        .parse::<u16>()
        .map_err(|e| e.to_string())?;
    Ok((status, body.into()))
}

fn check_config(
    body: &str,
    settings: &PhpSettings,
    cli: bool,
    managed_ini: &Path,
) -> Result<Value, String> {
    let result: Value =
        serde_json::from_str(body).map_err(|e| format!("Invalid PHP result: {e}: {body}"))?;
    let loaded_ini = result["ini"].as_str().ok_or_else(|| {
        format!(
            "PHP did not load the managed configuration {}: {result}",
            managed_ini.display()
        )
    })?;
    // Resolve aliases before comparing: Windows PHP can report an NTFS short
    // path, while the manager knows the ordinary application-data path.
    if fs::canonicalize(loaded_ini)
        .map_err(|e| format!("Cannot resolve loaded PHP configuration {loaded_ini}: {e}"))?
        != fs::canonicalize(managed_ini).map_err(|e| e.to_string())?
    {
        return Err(format!(
            "PHP loaded {loaded_ini} instead of {}",
            managed_ini.display()
        ));
    }
    let memory = if settings.memory_limit_mb == -1 {
        "-1".into()
    } else {
        format!("{}M", settings.memory_limit_mb)
    };
    if result["memory"] != memory
        || result["upload"] != format!("{}M", settings.upload_max_filesize_mb)
        || result["post"] != format!("{}M", settings.request_limit_mb())
        || result["timezone"] != settings.timezone
        || result["progress"] != settings.session_upload_progress
        || result["errors"] != settings.display_errors
        || result["execution"] != if cli { 0 } else { settings.max_execution_time }
    {
        return Err(format!("PHP settings mismatch: {result}"));
    }
    Ok(result)
}

fn main() -> Result<(), String> {
    let mut args = std::env::args_os().skip(1);
    let current = std::env::current_dir().map_err(|e| e.to_string())?;
    let root = current.join(args.next().ok_or("Pass a fresh test directory.")?);
    let runtime =
        current.join(args.next().map(PathBuf::from).unwrap_or_else(|| {
            PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("resources/runtime")
        }));
    if root.exists() {
        return Err("Use a fresh test directory.".into());
    }
    let layout = Layout::at(
        root.join("student home 日本"),
        // PHP's Windows startup configuration needs an ANSI-compatible path
        // or an existing NTFS short alias. The runner's workspace volume need
        // not create short aliases, so keep this fixture's data path portable.
        // The project root still exercises Unicode script and web paths.
        root.join(if cfg!(windows) {
            "student data"
        } else {
            "student data 日本"
        }),
        runtime,
    );
    let managed_ini = layout.data.join("config/php.ini");
    let mut manager = Manager::new(layout.clone())?;
    let listeners = (0..6)
        .map(|_| TcpListener::bind(("127.0.0.1", 0)))
        .collect::<std::io::Result<Vec<_>>>()
        .map_err(|e| e.to_string())?;
    let ports: Vec<_> = listeners
        .iter()
        .map(|l| l.local_addr().unwrap().port())
        .collect();
    manager.save_ports(Ports {
        website: ports[0],
        phpmyadmin: ports[1],
        mysql: ports[2],
        php: ports[3],
        mail: ports[4],
        smtp: ports[5],
    })?;
    drop(listeners);
    let script = manager.layout.www.join("settings.php");
    fs::write(
        &script,
        r#"<?php sleep(min(65, max(0, (int)($_GET['wait'] ?? 0)))); echo json_encode([
        'ini' => php_ini_loaded_file(),
        'memory' => ini_get('memory_limit'), 'upload' => ini_get('upload_max_filesize'),
        'post' => ini_get('post_max_size'), 'timezone' => date_default_timezone_get(),
        'execution' => (int)ini_get('max_execution_time'),
        'progress' => (bool)ini_get('session.upload_progress.enabled'),
        'errors' => (bool)ini_get('display_errors'), 'file' => $_FILES['sample'] ?? null]);"#,
    )
    .map_err(|e| e.to_string())?;
    let mut upload = b"--fairway\r\nContent-Disposition: form-data; name=\"PHP_SESSION_UPLOAD_PROGRESS\"\r\n\r\nupload-test\r\n--fairway\r\nContent-Disposition: form-data; name=\"sample\"; filename=\"test.txt\"\r\nContent-Type: text/plain\r\n\r\n".to_vec();
    upload.extend(vec![b'x'; 1536 * 1024]);
    upload.extend(b"\r\n--fairway--\r\n");
    for minor in manager.layout.php_versions()?.keys() {
        manager.save_php_version(minor.clone())?;
        let settings = PhpSettings {
            memory_limit_mb: 384,
            max_execution_time: 45,
            upload_max_filesize_mb: 2,
            timezone: "Asia/Yangon".into(),
            display_errors: false,
            session_upload_progress: false,
        };
        manager.save_php_settings(minor, Some(settings.clone()))?;
        // A reconstructed manager models app restarts and config regeneration during updates.
        drop(manager);
        manager = Manager::new(layout.clone())?;
        assert_eq!(manager.php_settings(minor)?.settings, settings);
        let output = process::command(manager.layout.php_exe("php")?)
            .arg("-c")
            .arg(fairway_core::config::windows_ansi_path(
                &layout.data.join("config/php.ini"),
            ))
            .arg(&script)
            .env("PHP_INI_SCAN_DIR", "")
            .env("MIBS", "")
            .output()
            .map_err(|e| e.to_string())?;
        if !output.status.success() || !output.stderr.is_empty() {
            return Err(format!(
                "CLI failed: {}",
                String::from_utf8_lossy(&output.stderr)
            ));
        }
        check_config(
            &String::from_utf8_lossy(&output.stdout),
            &settings,
            true,
            &managed_ini,
        )?;
        let output = process::command(manager.layout.php_exe("php")?)
            .arg(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../tests/fixtures/php_child_environment.php"
            ))
            .arg(layout.runtime.join("composer.phar"))
            .env(
                "PHPRC",
                fairway_core::config::windows_ansi_path(&managed_ini),
            )
            .env("PHP_INI_SCAN_DIR", "")
            .env("MIBS", "")
            .env("FAIRWAY_PROJECT_VALUE", "stale host value")
            .output()
            .map_err(|e| e.to_string())?;
        if !output.status.success() || !output.stderr.is_empty() {
            return Err(format!(
                "PHP {minor} child environment check failed: {} {}",
                String::from_utf8_lossy(&output.stdout),
                String::from_utf8_lossy(&output.stderr)
            ));
        }
        let web = (|| -> Result<(), String> {
            manager.start("nginx")?;
            assert!(manager.save_php_settings(minor, None).is_err());
            // Saving an inactive branch must not touch active settings or stop services.
            let other = manager
                .layout
                .php_versions()?
                .keys()
                .find(|other| *other != minor)
                .cloned()
                .ok_or("Need two PHP versions")?;
            let ini = fs::read(layout.data.join("config/php.ini")).map_err(|e| e.to_string())?;
            manager.save_php_settings(
                &other,
                Some(PhpSettings {
                    memory_limit_mb: 512,
                    ..PhpSettings::default()
                }),
            )?;
            assert_eq!(
                fs::read(layout.data.join("config/php.ini")).map_err(|e| e.to_string())?,
                ini
            );
            let (status, body) = request(&manager, Some(&upload))?;
            if status != 200 {
                return Err(format!("Upload failed: {status}: {body}"));
            }
            let result = check_config(&body, &settings, false, &managed_ini)?;
            if result["file"]["error"] != 0 || result["file"]["size"] != 1536 * 1024 {
                return Err(format!("Upload with progress disabled failed: {result}"));
            }
            Ok(())
        })();
        manager.stop_all()?;
        web?;
        // Lowering the per-file limit still allows PHP to report its normal upload-size error.
        let smaller = PhpSettings {
            upload_max_filesize_mb: 1,
            ..settings.clone()
        };
        manager.save_php_settings(minor, Some(smaller.clone()))?;
        let web = (|| -> Result<(), String> {
            manager.start("nginx")?;
            let (status, body) = request(&manager, Some(&upload))?;
            if status != 200
                || check_config(&body, &smaller, false, &managed_ini)?["file"]["error"] != 1
            {
                return Err(format!("Upload limit not enforced: {status}: {body}"));
            }
            Ok(())
        })();
        manager.stop_all()?;
        web?;
        // Force a late config-generation failure; saved overrides and generated files must roll back.
        let ini = fs::read(layout.data.join("config/php.ini")).map_err(|e| e.to_string())?;
        let saved = fs::read(layout.data.join("php-settings.json")).map_err(|e| e.to_string())?;
        let secret = fs::read(layout.data.join("credentials")).map_err(|e| e.to_string())?;
        fs::write(layout.data.join("credentials"), b"invalid").map_err(|e| e.to_string())?;
        assert!(manager
            .save_php_settings(minor, Some(PhpSettings::default()))
            .is_err());
        fs::write(layout.data.join("credentials"), secret).map_err(|e| e.to_string())?;
        assert_eq!(
            fs::read(layout.data.join("config/php.ini")).map_err(|e| e.to_string())?,
            ini
        );
        assert_eq!(
            fs::read(layout.data.join("php-settings.json")).map_err(|e| e.to_string())?,
            saved
        );
        let invalid = PhpSettings {
            timezone: "Invalid/Timezone".into(),
            ..settings
        };
        assert!(manager.save_php_settings(minor, Some(invalid)).is_err());
        manager.save_php_settings(minor, None)?;
        assert_eq!(
            manager.php_settings(minor)?.settings,
            PhpSettings::default()
        );
        let unlimited = PhpSettings {
            max_execution_time: 0,
            ..PhpSettings::default()
        };
        manager.save_php_settings(minor, Some(unlimited.clone()))?;
        let web = (|| -> Result<(), String> {
            manager.start("nginx")?;
            let (status, body) = request_with_wait(&manager, None, 1)?;
            if status != 200 {
                return Err(format!("Unlimited PHP request failed: {status}: {body}"));
            }
            check_config(&body, &unlimited, false, &managed_ini)?;
            Ok(())
        })();
        manager.stop_all()?;
        web?;
        manager.save_php_settings(minor, None)?;
        println!("PASS PHP {minor}: CLI/web values, child environment inheritance, real uploads with progress off, upload limits, persistence, inactive edits, rollback, reset and unlimited execution.");
    }
    // Reproduce the previous failure: a valid PHP request silent for more than
    // Nginx's old 60-second default must reach the browser successfully.
    let web = (|| -> Result<(), String> {
        manager.start("nginx")?;
        println!("Checking a quiet 65-second PHP response against the aligned Nginx timeout…");
        let (status, body) = request_with_wait(&manager, None, 65)?;
        if status != 200 {
            return Err(format!("Long PHP request failed: {status}: {body}"));
        }
        check_config(&body, &PhpSettings::default(), false, &managed_ini)?;
        Ok(())
    })();
    manager.stop_all()?;
    web?;
    println!("PASS: a quiet 65-second PHP request completes through Nginx with the default 120-second execution setting.");
    Ok(())
}
