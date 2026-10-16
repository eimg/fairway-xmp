//! Exercise real packaged runtimes against an isolated student directory.
use fairway_core::{
    config::{Layout, Ports, ProjectConfig},
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
    http_host(port, route, "localhost")
}

fn http_host(port: u16, route: &str, host: &str) -> String {
    let mut socket = TcpStream::connect(("127.0.0.1", port)).unwrap();
    socket
        .set_read_timeout(Some(std::time::Duration::from_secs(15)))
        .unwrap();
    write!(socket, "GET {route} HTTP/1.0\r\nHost: {host}\r\n\r\n").unwrap();
    let mut bytes = Vec::new();
    socket.read_to_end(&mut bytes).unwrap();
    String::from_utf8_lossy(&bytes).into()
}

fn https(layout: &Layout, host: &str) -> Result<String, String> {
    let port = fairway_core::tls::https_port();
    let mut command = process::command("curl");
    command
        .args([
            "--silent",
            "--show-error",
            "--fail",
            "--noproxy",
            "*",
            "--resolve",
        ])
        .arg(format!("{host}:{port}:127.0.0.1"))
        // Schannel otherwise fails on the local CA, which has no revocation endpoint.
        .args(if cfg!(windows) {
            &["--ssl-no-revoke"][..]
        } else {
            &[]
        })
        .arg("--cacert")
        .arg(layout.data.join("tls/root-ca.pem"))
        .arg(format!("https://{host}:{port}/"));
    process::output(command)
}

fn free_ports() -> Result<Ports, String> {
    let listeners = (0..4)
        .map(|_| TcpListener::bind(("127.0.0.1", 0)))
        .collect::<std::io::Result<Vec<_>>>()
        .map_err(|e| e.to_string())?;
    let ports = listeners
        .iter()
        .map(|listener| listener.local_addr().unwrap().port())
        .collect::<Vec<_>>();
    Ok(Ports {
        website: ports[0],
        phpmyadmin: ports[1],
        mysql: ports[2],
        php: ports[3],
    })
}

fn admin(manager: &Manager) -> Command {
    let mut command = process::command(manager.layout.exe("mysql", "mysql"));
    command
        .arg(format!(
            "--defaults-file={}",
            manager.layout.data.join("config/admin.cnf").display()
        ))
        .args([
            "--batch",
            "--skip-column-names",
            "--default-character-set=utf8mb4",
        ]);
    command
}

fn verify_restore(source: &Manager, root: &std::path::Path, backup: &str) -> Result<(), String> {
    let layout = Layout::at(
        root.join("restored home"),
        root.join("restored data"),
        source.layout.runtime.clone(),
    );
    let mut restored = Manager::new(layout)?;
    restored.save_ports(free_ports()?)?;
    let result = (|| -> Result<(), String> {
        restored.start_all()?;
        process::sql(
            admin(&restored),
            &fs::read_to_string(backup).map_err(|e| e.to_string())?,
        )?;
        let mut query = admin(&restored);
        query.args(["-e", "SELECT name FROM beta_school.student_names WHERE id=1; SELECT COUNT(*) FROM beta_archive.enrolments; CALL beta_school.student_count(); SELECT beta_school.plus_one(1); SELECT COUNT(*) FROM information_schema.EVENTS WHERE EVENT_SCHEMA='beta_school' AND EVENT_NAME='disabled_cleanup' AND STATUS='DISABLED'; INSERT INTO beta_archive.enrolments VALUES (2,1); SELECT COUNT(*) FROM beta_school.audit;"]);
        let output = process::output(query)?;
        assert_eq!(
            output.lines().collect::<Vec<_>>(),
            ["Student May", "1", "1", "2", "1", "2"]
        );
        let mut invalid = admin(&restored);
        invalid.args(["-e", "INSERT INTO beta_archive.enrolments VALUES (3,999);"]);
        assert!(
            process::output(invalid).is_err(),
            "Restored foreign key must reject an unknown student"
        );
        fs::write(restored.layout.www.join("restore.php"), format!("<?php $pdo = new PDO('mysql:host=127.0.0.1;port={};dbname=beta_school','fairway','fairway'); echo $pdo->query('SELECT name FROM student_names WHERE id=1')->fetchColumn();", restored.ports.mysql)).map_err(|e| e.to_string())?;
        assert!(http(restored.ports.website, "/restore.php").contains("Student May"));
        Ok(())
    })();
    let shutdown = restored.stop_all();
    result.and(shutdown)?;
    println!("PASS: backup restored into a fresh database; multiple databases, data, view, foreign key, trigger, procedure, function, event and PHP response verified.");
    Ok(())
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
    let listeners: Vec<_> = (0..5)
        .map(|_| TcpListener::bind(("127.0.0.1", 0)).unwrap())
        .collect();
    let ports: Vec<_> = listeners
        .iter()
        .map(|l| l.local_addr().unwrap().port())
        .collect();
    std::env::set_var("FAIRWAY_DEV_HTTPS_PORT", ports[4].to_string());
    let mut manager = Manager::new(layout)?;
    let chosen = Ports {
        website: ports[0],
        phpmyadmin: ports[1],
        mysql: ports[2],
        php: ports[3],
    };
    drop(listeners);
    manager.save_ports(chosen.clone())?;
    fs::create_dir_all(manager.layout.www.join("plain-project")).map_err(|e| e.to_string())?;
    fs::write(
        manager.layout.www.join("plain-project/index.php"),
        "<?php echo 'plain project';",
    )
    .map_err(|e| e.to_string())?;
    // Registered projects can live outside fairway/www and serve only their chosen web root.
    let shop = manager.layout.home.join("code/shop");
    fs::create_dir_all(shop.join("public")).map_err(|e| e.to_string())?;
    fs::write(shop.join("secret.txt"), "private project file").map_err(|e| e.to_string())?;
    fs::write(
        shop.join("public/index.php"),
        "<?php echo 'front controller ', $_SERVER['REQUEST_URI'];",
    )
    .map_err(|e| e.to_string())?;
    let project = |domain: &str, root: PathBuf, index_fallback: bool| ProjectConfig {
        name: domain.into(),
        domain: domain.into(),
        root,
        https: false,
        index_fallback,
    };
    manager.save_project(
        None,
        project(
            "plain-project",
            manager.layout.www.join("plain-project"),
            false,
        ),
    )?;
    manager.save_project(None, project("shop", shop.join("public"), true))?;
    integration::install(&manager.layout)?;
    let result = (|| -> Result<(), String> {
        let snapshot = manager.snapshot();
        assert!(snapshot.path_installed);
        assert_eq!(
            snapshot.cli_php,
            integration::launcher(&manager.layout, "php")
        );
        assert_eq!(
            snapshot.cli_composer,
            integration::launcher(&manager.layout, "composer")
        );
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
        assert!(http_host(chosen.website, "/", "plain-project.test").contains("plain project"));
        assert!(http_host(chosen.website, "/courses/1", "shop.test")
            .contains("front controller /courses/1"));
        assert!(
            !http_host(chosen.website, "/secret.txt", "shop.test").contains("private project file")
        );
        assert!(
            !http_host(chosen.website, "/courses/1", "plain-project.test")
                .contains("plain project")
        );
        fs::create_dir_all(manager.layout.www.join("new-project")).map_err(|e| e.to_string())?;
        fs::write(
            manager.layout.www.join("new-project/index.php"),
            "<?php echo 'refreshed project';",
        )
        .map_err(|e| e.to_string())?;
        manager.save_project(
            None,
            project("new-project", manager.layout.www.join("new-project"), false),
        )?;
        let mut refreshed = false;
        for _ in 0..20 {
            if http_host(chosen.website, "/", "new-project.test").contains("refreshed project") {
                refreshed = true;
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(100));
        }
        assert!(refreshed, "Nginx did not reload the added project site");
        manager.set_project_https("new-project", true)?;
        let mut tls_ready = false;
        for _ in 0..20 {
            if https(&manager.layout, "new-project.test")
                .is_ok_and(|body| body.contains("refreshed project"))
            {
                tls_ready = true;
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(100));
        }
        if !tls_ready {
            return Err(format!(
                "HTTPS project did not serve a trusted certificate: {:?}",
                https(&manager.layout, "new-project.test")
            ));
        }
        manager.set_project_https("plain-project", true)?;
        for _ in 0..20 {
            if https(&manager.layout, "plain-project.test")
                .is_ok_and(|body| body.contains("plain project"))
                && https(&manager.layout, "new-project.test")
                    .is_ok_and(|body| body.contains("refreshed project"))
            {
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(100));
        }
        if !https(&manager.layout, "plain-project.test")
            .is_ok_and(|body| body.contains("plain project"))
        {
            return Err("The second HTTPS project did not receive a valid certificate".into());
        }
        manager.set_project_https("new-project", false)?;
        manager.set_project_https("plain-project", false)?;
        manager.remove_project("new-project")?;
        let mut removed = false;
        for _ in 0..20 {
            if !http_host(chosen.website, "/", "new-project.test").contains("refreshed project") {
                removed = true;
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(100));
        }
        assert!(removed, "Nginx still serves a removed project");
        assert!(manager.layout.www.join("new-project/index.php").is_file());
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
        process::sql(admin(&manager), "CREATE DATABASE beta_school; CREATE DATABASE beta_archive; CREATE TABLE beta_school.students (id INT PRIMARY KEY, name VARCHAR(100)); CREATE TABLE beta_school.audit (enrolment_id INT); CREATE TABLE beta_archive.enrolments (id INT PRIMARY KEY, student_id INT, FOREIGN KEY (student_id) REFERENCES beta_school.students(id)); CREATE TRIGGER beta_archive.enrolment_insert AFTER INSERT ON beta_archive.enrolments FOR EACH ROW INSERT INTO beta_school.audit VALUES (NEW.id); CREATE VIEW beta_school.student_names AS SELECT id,name FROM beta_school.students; CREATE PROCEDURE beta_school.student_count() SELECT COUNT(*) FROM beta_school.students; CREATE FUNCTION beta_school.plus_one(n INT) RETURNS INT DETERMINISTIC RETURN n+1; CREATE EVENT beta_school.disabled_cleanup ON SCHEDULE EVERY 1 DAY DISABLE DO DELETE FROM beta_school.audit WHERE enrolment_id<0; INSERT INTO beta_school.students VALUES (1,'Student May'); INSERT INTO beta_archive.enrolments VALUES (1,1);")?;
        let backup = manager.backup()?;
        let sql = fs::read_to_string(&backup).map_err(|e| e.to_string())?;
        assert!(sql.contains("classroom works"));
        let second = manager.backup()?;
        assert_ne!(backup, second);
        assert_eq!(fs::read_to_string(&backup).map_err(|e| e.to_string())?, sql);
        // Failed dump setup must not leave a partial file or a completed backup.
        let config = manager.layout.data.join("config/admin.cnf");
        let saved = config.with_extension("saved");
        fs::rename(&config, &saved).map_err(|e| e.to_string())?;
        let failed = manager.backup();
        fs::rename(&saved, &config).map_err(|e| e.to_string())?;
        assert!(failed.is_err());
        assert!(!fs::read_dir(manager.layout.data.join("backups"))
            .map_err(|e| e.to_string())?
            .any(|entry| entry
                .unwrap()
                .path()
                .extension()
                .is_some_and(|extension| extension == "partial")));
        manager.stop_all()?;
        verify_restore(&manager, &root, &backup)?;
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
        let snapshot = manager.snapshot();
        assert!(!snapshot.path_installed);
        assert!(snapshot.cli_php.is_none() && snapshot.cli_composer.is_none());
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
