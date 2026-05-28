//! IPC commands for Pro/free entitlement state.

use crate::core::entitlement::{EntitlementStatus, EntitlementStore};
use crate::error::AppResult;
use tauri::State;

#[tauri::command]
pub fn get_entitlement_status(
    store: State<'_, EntitlementStore>,
) -> AppResult<EntitlementStatus> {
    store.get_status()
}
