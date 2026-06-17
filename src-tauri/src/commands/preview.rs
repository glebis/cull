use crate::preview::histogram::{load_image_histogram, ImageHistogram};
use crate::preview::state::{PreviewDisplayMode, PreviewOverlayConfig, PreviewState};
use crate::preview::web_stream::{PreviewWebStreamStatus, PREVIEW_WEB_STREAM_CHANGED_EVENT};
use crate::preview::window::{
    preview_display_window_spec, preview_monitor_key, PREVIEW_DISPLAY_LABEL,
};
use crate::AppState;
use serde::Serialize;
use tauri::webview::WebviewWindowBuilder;
use tauri::{
    AppHandle, Emitter, Manager, PhysicalPosition, PhysicalSize, Position, Size, State,
    WebviewWindow,
};

const PREVIEW_STATE_CHANGED_EVENT: &str = "preview:state-changed";
const PREVIEW_DISPLAY_MONITOR_SETTING: &str = "preview_display_monitor_id";
const PREVIEW_DISPLAY_FULLSCREEN_SETTING: &str = "preview_display_fullscreen";
const PREVIEW_DISPLAY_ALWAYS_ON_TOP_SETTING: &str = "preview_display_always_on_top";

#[derive(Debug, Clone, Serialize)]
pub struct PreviewDisplayMonitor {
    pub id: String,
    pub name: Option<String>,
    pub x: i32,
    pub y: i32,
    pub width: u32,
    pub height: u32,
    pub scale_factor: f64,
    pub primary: bool,
}

#[tauri::command]
pub async fn get_preview_state(state: State<'_, AppState>) -> Result<PreviewState, String> {
    Ok(state.preview_state.get())
}

#[tauri::command]
pub async fn update_preview_state(
    app: AppHandle,
    state: State<'_, AppState>,
    image_id: Option<String>,
    display_mode: PreviewDisplayMode,
    overlay: PreviewOverlayConfig,
    frozen: Option<bool>,
    blanked: Option<bool>,
) -> Result<PreviewState, String> {
    let preview_state =
        state
            .preview_state
            .update(image_id, display_mode, overlay, frozen, blanked);
    app.emit(PREVIEW_STATE_CHANGED_EVENT, preview_state.clone())
        .map_err(|e| format!("Failed to emit preview state update: {}", e))?;
    Ok(preview_state)
}

#[tauri::command]
pub async fn open_preview_display(app: AppHandle) -> Result<String, String> {
    let window = ensure_preview_display_window(&app)?;
    let _ = window.show();
    let _ = window.set_focus();
    apply_saved_preview_display_placement(&app)?;
    apply_saved_preview_display_always_on_top(&app)?;
    let _ = crate::menu::refresh_window_menu(&app);
    Ok(PREVIEW_DISPLAY_LABEL.to_string())
}

fn ensure_preview_display_window(app: &AppHandle) -> Result<WebviewWindow, String> {
    if let Some(window) = app.get_webview_window(PREVIEW_DISPLAY_LABEL) {
        apply_preview_display_always_on_top(
            app,
            &window,
            saved_preview_display_always_on_top(app)?,
        )?;
        return Ok(window);
    }

    let spec = preview_display_window_spec();
    let always_on_top = saved_preview_display_always_on_top(app)?;
    let url = tauri::WebviewUrl::App(spec.url.into());
    let window = WebviewWindowBuilder::new(app, spec.label, url)
        .title(spec.title)
        .inner_size(spec.width, spec.height)
        .min_inner_size(spec.min_width, spec.min_height)
        .title_bar_style(tauri::TitleBarStyle::Overlay)
        .hidden_title(true)
        .always_on_top(always_on_top)
        .build()
        .map_err(|e| format!("Failed to create Preview Display window: {}", e))?;

    let icon_variant = app
        .state::<AppState>()
        .db
        .get_setting("app_icon_variant")
        .ok()
        .flatten()
        .unwrap_or_else(|| crate::commands::window::DEFAULT_ICON_VARIANT.to_string());
    let _ = crate::commands::window::apply_app_icon_variant_to_app(app, &icon_variant);
    let _ = window.emit(
        "set-window-name",
        serde_json::json!({
            "label": spec.label,
            "name": spec.title,
        }),
    );

    Ok(window)
}

#[tauri::command]
pub async fn set_preview_display_always_on_top(
    app: AppHandle,
    always_on_top: bool,
) -> Result<bool, String> {
    app.state::<AppState>()
        .db
        .set_setting(
            PREVIEW_DISPLAY_ALWAYS_ON_TOP_SETTING,
            if always_on_top { "true" } else { "false" },
        )
        .map_err(|e| e.to_string())?;

    if let Some(window) = app.get_webview_window(PREVIEW_DISPLAY_LABEL) {
        apply_preview_display_always_on_top(&app, &window, always_on_top)?;
    }
    let _ = crate::menu::refresh_window_menu(&app);
    Ok(always_on_top)
}

#[tauri::command]
pub async fn list_preview_display_monitors(
    app: AppHandle,
) -> Result<Vec<PreviewDisplayMonitor>, String> {
    preview_display_monitors(&app)
}

#[tauri::command]
pub async fn place_preview_display(
    app: AppHandle,
    monitor_id: Option<String>,
    fullscreen: bool,
) -> Result<String, String> {
    let window = ensure_preview_display_window(&app)?;
    let state = app.state::<AppState>();
    let effective_monitor_id = match monitor_id {
        Some(id) => Some(id),
        None => state
            .db
            .get_setting(PREVIEW_DISPLAY_MONITOR_SETTING)
            .map_err(|e| e.to_string())?,
    };
    drop(state);

    place_preview_display_window(&app, &window, effective_monitor_id.as_deref(), fullscreen)?;
    let state = app.state::<AppState>();
    if let Some(id) = effective_monitor_id {
        state
            .db
            .set_setting(PREVIEW_DISPLAY_MONITOR_SETTING, &id)
            .map_err(|e| e.to_string())?;
    }
    state
        .db
        .set_setting(
            PREVIEW_DISPLAY_FULLSCREEN_SETTING,
            if fullscreen { "true" } else { "false" },
        )
        .map_err(|e| e.to_string())?;
    Ok(PREVIEW_DISPLAY_LABEL.to_string())
}

fn apply_saved_preview_display_placement(app: &AppHandle) -> Result<(), String> {
    let state = app.state::<AppState>();
    let monitor_id = state
        .db
        .get_setting(PREVIEW_DISPLAY_MONITOR_SETTING)
        .map_err(|e| e.to_string())?;
    let fullscreen = state
        .db
        .get_setting(PREVIEW_DISPLAY_FULLSCREEN_SETTING)
        .map_err(|e| e.to_string())?
        .map(|value| value == "true")
        .unwrap_or(false);
    drop(state);

    if monitor_id.is_none() && !fullscreen {
        return Ok(());
    }

    if let Some(window) = app.get_webview_window(PREVIEW_DISPLAY_LABEL) {
        place_preview_display_window(app, &window, monitor_id.as_deref(), fullscreen)?;
    }
    Ok(())
}

fn saved_preview_display_always_on_top(app: &AppHandle) -> Result<bool, String> {
    Ok(app
        .state::<AppState>()
        .db
        .get_setting(PREVIEW_DISPLAY_ALWAYS_ON_TOP_SETTING)
        .map_err(|e| e.to_string())?
        .map(|value| value == "true")
        .unwrap_or(false))
}

fn apply_saved_preview_display_always_on_top(app: &AppHandle) -> Result<(), String> {
    let always_on_top = saved_preview_display_always_on_top(app)?;
    if let Some(window) = app.get_webview_window(PREVIEW_DISPLAY_LABEL) {
        apply_preview_display_always_on_top(app, &window, always_on_top)?;
    }
    Ok(())
}

fn apply_preview_display_always_on_top(
    _app: &AppHandle,
    window: &WebviewWindow,
    always_on_top: bool,
) -> Result<(), String> {
    window
        .set_always_on_top(always_on_top)
        .map_err(|e| format!("Failed to update Preview Display Always on Top: {}", e))
}

fn place_preview_display_window(
    app: &AppHandle,
    window: &WebviewWindow,
    monitor_id: Option<&str>,
    fullscreen: bool,
) -> Result<(), String> {
    let monitors = app
        .available_monitors()
        .map_err(|e| format!("Failed to list displays: {}", e))?;
    let target = monitor_id
        .and_then(|id| {
            monitors
                .iter()
                .enumerate()
                .find(|(index, monitor)| monitor_key(*index, monitor) == id)
                .map(|(_, monitor)| monitor)
        })
        .or_else(|| monitors.first())
        .ok_or_else(|| "No displays available".to_string())?;

    let position = target.position();
    let size = target.size();
    let _ = window.set_fullscreen(false);
    window
        .set_position(Position::Physical(PhysicalPosition {
            x: position.x,
            y: position.y,
        }))
        .map_err(|e| format!("Failed to move Preview Display: {}", e))?;
    window
        .set_size(Size::Physical(PhysicalSize {
            width: size.width,
            height: size.height,
        }))
        .map_err(|e| format!("Failed to size Preview Display: {}", e))?;
    if fullscreen {
        window
            .set_fullscreen(true)
            .map_err(|e| format!("Failed to fullscreen Preview Display: {}", e))?;
    }
    Ok(())
}

fn preview_display_monitors(app: &AppHandle) -> Result<Vec<PreviewDisplayMonitor>, String> {
    let monitors = app
        .available_monitors()
        .map_err(|e| format!("Failed to list displays: {}", e))?;
    let primary_signature = app
        .primary_monitor()
        .ok()
        .flatten()
        .map(|monitor| monitor_signature(&monitor));

    Ok(monitors
        .iter()
        .enumerate()
        .map(|(index, monitor)| {
            let id = monitor_key(index, monitor);
            let position = monitor.position();
            let size = monitor.size();
            PreviewDisplayMonitor {
                primary: primary_signature
                    .as_ref()
                    .is_some_and(|signature| *signature == monitor_signature(monitor)),
                id,
                name: monitor.name().cloned(),
                x: position.x,
                y: position.y,
                width: size.width,
                height: size.height,
                scale_factor: monitor.scale_factor(),
            }
        })
        .collect())
}

fn monitor_key(index: usize, monitor: &tauri::Monitor) -> String {
    let position = monitor.position();
    let size = monitor.size();
    preview_monitor_key(
        index,
        monitor.name().map(String::as_str),
        position.x,
        position.y,
        size.width,
        size.height,
    )
}

fn monitor_signature(monitor: &tauri::Monitor) -> (Option<String>, i32, i32, u32, u32) {
    let position = monitor.position();
    let size = monitor.size();
    (
        monitor.name().cloned(),
        position.x,
        position.y,
        size.width,
        size.height,
    )
}

#[tauri::command]
pub async fn start_preview_display_web_stream(
    app: AppHandle,
    state: State<'_, AppState>,
    host: Option<String>,
    port: Option<u16>,
) -> Result<PreviewWebStreamStatus, String> {
    let status = state
        .preview_web_stream
        .start(app.clone(), host, port)
        .await?;
    emit_preview_web_stream_status(&app, &status)?;
    Ok(status)
}

#[tauri::command]
pub async fn stop_preview_display_web_stream(
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<PreviewWebStreamStatus, String> {
    let status = state.preview_web_stream.stop();
    emit_preview_web_stream_status(&app, &status)?;
    Ok(status)
}

#[tauri::command]
pub async fn get_preview_display_web_stream_status(
    state: State<'_, AppState>,
) -> Result<PreviewWebStreamStatus, String> {
    Ok(state.preview_web_stream.status())
}

#[tauri::command]
pub async fn get_image_histogram(
    state: State<'_, AppState>,
    image_id: String,
) -> Result<Option<ImageHistogram>, String> {
    let images = state
        .db
        .get_images_by_ids(&[image_id.as_str()])
        .map_err(|e| e.to_string())?;
    let Some(image) = images.first() else {
        return Ok(None);
    };
    load_image_histogram(image, &state.app_data_dir).map(Some)
}

fn emit_preview_web_stream_status(
    app: &AppHandle,
    status: &PreviewWebStreamStatus,
) -> Result<(), String> {
    app.emit(PREVIEW_WEB_STREAM_CHANGED_EVENT, status.clone())
        .map_err(|e| format!("Failed to emit Preview Display web stream status: {}", e))
}
