//! IPC commands for Pro/free entitlement state.

use crate::core::entitlement::{
    ActivateProResult, EntitlementStatus, EntitlementStore, TransferCodeStatus,
};
use crate::error::AppResult;
use tauri::State;

#[tauri::command]
pub fn get_entitlement_status(store: State<'_, EntitlementStore>) -> AppResult<EntitlementStatus> {
    store.get_status()
}

#[tauri::command]
pub async fn activate_pro(
    store: State<'_, EntitlementStore>,
    license_key: String,
) -> AppResult<ActivateProResult> {
    store.activate_pro(license_key).await
}

#[tauri::command]
pub async fn refresh_pro(store: State<'_, EntitlementStore>) -> AppResult<EntitlementStatus> {
    store.refresh_pro().await
}

#[tauri::command]
pub async fn deactivate_pro(store: State<'_, EntitlementStore>) -> AppResult<EntitlementStatus> {
    store.deactivate_pro().await
}

#[tauri::command]
pub async fn send_transfer_code(
    store: State<'_, EntitlementStore>,
    license_key: String,
) -> AppResult<TransferCodeStatus> {
    store.send_transfer_code(license_key).await
}

#[tauri::command]
pub async fn activate_with_transfer_code(
    store: State<'_, EntitlementStore>,
    license_key: String,
    transfer_code: String,
) -> AppResult<EntitlementStatus> {
    store
        .activate_with_transfer_code(license_key, transfer_code)
        .await
}
