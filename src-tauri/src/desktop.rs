use crate::{
    config::{Layout, Ports, Result},
    integration,
    lifecycle::{Lifecycle, Operation},
    manager::{Manager, Snapshot},
    tray::NativeTray,
};
use serde::Serialize;
use std::{
    path::PathBuf,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, Mutex,
    },
    time::Duration,
};
use tauri::{Emitter, Manager as _, State};
use tauri_plugin_updater::UpdaterExt;

type Shared = Arc<Mutex<Manager>>;

#[derive(Default)]
struct DesktopControl {
    lifecycle: Lifecycle,
    monitor_stopped: AtomicBool,
}

struct DesktopOperation {
    operation: Option<Operation>,
    app: tauri::AppHandle,
}

impl Drop for DesktopOperation {
    fn drop(&mut self) {
        // Release the operation before publishing status or waking shutdown.
        self.operation.take();
        let control = self.app.state::<DesktopControl>();
        if !control.lifecycle.quitting() {
            let _ = self.app.emit("fairway-operation-finished", ());
            refresh_status(&self.app);
        }
    }
}

fn begin_operation(app: &tauri::AppHandle, label: &str) -> Result<DesktopOperation> {
    let operation = app.state::<DesktopControl>().lifecycle.begin()?;
    let _ = app.state::<NativeTray>().set_busy(false);
    let _ = app.emit("fairway-operation-started", label);
    Ok(DesktopOperation {
        operation: Some(operation),
        app: app.clone(),
    })
}

pub(crate) fn show_window(app: &tauri::AppHandle) {
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.unminimize();
        let _ = window.show();
        let _ = window.set_focus();
    }
}

fn report_error(app: &tauri::AppHandle, error: String) {
    show_window(app);
    let _ = app.emit("fairway-error", error);
}

fn publish_status(app: &tauri::AppHandle, snapshot: &Snapshot) {
    let control = app.state::<DesktopControl>();
    let _ = app
        .state::<NativeTray>()
        .refresh(snapshot, &control.lifecycle);
    let _ = app.emit("fairway-status", snapshot);
}

fn refresh_status(app: &tauri::AppHandle) {
    let app = app.clone();
    let state = app.state::<Shared>().inner().clone();
    tauri::async_runtime::spawn(async move {
        if let Ok(snapshot) = access(state, |m| Ok(m.snapshot())).await {
            publish_status(&app, &snapshot);
        }
    });
}

fn remove_app_marker(manager: &Manager) -> Result<()> {
    let marker = manager.layout.data.join("app.pid");
    if marker.exists() {
        std::fs::remove_file(marker).map_err(|e| e.to_string())?;
    }
    Ok(())
}

pub(crate) fn request_quit(app: &tauri::AppHandle) {
    let lifecycle = app.state::<DesktopControl>().lifecycle.clone();
    if !lifecycle.request_quit() {
        return;
    }
    let _ = app.state::<NativeTray>().set_busy(true);
    let _ = app.emit("fairway-shutdown", ());
    let state = app.state::<Shared>().inner().clone();
    let app = app.clone();
    tauri::async_runtime::spawn(async move {
        let waiting = lifecycle.clone();
        let result = tauri::async_runtime::spawn_blocking(move || {
            waiting.wait_for_operation();
            let mut manager = state.lock().map_err(|_| {
                "The service manager is unavailable. Restart Fairway XMP.".to_string()
            })?;
            manager.stop_all()?;
            remove_app_marker(&manager)
        })
        .await
        .map_err(|e| e.to_string())
        .and_then(|result| result);
        match result {
            Ok(()) => {
                lifecycle.allow_exit();
                app.exit(0);
            }
            Err(error) => {
                lifecycle.cancel_quit();
                let _ = app.emit("fairway-operation-finished", ());
                report_error(&app, error);
                refresh_status(&app);
            }
        }
    });
}

pub(crate) fn tray_action(app: &tauri::AppHandle, id: &str) {
    let app = app.clone();
    let id = id.to_string();
    tauri::async_runtime::spawn(async move {
        let state = app.state::<Shared>().inner().clone();
        let result = if id == "phpmyadmin" {
            access(state, |m| {
                integration::open(&m.layout, "phpmyadmin", m.ports.website, m.ports.phpmyadmin)
            })
            .await
        } else {
            let (action, service) = id
                .split_once(':')
                .map_or((id.as_str(), None), |(action, service)| {
                    (action, Some(service.to_string()))
                });
            perform_service_action(&app, state, action.to_string(), service)
                .await
                .map(|_| ())
        };
        if let Err(error) = result {
            report_error(&app, error);
        }
    });
}

async fn access<T: Send + 'static>(
    state: Shared,
    f: impl FnOnce(&mut Manager) -> Result<T> + Send + 'static,
) -> Result<T> {
    tauri::async_runtime::spawn_blocking(move || {
        let mut manager = state
            .lock()
            .map_err(|_| "The service manager is unavailable. Restart Fairway XMP.")?;
        f(&mut manager)
    })
    .await
    .map_err(|e| e.to_string())?
}

#[tauri::command]
async fn snapshot(state: State<'_, Shared>) -> Result<Snapshot> {
    access(state.inner().clone(), |m| Ok(m.snapshot())).await
}

#[tauri::command]
async fn service_action(
    app: tauri::AppHandle,
    state: State<'_, Shared>,
    action: String,
    service: Option<String>,
) -> Result<Snapshot> {
    perform_service_action(&app, state.inner().clone(), action, service).await
}

async fn perform_service_action(
    app: &tauri::AppHandle,
    state: Shared,
    action: String,
    service: Option<String>,
) -> Result<Snapshot> {
    let label = if action.starts_with("stop") {
        "Stopping services…"
    } else {
        "Starting services…"
    };
    let _operation = begin_operation(app, label)?;
    access(state, move |m| {
        match action.as_str() {
            "start_all" => m.start_all()?,
            "stop_all" => m.stop_all()?,
            "start" => m.start(service.as_deref().ok_or("Choose a service.")?)?,
            "stop" => {
                let id = service.as_deref().ok_or("Choose a service.")?;
                // Nginx is the user-facing web service. Stopping PHP first
                // stops Nginx and then its associated PHP worker.
                m.stop(if id == "nginx" { "php" } else { id })?;
            }
            "restart" => {
                let id = service.as_deref().ok_or("Choose a service.")?;
                let restore_web = id == "php"
                    && m.snapshot()
                        .services
                        .iter()
                        .any(|s| s.id == "nginx" && s.running);
                m.stop(if id == "nginx" { "php" } else { id })?;
                m.start(id)?;
                if restore_web {
                    m.start("nginx")?;
                }
            }
            _ => return Err("Unknown action.".into()),
        }
        Ok(m.snapshot())
    })
    .await
}

#[tauri::command]
async fn save_settings(
    app: tauri::AppHandle,
    state: State<'_, Shared>,
    ports: Ports,
    version: String,
) -> Result<Snapshot> {
    let _operation = begin_operation(&app, "Saving settings…")?;
    access(state.inner().clone(), move |m| {
        m.save_settings(ports, version)?;
        Ok(m.snapshot())
    })
    .await
}

#[tauri::command]
async fn open_target(state: State<'_, Shared>, target: String) -> Result<()> {
    access(state.inner().clone(), move |m| {
        integration::open(&m.layout, &target, m.ports.website, m.ports.phpmyadmin)
    })
    .await
}

#[tauri::command]
async fn read_log(state: State<'_, Shared>, service: String) -> Result<String> {
    access(state.inner().clone(), move |m| m.log(&service)).await
}

#[tauri::command]
async fn cli_integration(
    app: tauri::AppHandle,
    state: State<'_, Shared>,
    enabled: bool,
) -> Result<Snapshot> {
    let _operation = begin_operation(&app, "Updating command-line integration…")?;
    access(state.inner().clone(), move |m| {
        if enabled {
            integration::install(&m.layout)?;
        } else {
            integration::remove(&m.layout)?;
        }
        Ok(m.snapshot())
    })
    .await
}

#[tauri::command]
async fn backup_database(app: tauri::AppHandle, state: State<'_, Shared>) -> Result<String> {
    let _operation = begin_operation(&app, "Backing up databases…")?;
    access(state.inner().clone(), |m| m.backup()).await
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct UpdateInfo {
    configured: bool,
    version: Option<String>,
    notes: Option<String>,
}

#[tauri::command]
async fn check_update(app: tauri::AppHandle) -> Result<UpdateInfo> {
    // The release workflow injects a real update endpoint and public key.
    if !app.config().plugins.0.contains_key("updater") {
        return Ok(UpdateInfo {
            configured: false,
            version: None,
            notes: None,
        });
    }
    let update = app
        .updater()
        .map_err(|e| e.to_string())?
        .check()
        .await
        .map_err(|e| e.to_string())?;
    Ok(UpdateInfo {
        configured: true,
        version: update.as_ref().map(|u| u.version.clone()),
        notes: update.and_then(|u| u.body),
    })
}

#[tauri::command]
async fn install_update(app: tauri::AppHandle, state: State<'_, Shared>) -> Result<()> {
    let _operation = begin_operation(&app, "Installing update…")?;
    let updater = app.updater().map_err(|e| e.to_string())?;
    let update = updater
        .check()
        .await
        .map_err(|e| e.to_string())?
        .ok_or("No update is available.")?;
    // Stop and make a logical backup before replacing any bundled runtime.
    access(state.inner().clone(), |m| {
        if m.layout.data.join("database-ready").exists() {
            m.start("mysql")?;
            m.backup()?;
        }
        m.stop_all()
    })
    .await?;
    update
        .download_and_install(|_, _| {}, || {})
        .await
        .map_err(|e| e.to_string())?;
    access(state.inner().clone(), |m| remove_app_marker(m)).await?;
    app.state::<DesktopControl>().lifecycle.allow_exit();
    app.restart();
}

pub fn runtime_location(exe: &std::path::Path) -> Result<PathBuf> {
    let directory = exe.parent().ok_or("Cannot locate the application.")?;
    #[cfg(target_os = "macos")]
    {
        Ok(directory.join("../Resources/runtime"))
    }
    #[cfg(not(target_os = "macos"))]
    {
        Ok(directory.join("runtime"))
    }
}

// Installer hooks use the same tested integration code as the panel.
pub fn maintenance(args: &[String]) -> Result<bool> {
    let Some(action) = args.get(1) else {
        return Ok(false);
    };
    if ![
        "--install-cli",
        "--remove-cli",
        "--remove-data",
        "--remove-coursework",
        "--prepare-uninstall",
    ]
    .contains(&action.as_str())
    {
        return Ok(false);
    }
    let runtime = runtime_location(&std::env::current_exe().map_err(|e| e.to_string())?)?;
    let layout = Layout::new(runtime)?;
    if [
        "--prepare-uninstall",
        "--remove-data",
        "--remove-coursework",
    ]
    .contains(&action.as_str())
    {
        for name in ["app.pid", "mysql.pid", "nginx.pid", "php.pid"] {
            let path = layout.data.join(name);
            if let Ok(text) = std::fs::read_to_string(&path) {
                let pid: u32 = text.trim().parse().map_err(|_| {
                    "Cannot verify a service PID; stop services before uninstalling."
                })?;
                if crate::process::pid_alive(pid) {
                    return Err(
                        "Open Fairway XMP, stop all services, and quit before uninstalling.".into(),
                    );
                }
                std::fs::remove_file(path).map_err(|e| e.to_string())?;
            }
        }
    }
    match action.as_str() {
        "--install-cli" => integration::install(&layout)?,
        "--remove-cli" => integration::remove(&layout)?,
        "--remove-data" => {
            integration::remove(&layout)?;
            // Refuse cleanup while an owned server may still be using the data directory.
            if layout.data.join("mysql.pid").exists() || layout.data.join("nginx.pid").exists() {
                return Err("Open Fairway XMP and stop all services before deleting data.".into());
            }
            if layout.data.exists() {
                std::fs::remove_dir_all(&layout.data).map_err(|e| e.to_string())?;
            }
        }
        "--remove-coursework" => {
            if layout.www.exists() {
                std::fs::remove_dir_all(&layout.www).map_err(|e| e.to_string())?;
            }
        }
        "--prepare-uninstall" => (),
        _ => unreachable!(),
    }
    Ok(true)
}

pub fn run() {
    match maintenance(&std::env::args().collect::<Vec<_>>()) {
        Ok(true) => return,
        Err(error) => {
            eprintln!("{error}");
            std::process::exit(1);
        }
        _ => (),
    }
    let app = tauri::Builder::default()
        .plugin(tauri_plugin_single_instance::init(|app, _, _| {
            show_window(app);
        }))
        .setup(|app| {
            if app.config().plugins.0.contains_key("updater") {
                app.handle()
                    .plugin(tauri_plugin_updater::Builder::new().build())?;
            }
            let runtime = if cfg!(debug_assertions) {
                std::env::var_os("FAIRWAY_RUNTIME_DIR")
                    .map(PathBuf::from)
                    .unwrap_or_else(|| {
                        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("resources/runtime")
                    })
            } else {
                app.path().resource_dir()?.join("runtime")
            };
            let manager = Manager::new(Layout::new(runtime).map_err(std::io::Error::other)?)
                .map_err(std::io::Error::other)?;
            crate::config::atomic_write(
                &manager.layout.data.join("app.pid"),
                std::process::id().to_string().as_bytes(),
            )
            .map_err(std::io::Error::other)?;
            app.manage(Arc::new(Mutex::new(manager)));
            app.manage(DesktopControl::default());
            // Fail initialization if the tray cannot be created; otherwise a
            // hidden window could leave users without a way to control the app.
            NativeTray::install(app.handle())?;
            let handle = app.handle().clone();
            let state = handle.state::<Shared>().inner().clone();
            std::thread::spawn(move || loop {
                let control = handle.state::<DesktopControl>();
                if control.monitor_stopped.load(Ordering::SeqCst) {
                    break;
                }
                // Native monitoring keeps working when the hidden webview is
                // throttled. Never hold the manager lock while updating menus.
                if !control.lifecycle.busy() {
                    let snapshot = state.try_lock().ok().map(|mut m| m.snapshot());
                    if let Some(snapshot) = snapshot {
                        publish_status(&handle, &snapshot);
                    }
                }
                std::thread::sleep(Duration::from_millis(2500));
            });
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            snapshot,
            service_action,
            save_settings,
            open_target,
            read_log,
            cli_integration,
            backup_database,
            check_update,
            install_update
        ])
        .on_window_event(|window, event| {
            if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                api.prevent_close();
                if let Err(error) = window.hide() {
                    report_error(window.app_handle(), error.to_string());
                }
            }
        })
        .build(tauri::generate_context!())
        .expect("Fairway XMP could not initialize");
    app.run(|app, event| match event {
        tauri::RunEvent::ExitRequested { api, .. } => {
            if !app.state::<DesktopControl>().lifecycle.exit_allowed() {
                api.prevent_exit();
                request_quit(app);
            }
        }
        #[cfg(target_os = "macos")]
        tauri::RunEvent::Reopen { .. } => show_window(app),
        tauri::RunEvent::Exit => {
            app.state::<DesktopControl>()
                .monitor_stopped
                .store(true, Ordering::SeqCst);
        }
        _ => (),
    });
}
