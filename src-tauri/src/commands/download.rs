use crate::core::download::{
    default_output_dir, BatchEnqueueResult, DownloadJob, EnqueueBatchRequest,
    EnqueueRequest, QueueManager,
};
use crate::error::AppResult;
use tauri::{AppHandle, State};

#[tauri::command]
pub fn enqueue_download(
    app: AppHandle,
    queue: State<'_, QueueManager>,
    req: EnqueueRequest,
) -> AppResult<String> {
    queue.enqueue(&app, req)
}

#[tauri::command]
pub fn enqueue_batch(
    app: AppHandle,
    queue: State<'_, QueueManager>,
    req: EnqueueBatchRequest,
) -> AppResult<BatchEnqueueResult> {
    queue.enqueue_batch(&app, req)
}

#[tauri::command]
pub fn list_jobs(queue: State<'_, QueueManager>) -> Vec<DownloadJob> {
    queue.list()
}

#[tauri::command]
pub fn cancel_job(queue: State<'_, QueueManager>, id: String) -> AppResult<()> {
    queue.cancel(&id)
}

#[tauri::command]
pub fn cancel_batch(
    queue: State<'_, QueueManager>,
    batch_id: String,
) -> AppResult<usize> {
    queue.cancel_batch(&batch_id)
}

#[tauri::command]
pub fn clear_finished(queue: State<'_, QueueManager>) {
    queue.clear_finished();
}

#[tauri::command]
pub fn default_download_dir() -> String {
    default_output_dir().to_string_lossy().into_owned()
}
