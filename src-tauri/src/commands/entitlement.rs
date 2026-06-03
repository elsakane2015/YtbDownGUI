//! IPC commands for Pro/free entitlement state.

use crate::core::entitlement::{
    ActivateProResult, CheckoutSession, EntitlementStatus, EntitlementStore, FreeQuotaReservation,
    FreeQuotaStatus, ResendLicenseResponse, SupportContact, TransferCodeStatus,
};
use crate::error::AppResult;
use tauri::{Emitter, State};

const ENTITLEMENT_UPDATED_EVENT: &str = "entitlement:updated";

fn emit_entitlement_updated(app: &tauri::AppHandle, status: &EntitlementStatus) {
    let _ = app.emit(ENTITLEMENT_UPDATED_EVENT, status);
}

#[tauri::command]
pub fn get_entitlement_status(store: State<'_, EntitlementStore>) -> AppResult<EntitlementStatus> {
    store.get_status()
}

#[tauri::command]
pub async fn activate_pro(
    app: tauri::AppHandle,
    store: State<'_, EntitlementStore>,
    license_key: String,
) -> AppResult<ActivateProResult> {
    let result = store.activate_pro(license_key).await?;
    if let ActivateProResult::Activated { status } = &result {
        emit_entitlement_updated(&app, status);
    }
    Ok(result)
}

#[tauri::command]
pub async fn refresh_pro(
    app: tauri::AppHandle,
    store: State<'_, EntitlementStore>,
) -> AppResult<EntitlementStatus> {
    let status = store.refresh_pro().await?;
    emit_entitlement_updated(&app, &status);
    Ok(status)
}

#[tauri::command]
pub async fn deactivate_pro(
    app: tauri::AppHandle,
    store: State<'_, EntitlementStore>,
) -> AppResult<EntitlementStatus> {
    let status = store.deactivate_pro().await?;
    emit_entitlement_updated(&app, &status);
    Ok(status)
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
    app: tauri::AppHandle,
    store: State<'_, EntitlementStore>,
    license_key: String,
    transfer_code: String,
) -> AppResult<EntitlementStatus> {
    let status = store
        .activate_with_transfer_code(license_key, transfer_code)
        .await?;
    emit_entitlement_updated(&app, &status);
    Ok(status)
}

#[tauri::command]
pub async fn create_checkout_session(
    store: State<'_, EntitlementStore>,
    purchase_email: String,
) -> AppResult<CheckoutSession> {
    store.create_checkout_session(purchase_email).await
}

#[tauri::command]
pub async fn resend_license(
    store: State<'_, EntitlementStore>,
    purchase_email: String,
) -> AppResult<ResendLicenseResponse> {
    store.resend_license(purchase_email).await
}

#[tauri::command]
pub async fn get_support_contact(store: State<'_, EntitlementStore>) -> AppResult<SupportContact> {
    store.support_contact().await
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
