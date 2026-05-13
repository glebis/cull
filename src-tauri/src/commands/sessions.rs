use crate::db_core::models::{ActivityContext, Canvas, Session, SessionEvent};
use crate::services::sessions as svc;
use crate::services::ServiceContext;
use crate::AppState;
use tauri::State;

#[tauri::command]
pub async fn create_session(state: State<'_, AppState>, name: String) -> Result<Session, String> {
    let ctx = ServiceContext::from_app_state(&state, None);
    let sessions_root = ctx
        .db
        .get_setting("sessions_root")
        .ok()
        .flatten()
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| {
            state
                .app_data_dir
                .parent()
                .unwrap_or(&state.app_data_dir)
                .join("Cull")
                .join("Sessions")
        });
    svc::create_session(&ctx, &name, &sessions_root).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn list_sessions(state: State<'_, AppState>) -> Result<Vec<Session>, String> {
    let ctx = ServiceContext::from_app_state(&state, None);
    svc::list_sessions(&ctx).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn list_session_events(
    state: State<'_, AppState>,
    session_id: Option<String>,
    limit: Option<u32>,
) -> Result<Vec<SessionEvent>, String> {
    state
        .db
        .list_session_events(session_id.as_deref(), limit.unwrap_or(50))
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn get_activity_context(
    state: State<'_, AppState>,
    session_id: Option<String>,
) -> Result<ActivityContext, String> {
    state
        .db
        .get_activity_context(session_id.as_deref(), 25)
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn get_session(
    state: State<'_, AppState>,
    session_id: String,
) -> Result<Session, String> {
    let ctx = ServiceContext::from_app_state(&state, None);
    svc::get_session(&ctx, &session_id).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn delete_session(
    state: State<'_, AppState>,
    session_id: String,
    delete_files: bool,
) -> Result<(), String> {
    let ctx = ServiceContext::from_app_state(&state, None);
    svc::delete_session(&ctx, &session_id, delete_files).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn convert_session_to_collection(
    state: State<'_, AppState>,
    session_id: String,
) -> Result<(), String> {
    let ctx = ServiceContext::from_app_state(&state, None);
    svc::convert_session_to_collection(&ctx, &session_id).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn validate_session_folder(
    state: State<'_, AppState>,
    session_id: String,
) -> Result<bool, String> {
    let ctx = ServiceContext::from_app_state(&state, None);
    svc::validate_session_folder(&ctx, &session_id).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn create_canvas(
    state: State<'_, AppState>,
    session_id: String,
    name: String,
    canvas_type: String,
) -> Result<Canvas, String> {
    let ctx = ServiceContext::from_app_state(&state, None);
    svc::create_canvas(&ctx, &session_id, &name, &canvas_type).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn list_canvases(
    state: State<'_, AppState>,
    session_id: String,
) -> Result<Vec<Canvas>, String> {
    let ctx = ServiceContext::from_app_state(&state, None);
    svc::list_canvases(&ctx, &session_id).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn update_canvas_layout(
    state: State<'_, AppState>,
    canvas_id: String,
    layout_json: String,
) -> Result<(), String> {
    let ctx = ServiceContext::from_app_state(&state, None);
    svc::update_canvas_layout(&ctx, &canvas_id, &layout_json).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn delete_canvas(state: State<'_, AppState>, canvas_id: String) -> Result<(), String> {
    let ctx = ServiceContext::from_app_state(&state, None);
    svc::delete_canvas(&ctx, &canvas_id).map_err(|e| e.to_string())
}
