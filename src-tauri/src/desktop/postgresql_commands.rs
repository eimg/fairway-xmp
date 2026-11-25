//! No manager lock, cluster operations, socket connections or raw executable arguments.
use crate::postgresql::{self, Discovery, Error, ErrorCode};
use std::sync::Arc;
use tauri::State;

#[tauri::command]
pub(super) async fn postgres_runtime_discover(
    state: State<'_, Arc<Discovery>>,
) -> postgresql::Result<postgresql::Inventory> {
    let state = state.inner().clone();
    tauri::async_runtime::spawn_blocking(move || state.discover())
        .await
        .map_err(|_| Error {
            code: ErrorCode::Unavailable,
            message: "PostgreSQL discovery could not complete.".into(),
        })
}

#[tauri::command]
pub(super) async fn postgres_runtime_inspect(
    state: State<'_, Arc<Discovery>>,
    request: postgresql::InspectRequest,
) -> postgresql::Result<postgresql::Candidate> {
    let state = state.inner().clone();
    tauri::async_runtime::spawn_blocking(move || state.inspect(request))
        .await
        .map_err(|_| Error {
            code: ErrorCode::Unavailable,
            message: "PostgreSQL version inspection could not complete.".into(),
        })?
}
