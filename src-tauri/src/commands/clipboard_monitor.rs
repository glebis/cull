use crate::services::clipboard_monitor::{
    create_monitor_session, default_capture_dir, resolve_capture_dir, ClipboardMonitorState,
};
use crate::AppState;
use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager, State};

#[derive(Debug, Clone, Serialize)]
pub struct ClipboardMonitorStatus {
    pub running: bool,
    pub supported: bool,
    pub access_status: String,
    pub collection_id: Option<String>,
    pub collection_name: Option<String>,
    pub capture_dir: String,
    pub captured_count: u32,
    pub last_error: Option<String>,
}

fn status_from_state(state: &AppState, monitor: &ClipboardMonitorState) -> ClipboardMonitorStatus {
    let capture_dir = monitor
        .capture_dir
        .clone()
        .or_else(|| resolve_capture_dir(&state.db, &state.app_data_dir, None).ok())
        .unwrap_or_else(|| default_capture_dir(&state.app_data_dir));

    ClipboardMonitorStatus {
        running: monitor.running,
        supported: cfg!(target_os = "macos"),
        access_status: if cfg!(target_os = "macos") {
            "supported"
        } else {
            "unsupported_platform"
        }
        .to_string(),
        collection_id: monitor.collection_id.clone(),
        collection_name: monitor.collection_name.clone(),
        capture_dir: capture_dir.to_string_lossy().to_string(),
        captured_count: monitor.captured_count,
        last_error: monitor.last_error.clone(),
    }
}

#[tauri::command]
pub async fn get_clipboard_monitor_status(
    state: State<'_, AppState>,
) -> Result<ClipboardMonitorStatus, String> {
    let monitor = state.clipboard_monitor.lock();
    Ok(status_from_state(state.inner(), &monitor))
}

#[tauri::command]
pub async fn start_clipboard_monitor(
    app: AppHandle,
    state: State<'_, AppState>,
    capture_dir: Option<String>,
) -> Result<ClipboardMonitorStatus, String> {
    if !cfg!(target_os = "macos") {
        let mut monitor = state.clipboard_monitor.lock();
        monitor.last_error =
            Some("Clipboard Monitor is not supported on this platform yet".to_string());
        return Ok(status_from_state(state.inner(), &monitor));
    }

    {
        let monitor = state.clipboard_monitor.lock();
        if monitor.running {
            return Ok(status_from_state(state.inner(), &monitor));
        }
    }

    let session = create_monitor_session(&state.db, &state.app_data_dir, capture_dir.as_deref())?;
    let capture_path = std::path::PathBuf::from(&session.capture_dir);
    let _ = app.asset_protocol_scope().allow_directory(&capture_path, true);

    {
        let mut monitor = state.clipboard_monitor.lock();
        monitor.running = true;
        monitor.collection_id = Some(session.collection_id.clone());
        monitor.collection_name = Some(session.collection_name.clone());
        monitor.capture_dir = Some(capture_path);
        monitor.captured_count = 0;
        monitor.last_error = None;
    }

    let _ = app.emit(
        "navigate-collection",
        serde_json::json!({ "collection_id": session.collection_id }),
    );
    let monitor = state.clipboard_monitor.lock();
    Ok(status_from_state(state.inner(), &monitor))
}

#[tauri::command]
pub async fn stop_clipboard_monitor(
    state: State<'_, AppState>,
) -> Result<ClipboardMonitorStatus, String> {
    let mut monitor = state.clipboard_monitor.lock();
    monitor.running = false;
    Ok(status_from_state(state.inner(), &monitor))
}

#[tauri::command]
pub async fn set_clipboard_monitor_capture_dir(
    state: State<'_, AppState>,
    path: String,
) -> Result<ClipboardMonitorStatus, String> {
    let capture_dir = resolve_capture_dir(&state.db, &state.app_data_dir, Some(&path))?;
    state
        .db
        .set_setting(
            crate::services::clipboard_monitor::CAPTURE_DIR_SETTING,
            &capture_dir.to_string_lossy(),
        )
        .map_err(|e| e.to_string())?;
    let mut monitor = state.clipboard_monitor.lock();
    monitor.capture_dir = Some(capture_dir);
    Ok(status_from_state(state.inner(), &monitor))
}

#[tauri::command]
pub async fn move_clipboard_capture_folder(
    state: State<'_, AppState>,
    new_path: String,
) -> Result<ClipboardMonitorStatus, String> {
    set_clipboard_monitor_capture_dir(state, new_path).await
}

#[tauri::command]
pub async fn publish_clipboard_collection(
    _app: AppHandle,
    _state: State<'_, AppState>,
    _collection_id: Option<String>,
) -> Result<serde_json::Value, String> {
    Err("Clipboard collection publishing is wired in the publishing task".to_string())
}
