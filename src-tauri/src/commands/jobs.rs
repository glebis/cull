use tauri::State;

use crate::services::jobs::JobSnapshot;
use crate::AppState;

#[tauri::command]
pub async fn get_job(
    state: State<'_, AppState>,
    job_id: String,
) -> Result<Option<JobSnapshot>, String> {
    Ok(state.jobs.get(&job_id))
}

#[tauri::command]
pub async fn list_jobs(state: State<'_, AppState>) -> Result<Vec<JobSnapshot>, String> {
    Ok(state.jobs.list())
}

#[tauri::command]
pub async fn cancel_job(state: State<'_, AppState>, job_id: String) -> Result<(), String> {
    state.jobs.cancel(&job_id)
}

#[tauri::command]
pub async fn pause_job(state: State<'_, AppState>, job_id: String) -> Result<(), String> {
    state.jobs.pause(&job_id).map(|_| ())
}

#[tauri::command]
pub async fn resume_job(state: State<'_, AppState>, job_id: String) -> Result<(), String> {
    state.jobs.resume(&job_id)
}
