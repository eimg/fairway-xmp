//! Startup failures use disposable Python fixture binaries, never real databases.
use super::*;
use std::{net::TcpListener, os::unix::fs::PermissionsExt, path::PathBuf};

struct Fixture {
    manager: Manager,
    root: PathBuf,
}

impl Fixture {
    fn new() -> Self {
        let root = std::env::temp_dir().join(format!(
            "fairway-mysql-startup-{}-{}",
            std::process::id(),
            rand::random::<u64>()
        ));
        let layout = Layout::at(
            root.join("student's home မြန်မာ"),
            root.join("student data"),
            root.join("runtime"),
        );
        fs::create_dir_all(&layout.runtime).unwrap();
        fs::write(
            layout.runtime.join("manifest.json"),
            br#"{"php":"8.5.11","mysql":"9.7.2"}"#,
        )
        .unwrap();
        for (component, executable) in [
            ("mysql", "mysqld"),
            ("mysql", "mysql"),
            ("mysql", "mysqladmin"),
            ("mysql", "mysqldump"),
            ("php", "php"),
            ("php", "php-fpm"),
            ("nginx", "nginx"),
        ] {
            let path = layout.exe(component, executable);
            fs::create_dir_all(path.parent().unwrap()).unwrap();
            fs::write(&path, include_str!("../../tests/fixtures/mysql_startup.py")).unwrap();
            fs::set_permissions(path, fs::Permissions::from_mode(0o755)).unwrap();
        }
        for path in [
            "composer.phar",
            "phpmyadmin/index.php",
            "cacert.pem",
            "nginx/conf/mime.types",
        ] {
            let path = layout.runtime.join(path);
            fs::create_dir_all(path.parent().unwrap()).unwrap();
            fs::write(path, b"fixture").unwrap();
        }
        let mut manager = Manager::new(layout).unwrap();
        let listeners = (0..6)
            .map(|_| TcpListener::bind(("127.0.0.1", 0)).unwrap())
            .collect::<Vec<_>>();
        let ports = listeners
            .iter()
            .map(|l| l.local_addr().unwrap().port())
            .collect::<Vec<_>>();
        manager.ports = Ports {
            website: ports[0],
            phpmyadmin: ports[1],
            mysql: ports[2],
            php: ports[3],
            mail: ports[4],
            smtp: ports[5],
        };
        fs::create_dir(manager.layout.data.join("tmp/fixture")).unwrap();
        Self { manager, root }
    }

    fn flag(&self, name: &str) -> PathBuf {
        self.manager.layout.data.join("tmp/fixture").join(name)
    }

    fn set(&self, name: &str) {
        fs::write(self.flag(name), b"").unwrap();
    }

    fn legacy(&self) {
        fs::create_dir(self.manager.layout.data.join("mysql/mysql")).unwrap();
        fs::write(self.manager.layout.data.join("database-version"), b"9.7").unwrap();
    }

    fn start(&mut self) -> Result<()> {
        self.manager
            .start_with_timeout("mysql", Duration::from_secs(5))
    }

    fn events(&self) -> String {
        fs::read_to_string(self.flag("events")).unwrap_or_default()
    }

    fn mysql(&mut self) -> ServiceStatus {
        self.manager
            .snapshot()
            .services
            .into_iter()
            .find(|s| s.id == "mysql")
            .unwrap()
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        // Fixture-only stop file also cleans up when assertions or simulated
        // mysqladmin failures interrupt a test. No real MySQL is signalled.
        let _ = fs::write(self.flag("stop"), b"");
        for process in self.manager.processes.values_mut() {
            assert!(process.wait(5), "fixture process did not exit");
        }
        let _ = fs::remove_dir_all(&self.root);
    }
}

#[test]
fn secure_initialization_removes_one_shot_secret_on_success_and_failure() {
    for fail in [false, true] {
        let fixture = Fixture::new();
        if fail {
            fixture.set("fail-initialize");
        }
        let secret = fixture.manager.layout.secret().unwrap();
        let result = fixture.manager.initialize_database();
        assert_eq!(result.is_err(), fail);
        if let Err(error) = result {
            assert!(!error.contains(&secret));
        }
        assert!(!fixture
            .manager
            .layout
            .data
            .join("tmp/mysql-init.sql")
            .exists());
        assert_eq!(
            fixture
                .manager
                .layout
                .data
                .join("database-version")
                .exists(),
            !fail
        );
        assert!(fixture.events().contains("initialize-secure"));
    }
}

#[test]
fn readiness_timeout_retry_provisions_same_process_without_reinitializing() {
    let mut fixture = Fixture::new();
    fixture.set("never-ready");
    let error = fixture
        .manager
        .start_with_timeout("mysql", Duration::from_millis(200))
        .unwrap_err();
    assert!(error.contains("did not become ready"));
    let pending = fixture.mysql();
    assert!(pending.running && !pending.ready);
    let credentials = fs::read(fixture.manager.layout.data.join("credentials")).unwrap();
    fs::remove_file(fixture.flag("never-ready")).unwrap();
    fixture.start().unwrap();
    let ready = fixture.mysql();
    assert!(ready.ready && ready.message.is_none());
    assert_eq!(pending.pid, ready.pid);
    assert_eq!(fixture.events().matches("initialize-secure").count(), 1);
    assert_eq!(
        fixture
            .events()
            .lines()
            .filter(|line| *line == "spawn")
            .count(),
        1
    );
    assert_eq!(
        fs::read(fixture.manager.layout.data.join("credentials")).unwrap(),
        credentials
    );
    fixture.manager.stop("mysql").unwrap();
}

#[test]
fn unprovisioned_listener_is_not_reported_ready_and_stop_uses_legacy_authentication() {
    let mut fixture = Fixture::new();
    fixture.legacy();
    fixture
        .manager
        .layout
        .write_configs(&fixture.manager.ports)
        .unwrap();
    fixture
        .manager
        .spawn_service("mysql", Duration::from_secs(5))
        .unwrap();
    assert!(fixture.mysql().running);
    assert!(!fixture.mysql().ready);
    fixture.manager.stop("mysql").unwrap();
    assert!(!fixture.mysql().running);
    assert!(fixture
        .events()
        .contains("mysqladmin:secure\nmysqladmin:passwordless"));
    assert!(!fixture.flag("root-secure").exists());
}

#[test]
fn failed_root_setup_is_stopped_gracefully_and_retry_preserves_existing_data() {
    let mut fixture = Fixture::new();
    fixture.legacy();
    let student = fixture.manager.layout.data.join("mysql/student.ibd");
    fs::write(&student, b"student data").unwrap();
    fixture.set("fail-alter");
    assert!(fixture.start().unwrap_err().contains("root setup failed"));
    assert!(!fixture.mysql().running);
    assert!(!fixture.manager.layout.data.join("database-ready").exists());
    fs::remove_file(fixture.flag("fail-alter")).unwrap();
    fixture.start().unwrap();
    assert!(fixture.mysql().ready);
    assert_eq!(fs::read(student).unwrap(), b"student data");
    assert!(!fixture.events().contains("initialize-secure"));
    fixture.manager.stop("mysql").unwrap();
}

#[test]
fn failed_account_setup_after_password_change_retries_with_saved_password() {
    let mut fixture = Fixture::new();
    fixture.legacy();
    fixture.set("fail-provision");
    assert!(fixture
        .start()
        .unwrap_err()
        .contains("teaching-account setup failed"));
    assert!(!fixture.mysql().running);
    assert!(fixture.flag("root-secure").exists());
    assert!(!fixture.events().contains("mysqladmin:passwordless"));
    fs::remove_file(fixture.flag("fail-provision")).unwrap();
    fixture.start().unwrap();
    assert!(fixture.mysql().ready);
    assert_eq!(fixture.events().matches("alter-root").count(), 1);
    fixture.manager.stop("mysql").unwrap();
}

#[test]
fn failed_shutdown_keeps_tracking_and_retry_finishes_existing_provisioning() {
    let mut fixture = Fixture::new();
    fixture.set("fail-provision");
    fixture.set("fail-shutdown");
    assert!(fixture.start().unwrap_err().contains("shutdown failed"));
    let pending = fixture.mysql();
    assert!(pending.running && !pending.ready);
    fs::remove_file(fixture.flag("fail-provision")).unwrap();
    fs::remove_file(fixture.flag("fail-shutdown")).unwrap();
    fixture.start().unwrap();
    assert_eq!(fixture.mysql().pid, pending.pid);
    assert!(fixture.mysql().ready);
    fixture.manager.stop("mysql").unwrap();
}

#[test]
fn provisioned_database_never_uses_passwordless_shutdown_or_resets_credentials() {
    let mut fixture = Fixture::new();
    fixture.start().unwrap();
    let credentials = fs::read(fixture.manager.layout.data.join("credentials")).unwrap();
    fs::remove_file(fixture.flag("root-secure")).unwrap();
    assert!(fixture.manager.stop("mysql").is_err());
    assert!(fixture.mysql().running);
    assert!(!fixture.events().contains("mysqladmin:passwordless"));
    fixture
        .manager
        .errors
        .insert("mysql".into(), "retry authentication".into());
    assert!(fixture
        .start()
        .unwrap_err()
        .contains("Credentials and data have been preserved"));
    assert!(!fixture.events().contains("alter-root"));
    assert_eq!(
        fs::read(fixture.manager.layout.data.join("credentials")).unwrap(),
        credentials
    );
    fixture.set("root-secure");
    fixture.manager.stop("mysql").unwrap();
}

#[test]
fn start_all_rolls_back_new_unprovisioned_mysql_without_losing_student_data() {
    let mut fixture = Fixture::new();
    fixture.legacy();
    fixture.set("fail-alter");
    let student = fixture.manager.layout.data.join("mysql/student.ibd");
    fs::write(&student, b"coursework").unwrap();
    assert!(fixture
        .manager
        .start_all()
        .unwrap_err()
        .contains("root setup failed"));
    assert!(fixture.manager.processes.is_empty());
    assert_eq!(fs::read(student).unwrap(), b"coursework");
    assert!(fixture.events().contains("mysqladmin:passwordless"));
}

#[test]
#[cfg(feature = "cli")]
fn desktop_backup_cancellation_bypasses_its_reserved_supervisor_connection() {
    use crate::cli::client::Client;
    let fixture = Fixture::new();
    fixture
        .manager
        .layout
        .save_ports(&fixture.manager.ports)
        .unwrap();
    let mut client = Client::new(fixture.manager.layout.clone()).unwrap();
    client.begin().unwrap();
    client.start("mysql").unwrap();
    fixture.set("hang-dump");
    let start = Instant::now();
    let result = client.backup(|| fixture.flag("dump-started").exists());
    client.end();
    assert!(result.unwrap_err().contains("cancelled"));
    assert!(start.elapsed() < Duration::from_secs(4));
    assert!(client
        .snapshot()
        .unwrap()
        .services
        .iter()
        .any(|s| s.id == "mysql" && s.ready));
    fs::remove_file(fixture.flag("hang-dump")).unwrap();
    client.begin().unwrap();
    assert!(client.backup(|| false).is_ok());
    client.end();
    client.shutdown().unwrap();
}

#[test]
fn cancelled_protective_exports_never_proceed_to_import_or_delete() {
    use std::sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    };
    let mut fixture = Fixture::new();
    fixture
        .manager
        .layout
        .save_ports(&fixture.manager.ports)
        .unwrap();
    fixture.start().unwrap();
    let sql = fixture.root.join("import.sql");
    fs::write(&sql, b"DROP TABLE lessons;").unwrap();
    let previous = fixture.manager.layout.data.join("backups/previous.sql");
    fs::write(&previous, b"previous backup").unwrap();
    for request in [
        serde_json::json!({"op":"drop_database", "database":"fairway", "confirmed":true}),
        serde_json::json!({"op":"import", "database":"fairway", "confirmed":true,"sqlFile":sql}),
        serde_json::json!({"op":"export", "database":"fairway"}),
    ] {
        fixture.set("hang-dump");
        let cancel = Arc::new(AtomicBool::new(false));
        let result = thread::scope(|scope| {
            let flag = cancel.clone();
            let started = fixture.flag("dump-started");
            scope.spawn(move || {
                let deadline = Instant::now() + Duration::from_secs(3);
                while !started.exists() && Instant::now() < deadline {
                    thread::sleep(Duration::from_millis(10));
                }
                flag.store(true, Ordering::SeqCst);
            });
            crate::data_browser::execute(&fixture.manager.layout, request, cancel)
        });
        assert!(result.unwrap_err().to_lowercase().contains("cancel"));
        assert_eq!(
            fs::read_dir(previous.parent().unwrap()).unwrap().count(),
            1,
            "cancelling the dump must not publish a completed backup"
        );
        assert_eq!(fs::read(&previous).unwrap(), b"previous backup");
        assert!(fixture.mysql().ready);
        fs::remove_file(fixture.flag("hang-dump")).unwrap();
        fs::remove_file(fixture.flag("dump-started")).unwrap();
    }
    fixture.manager.stop("mysql").unwrap();
}

#[test]
fn cancelling_a_backup_preserves_mysql_and_prior_backups() {
    let mut fixture = Fixture::new();
    fixture.start().unwrap();
    fixture.set("hang-dump");
    let previous = fixture.manager.layout.data.join("backups/previous.sql");
    fs::write(&previous, b"previous backup").unwrap();
    let error = fixture
        .manager
        .backup_with_cancel(&|| fixture.flag("dump-started").exists())
        .unwrap_err();
    assert!(error.contains("cancelled"), "{error}");
    assert_eq!(fs::read(&previous).unwrap(), b"previous backup");
    assert_eq!(fs::read_dir(previous.parent().unwrap()).unwrap().count(), 1);
    assert!(fixture.mysql().ready);
    fs::remove_file(fixture.flag("hang-dump")).unwrap();
    assert!(
        fixture.manager.backup().is_ok(),
        "backup should be retryable"
    );
    fixture.manager.stop("mysql").unwrap();
}

#[test]
fn logical_backups_publish_success_and_remove_failed_partial_exports() {
    let mut fixture = Fixture::new();
    fixture.start().unwrap();
    let backups = fixture.manager.layout.data.join("backups");
    let previous = backups.join("previous.sql");
    fs::write(&previous, b"previous backup").unwrap();
    for flag in ["fail-list", "fail-dump"] {
        fixture.set(flag);
        assert!(fixture.manager.backup().is_err());
        fs::remove_file(fixture.flag(flag)).unwrap();
        assert_eq!(fs::read_dir(&backups).unwrap().count(), 1);
        assert_eq!(fs::read(&previous).unwrap(), b"previous backup");
    }
    for empty in [false, true] {
        if empty {
            fixture.set("empty-backup");
        }
        let path = PathBuf::from(fixture.manager.backup().unwrap());
        let contents = fs::read_to_string(&path).unwrap();
        assert!(contents.contains(if empty {
            "no student databases"
        } else {
            "CREATE DATABASE"
        }));
        assert!(!path.with_extension("sql.partial").exists());
    }
    fixture.manager.stop("mysql").unwrap();
}

#[test]
fn failed_nginx_control_uses_owned_graceful_signal() {
    let mut fixture = Fixture::new();
    let process = OwnedProcess::spawn(
        process::command(fixture.manager.layout.exe("nginx", "nginx")),
        &fixture.manager.layout.data.join("logs/nginx-process.log"),
        false,
    )
    .unwrap();
    fixture.manager.processes.insert("nginx".into(), process);
    let deadline = Instant::now() + Duration::from_secs(5);
    while !fixture.flag("nginx-ready").exists() && Instant::now() < deadline {
        thread::sleep(Duration::from_millis(10));
    }
    assert!(fixture.flag("nginx-ready").exists());
    fixture.manager.stop("nginx").unwrap();
    assert!(!fixture.manager.processes.contains_key("nginx"));
    assert!(fixture.events().contains("nginx-graceful-fallback"));
}
