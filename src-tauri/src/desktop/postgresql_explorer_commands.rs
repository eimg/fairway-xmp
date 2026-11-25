//! Typed independent PostgreSQL jobs. No MySQL lock/credentials or external service controls.
use crate::postgresql::{explorer::Explorer, explorer_contract::*, Managed};
use std::sync::Arc;
use tauri::State;
fn task_error(_: impl std::fmt::Debug) -> Error {
    fail(Code::Unavailable, "PostgreSQL task could not complete")
}
#[tauri::command]
pub(super) async fn postgres_connection_open(
    state: State<'_, Arc<Explorer>>,
    request: Connect,
) -> Result<Session> {
    state.connect(request)
}
#[tauri::command]
pub(super) async fn postgres_connection_close(
    state: State<'_, Arc<Explorer>>,
    request: SessionRequest,
) -> Result<()> {
    state.disconnect(request)
}
#[tauri::command]
pub(super) async fn postgres_explorer_execute(
    app: tauri::AppHandle,
    state: State<'_, Arc<Explorer>>,
    managed: State<'_, Arc<Managed>>,
    request: Request,
) -> Result<Output> {
    let _operation = super::begin_operation(&app, "Running PostgreSQL operation…")
        .map_err(|_| fail(Code::Busy, "Service operation or shutdown in progress"))?;
    let state = state.inner().clone();
    let managed = managed.inner().clone();
    tauri::async_runtime::spawn_blocking(move || state.execute(&managed, request))
        .await
        .map_err(task_error)?
}
#[tauri::command]
pub(super) async fn postgres_explorer_cancel(
    state: State<'_, Arc<Explorer>>,
    request: Cancel,
) -> Result<()> {
    state.cancel(request)
}
