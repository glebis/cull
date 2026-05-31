#[cfg(target_os = "macos")]
use crate::services::clipboard_monitor::ClipboardImageReader;
use crate::services::clipboard_monitor::{
    capture_existing_on_start_enabled, create_monitor_session, default_capture_dir,
    resolve_capture_dir, set_capture_existing_on_start, ClipboardMonitorSession,
    ClipboardMonitorState,
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
    pub capture_existing_on_start: bool,
    pub last_error: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ClipboardPublishResult {
    pub collection_id: String,
    pub image_count: usize,
    pub site_dir: String,
    pub url: String,
    pub manifest_path: String,
    pub instructions_path: String,
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
        access_status: platform_access_status(),
        collection_id: monitor.collection_id.clone(),
        collection_name: monitor.collection_name.clone(),
        capture_dir: capture_dir.to_string_lossy().to_string(),
        captured_count: monitor.captured_count,
        capture_existing_on_start: if monitor.running {
            monitor.capture_existing_on_start
        } else {
            capture_existing_on_start_enabled(&state.db).unwrap_or(false)
        },
        last_error: monitor.last_error.clone(),
    }
}

fn platform_access_status() -> String {
    #[cfg(target_os = "macos")]
    {
        crate::services::clipboard_monitor_macos::MacPasteboardReader::new()
            .status()
            .label()
    }
    #[cfg(not(target_os = "macos"))]
    {
        crate::services::clipboard_monitor::ClipboardAccessStatus::UnsupportedPlatform.label()
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
    let status = start_clipboard_monitor_inner(&app, state.inner(), capture_dir)?;
    let _ = crate::tray::set_clipboard_monitor_checked(&app, status.running);
    Ok(status)
}

pub fn start_clipboard_monitor_inner(
    app: &AppHandle,
    state: &AppState,
    capture_dir: Option<String>,
) -> Result<ClipboardMonitorStatus, String> {
    if !cfg!(target_os = "macos") {
        let mut monitor = state.clipboard_monitor.lock();
        monitor.last_error =
            Some("Clipboard Monitor is not supported on this platform yet".to_string());
        return Ok(status_from_state(state, &monitor));
    }

    {
        let monitor = state.clipboard_monitor.lock();
        if monitor.running {
            return Ok(status_from_state(state, &monitor));
        }
    }

    let session = create_monitor_session(&state.db, &state.app_data_dir, capture_dir.as_deref())?;
    let capture_path = std::path::PathBuf::from(&session.capture_dir);
    let capture_existing_on_start = capture_existing_on_start_enabled(&state.db)?;

    {
        let mut monitor = state.clipboard_monitor.lock();
        monitor.running = true;
        monitor.collection_id = Some(session.collection_id.clone());
        monitor.collection_name = Some(session.collection_name.clone());
        monitor.capture_dir = Some(capture_path);
        monitor.captured_count = 0;
        monitor.capture_existing_on_start = capture_existing_on_start;
        monitor.baseline_complete = capture_existing_on_start;
        monitor.last_change_count = None;
        monitor.last_hash = None;
        monitor.last_error = None;
    }

    #[cfg(target_os = "macos")]
    {
        let app_clone = app.clone();
        let db = state.db.clone();
        let app_data_dir = state.app_data_dir.clone();
        let session = ClipboardMonitorSession {
            collection_id: session.collection_id.clone(),
            collection_name: session.collection_name.clone(),
            capture_dir: session.capture_dir.clone(),
        };
        crate::spawn_guarded(app.clone(), "clipboard-monitor", move || async move {
            let mut reader = crate::services::clipboard_monitor_macos::MacPasteboardReader::new();
            let mut interval = tokio::time::interval(std::time::Duration::from_millis(
                crate::services::clipboard_monitor::DEFAULT_POLL_MS,
            ));
            loop {
                interval.tick().await;
                let app_state = app_clone.state::<AppState>();
                let mut monitor = app_state.clipboard_monitor.lock();
                if !monitor.running {
                    break;
                }
                match crate::services::clipboard_monitor::process_reader_once(
                    &db,
                    &app_data_dir,
                    &session,
                    &mut monitor,
                    &mut reader,
                ) {
                    Ok(Some(result)) => {
                        let _ = app_clone.emit("clipboard-monitor:capture", &result);
                        let _ = app_clone.emit("images:changed", ());
                    }
                    Ok(None) => {}
                    Err(error) => {
                        monitor.last_error = Some(error.clone());
                        let _ = app_clone.emit(
                            "clipboard-monitor:error",
                            serde_json::json!({ "message": error }),
                        );
                    }
                }
            }
        });
    }

    let _ = app.emit(
        "navigate-collection",
        serde_json::json!({ "collection_id": session.collection_id }),
    );
    let monitor = state.clipboard_monitor.lock();
    Ok(status_from_state(state, &monitor))
}

#[tauri::command]
pub async fn stop_clipboard_monitor(
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<ClipboardMonitorStatus, String> {
    let status = stop_clipboard_monitor_inner(state.inner());
    let _ = crate::tray::set_clipboard_monitor_checked(&app, status.running);
    Ok(status)
}

pub fn stop_clipboard_monitor_inner(state: &AppState) -> ClipboardMonitorStatus {
    let mut monitor = state.clipboard_monitor.lock();
    monitor.running = false;
    status_from_state(state, &monitor)
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
pub async fn set_clipboard_monitor_capture_existing_on_start(
    state: State<'_, AppState>,
    enabled: bool,
) -> Result<ClipboardMonitorStatus, String> {
    set_capture_existing_on_start(&state.db, enabled)?;
    let mut monitor = state.clipboard_monitor.lock();
    monitor.capture_existing_on_start = enabled;
    if enabled {
        monitor.baseline_complete = true;
    }
    Ok(status_from_state(state.inner(), &monitor))
}

#[tauri::command]
pub async fn move_clipboard_capture_folder(
    app: AppHandle,
    state: State<'_, AppState>,
    new_path: String,
) -> Result<ClipboardMonitorStatus, String> {
    let new_dir = resolve_capture_dir(&state.db, &state.app_data_dir, Some(&new_path))?;
    let old_dir = {
        let monitor = state.clipboard_monitor.lock();
        monitor
            .capture_dir
            .as_ref()
            .map(|path| path.to_string_lossy().to_string())
            .or_else(|| {
                state
                    .db
                    .get_setting(crate::services::clipboard_monitor::CAPTURE_DIR_SETTING)
                    .ok()
                    .flatten()
            })
            .unwrap_or_else(|| {
                default_capture_dir(&state.app_data_dir)
                    .to_string_lossy()
                    .to_string()
            })
    };
    crate::services::clipboard_monitor::move_capture_folder(&state.db, &old_dir, &new_dir)?;
    let _ = app.emit("images:changed", ());
    let mut monitor = state.clipboard_monitor.lock();
    monitor.capture_dir = Some(new_dir);
    Ok(status_from_state(state.inner(), &monitor))
}

#[tauri::command]
pub async fn publish_clipboard_collection(
    app: AppHandle,
    state: State<'_, AppState>,
    collection_id: Option<String>,
) -> Result<ClipboardPublishResult, String> {
    let collection_id = collection_id
        .or_else(|| state.clipboard_monitor.lock().collection_id.clone())
        .or_else(|| {
            state
                .db
                .get_setting(crate::services::clipboard_monitor::LAST_COLLECTION_SETTING)
                .ok()
                .flatten()
        })
        .ok_or_else(|| "No clipboard monitor collection is available".to_string())?;
    let export = crate::commands::static_publishing::export_static_publish_collection_inner(
        state.inner(),
        collection_id.clone(),
        None,
        None,
    )?;
    let server = crate::commands::static_publishing::serve_static_publish_package_inner(
        state.inner(),
        export.site_dir.clone(),
        Some("127.0.0.1".to_string()),
        None,
    )
    .await?;
    let result = ClipboardPublishResult {
        collection_id,
        image_count: export.image_count,
        site_dir: export.site_dir,
        url: server.url,
        manifest_path: export.manifest_path,
        instructions_path: export.instructions_path,
    };
    let _ = app.emit("clipboard-monitor:published", &result);
    state
        .db
        .set_setting(
            "clipboard_monitor_last_publish",
            &serde_json::to_string(&result).unwrap_or_default(),
        )
        .map_err(|e| e.to_string())?;
    Ok(result)
}
