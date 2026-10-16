use crate::{
    config::{Layout, Ports, ProjectConfig, Result},
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
            // Terminal processes are owned separately from managed services.
            // A hidden window keeps them alive; explicit Quit closes them.
            remove_app_marker(&manager)
        })
        .await
        .map_err(|e| e.to_string())
        .and_then(|result| result);
        match result {
            Ok(()) => {
                app.state::<crate::terminal::Terminals>().close_all();
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
async fn terminal_start(
    app: tauri::AppHandle,
    state: State<'_, Shared>,
    request: crate::terminal::StartRequest,
) -> Result<crate::terminal::TerminalInfo> {
    let _operation = begin_operation(&app, "Opening terminal…")?;
    let layout = access(state.inner().clone(), |m| Ok(m.layout.clone())).await?;
    tauri::async_runtime::spawn_blocking(move || {
        app.state::<crate::terminal::Terminals>()
            .start(&layout, request)
    })
    .await
    .map_err(|e| e.to_string())?
}

#[tauri::command]
async fn terminal_context(state: State<'_, Shared>) -> Result<crate::terminal::TerminalContext> {
    access(state.inner().clone(), |m| {
        crate::terminal::context(&m.layout)
    })
    .await
}

#[tauri::command]
async fn terminal_read(
    app: tauri::AppHandle,
    id: String,
) -> Result<crate::terminal::TerminalOutput> {
    tauri::async_runtime::spawn_blocking(move || {
        app.state::<crate::terminal::Terminals>().read(&id)
    })
    .await
    .map_err(|e| e.to_string())?
}

#[tauri::command]
async fn terminal_write(app: tauri::AppHandle, id: String, data: String) -> Result<()> {
    if app.state::<DesktopControl>().lifecycle.quitting() {
        return Err("Fairway XMP is quitting.".into());
    }
    tauri::async_runtime::spawn_blocking(move || {
        app.state::<crate::terminal::Terminals>().write(&id, &data)
    })
    .await
    .map_err(|e| e.to_string())?
}

#[tauri::command]
async fn terminal_resize(app: tauri::AppHandle, id: String, cols: u16, rows: u16) -> Result<()> {
    tauri::async_runtime::spawn_blocking(move || {
        app.state::<crate::terminal::Terminals>()
            .resize(&id, cols, rows)
    })
    .await
    .map_err(|e| e.to_string())?
}

#[tauri::command]
async fn terminal_close(app: tauri::AppHandle, id: String) -> Result<()> {
    tauri::async_runtime::spawn_blocking(move || {
        app.state::<crate::terminal::Terminals>().close(&id)
    })
    .await
    .map_err(|e| e.to_string())
}

#[tauri::command]
async fn data_browser(
    app: tauri::AppHandle,
    state: State<'_, Shared>,
    request: serde_json::Value,
    id: String,
) -> Result<serde_json::Value> {
    let _operation = begin_operation(&app, "Working with database…")?;
    let jobs = app.state::<crate::data_browser::Jobs>();
    let cancel = jobs.register(&id)?;
    let layout = access(state.inner().clone(), |m| Ok(m.layout.clone())).await;
    let result = match layout {
        Ok(layout) => tauri::async_runtime::spawn_blocking(move || {
            crate::data_browser::execute(&layout, request, cancel)
        })
        .await
        .map_err(|e| e.to_string())
        .and_then(|r| r),
        Err(error) => Err(error),
    };
    jobs.finish(&id);
    result
}

#[tauri::command]
async fn cancel_data_browser(app: tauri::AppHandle, id: String) -> Result<()> {
    app.state::<crate::data_browser::Jobs>().cancel(&id);
    Ok(())
}

#[tauri::command]
async fn diagnostic_report(state: State<'_, Shared>) -> Result<String> {
    access(state.inner().clone(), |m| {
        serde_json::to_string_pretty(&crate::diagnostics::report(&m.snapshot()))
            .map_err(|e| e.to_string())
    })
    .await
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

/// Match the hosts file to the registered domains. Runs outside the manager lock
/// because the OS approval prompt waits for the user.
async fn sync_hosts(state: Shared) -> Result<()> {
    let domains = access(state, |m| m.domains()).await?;
    tauri::async_runtime::spawn_blocking(move || crate::hosts::ensure(&domains, "--sync-hosts"))
        .await
        .map_err(|e| e.to_string())?
}

#[tauri::command]
async fn save_project(
    app: tauri::AppHandle,
    state: State<'_, Shared>,
    previous: Option<String>,
    project: ProjectConfig,
) -> Result<Snapshot> {
    let _operation = begin_operation(&app, "Saving project…")?;
    let state = state.inner().clone();
    access(state.clone(), move |m| {
        m.save_project(previous.as_deref(), project)
    })
    .await?;
    sync_hosts(state.clone())
        .await
        .map_err(|e| format!("The project was saved, but the hosts file was not updated: {e}"))?;
    access(state, |m| Ok(m.snapshot())).await
}

#[tauri::command]
async fn remove_project(
    app: tauri::AppHandle,
    state: State<'_, Shared>,
    domain: String,
) -> Result<Snapshot> {
    let _operation = begin_operation(&app, "Removing project…")?;
    let state = state.inner().clone();
    let unused_trust = access(state.clone(), move |m| {
        m.remove_project(&domain)?;
        Ok(m.unused_trust()?.then(|| m.layout.clone()))
    })
    .await?;
    sync_hosts(state.clone())
        .await
        .map_err(|e| format!("The project was removed, but the hosts file was not updated: {e}"))?;
    if let Some(layout) = unused_trust {
        tauri::async_runtime::spawn_blocking(move || crate::tls::remove_trust(&layout))
            .await
            .map_err(|e| e.to_string())?
            .map_err(|e| {
                format!("The project was removed, but certificate trust was not removed: {e}")
            })?;
    }
    access(state, |m| Ok(m.snapshot())).await
}

#[tauri::command]
async fn sync_domains(app: tauri::AppHandle, state: State<'_, Shared>) -> Result<Snapshot> {
    let _operation = begin_operation(&app, "Updating the hosts file…")?;
    let state = state.inner().clone();
    sync_hosts(state.clone()).await?;
    access(state, |m| Ok(m.snapshot())).await
}

#[tauri::command]
async fn set_project_https(
    app: tauri::AppHandle,
    state: State<'_, Shared>,
    domain: String,
    enabled: bool,
) -> Result<Snapshot> {
    let _operation = begin_operation(&app, "Updating project HTTPS…")?;
    let shared = state.inner().clone();
    if enabled {
        let layout = access(shared.clone(), {
            let domain = domain.clone();
            move |m| {
                if !m.domains()?.contains(&domain) {
                    return Err("This project is no longer registered.".into());
                }
                Ok(m.layout.clone())
            }
        })
        .await?;
        tauri::async_runtime::spawn_blocking(move || crate::tls::ensure_trust(&layout))
            .await
            .map_err(|e| e.to_string())??;
    }
    access(shared, move |m| {
        m.set_project_https(&domain, enabled)?;
        Ok(m.snapshot())
    })
    .await
}

#[tauri::command]
async fn remove_https_trust(app: tauri::AppHandle, state: State<'_, Shared>) -> Result<Snapshot> {
    let _operation = begin_operation(&app, "Removing certificate trust…")?;
    let shared = state.inner().clone();
    let layout = access(shared.clone(), |m| {
        if m.layout.registered_projects()?.iter().any(|p| p.https) {
            return Err(
                "Turn off HTTPS for every project before removing certificate trust.".into(),
            );
        }
        Ok(m.layout.clone())
    })
    .await?;
    tauri::async_runtime::spawn_blocking(move || crate::tls::remove_trust(&layout))
        .await
        .map_err(|e| e.to_string())??;
    access(shared, |m| Ok(m.snapshot())).await
}

#[tauri::command]
async fn pick_folder(app: tauri::AppHandle) -> Result<Option<PathBuf>> {
    use tauri_plugin_dialog::DialogExt;
    tauri::async_runtime::spawn_blocking(move || {
        app.dialog()
            .file()
            .blocking_pick_folder()
            .map(|path| path.into_path().map_err(|e| e.to_string()))
            .transpose()
    })
    .await
    .map_err(|e| e.to_string())?
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
    app.state::<crate::terminal::Terminals>().close_all();
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
    if action == "--sync-hosts" {
        crate::hosts::apply(&args[2..])?;
        return Ok(true);
    }
    #[cfg(target_os = "linux")]
    if action == "--trust-ca" {
        crate::tls::apply_linux_trust(
            std::path::Path::new(args.get(2).ok_or("Missing certificate path.")?),
            false,
        )?;
        return Ok(true);
    }
    #[cfg(target_os = "linux")]
    if action == "--remove-ca" {
        crate::tls::apply_linux_trust(
            std::path::Path::new(args.get(2).ok_or("Missing certificate path.")?),
            true,
        )?;
        return Ok(true);
    }
    if action == "db" {
        #[cfg(debug_assertions)]
        let runtime = std::env::var_os("FAIRWAY_RUNTIME_DIR")
            .map(PathBuf::from)
            .unwrap_or_else(|| PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("resources/runtime"));
        #[cfg(not(debug_assertions))]
        let runtime = runtime_location(&std::env::current_exe().map_err(|e| e.to_string())?)?;
        crate::data_browser::cli(&Layout::new(runtime)?, &args[2..])?;
        return Ok(true);
    }
    if action == "--build-info" {
        println!("{}", crate::diagnostics::build_identity());
        return Ok(true);
    }
    if ![
        "--install-cli",
        "--remove-cli",
        "--remove-domains",
        "--remove-https-trust",
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
                // Nginx can leave an empty PID file after a failed start.
                if name == "nginx.pid" && text.trim().is_empty() {
                    if crate::process::health(layout.ports()?.website) {
                        return Err("Open Fairway XMP, stop all services, and quit before installing or uninstalling.".into());
                    }
                    std::fs::remove_file(path).map_err(|e| e.to_string())?;
                    continue;
                }
                let pid: u32 = text.trim().parse().map_err(|_| {
                    "Cannot verify a service PID; stop services before installing or uninstalling."
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
        "--remove-domains" => crate::hosts::ensure(&[], "--sync-hosts")?,
        "--remove-https-trust" => crate::tls::remove_trust(&layout)?,
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
    let args = std::env::args().collect::<Vec<_>>();
    #[cfg(windows)]
    if args
        .get(1)
        .is_some_and(|arg| arg == "db" || arg == "--build-info")
    {
        use windows_sys::Win32::{Foundation::INVALID_HANDLE_VALUE, System::Console::*};
        // A GUI executable has no console when launched from cmd. Attach for
        // command-line use, preserving any pipes supplied by the caller.
        unsafe {
            let handles = [STD_INPUT_HANDLE, STD_OUTPUT_HANDLE, STD_ERROR_HANDLE]
                .map(|kind| (kind, GetStdHandle(kind)));
            AttachConsole(ATTACH_PARENT_PROCESS);
            for (kind, handle) in handles {
                if !handle.is_null() && handle != INVALID_HANDLE_VALUE {
                    SetStdHandle(kind, handle);
                }
            }
        }
    }
    match maintenance(&args) {
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
        .plugin(tauri_plugin_dialog::init())
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
            app.manage(crate::data_browser::Jobs::default());
            app.manage(crate::terminal::Terminals::default());
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
            terminal_start,
            terminal_context,
            terminal_read,
            terminal_write,
            terminal_resize,
            terminal_close,
            diagnostic_report,
            data_browser,
            cancel_data_browser,
            service_action,
            save_settings,
            save_project,
            remove_project,
            sync_domains,
            set_project_https,
            remove_https_trust,
            pick_folder,
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
            app.state::<crate::terminal::Terminals>().close_all();
            app.state::<DesktopControl>()
                .monitor_stopped
                .store(true, Ordering::SeqCst);
        }
        _ => (),
    });
}
