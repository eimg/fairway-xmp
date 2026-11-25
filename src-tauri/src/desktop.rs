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

mod node_project_commands;
mod postgresql_commands;
mod postgresql_explorer_commands;
mod postgresql_managed_commands;

#[cfg(target_os = "macos")]
pub(crate) mod authorization;

#[cfg(any(target_os = "macos", windows))]
use crate::node_tools as node_tools_view;
#[cfg(not(any(target_os = "macos", windows)))]
mod node_tools_view {
    use crate::config::{Layout, Result};
    pub type NodeStatus = serde_json::Value;
    pub fn status(_: &Layout) -> Result<NodeStatus> {
        Err("Node management is currently available on macOS and Windows.".into())
    }
    pub fn remote(_: &Layout) -> Result<Vec<String>> {
        Err("Node management is currently available on macOS and Windows.".into())
    }
    pub fn activation(_: &Layout, _: &str) -> Result<()> {
        Err("Node management is currently available on macOS and Windows.".into())
    }
    pub fn prepare_uninstall(_: &Layout, _: &str) -> Result<()> {
        Err("Node management is currently available on macOS and Windows.".into())
    }

    pub fn change(_: &Layout, _: &str, _: &str, _: &dyn Fn() -> bool) -> Result<()> {
        Err("Node management is currently available on macOS and Windows.".into())
    }
    pub fn use_version(_: &Layout, _: &str, _: &dyn Fn() -> bool) -> Result<()> {
        Err("Node management is currently available on macOS and Windows.".into())
    }
}

type Shared = Arc<Mutex<crate::startup::Startup>>;

#[derive(Default)]
struct DesktopControl {
    lifecycle: Lifecycle,
    monitor_stopped: AtomicBool,
    #[cfg(any(target_os = "macos", windows))]
    node_installations: crate::node_tools::Installations,
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
    if let Some(tray) = app.try_state::<NativeTray>() {
        let _ = tray.set_busy(false);
    }
    let _ = app.emit("fairway-operation-started", label);
    Ok(DesktopOperation {
        operation: Some(operation),
        app: app.clone(),
    })
}

pub(crate) fn show_window(app: &tauri::AppHandle) {
    let handle = app.clone();
    let _ = app.run_on_main_thread(move || {
        // Check on the UI thread, including requests queued before a prompt.
        #[cfg(target_os = "macos")]
        if authorization::in_progress() {
            return;
        }
        if let Some(window) = handle.get_webview_window("main") {
            #[cfg(target_os = "macos")]
            let _ = handle.set_activation_policy(tauri::ActivationPolicy::Regular);
            let _ = window.unminimize();
            let _ = window.show();
            let _ = window.set_focus();
        }
    });
}

fn report_error(app: &tauri::AppHandle, error: String) {
    // Quit and tray failures must reopen a hidden panel. show_window itself
    // skips activation while a macOS authorization prompt is still in progress.
    // Site-save approval returns to the already open panel and does not use this path.
    show_window(app);
    let _ = app.emit("fairway-action-error", error);
}

fn publish_status(app: &tauri::AppHandle, snapshot: &Snapshot) {
    let control = app.state::<DesktopControl>();
    if let Some(tray) = app.try_state::<NativeTray>() {
        let _ = tray.refresh(snapshot, &control.lifecycle);
    }
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
    if let Some(tray) = app.try_state::<NativeTray>() {
        let _ = tray.set_busy(true);
    }
    let _ = app.emit("fairway-shutdown", ());
    app.state::<Arc<crate::postgresql::explorer::Explorer>>()
        .cancel_all();
    let state = app.state::<Shared>().inner().clone();
    let nodes = app
        .state::<Arc<crate::node_projects::NodeProjects>>()
        .inner()
        .clone();
    let postgres = app
        .state::<Arc<crate::postgresql::Managed>>()
        .inner()
        .clone();
    let app = app.clone();
    tauri::async_runtime::spawn(async move {
        let waiting = lifecycle.clone();
        let result = tauri::async_runtime::spawn_blocking(move || {
            waiting.wait_for_operation();
            crate::lifecycle::stop_components(
                || {
                    crate::lifecycle::stop_components(
                        || nodes.stop_all_runs().map_err(|error| error.message),
                        || postgres.stop_for_shutdown().map_err(|error| error.message),
                    )
                },
                || {
                    let mut startup = state.lock().map_err(|_| {
                        "The service manager is unavailable. Restart Fairway XMP.".to_string()
                    })?;
                    startup.manager()?.stop_all()
                },
            )?;
            let mut startup = state
                .lock()
                .map_err(|_| "The service manager is unavailable.".to_string())?;
            remove_app_marker(startup.manager()?)
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
                use tauri_plugin_dialog::{DialogExt, MessageDialogButtons};
                show_window(&app);
                let handle = app.clone();
                app.dialog().message(format!("{error}\n\nKeep Fairway XMP open to retry, or quit and leave any remaining services running. Managed Node projects and embedded terminal sessions will close. PostgreSQL will not be forcibly killed; an unfinished shutdown requires manual review before its next start. Your project and database files will be kept."))
                    .title("Services could not be stopped")
                    .buttons(MessageDialogButtons::OkCancelCustom("Quit anyway".into(), "Keep open".into()))
                    .show(move |leave| {
                        let handle = handle.clone();
                        let lifecycle = lifecycle.clone();
                        tauri::async_runtime::spawn(async move {
                            if leave {
                                let state = handle.state::<Shared>().inner().clone();
                                let detached = tauri::async_runtime::spawn_blocking(move || {
                                    let mut startup = state.lock().map_err(|_| "The service manager is unavailable.")?;
                                    if let Some(manager) = startup.manager.as_mut() {
                                        manager.leave_services_running()?;
                                        let _ = remove_app_marker(manager);
                                    }
                                    Ok::<_, String>(())
                                }).await.map_err(|error| error.to_string()).and_then(|result| result);
                                if let Err(error) = detached {
                                    lifecycle.cancel_quit();
                                    let _ = handle.emit("fairway-operation-finished", ());
                                    report_error(&handle, error);
                                    return;
                                }
                                handle.state::<Arc<crate::node_projects::NodeProjects>>().disconnect_runs();
                                handle.state::<crate::terminal::Terminals>().close_all();
                                lifecycle.allow_exit();
                                handle.exit(0);
                            } else {
                                lifecycle.cancel_quit();
                                let _ = handle.emit("fairway-operation-finished", ());
                                report_error(&handle, error);
                                refresh_status(&handle);
                            }
                        });
                    });
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
                integration::open(&m.layout, "phpmyadmin", &m.ports)
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
        f(manager.manager()?)
    })
    .await
    .map_err(|e| e.to_string())?
}

#[tauri::command]
async fn snapshot(state: State<'_, Shared>) -> Result<Snapshot> {
    access(state.inner().clone(), |m| Ok(m.snapshot())).await
}

#[tauri::command]
fn startup_status(state: State<'_, Shared>) -> Result<crate::startup::Status> {
    Ok(state
        .lock()
        .map_err(|_| "Startup status is unavailable.")?
        .status())
}

#[tauri::command]
async fn retry_startup(
    app: tauri::AppHandle,
    state: State<'_, Shared>,
) -> Result<crate::startup::Status> {
    let _operation = begin_operation(&app, "Retrying startup…")?;
    let state = state.inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        Ok(state
            .lock()
            .map_err(|_| "Startup status is unavailable.")?
            .retry())
    })
    .await
    .map_err(|error| error.to_string())?
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
async fn terminal_external(app: tauri::AppHandle, id: String) -> Result<()> {
    let _operation = begin_operation(&app, "Opening external terminal…")?;
    tauri::async_runtime::spawn_blocking(move || {
        let directory = app
            .state::<crate::terminal::Terminals>()
            .current_directory(&id)?;
        crate::terminal::open_external(&directory)
    })
    .await
    .map_err(|e| e.to_string())?
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
    let progress_app = app.clone();
    let progress_id = id.clone();
    let result = match layout {
        Ok(layout) => tauri::async_runtime::spawn_blocking(move || {
            crate::data_browser::execute_with_progress(&layout, request, cancel, |phase| {
                let _ = progress_app.emit(
                    "data-browser-progress",
                    serde_json::json!({ "id": progress_id, "phase": phase }),
                );
            })
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
async fn mail_inbox(
    state: State<'_, Shared>,
    request: crate::mail::Request,
) -> Result<serde_json::Value> {
    let port = access(state.inner().clone(), |m| Ok(m.ports.mail)).await?;
    tauri::async_runtime::spawn_blocking(move || crate::mail::execute(port, request))
        .await
        .map_err(|e| e.to_string())?
}

#[tauri::command]
async fn load_mail_image(url: String) -> Result<String> {
    crate::mail_images::load(&url).await
}

#[tauri::command]
async fn save_mail_attachment(
    app: tauri::AppHandle,
    state: State<'_, Shared>,
    id: String,
    part: String,
    name: String,
) -> Result<Option<PathBuf>> {
    use tauri_plugin_dialog::DialogExt;
    let port = access(state.inner().clone(), |m| Ok(m.ports.mail)).await?;
    tauri::async_runtime::spawn_blocking(move || {
        let bytes = crate::mail::attachment(port, &id, &part)?;
        let Some(path) = app.dialog().file().set_file_name(name).blocking_save_file() else {
            return Ok(None);
        };
        let path = path.into_path().map_err(|e| e.to_string())?;
        std::fs::write(&path, bytes).map_err(|e| format!("Could not save the attachment: {e}"))?;
        Ok(Some(path))
    })
    .await
    .map_err(|e| e.to_string())?
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

#[tauri::command]
async fn get_php_settings(
    state: State<'_, Shared>,
    version: String,
) -> Result<crate::php_settings::PhpSettingsView> {
    access(state.inner().clone(), move |m| m.php_settings(&version)).await
}

#[tauri::command]
async fn save_php_settings(
    app: tauri::AppHandle,
    state: State<'_, Shared>,
    version: String,
    settings: Option<crate::php_settings::PhpSettings>,
) -> Result<crate::php_settings::PhpSettingsView> {
    let _operation = begin_operation(&app, "Saving PHP settings…")?;
    access(state.inner().clone(), move |m| {
        m.save_php_settings(&version, settings)
    })
    .await
}

/// Match the hosts file to the registered domains. Runs outside the manager lock
/// because the OS approval prompt waits for the user.
async fn sync_hosts(state: Shared) -> Result<()> {
    sync_hosts_with(state, crate::hosts::ensure_for_desktop).await
}

async fn sync_hosts_with(
    state: Shared,
    ensure: fn(&[String]) -> Result<crate::hosts::SetupStatus>,
) -> Result<()> {
    // Read the latest desired sites under the operation guard, including edits
    // or removals made while Settings was open. A failed write needs an explicit
    // retry rather than repeating an error on every monitoring tick.
    let domains = access(state.clone(), |m| {
        let domains = m.domains()?;
        m.domains_approval_pending = false;
        Ok(domains)
    })
    .await?;
    let result = tauri::async_runtime::spawn_blocking(move || ensure(&domains))
        .await
        .map_err(|e| e.to_string())
        .and_then(|result| result);
    access(state, move |m| {
        let pending = result
            .as_ref()
            .is_ok_and(|status| *status == crate::hosts::SetupStatus::ApprovalRequired);
        let remembered = m.remember_address_approval(pending);
        match (result, remembered) {
            (Err(error), Err(cleanup)) => Err(format!("{error} {cleanup}")),
            (Err(error), _) | (_, Err(error)) => Err(error),
            (Ok(_), Ok(())) => Ok(()),
        }
    })
    .await
}

#[tauri::command]
async fn save_project(
    app: tauri::AppHandle,
    state: State<'_, Shared>,
    previous: Option<String>,
    project: ProjectConfig,
) -> Result<Snapshot> {
    let _operation = begin_operation(&app, "Saving site…")?;
    let state = state.inner().clone();
    access(state.clone(), move |m| {
        m.save_project(previous.as_deref(), project)
    })
    .await?;
    sync_hosts(state.clone())
        .await
        .map_err(|e| format!("The site was saved, but the hosts file was not updated: {e}"))?;
    access(state, |m| Ok(m.snapshot())).await
}

#[tauri::command]
async fn remove_project(
    app: tauri::AppHandle,
    state: State<'_, Shared>,
    domain: String,
) -> Result<Snapshot> {
    let _operation = begin_operation(&app, "Removing site…")?;
    let state = state.inner().clone();
    let unused_trust = access(state.clone(), move |m| {
        m.remove_project(&domain)?;
        Ok(m.unused_trust()?.then(|| m.layout.clone()))
    })
    .await?;
    sync_hosts(state.clone())
        .await
        .map_err(|e| format!("The site was removed, but the hosts file was not updated: {e}"))?;
    if access(state.clone(), |m| Ok(m.domains_approval_pending)).await? {
        // Complete the address handoff first. Unused certificate trust remains
        // visible for an explicit retry, rather than opening a second OS prompt.
        return access(state, |m| Ok(m.snapshot())).await;
    }
    if let Some(layout) = unused_trust {
        tauri::async_runtime::spawn_blocking(move || crate::tls::remove_trust(&layout))
            .await
            .map_err(|e| e.to_string())?
            .map_err(|e| {
                format!("The site was removed, but certificate trust was not removed: {e}")
            })?;
    }
    access(state, |m| Ok(m.snapshot())).await
}

#[tauri::command]
async fn sync_domains(
    app: tauri::AppHandle,
    state: State<'_, Shared>,
    check_approval: Option<bool>,
) -> Result<Snapshot> {
    let _operation = begin_operation(&app, "Updating the hosts file…")?;
    let state = state.inner().clone();
    #[cfg(all(target_os = "macos", feature = "native-macos-hosts"))]
    if check_approval.unwrap_or(false) {
        sync_hosts_with(state.clone(), crate::hosts::macos_native::check_approval).await?;
    } else {
        sync_hosts(state.clone()).await?;
    }
    #[cfg(not(all(target_os = "macos", feature = "native-macos-hosts")))]
    {
        let _ = check_approval;
        sync_hosts(state.clone()).await?;
    }
    access(state, |m| Ok(m.snapshot())).await
}

#[cfg(all(target_os = "macos", feature = "native-macos-hosts"))]
fn resume_approved_addresses(app: &tauri::AppHandle, state: &Shared) {
    if !crate::hosts::macos_native::approval_enabled() {
        return;
    }
    // A manual action or Quit may have started since the monitor took its
    // snapshot. Reserving the normal operation guard keeps the same ordering.
    let Ok(operation) = begin_operation(app, "Finishing site address setup…") else {
        return;
    };
    let app = app.clone();
    let state = state.clone();
    tauri::async_runtime::spawn(async move {
        let _operation = operation;
        if let Err(error) =
            sync_hosts_with(state, crate::hosts::macos_native::finish_approved).await
        {
            // Keep the error visible without taking focus from System Settings.
            let _ = app.emit(
                "fairway-action-error",
                format!(
                    "Address setup could not finish. Choose Set up addresses to retry. {error}"
                ),
            );
        }
    });
}

#[tauri::command]
async fn set_project_https(
    app: tauri::AppHandle,
    state: State<'_, Shared>,
    domain: String,
    enabled: bool,
) -> Result<Snapshot> {
    let _operation = begin_operation(&app, "Updating site HTTPS…")?;
    let shared = state.inner().clone();
    if enabled {
        let layout = access(shared.clone(), {
            let domain = domain.clone();
            move |m| {
                if !m.domains()?.contains(&domain) {
                    return Err("This site is no longer registered.".into());
                }
                Ok(m.layout.clone())
            }
        })
        .await?;
        tauri::async_runtime::spawn_blocking(move || crate::tls::ensure_trust(&layout))
            .await
            .map_err(|e| e.to_string())??;
    }
    let changed_domain = domain.clone();
    let unused_trust = access(shared.clone(), move |m| {
        m.set_project_https(&domain, enabled)?;
        Ok((!enabled && m.unused_trust()?).then(|| m.layout.clone()))
    })
    .await?;
    if enabled {
        let layout = access(shared.clone(), |m| Ok(m.layout.clone())).await?;
        tauri::async_runtime::spawn_blocking(move || crate::tls::finish_trust_update(&layout))
            .await.map_err(|e| e.to_string())?
            .map_err(|e| format!("HTTPS is enabled, but retiring the previous certificate trust did not finish. Choose Update certificate trust to retry. {e}"))?;
    }
    if let Some(layout) = unused_trust {
        // The site/config change has succeeded. A cancelled cleanup leaves
        // HTTPS off and exposes the existing unused-trust retry in the panel.
        tauri::async_runtime::spawn_blocking(move || crate::tls::remove_trust(&layout))
            .await
            .map_err(|e| e.to_string())?
            .map_err(|e| {
                format!("HTTPS is off for {changed_domain}.test, but certificate cleanup did not finish. Choose Remove certificate trust to retry. {e}")
            })?;
    }
    access(shared, |m| Ok(m.snapshot())).await
}

#[tauri::command]
async fn remove_https_trust(app: tauri::AppHandle, state: State<'_, Shared>) -> Result<Snapshot> {
    let _operation = begin_operation(&app, "Removing certificate trust…")?;
    let shared = state.inner().clone();
    let layout = access(shared.clone(), |m| {
        if m.layout.registered_projects()?.iter().any(|p| p.https) {
            return Err("Turn off HTTPS for every site before removing certificate trust.".into());
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
async fn pick_sql_file(app: tauri::AppHandle) -> Result<Option<PathBuf>> {
    use tauri_plugin_dialog::DialogExt;
    tauri::async_runtime::spawn_blocking(move || {
        app.dialog()
            .file()
            .set_title("Import SQL file")
            .add_filter("SQL dump (.sql)", &["sql"])
            .add_filter("All files", &["*"])
            .blocking_pick_file()
            .map(|path| path.into_path().map_err(|e| e.to_string()))
            .transpose()
    })
    .await
    .map_err(|e| e.to_string())?
}

#[tauri::command]
async fn pick_sqlite_file(app: tauri::AppHandle) -> Result<Option<PathBuf>> {
    use tauri_plugin_dialog::DialogExt;
    tauri::async_runtime::spawn_blocking(move || {
        app.dialog()
            .file()
            .set_title("Open SQLite file")
            .add_filter(
                "SQLite database (.sqlite, .db)",
                &["sqlite", "db", "sqlite3", "db3"],
            )
            .add_filter("All files", &["*"])
            .blocking_pick_file()
            .map(|path| path.into_path().map_err(|e| e.to_string()))
            .transpose()
    })
    .await
    .map_err(|e| e.to_string())?
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
async fn open_mail_link(url: String) -> Result<()> {
    tauri::async_runtime::spawn_blocking(move || integration::open_http_url(&url))
        .await
        .map_err(|e| e.to_string())?
}

#[tauri::command]
async fn open_target(state: State<'_, Shared>, target: String) -> Result<()> {
    if target == "data" {
        let state = state.inner().clone();
        return tauri::async_runtime::spawn_blocking(move || {
            let startup = state.lock().map_err(|_| "Startup status is unavailable.")?;
            integration::open(
                startup
                    .layout
                    .as_ref()
                    .ok_or("The data directory is unavailable.")?,
                "data",
                &Ports::default(),
            )
        })
        .await
        .map_err(|error| error.to_string())?;
    }
    access(state.inner().clone(), move |m| {
        integration::open(&m.layout, &target, &m.ports)
    })
    .await
}

#[tauri::command]
async fn read_logs(
    directory: State<'_, crate::logs::LogDirectory>,
) -> Result<crate::logs::Snapshot> {
    let (path, permit) = directory.begin()?;
    tauri::async_runtime::spawn_blocking(move || {
        let _permit = permit;
        crate::logs::read(&path)
    })
    .await
    .map_err(|e| e.to_string())
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

#[tauri::command]
async fn node_status(
    app: tauri::AppHandle,
    state: State<'_, Shared>,
) -> Result<node_tools_view::NodeStatus> {
    let layout = access(state.inner().clone(), |m| Ok(m.layout.clone())).await?;
    tauri::async_runtime::spawn_blocking(move || {
        #[cfg(any(target_os = "macos", windows))]
        {
            let mut status = node_tools_view::status(&layout)?;
            status.installation = app.state::<DesktopControl>().node_installations.current();
            Ok(status)
        }
        #[cfg(not(any(target_os = "macos", windows)))]
        {
            let _ = app;
            node_tools_view::status(&layout)
        }
    })
    .await
    .map_err(|e| e.to_string())?
}

#[tauri::command]
async fn node_versions(state: State<'_, Shared>) -> Result<Vec<String>> {
    let layout = access(state.inner().clone(), |m| Ok(m.layout.clone())).await?;
    tauri::async_runtime::spawn_blocking(move || node_tools_view::remote(&layout))
        .await
        .map_err(|e| e.to_string())?
}

#[tauri::command]
async fn node_action(
    app: tauri::AppHandle,
    state: State<'_, Shared>,
    action: String,
    version: String,
    id: Option<String>,
) -> Result<()> {
    let _operation = begin_operation(&app, "Updating Node tools…")?;
    if ["enable", "disable"].contains(&action.as_str()) {
        return access(state.inner().clone(), move |m| {
            node_tools_view::activation(&m.layout, &action)
        })
        .await;
    }
    let layout = access(state.inner().clone(), |m| Ok(m.layout.clone())).await?;
    #[cfg(any(target_os = "macos", windows))]
    if ["install", "install-use", "uninstall"].contains(&action.as_str())
        && crate::node_tools::valid_version(&version)
    {
        let node = crate::node_tools::directory(&layout)
            .join("node-versions")
            .join(format!("v{}", version.trim_start_matches('v')))
            .join("installation")
            .join(if cfg!(windows) {
                "node.exe"
            } else {
                "bin/node"
            });
        app.state::<Arc<crate::node_projects::NodeProjects>>()
            .ensure_runtime_unused(&node)
            .map_err(|error| error.message)?;
    }
    #[cfg(any(target_os = "macos", windows))]
    if action == "install" || action == "install-use" {
        let id = id.ok_or("A Node installation request ID is required.")?;
        let jobs = app.state::<DesktopControl>().node_installations.clone();
        let job = jobs.start(&id, &version)?;
        let _ = app.emit("node-installation", jobs.current());
        let worker_app = app.clone();
        let result = tauri::async_runtime::spawn_blocking(move || {
            let cancelled =
                || job.cancelled() || worker_app.state::<DesktopControl>().lifecycle.quitting();
            node_tools_view::change(&layout, "install", &version, &cancelled)?;
            if action == "install-use" {
                job.activating()?;
                let _ = worker_app.emit("node-installation", jobs.current());
                node_tools_view::use_version(&layout, &version, &cancelled)?;
            }
            Ok(())
        })
        .await
        .map_err(|error| error.to_string())
        .and_then(|result| result);
        let _ = app.emit(
            "node-installation",
            Option::<crate::node_tools::Installation>::None,
        );
        return result;
    }
    #[cfg(not(any(target_os = "macos", windows)))]
    let _ = id;
    if action == "uninstall" {
        let check_version = version.clone();
        access(state.inner().clone(), move |m| {
            node_tools_view::prepare_uninstall(&m.layout, &check_version)
        })
        .await?;
    }
    tauri::async_runtime::spawn_blocking(move || {
        let cancelled = || app.state::<DesktopControl>().lifecycle.quitting();
        if action == "use" {
            node_tools_view::use_version(&layout, &version, &cancelled)
        } else {
            node_tools_view::change(&layout, &action, &version, &cancelled)
        }
    })
    .await
    .map_err(|e| e.to_string())?
}

#[tauri::command]
fn cancel_node_installation(app: tauri::AppHandle, id: String) {
    #[cfg(any(target_os = "macos", windows))]
    app.state::<DesktopControl>().node_installations.cancel(&id);
    #[cfg(not(any(target_os = "macos", windows)))]
    let _ = (app, id);
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
        .updater_builder()
        .timeout(Duration::from_secs(30))
        .build()
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
async fn install_update(
    app: tauri::AppHandle,
    state: State<'_, Shared>,
    version: String,
) -> Result<()> {
    let download = app.state::<DesktopControl>().lifecycle.begin_download()?;
    let (update, bytes) = download.run(async {
    let updater = app
        .updater_builder()
        .timeout(Duration::from_secs(30))
        // All owned processes and markers are handled below. Avoid tearing down
        // the webview before ShellExecute, which can still fail on Windows.
        .on_before_exit(|| {})
        .build()
        .map_err(|e| e.to_string())?;
    let mut update = updater
        .check()
        .await
        .map_err(|e| e.to_string())?
        .ok_or("No update is available.")?;
    if update.version != version {
        return Err(
            "The available version changed. Check for updates again before installing.".into(),
        );
    }
    update.timeout = Some(Duration::from_secs(30 * 60));
    let mut downloaded = 0u64;
    let mut last_progress = std::time::Instant::now();
    let bytes = update.download(|chunk, total| {
        downloaded = downloaded.saturating_add(chunk as u64);
        if last_progress.elapsed() >= Duration::from_millis(150) {
            let _ = app.emit("fairway-update-progress", serde_json::json!({
                "phase": "downloading", "downloaded": downloaded, "total": total,
            }));
            last_progress = std::time::Instant::now();
        }
    }, || {
        let _ = app.emit("fairway-update-progress", serde_json::json!({"phase": "verifying"}));
    }).await.map_err(|e| format!("Could not download or verify the update. Services are still available. Try again later. {e}"))?;

    Ok((update, bytes))
    }).await?;

    // Only verified bytes may reserve normal operations and bundle replacement.
    // Wait for an active service/database action; Quit cancels this wait too.
    let operation = download.begin_operation().await?;
    let _operation = DesktopOperation {
        operation: Some(operation),
        app: app.clone(),
    };
    if let Some(tray) = app.try_state::<NativeTray>() {
        let _ = tray.set_busy(false);
    }
    let _ = app.emit("fairway-operation-started", "Preparing update…");
    let _installation = app.state::<DesktopControl>().lifecycle.begin_install()?;
    let _ = app.emit(
        "fairway-update-progress",
        serde_json::json!({"phase": "preparing"}),
    );
    app.state::<Arc<crate::postgresql::explorer::Explorer>>()
        .cancel_all();
    access(state.inner().clone(), crate::updates::prepare).await?;
    let nodes = app
        .state::<Arc<crate::node_projects::NodeProjects>>()
        .inner()
        .clone();
    let postgres = app
        .state::<Arc<crate::postgresql::Managed>>()
        .inner()
        .clone();
    tauri::async_runtime::spawn_blocking(move || {
        crate::lifecycle::stop_components(
            || nodes.stop_all_runs().map_err(|e| e.message),
            || postgres.stop_for_shutdown().map_err(|e| e.message),
        )
    })
    .await
    .map_err(|_| "Managed-service cleanup failed; update was not installed".to_string())??;
    app.state::<crate::terminal::Terminals>().close_all();
    // Windows exits inside install(); post-install cleanup is unreachable.
    access(state.inner().clone(), |m| remove_app_marker(m)).await?;
    let _ = app.emit(
        "fairway-update-progress",
        serde_json::json!({"phase": "installing"}),
    );
    let result = tauri::async_runtime::spawn_blocking(move || -> Result<()> {
        // The updater may fall back to administrator authorization for a
        // root-owned installation. Downloads and backups do not hand off focus.
        #[cfg(target_os = "macos")]
        let _authorization = authorization::begin()?;
        update.install(bytes).map_err(|e| e.to_string())
    })
    .await
    .map_err(|e| e.to_string())
    .and_then(|r| r);
    if let Err(error) = result {
        access(state.inner().clone(), |m| {
            crate::config::atomic_write(
                &m.layout.data.join("app.pid"),
                std::process::id().to_string().as_bytes(),
            )
        })
        .await?;
        return Err(format!("The update could not be installed. Your projects and backups are kept. Services are stopped; you can start them again or retry the update. {error}"));
    }
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
    #[cfg(all(target_os = "macos", feature = "native-macos-hosts"))]
    if action == "--test-native-hosts-registration" {
        crate::hosts::macos_native::test_registration()?;
        return Ok(true);
    }
    #[cfg(all(target_os = "macos", feature = "native-macos-hosts"))]
    if action == "--remove-native-hosts-helper" {
        crate::hosts::macos_native::unregister()?;
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
    if action == "--remove-data" {
        crate::postgresql::ensure_data_removal_allowed(&layout.data)?;
    }
    if [
        "--prepare-uninstall",
        "--remove-data",
        "--remove-coursework",
    ]
    .contains(&action.as_str())
    {
        for name in ["app.pid", "mysql.pid", "nginx.pid", "php.pid", "mail.pid"] {
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
    if crate::postgresql::explorer::worker_entrypoint() {
        return;
    }
    if crate::postgresql::managed_health_worker() {
        return;
    }
    // Dispatch before Tauri single-instance/plugin initialization.
    match crate::node_projects::runner_entrypoint() {
        Ok(true) => return,
        Err(error) => {
            eprintln!("{}", error.message);
            std::process::exit(1);
        }
        Ok(false) => (),
    }
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
            #[cfg(target_os = "macos")]
            authorization::initialize(app.handle());
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
            app.manage(Arc::new(crate::postgresql::Discovery::new(Some(runtime.clone()))));
            let startup = crate::startup::Startup::new(runtime);
            app.manage(Arc::new(crate::postgresql::Managed::new(startup.layout.clone())));
            app.manage(Arc::new(crate::postgresql::explorer::Explorer::default()));
            let nodes = Arc::new(crate::node_projects::NodeProjects::new(startup.layout.clone()));
            let event_app = app.handle().clone();
            nodes.observe_runs(move |status| { let _ = event_app.emit("node-project-run-status", status); });
            app.manage(nodes);
            if let Some(layout) = &startup.layout { app.manage(crate::logs::LogDirectory::new(layout.data.join("logs"))); }
            app.manage(Arc::new(Mutex::new(startup)));
            app.manage(DesktopControl::default());
            app.manage(crate::data_browser::Jobs::default());
            app.manage(crate::terminal::Terminals::default());
            if let Err(error) = NativeTray::install(app.handle()) {
                app.state::<Shared>().lock().unwrap().add_error(format!("The tray could not be created. Closing the window will quit Fairway XMP. {error}"));
            }
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
                    let snapshot = state.try_lock().ok().and_then(|mut startup| startup.manager.as_mut().map(Manager::snapshot));
                    if let Some(snapshot) = snapshot {
                        publish_status(&handle, &snapshot);
                        #[cfg(all(target_os = "macos", feature = "native-macos-hosts"))]
                        if snapshot.domains_approval_pending {
                            resume_approved_addresses(&handle, &state);
                        }
                    }
                }
                std::thread::sleep(Duration::from_millis(2500));
            });
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            postgresql_commands::postgres_runtime_discover,
            postgresql_commands::postgres_runtime_inspect,
            postgresql_managed_commands::postgres_managed_provision,
            postgresql_managed_commands::postgres_managed_initialize,
            postgresql_managed_commands::postgres_managed_start,
            postgresql_managed_commands::postgres_managed_stop,
            postgresql_managed_commands::postgres_managed_restart,
            postgresql_managed_commands::postgres_managed_snapshot,
            postgresql_managed_commands::postgres_managed_health,
            postgresql_managed_commands::postgres_managed_logs,
            postgresql_explorer_commands::postgres_connection_open,
            postgresql_explorer_commands::postgres_connection_close,
            postgresql_explorer_commands::postgres_explorer_execute,
            postgresql_explorer_commands::postgres_explorer_cancel,

            snapshot,
            startup_status,
            retry_startup,
            terminal_start,
            terminal_context,
            terminal_external,
            terminal_read,
            terminal_write,
            terminal_resize,
            terminal_close,
            diagnostic_report,
            data_browser,
            cancel_data_browser,
            mail_inbox,
            load_mail_image,
            save_mail_attachment,
            service_action,
            save_settings,
            get_php_settings,
            save_php_settings,
            save_project,
            remove_project,
            sync_domains,
            set_project_https,
            remove_https_trust,
            pick_folder,
            pick_sql_file,
            pick_sqlite_file,
            open_target,
            open_mail_link,
            read_logs,
            cli_integration,
            node_status,
            node_project_commands::node_projects_snapshot,
            node_project_commands::node_project_inspect,
            node_project_commands::node_project_prepare_runtime,
            node_project_commands::node_project_start,
            node_project_commands::node_project_stop,
            node_project_commands::node_project_restart,
            node_project_commands::node_project_runs,
            node_project_commands::node_project_logs,
            node_project_commands::node_project_save,
            node_project_commands::node_project_remove,
            node_versions,
            node_action,
            cancel_node_installation,
            backup_database,
            check_update,
            install_update
        ])
        .on_window_event(|window, event| {
            if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                api.prevent_close();
                if window.app_handle().try_state::<NativeTray>().is_none() { request_quit(window.app_handle()); return; }
                if let Err(error) = window.hide() {
                    report_error(window.app_handle(), error.to_string());
                    #[cfg(target_os = "macos")]
                    return;
                }
                // Keep services and the menu bar available without a Dock icon.
                // Restore the normal policy in the shared window-show path.
                #[cfg(target_os = "macos")]
                if let Err(error) = window.app_handle().set_activation_policy(tauri::ActivationPolicy::Accessory) {
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
            app.state::<Arc<crate::node_projects::NodeProjects>>()
                .disconnect_runs();
            app.state::<crate::terminal::Terminals>().close_all();
            app.state::<DesktopControl>()
                .monitor_stopped
                .store(true, Ordering::SeqCst);
        }
        _ => (),
    });
}
