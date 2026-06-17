use serde::Serialize;
use std::sync::atomic::{AtomicU32, Ordering};
use tauri::webview::WebviewWindowBuilder;
use tauri::{image::Image, AppHandle, Emitter, Manager};

static WINDOW_COUNTER: AtomicU32 = AtomicU32::new(2);
const TRAY_ID: &str = "main";
pub(crate) const DEFAULT_ICON_VARIANT: &str = "dark";

#[derive(Serialize, Clone)]
pub struct WindowInfo {
    pub label: String,
    pub title: String,
}

struct IconVariant {
    id: &'static str,
    bytes: &'static [u8],
}

const ICON_VARIANTS: &[IconVariant] = &[
    IconVariant {
        id: "primary",
        bytes: include_bytes!("../../icons/variants/cull-primary.png"),
    },
    IconVariant {
        id: "red",
        bytes: include_bytes!("../../icons/variants/cull-red.png"),
    },
    IconVariant {
        id: "blue",
        bytes: include_bytes!("../../icons/variants/cull-blue.png"),
    },
    IconVariant {
        id: "dark",
        bytes: include_bytes!("../../icons/variants/cull-dark.png"),
    },
    IconVariant {
        id: "yellow",
        bytes: include_bytes!("../../icons/variants/cull-yellow.png"),
    },
];

fn icon_variant(id: &str) -> Option<&'static IconVariant> {
    ICON_VARIANTS.iter().find(|variant| variant.id == id)
}

fn icon_variant_or_default(id: &str) -> &'static IconVariant {
    icon_variant(id)
        .or_else(|| icon_variant(DEFAULT_ICON_VARIANT))
        .expect("default icon variant must exist")
}

fn tauri_icon_from_png_bytes(bytes: &[u8], id: &str) -> Result<Image<'static>, String> {
    let rgba = image::load_from_memory(bytes)
        .map_err(|e| format!("Failed to decode icon '{}': {}", id, e))?
        .into_rgba8();
    let width = rgba.width();
    let height = rgba.height();
    Ok(Image::new_owned(rgba.into_raw(), width, height))
}

pub fn current_app_icon_image(app: &AppHandle) -> Result<Image<'static>, String> {
    let variant_id = app
        .state::<crate::AppState>()
        .db
        .get_setting("app_icon_variant")
        .ok()
        .flatten()
        .unwrap_or_else(|| DEFAULT_ICON_VARIANT.to_string());
    let variant = icon_variant_or_default(&variant_id);
    tauri_icon_from_png_bytes(variant.bytes, variant.id)
}

pub fn apply_app_icon_variant_to_app(app: &AppHandle, variant_id: &str) -> Result<(), String> {
    let variant = icon_variant_or_default(variant_id);
    let icon = tauri_icon_from_png_bytes(variant.bytes, variant.id)?;

    for window in app.webview_windows().values() {
        window
            .set_icon(icon.clone())
            .map_err(|e| format!("Failed to set window icon: {}", e))?;
    }

    if let Some(tray) = app.tray_by_id(TRAY_ID) {
        let tray_icon = if app
            .state::<crate::AppState>()
            .clipboard_monitor
            .lock()
            .running
        {
            crate::tray::recording_tray_icon(&icon)
        } else {
            icon.clone()
        };
        tray.set_icon(Some(tray_icon))
            .map_err(|e| format!("Failed to set tray icon: {}", e))?;
    }

    set_native_application_icon(app, variant.bytes)?;
    Ok(())
}

#[tauri::command]
pub async fn apply_app_icon_variant(app: AppHandle, variant: String) -> Result<(), String> {
    if icon_variant(&variant).is_none() {
        return Err(format!("Unknown icon variant '{}'", variant));
    }
    apply_app_icon_variant_to_app(&app, &variant)
}

#[cfg(target_os = "macos")]
fn set_native_application_icon(app: &AppHandle, bytes: &'static [u8]) -> Result<(), String> {
    if objc2::MainThreadMarker::new().is_some() {
        return set_macos_application_icon_on_main(bytes);
    }

    let (tx, rx) = std::sync::mpsc::channel();
    app.run_on_main_thread(move || {
        let _ = tx.send(set_macos_application_icon_on_main(bytes));
    })
    .map_err(|e| format!("Failed to schedule app icon update: {}", e))?;
    rx.recv()
        .map_err(|_| "Failed to receive app icon update result".to_string())?
}

#[cfg(target_os = "macos")]
fn set_macos_application_icon_on_main(bytes: &'static [u8]) -> Result<(), String> {
    use objc2::{AllocAnyThread, MainThreadMarker};
    use objc2_app_kit::{NSApplication, NSImage};
    use objc2_foundation::NSData;

    let mtm = MainThreadMarker::new()
        .ok_or_else(|| "macOS app icon update must run on the main thread".to_string())?;
    let data = NSData::with_bytes(bytes);
    let image = NSImage::initWithData(NSImage::alloc(), &data)
        .ok_or_else(|| "Failed to decode macOS app icon image".to_string())?;
    let app = NSApplication::sharedApplication(mtm);
    unsafe { app.setApplicationIconImage(Some(&image)) };
    Ok(())
}

#[cfg(not(target_os = "macos"))]
fn set_native_application_icon(_app: &AppHandle, _bytes: &'static [u8]) -> Result<(), String> {
    Ok(())
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

    let icon_variant = app
        .state::<crate::AppState>()
        .db
        .get_setting("app_icon_variant")
        .ok()
        .flatten()
        .unwrap_or_else(|| DEFAULT_ICON_VARIANT.to_string());
    let _ = apply_app_icon_variant_to_app(&app, &icon_variant);
    let _ = crate::menu::refresh_window_menu(&app);

    // Tell the new window its name via an event after a short delay
    // (the JS runtime needs time to initialize)
    let title_clone = title.clone();
    let label_clone = label.clone();
    tokio::spawn(async move {
        tokio::time::sleep(std::time::Duration::from_millis(500)).await;
        let _ = window.emit(
            "set-window-name",
            serde_json::json!({
                "label": label_clone,
                "name": title_clone,
            }),
        );
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
    let _ = window.emit(
        "set-window-name",
        serde_json::json!({
            "label": label,
            "name": new_name,
        }),
    );
    let _ = crate::menu::refresh_window_menu(&app);
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
    let target = windows
        .values()
        .find(|w| w.title().map(|t| t == window_name).unwrap_or(false))
        .or_else(|| windows.get(&window_name));

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
