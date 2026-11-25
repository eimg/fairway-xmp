//! Explicit managed-only lifecycle IPC. No external target, arbitrary path or SQL.
use crate::postgresql::{self, managed_contract::*, Managed};
use std::sync::Arc;
use tauri::State;
fn unavailable() -> postgresql::Error {
    postgresql::Error {
        code: postgresql::ErrorCode::Unavailable,
        message: "Managed PostgreSQL operation could not complete".into(),
    }
}
macro_rules! mutation {
    ($name:ident,$method:ident,$request:ty,$label:literal) => {
        #[tauri::command]
        pub(super) async fn $name(
            app: tauri::AppHandle,
            state: State<'_, Arc<Managed>>,
            request: $request,
        ) -> postgresql::Result<Snapshot> {
            let _operation =
                super::begin_operation(&app, $label).map_err(|_| postgresql::Error {
                    code: postgresql::ErrorCode::Busy,
                    message: "A service operation or shutdown is in progress".into(),
                })?;
            let state = state.inner().clone();
            tauri::async_runtime::spawn_blocking(move || state.$method(request))
                .await
                .map_err(|_| unavailable())?
        }
    };
}
mutation!(
    postgres_managed_provision,
    provision,
    Confirmation,
    "Preparing PostgreSQL runtime…"
);
mutation!(
    postgres_managed_initialize,
    initialize,
    Initialize,
    "Initializing managed PostgreSQL…"
);
mutation!(
    postgres_managed_start,
    start,
    Confirmation,
    "Starting managed PostgreSQL…"
);
mutation!(
    postgres_managed_stop,
    stop,
    Confirmation,
    "Stopping managed PostgreSQL…"
);
mutation!(
    postgres_managed_restart,
    restart,
    Confirmation,
    "Restarting managed PostgreSQL…"
);
#[tauri::command]
pub(super) async fn postgres_managed_snapshot(
    state: State<'_, Arc<Managed>>,
) -> postgresql::Result<Snapshot> {
    let state = state.inner().clone();
    tauri::async_runtime::spawn_blocking(move || state.snapshot())
        .await
        .map_err(|_| unavailable())?
}
#[tauri::command]
pub(super) async fn postgres_managed_health(
    state: State<'_, Arc<Managed>>,
) -> postgresql::Result<Snapshot> {
    let state = state.inner().clone();
    tauri::async_runtime::spawn_blocking(move || state.health())
        .await
        .map_err(|_| unavailable())?
}
#[tauri::command]
pub(super) async fn postgres_managed_logs(
    state: State<'_, Arc<Managed>>,
) -> postgresql::Result<Logs> {
    let state = state.inner().clone();
    tauri::async_runtime::spawn_blocking(move || state.logs())
        .await
        .map_err(|_| unavailable())?
}
