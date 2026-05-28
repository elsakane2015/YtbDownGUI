use crate::core::download::{
    BatchEnqueueResult, DownloadJob, EnqueueBatchRequest, EnqueueRequest, QueueManager,
};
use crate::core::entitlement::EntitlementStore;
use crate::core::settings::SettingsStore;
use crate::error::{AppError, AppResult};
use tauri::{AppHandle, State};

#[tauri::command]
pub async fn enqueue_download(
    app: AppHandle,
    queue: State<'_, QueueManager>,
    entitlement: State<'_, EntitlementStore>,
    req: EnqueueRequest,
) -> AppResult<String> {
    if entitlement.get_status()?.pro_active {
        return queue.enqueue(&app, req, None);
    }

    let reservation_id = reserve_one_free_quota(&entitlement).await?;
    match queue.enqueue(&app, req, Some(reservation_id.clone())) {
        Ok(job_id) => Ok(job_id),
        Err(error) => {
            let _ = entitlement.release_free_quota(reservation_id).await;
            Err(error)
        }
    }
}

#[tauri::command]
pub async fn enqueue_batch(
    app: AppHandle,
    queue: State<'_, QueueManager>,
    entitlement: State<'_, EntitlementStore>,
    req: EnqueueBatchRequest,
) -> AppResult<BatchEnqueueResult> {
    if entitlement.get_status()?.pro_active {
        let entry_count = req.entries.len();
        return queue.enqueue_batch(&app, req, vec![None; entry_count]);
    }

    let mut reservation_ids = Vec::with_capacity(req.entries.len());
    for _ in &req.entries {
        match reserve_one_free_quota(&entitlement).await {
            Ok(reservation_id) => reservation_ids.push(reservation_id),
            Err(error) => {
                for reservation_id in reservation_ids {
                    let _ = entitlement.release_free_quota(reservation_id).await;
                }
                return Err(error);
            }
        }
    }

    let quota_reservations = reservation_ids
        .iter()
        .cloned()
        .map(Some)
        .collect::<Vec<_>>();
    match queue.enqueue_batch(&app, req, quota_reservations) {
        Ok(result) => Ok(result),
        Err(error) => {
            for reservation_id in reservation_ids {
                let _ = entitlement.release_free_quota(reservation_id).await;
            }
            Err(error)
        }
    }
}

#[tauri::command]
pub fn list_jobs(queue: State<'_, QueueManager>) -> Vec<DownloadJob> {
    queue.list()
}

#[tauri::command]
pub fn cancel_job(app: AppHandle, queue: State<'_, QueueManager>, id: String) -> AppResult<()> {
    queue.cancel(&app, &id)
}

#[tauri::command]
pub fn cancel_batch(
    app: AppHandle,
    queue: State<'_, QueueManager>,
    batch_id: String,
) -> AppResult<usize> {
    queue.cancel_batch(&app, &batch_id)
}

#[tauri::command]
pub fn clear_finished(queue: State<'_, QueueManager>) -> Vec<DownloadJob> {
    queue.clear_finished()
}

#[tauri::command]
pub fn default_download_dir(settings: State<'_, SettingsStore>) -> String {
    settings.get().download_dir
}

async fn reserve_one_free_quota(entitlement: &EntitlementStore) -> AppResult<String> {
    let reservation = entitlement.reserve_free_quota(1).await?;
    reservation.reservation_id.ok_or_else(|| {
        AppError::Other(
            r#"{"code":"quota_reservation_missing","message":"授权服务端没有返回免费额度 reservation。"}"#
                .into(),
        )
    })
}
