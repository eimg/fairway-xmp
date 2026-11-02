use super::{client::Client, supervisor, transport, Request};
use crate::config::{Layout, Ports};
use std::{
    fs,
    time::{Duration, Instant},
};

struct Fixture {
    root: std::path::PathBuf,
    layout: Layout,
}

#[test]
fn concurrent_installation_probes_do_not_look_like_an_update() {
    let fixture = Fixture::new();
    fixture.layout.initialize().unwrap();
    let path = fixture.layout.data.join("installation.lock");
    let first_probe = transport::shared_lock(&path).unwrap().unwrap();
    let second_probe = transport::shared_lock(&path).unwrap().unwrap();
    let desktop = Client::new(fixture.layout.clone()).unwrap();
    desktop.begin().unwrap();
    assert!(transport::lock_path(&path).unwrap().is_none());
    let release = std::thread::spawn(move || {
        std::thread::sleep(Duration::from_millis(25));
        drop(first_probe);
        drop(second_probe);
    });
    let installation = desktop.reserve_installation().unwrap();
    release.join().unwrap();
    assert!(transport::shared_lock(&path).unwrap().is_none());
    drop(installation);
    desktop.end();
}
impl Fixture {
    fn new() -> Self {
        let root = std::env::temp_dir().join(format!("fw-controller-{}", rand::random::<u64>()));
        let layout = Layout::at(
            root.join("student's home 日本"),
            root.join("student data 日本"),
            root.join("runtime"),
        );
        fs::create_dir_all(&layout.runtime).unwrap();
        fs::write(
            layout.runtime.join("manifest.json"),
            r#"{"php":"8.5.12","mysql":"9.7.4"}"#,
        )
        .unwrap();
        Self { root, layout }
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = supervisor::call(&self.layout, &Request::Shutdown);
        let _ = supervisor::wait_shutdown(&self.layout);
        let _ = fs::remove_dir_all(&self.root);
    }
}

#[test]
fn desktop_and_cli_share_settings_and_exclude_overlapping_operations() {
    let fixture = Fixture::new();
    let desktop = Client::new(fixture.layout.clone()).unwrap();
    let mut cli = Client::new(fixture.layout.clone()).unwrap();
    desktop.begin().unwrap();
    assert!(cli.begin().unwrap_err().contains("in progress"));
    assert!(cli
        .call::<serde_json::Value>(Request::Start {
            service: "mail".into()
        })
        .unwrap_err()
        .contains("in progress"));
    let ports = Ports {
        website: 28800,
        phpmyadmin: 28801,
        mysql: 23306,
        php: 29000,
        mail: 28802,
        smtp: 21025,
    };
    desktop
        .call::<serde_json::Value>(Request::Ports {
            ports: ports.clone(),
        })
        .unwrap();
    desktop.end();
    assert_eq!(cli.snapshot().unwrap().ports.website, ports.website);
}

#[test]
fn disconnect_releases_reservation_and_failed_commands_do_not_poison_controller() {
    let fixture = Fixture::new();
    let desktop = Client::new(fixture.layout.clone()).unwrap();
    let cli = Client::new(fixture.layout.clone()).unwrap();
    desktop.begin().unwrap();
    assert!(desktop
        .call::<serde_json::Value>(Request::Start {
            service: "unrelated".into()
        })
        .is_err());
    desktop.clear_session();
    let deadline = Instant::now() + Duration::from_secs(3);
    loop {
        if cli.begin().is_ok() {
            break;
        }
        assert!(Instant::now() < deadline);
        std::thread::sleep(Duration::from_millis(10));
    }
    cli.end();
}

#[test]
fn installation_blocks_new_clients_and_recovers_after_failure() {
    let fixture = Fixture::new();
    let desktop = Client::new(fixture.layout.clone()).unwrap();
    desktop.begin().unwrap();
    let installation = desktop.reserve_installation().unwrap();
    desktop.shutdown().unwrap();
    assert!(supervisor::ensure(&fixture.layout)
        .unwrap_err()
        .contains("being updated"));
    drop(installation);
    #[cfg(feature = "desktop")]
    crate::startup::mark_desktop(&fixture.layout).unwrap();
    supervisor::ensure(&fixture.layout).unwrap();
    let status = supervisor::call(&fixture.layout, &Request::Status).unwrap();
    assert!(status["services"]
        .as_array()
        .unwrap()
        .iter()
        .all(|s| s["running"] == false));
}

#[test]
fn a_live_legacy_desktop_cannot_be_adopted_using_a_stale_protocol_marker() {
    let fixture = Fixture::new();
    fixture.layout.initialize().unwrap();
    fs::write(
        fixture.layout.data.join("app.pid"),
        std::process::id().to_string(),
    )
    .unwrap();
    fs::write(fixture.layout.data.join("desktop-supervisor"), "stale").unwrap();
    assert!(supervisor::serve(&fixture.layout)
        .unwrap_err()
        .contains("older Fairway desktop"));
}

#[test]
fn another_installation_cannot_mutate_the_running_controller() {
    let fixture = Fixture::new();
    let mut client = Client::new(fixture.layout.clone()).unwrap();
    let before = client.snapshot().unwrap().ports.website;
    let mut other = fixture.layout.clone();
    other.runtime = fixture.root.join("other runtime");
    assert!(supervisor::call(
        &other,
        &Request::Ports {
            ports: Ports::default()
        }
    )
    .unwrap_err()
    .contains("Another Fairway installation"));
    assert_eq!(client.snapshot().unwrap().ports.website, before);
}

#[test]
fn shutdown_waits_for_a_reserved_operation_then_releases_ownership() {
    let fixture = Fixture::new();
    let desktop = Client::new(fixture.layout.clone()).unwrap();
    desktop.begin().unwrap();
    let layout = desktop.layout.clone();
    let (send, receive) = std::sync::mpsc::channel();
    let worker = std::thread::spawn(move || {
        let _ = send.send(supervisor::call(&layout, &Request::Shutdown));
    });
    assert!(receive.recv_timeout(Duration::from_millis(150)).is_err());
    desktop.end();
    receive
        .recv_timeout(Duration::from_secs(5))
        .unwrap()
        .unwrap();
    worker.join().unwrap();
    supervisor::wait_shutdown(&fixture.layout).unwrap();
}
