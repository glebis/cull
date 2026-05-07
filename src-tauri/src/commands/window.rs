use std::sync::atomic::{AtomicU32, Ordering};
use tauri::{AppHandle, Emitter, Manager};
use tauri::webview::WebviewWindowBuilder;
use serde::Serialize;

static WINDOW_COUNTER: AtomicU32 = AtomicU32::new(2);

#[derive(Serialize, Clone)]
pub struct WindowInfo {
    pub label: String,
    pub title: String,
}

/// Create a new window with an optional name.
/// Returns the window label.
#[tauri::command]
pub async fn create_window(app: AppHandle, name: Option<String>) -> Result<String, String> {
    let n = WINDOW_COUNTER.fetch_add(1, Ordering::SeqCst);
    let title = name.unwrap_or_else(|| format!("Window {}", n));
    // Label must be unique and URL-safe
    let label = format!("window-{}", n);

    let url = tauri::WebviewUrl::App("index.html".into());

    let window = WebviewWindowBuilder::new(&app, &label, url)
        .title(&title)
        .inner_size(1200.0, 800.0)
        .title_bar_style(tauri::TitleBarStyle::Overlay)
        .hidden_title(true)
        .build()
        .map_err(|e| format!("Failed to create window: {}", e))?;

    // Tell the new window its name via an event after a short delay
    // (the JS runtime needs time to initialize)
    let title_clone = title.clone();
    let label_clone = label.clone();
    tokio::spawn(async move {
        tokio::time::sleep(std::time::Duration::from_millis(500)).await;
        let _ = window.emit("set-window-name", serde_json::json!({
            "label": label_clone,
            "name": title_clone,
        }));
    });

    Ok(label)
}

/// List all open windows.
#[tauri::command]
pub async fn list_windows(app: AppHandle) -> Result<Vec<WindowInfo>, String> {
    let windows: Vec<WindowInfo> = app
        .webview_windows()
        .iter()
        .map(|(label, w)| WindowInfo {
            label: label.clone(),
            title: w.title().unwrap_or_else(|_| label.clone()),
        })
        .collect();
    Ok(windows)
}

/// Rename a window (change its title).
#[tauri::command]
pub async fn rename_window(app: AppHandle, label: String, new_name: String) -> Result<(), String> {
    let window = app
        .get_webview_window(&label)
        .ok_or_else(|| format!("Window '{}' not found", label))?;
    window
        .set_title(&new_name)
        .map_err(|e| format!("Failed to set title: {}", e))?;
    // Notify the window's frontend of the name change
    let _ = window.emit("set-window-name", serde_json::json!({
        "label": label,
        "name": new_name,
    }));
    Ok(())
}

/// Send a command to a specific window by name or label.
/// If the window doesn't exist, create it first.
#[tauri::command]
pub async fn send_to_window(
    app: AppHandle,
    window_name: String,
    event: String,
    payload: serde_json::Value,
) -> Result<String, String> {
    // Try to find window by title first, then by label
    let windows = app.webview_windows();
    let target = windows.values().find(|w| {
        w.title().map(|t| t == window_name).unwrap_or(false)
    }).or_else(|| {
        windows.get(&window_name)
    });

    let (window, label) = if let Some(w) = target {
        let l = w.label().to_string();
        (w.clone(), l)
    } else {
        // Window doesn't exist, create it
        let label = create_window(app.clone(), Some(window_name.clone())).await?;
        // Wait for window to initialize
        tokio::time::sleep(std::time::Duration::from_millis(600)).await;
        let w = app
            .get_webview_window(&label)
            .ok_or_else(|| "Failed to get newly created window".to_string())?;
        (w, label)
    };

    // Focus the window
    let _ = window.set_focus();

    // Send the event to the specific window
    window
        .emit(&event, payload)
        .map_err(|e| format!("Failed to emit event: {}", e))?;

    Ok(label)
}
