use crate::services::agent_snapshots::{
    snapshot_response_value, write_snapshot_package, AgentSnapshotPackage,
};
use crate::AppState;
use base64::Engine;
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter, Manager, State};

const SNAPSHOT_RETENTION_LIMIT: usize = 25;
const SNAPSHOT_REQUEST_TIMEOUT_SECS: u64 = 15;

#[derive(Debug, Deserialize)]
pub struct CompleteAgentViewSnapshotRequest {
    pub request_id: Option<String>,
    pub snapshot_id: String,
    pub manifest: serde_json::Value,
    pub raw_png_base64: String,
    pub annotated_png_base64: String,
    pub clipboard: bool,
}

#[derive(Debug, Serialize, Clone)]
struct AgentSnapshotRequestPayload {
    request_id: String,
    snapshot_id: String,
    clipboard: bool,
    capture_reason: String,
}

#[tauri::command]
pub async fn capture_agent_window_snapshot(app: AppHandle) -> Result<String, String> {
    let png = capture_main_window_png(&app)?;
    Ok(base64::engine::general_purpose::STANDARD.encode(png))
}

#[tauri::command]
pub async fn complete_agent_view_snapshot(
    state: State<'_, AppState>,
    request: CompleteAgentViewSnapshotRequest,
) -> Result<serde_json::Value, String> {
    let raw_png = base64::engine::general_purpose::STANDARD
        .decode(strip_data_url_prefix(&request.raw_png_base64))
        .map_err(|e| format!("Failed to decode raw snapshot PNG: {}", e))?;
    let annotated_png = base64::engine::general_purpose::STANDARD
        .decode(strip_data_url_prefix(&request.annotated_png_base64))
        .map_err(|e| format!("Failed to decode annotated snapshot PNG: {}", e))?;

    let package = {
        let mut registry = state.agent_snapshots.lock();
        write_snapshot_package(
            &state.app_data_dir,
            &mut registry,
            &request.snapshot_id,
            request.manifest,
            &raw_png,
            &annotated_png,
            SNAPSHOT_RETENTION_LIMIT,
        )?
    };

    if request.clipboard {
        if let Err(e) = copy_snapshot_to_clipboard(&package) {
            crate::safe_eprintln!(
                "[agent-snapshot] Failed to copy snapshot to clipboard: {}",
                e
            );
        }
    }

    if let Some(request_id) = request.request_id {
        if let Some(sender) = state.agent_snapshot_requests.lock().remove(&request_id) {
            let _ = sender.send(package.clone());
        }
    }

    Ok(snapshot_response_value(&package, false))
}

#[tauri::command]
pub async fn get_last_agent_view_snapshot(
    state: State<'_, AppState>,
    snapshot_id: Option<String>,
) -> Result<Option<serde_json::Value>, String> {
    let registry = state.agent_snapshots.lock();
    let package = match snapshot_id {
        Some(snapshot_id) => registry.get_snapshot(&snapshot_id),
        None => registry.latest_snapshot(),
    };
    Ok(package.map(|package| snapshot_response_value(package, false)))
}

#[tauri::command]
pub async fn request_agent_view_snapshot(
    app: AppHandle,
    state: State<'_, AppState>,
    clipboard: bool,
) -> Result<serde_json::Value, String> {
    let request_id = format!("req_{}", uuid::Uuid::new_v4().simple());
    let snapshot_id = format!("snap_{}", uuid::Uuid::new_v4().simple());
    let payload = AgentSnapshotRequestPayload {
        request_id: request_id.clone(),
        snapshot_id,
        clipboard,
        capture_reason: "mcp".to_string(),
    };
    let (sender, receiver) = tokio::sync::oneshot::channel::<AgentSnapshotPackage>();
    state
        .agent_snapshot_requests
        .lock()
        .insert(request_id.clone(), sender);

    if let Err(e) = app.emit("agent-view-snapshot:request", payload) {
        state.agent_snapshot_requests.lock().remove(&request_id);
        return Err(format!("Failed to request frontend snapshot: {}", e));
    }

    match tokio::time::timeout(
        std::time::Duration::from_secs(SNAPSHOT_REQUEST_TIMEOUT_SECS),
        receiver,
    )
    .await
    {
        Ok(Ok(package)) => Ok(snapshot_response_value(&package, false)),
        Ok(Err(_)) => Err("Agent snapshot request was cancelled".to_string()),
        Err(_) => {
            state.agent_snapshot_requests.lock().remove(&request_id);
            Err("Timed out waiting for the visible app to capture an agent snapshot".to_string())
        }
    }
}

fn strip_data_url_prefix(value: &str) -> &str {
    value
        .split_once(',')
        .filter(|(prefix, _)| prefix.starts_with("data:image/png;base64"))
        .map(|(_, data)| data)
        .unwrap_or(value)
}

#[cfg(target_os = "macos")]
fn capture_main_window_png(app: &AppHandle) -> Result<Vec<u8>, String> {
    let window = app
        .get_webview_window("main")
        .ok_or_else(|| "Main window is not available".to_string())?;
    let position = window
        .outer_position()
        .map_err(|e| format!("Failed to read window position: {}", e))?;
    let size = window
        .outer_size()
        .map_err(|e| format!("Failed to read window size: {}", e))?;
    if size.width == 0 || size.height == 0 {
        return Err("Main window has no captureable size".to_string());
    }

    let capture_dir = app
        .path()
        .app_data_dir()
        .map_err(|e| format!("Failed to resolve app data directory: {}", e))?
        .join(crate::services::agent_snapshots::AGENT_SNAPSHOTS_DIR);
    std::fs::create_dir_all(&capture_dir)
        .map_err(|e| format!("Failed to create snapshot temp directory: {}", e))?;
    let output = capture_dir.join(".window-capture-current.png");
    let rect = format!(
        "{},{},{},{}",
        position.x, position.y, size.width, size.height
    );
    let status = std::process::Command::new("/usr/sbin/screencapture")
        .args(["-x", "-t", "png", "-R", &rect])
        .arg(&output)
        .status()
        .map_err(|e| format!("Failed to run screencapture: {}", e))?;
    if !status.success() {
        return Err(format!("screencapture failed with status {}", status));
    }
    std::fs::read(&output).map_err(|e| format!("Failed to read captured window PNG: {}", e))
}

#[cfg(not(target_os = "macos"))]
fn capture_main_window_png(_app: &AppHandle) -> Result<Vec<u8>, String> {
    Err("Agent view snapshots are currently available on macOS only".to_string())
}

#[cfg(target_os = "macos")]
fn copy_snapshot_to_clipboard(package: &AgentSnapshotPackage) -> Result<(), String> {
    use objc2_app_kit::{NSPasteboard, NSPasteboardTypePNG, NSPasteboardTypeString};
    use objc2_foundation::{NSData, NSString};

    let png_bytes = std::fs::read(&package.annotated_png_path)
        .map_err(|e| format!("Failed to read annotated snapshot PNG: {}", e))?;
    let fallback_text = format!(
        "Cull agent snapshot {}\n{}",
        package.snapshot_id,
        package.manifest_json_path.to_string_lossy()
    );

    let pasteboard = NSPasteboard::generalPasteboard();
    pasteboard.clearContents();
    let data = NSData::with_bytes(&png_bytes);
    let mut wrote = pasteboard.setData_forType(Some(&data), unsafe { NSPasteboardTypePNG });
    let text = NSString::from_str(&fallback_text);
    wrote |= pasteboard.setString_forType(&text, unsafe { NSPasteboardTypeString });

    if wrote {
        Ok(())
    } else {
        Err("Failed to write snapshot to clipboard".to_string())
    }
}

#[cfg(not(target_os = "macos"))]
fn copy_snapshot_to_clipboard(_package: &AgentSnapshotPackage) -> Result<(), String> {
    Err("Agent snapshot clipboard copy is currently available on macOS only".to_string())
}
