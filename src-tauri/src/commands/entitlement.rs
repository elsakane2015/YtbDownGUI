//! IPC commands for Pro/free entitlement state.

use crate::core::entitlement::{
    ActivateProResult, EntitlementStatus, EntitlementStore, FreeQuotaReservation,
    FreeQuotaStatus, TransferCodeStatus,
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

#[tauri::command]
pub async fn sync_free_quota_status(
    store: State<'_, EntitlementStore>,
) -> AppResult<FreeQuotaStatus> {
    store.sync_free_quota_status().await
}

#[tauri::command]
pub async fn reserve_free_quota(
    store: State<'_, EntitlementStore>,
    count: u32,
) -> AppResult<FreeQuotaReservation> {
    store.reserve_free_quota(count).await
}

#[tauri::command]
pub async fn confirm_free_quota(
    store: State<'_, EntitlementStore>,
    reservation_id: String,
) -> AppResult<FreeQuotaReservation> {
    store.confirm_free_quota(reservation_id).await
}

#[tauri::command]
pub async fn release_free_quota(
    store: State<'_, EntitlementStore>,
    reservation_id: String,
) -> AppResult<FreeQuotaReservation> {
    store.release_free_quota(reservation_id).await
}
