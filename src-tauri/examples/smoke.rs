//! Exercise real packaged runtimes against an isolated student directory.
use fairway_core::{
    config::{Layout, Ports},
    integration,
    manager::Manager,
    process,
};
use std::{
    fs,
    io::{Read, Write},
    net::{TcpListener, TcpStream},
    path::PathBuf,
    process::Command,
};

fn without_startup_warnings(mut command: Command) -> Result<String, String> {
    let output = command.output().map_err(|error| error.to_string())?;
    let stderr = String::from_utf8_lossy(&output.stderr);
    let stdout = String::from_utf8_lossy(&output.stdout);
    if !output.status.success() || stderr.contains("PHP Startup") || stdout.contains("PHP Startup")
    {
        return Err(format!(
            "Command failed or emitted a PHP startup warning: {stdout} {stderr}"
        ));
    }
    Ok(stdout.trim().to_string())
}

fn http(port: u16, route: &str) -> String {
    let mut socket = TcpStream::connect(("127.0.0.1", port)).unwrap();
    socket
        .set_read_timeout(Some(std::time::Duration::from_secs(15)))
        .unwrap();
    write!(socket, "GET {route} HTTP/1.0\r\nHost: localhost\r\n\r\n").unwrap();
    let mut bytes = Vec::new();
    socket.read_to_end(&mut bytes).unwrap();
    String::from_utf8_lossy(&bytes).into()
}

fn main() -> Result<(), String> {
    let root = PathBuf::from(
        std::env::args()
            .nth(1)
            .ok_or("Provide the test directory.")?,
    );
    let runtime = std::env::args()
        .nth(2)
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("resources/runtime"));
    let upgrade_runtime = std::env::args().nth(3).map(PathBuf::from);
    let layout = Layout::at(
        root.join("student's home"),
        root.join("student data"),
        runtime,
    );
    let mut manager = Manager::new(layout)?;
    let listeners: Vec<_> = (0..4)
        .map(|_| TcpListener::bind(("127.0.0.1", 0)).unwrap())
        .collect();
    let ports: Vec<_> = listeners
        .iter()
        .map(|l| l.local_addr().unwrap().port())
        .collect();
    let chosen = Ports {
        website: ports[0],
        phpmyadmin: ports[1],
        mysql: ports[2],
        php: ports[3],
    };
    drop(listeners);
    manager.save_ports(chosen.clone())?;
    integration::install(&manager.layout)?;
    let result = (|| -> Result<(), String> {
        let mut php = process::command(manager.layout.bin.join(if cfg!(windows) {
            "php.cmd"
        } else {
            "php"
        }));
        php.args(["-r", "echo PHP_VERSION;"]);
        let manifest: serde_json::Value = serde_json::from_slice(
            &fs::read(manager.layout.runtime.join("manifest.json")).map_err(|e| e.to_string())?,
        )
        .map_err(|e| e.to_string())?;
        assert_eq!(process::output(php)?, manifest["php"].as_str().unwrap());
        let mut composer = process::command(manager.layout.bin.join(if cfg!(windows) {
            "composer.cmd"
        } else {
            "composer"
        }));
        composer.arg("--version");
        assert!(process::output(composer)?.contains("Composer version"));
        manager.start_all()?;
        assert!(manager.snapshot().services.iter().all(|s| s.ready));
        let page = http(chosen.website, "/");
        assert!(page.contains("Your environment"), "{page}");
        let probe = manager.layout.www.join("smoke.php");
        fs::write(&probe, format!("<?php $pdo = new PDO('mysql:host=127.0.0.1;port={};dbname=fairway','fairway','fairway'); $pdo->exec('CREATE TABLE IF NOT EXISTS smoke (id INT PRIMARY KEY, title VARCHAR(30))'); $pdo->exec(\"REPLACE INTO smoke VALUES (1, 'classroom works')\"); echo $pdo->query('SELECT title FROM smoke WHERE id=1')->fetchColumn();", chosen.mysql)).map_err(|e| e.to_string())?;
        let result = http(chosen.website, "/smoke.php");
        assert!(result.contains("classroom works"), "{result}");
        let pma = http(chosen.phpmyadmin, "/");
        assert!(
            pma.contains("phpMyAdmin") && !pma.contains("Fatal error"),
            "{pma}"
        );
        // Recover a verified MySQL process after its controller disappears.
        manager.stop("php")?;
        let layout = manager.layout.clone();
        let mut recovered = Manager::new(layout)?;
        if !recovered
            .snapshot()
            .services
            .iter()
            .any(|s| s.id == "mysql" && s.ready)
        {
            return Err("Could not recover the owned database process.".into());
        }
        let previous = std::mem::replace(&mut manager, recovered);
        drop(previous);
        manager.start("nginx")?;
        assert!(http(chosen.website, "/smoke.php").contains("classroom works"));
        let backup = manager.backup()?;
        let sql = fs::read_to_string(&backup).map_err(|e| e.to_string())?;
        assert!(sql.contains("classroom works"));
        manager.stop_all()?;
        assert!(manager.snapshot().services.iter().all(|s| !s.running));
        assert!(process::available(chosen.website) && process::available(chosen.mysql));
        if let Some(runtime) = upgrade_runtime {
            let probe = manager.layout.www.join("upgrade-check.php");
            fs::write(&probe, format!("<?php $pdo = new PDO('mysql:host=127.0.0.1;port={};dbname=fairway','fairway','fairway'); echo $pdo->query('SELECT title FROM smoke WHERE id=1')->fetchColumn();", chosen.mysql)).map_err(|e| e.to_string())?;
            let credentials =
                fs::read(manager.layout.data.join("credentials")).map_err(|e| e.to_string())?;
            let previous = manager.layout.clone();
            manager = Manager::new(Layout::at(previous.home, previous.data, runtime))?;
            manager.start_all()?;
            assert!(http(chosen.website, "/upgrade-check.php").contains("classroom works"));
            assert_eq!(
                fs::read(manager.layout.data.join("credentials")).map_err(|e| e.to_string())?,
                credentials
            );
            assert_eq!(
                fs::read_to_string(manager.layout.data.join("database-ready"))
                    .map_err(|e| e.to_string())?,
                "9.7"
            );
            let journal: serde_json::Value = serde_json::from_slice(
                &fs::read(manager.layout.data.join("database-upgrade.json"))
                    .map_err(|e| e.to_string())?,
            )
            .map_err(|e| e.to_string())?;
            assert!(PathBuf::from(journal["backup"].as_str().unwrap())
                .join("mysql/mysql")
                .is_dir());
            assert!(fs::read_to_string(manager.backup()?)
                .map_err(|e| e.to_string())?
                .contains("classroom works"));
            manager.stop_all()?;
            manager.start_all()?;
            assert!(http(chosen.website, "/upgrade-check.php").contains("classroom works"));
            manager.stop_all()?;
            println!("PASS: MySQL 8.4 → 9.7 upgrade, original database backup, preserved credentials/data, logical backup, and restart.");
        }
        let php_versions = manager.layout.php_versions()?;
        if php_versions.len() > 1 {
            let probe = manager.layout.www.join("php-version.php");
            fs::write(&probe, "<?php echo PHP_VERSION;").map_err(|e| e.to_string())?;
            for (minor, full) in php_versions {
                let full = full.as_str().ok_or("Invalid PHP version in manifest")?;
                manager.save_php_version(minor.clone())?;
                if cfg!(windows) {
                    let ini = fs::read_to_string(manager.layout.data.join("config/php.ini"))
                        .map_err(|e| e.to_string())?;
                    assert_eq!(
                        ini.lines().any(|line| line == "extension=zip"),
                        manager.layout.php_root()?.join("ext/php_zip.dll").is_file(),
                        "PHP {minor} ZIP configuration does not match its runtime"
                    );
                }
                let mut php = process::command(manager.layout.bin.join(if cfg!(windows) {
                    "php.cmd"
                } else {
                    "php"
                }));
                php.args(["-r", "echo PHP_VERSION;"]);
                assert_eq!(without_startup_warnings(php)?, full);
                let mut composer = process::command(manager.layout.bin.join(if cfg!(windows) {
                    "composer.cmd"
                } else {
                    "composer"
                }));
                composer.arg("--version");
                assert!(without_startup_warnings(composer)?.contains("Composer version"));
                manager.start_all()?;
                assert!(http(chosen.website, "/php-version.php").contains(full));
                if cfg!(windows) {
                    let logs = manager.layout.data.join("logs");
                    for name in ["php.log", "php-process.log"] {
                        let contents = fs::read_to_string(logs.join(name)).unwrap_or_default();
                        assert!(!contents.contains("PHP Startup"), "{name}: {contents}");
                    }
                }
                manager.stop_all()?;
                let resumed = Manager::new(manager.layout.clone())?;
                assert_eq!(resumed.layout.php_version()?, minor);
                manager = resumed;
                println!("PASS: PHP {minor} CLI and web runtime");
            }
            manager.save_php_version("8.5".into())?;
        }
        // Settings survive restart; coursework survives removal of PATH integration.
        assert_eq!(manager.layout.ports()?, chosen);
        integration::remove(&manager.layout)?;
        assert!(probe.exists());
        assert!(!manager
            .layout
            .bin
            .join(if cfg!(windows) { "php.cmd" } else { "php" })
            .exists());
        println!("PASS: bundled PHP/Composer commands, real PHP → MySQL → browser response, phpMyAdmin, backup, database recovery, ports, graceful shutdown, user-PATH cleanup, and coursework preservation.");
        Ok(())
    })();
    let shutdown = manager.stop_all();
    let _ = integration::remove(&manager.layout);
    result.and(shutdown)
}
