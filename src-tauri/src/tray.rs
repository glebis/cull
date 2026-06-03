use tauri::{
    image::Image,
    menu::{CheckMenuItem, Menu, MenuItem, PredefinedMenuItem},
    tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent},
    AppHandle, Emitter, Manager, Wry,
};

const TRAY_ID: &str = "main";
const SHOW_HIDE_ID: &str = "show_hide";
const TRAY_STATS_ID: &str = "stats";
const TRAY_MCP_STATUS_ID: &str = "mcp_status";
const QUIT_APP_ID: &str = "quit_app";
const TRAY_CLIPBOARD_MONITOR_ID: &str = "tray_clipboard_monitor";
const RECORDING_BADGE_RGBA: [u8; 4] = [247, 118, 142, 255];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct TrayMenuState {
    image_count: Option<u32>,
    mcp_connections: u32,
    clipboard_monitor_checked: bool,
}

#[cfg(test)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum TrayMenuItemKind {
    Item,
    Check,
}

#[cfg(test)]
#[derive(Debug, Clone, PartialEq, Eq)]
struct TrayMenuItemSpec {
    id: &'static str,
    label: String,
    kind: TrayMenuItemKind,
    enabled: bool,
    checked: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum TrayMenuAction {
    ToggleWindow,
    ToggleClipboardMonitor,
    Quit,
    None,
}

pub fn setup_tray(app: &AppHandle) -> tauri::Result<()> {
    let menu = build_tray_menu(app, current_tray_menu_state(app, None))?;

    let _tray = TrayIconBuilder::with_id(TRAY_ID)
        .icon(app.default_window_icon().unwrap().clone())
        .menu(&menu)
        .on_menu_event(
            move |app, event| match tray_action_for_id(event.id().0.as_str()) {
                TrayMenuAction::ToggleWindow => toggle_window(app),
                TrayMenuAction::ToggleClipboardMonitor => toggle_clipboard_monitor(app),
                TrayMenuAction::Quit => {
                    app.exit(0);
                }
                TrayMenuAction::None => {}
            },
        )
        .on_tray_icon_event(|tray, event| {
            if let TrayIconEvent::Click {
                button: MouseButton::Left,
                button_state: MouseButtonState::Up,
                ..
            } = event
            {
                toggle_window(tray.app_handle());
            }
        })
        .build(app)?;

    Ok(())
}

pub fn refresh_tray_menu(app: &AppHandle) -> Result<(), String> {
    let Some(tray) = app.tray_by_id(TRAY_ID) else {
        return Ok(());
    };
    let menu = build_tray_menu(app, current_tray_menu_state(app, None))
        .map_err(|e| format!("Failed to rebuild tray menu: {}", e))?;
    tray.set_menu(Some(menu))
        .map_err(|e| format!("Failed to update tray menu: {}", e))
}

pub fn set_clipboard_monitor_checked(app: &AppHandle, checked: bool) -> Result<(), String> {
    let Some(tray) = app.tray_by_id(TRAY_ID) else {
        return Ok(());
    };
    let menu = build_tray_menu(app, current_tray_menu_state(app, Some(checked)))
        .map_err(|e| format!("Failed to rebuild tray menu: {}", e))?;
    tray.set_menu(Some(menu))
        .map_err(|e| format!("Failed to update tray menu: {}", e))?;
    set_clipboard_monitor_icon(app, checked)
}

fn set_clipboard_monitor_icon(app: &AppHandle, recording: bool) -> Result<(), String> {
    let Some(tray) = app.tray_by_id(TRAY_ID) else {
        return Ok(());
    };
    let icon = crate::commands::window::current_app_icon_image(app)?;
    let icon = if recording {
        recording_tray_icon(&icon)
    } else {
        icon
    };
    tray.set_icon(Some(icon))
        .map_err(|e| format!("Failed to update tray recording icon: {}", e))
}

pub fn recording_tray_icon(base: &Image<'_>) -> Image<'static> {
    let mut rgba = base.rgba().to_vec();
    apply_recording_badge_to_rgba(&mut rgba, base.width(), base.height());
    Image::new_owned(rgba, base.width(), base.height())
}

fn current_tray_menu_state(
    app: &AppHandle,
    clipboard_monitor_checked: Option<bool>,
) -> TrayMenuState {
    let state = app.state::<crate::AppState>();
    TrayMenuState {
        image_count: state.db.image_count().ok(),
        mcp_connections: crate::mcp::socket::active_connections(),
        clipboard_monitor_checked: clipboard_monitor_checked
            .unwrap_or_else(|| state.clipboard_monitor.lock().running),
    }
}

fn build_tray_menu(app: &AppHandle, state: TrayMenuState) -> tauri::Result<Menu<Wry>> {
    let show_hide = MenuItem::with_id(app, SHOW_HIDE_ID, "Show Window", true, None::<&str>)?;
    let sep1 = PredefinedMenuItem::separator(app)?;
    let clipboard_monitor = CheckMenuItem::with_id(
        app,
        TRAY_CLIPBOARD_MONITOR_ID,
        "Clipboard Monitor",
        cfg!(target_os = "macos"),
        state.clipboard_monitor_checked,
        None::<&str>,
    )?;
    let stats_label = tray_library_label(state.image_count);
    let mcp_label = tray_mcp_label(state.mcp_connections);
    let stats = MenuItem::with_id(app, TRAY_STATS_ID, &stats_label, false, None::<&str>)?;
    let mcp_status = MenuItem::with_id(app, TRAY_MCP_STATUS_ID, &mcp_label, false, None::<&str>)?;
    let sep2 = PredefinedMenuItem::separator(app)?;
    let quit = MenuItem::with_id(app, QUIT_APP_ID, "Quit Cull", true, None::<&str>)?;

    Menu::with_items(
        app,
        &[
            &show_hide,
            &sep1,
            &clipboard_monitor,
            &stats,
            &mcp_status,
            &sep2,
            &quit,
        ],
    )
}

#[cfg(test)]
fn tray_menu_specs() -> Vec<TrayMenuItemSpec> {
    tray_menu_specs_for_state(TrayMenuState {
        image_count: Some(0),
        mcp_connections: 0,
        clipboard_monitor_checked: false,
    })
}

#[cfg(test)]
fn tray_menu_specs_for_state(state: TrayMenuState) -> Vec<TrayMenuItemSpec> {
    vec![
        TrayMenuItemSpec {
            id: SHOW_HIDE_ID,
            label: "Show Window".to_string(),
            kind: TrayMenuItemKind::Item,
            enabled: true,
            checked: false,
        },
        TrayMenuItemSpec {
            id: TRAY_CLIPBOARD_MONITOR_ID,
            label: "Clipboard Monitor".to_string(),
            kind: TrayMenuItemKind::Check,
            enabled: cfg!(target_os = "macos"),
            checked: state.clipboard_monitor_checked,
        },
        TrayMenuItemSpec {
            id: TRAY_STATS_ID,
            label: tray_library_label(state.image_count),
            kind: TrayMenuItemKind::Item,
            enabled: false,
            checked: false,
        },
        TrayMenuItemSpec {
            id: TRAY_MCP_STATUS_ID,
            label: tray_mcp_label(state.mcp_connections),
            kind: TrayMenuItemKind::Item,
            enabled: false,
            checked: false,
        },
        TrayMenuItemSpec {
            id: QUIT_APP_ID,
            label: "Quit Cull".to_string(),
            kind: TrayMenuItemKind::Item,
            enabled: true,
            checked: false,
        },
    ]
}

fn tray_library_label(image_count: Option<u32>) -> String {
    match image_count {
        Some(1) => "Library: 1 image".to_string(),
        Some(count) => format!("Library: {count} images"),
        None => "Library: unavailable".to_string(),
    }
}

fn tray_mcp_label(connections: u32) -> String {
    match connections {
        0 => "MCP: idle".to_string(),
        1 => "MCP: 1 connection".to_string(),
        count => format!("MCP: {count} connections"),
    }
}

fn tray_action_for_id(id: &str) -> TrayMenuAction {
    match id {
        SHOW_HIDE_ID => TrayMenuAction::ToggleWindow,
        TRAY_CLIPBOARD_MONITOR_ID => TrayMenuAction::ToggleClipboardMonitor,
        QUIT_APP_ID => TrayMenuAction::Quit,
        _ => TrayMenuAction::None,
    }
}

fn apply_recording_badge_to_rgba(rgba: &mut [u8], width: u32, height: u32) {
    if width == 0 || height == 0 || rgba.len() < (width as usize * height as usize * 4) {
        return;
    }

    let min_side = width.min(height) as i32;
    let radius = (min_side / 8).max(2);
    let center_x = width as i32 - radius;
    let center_y = radius;
    let radius_sq = radius * radius;

    for y in 0..height as i32 {
        for x in 0..width as i32 {
            let dx = x - center_x;
            let dy = y - center_y;
            if dx * dx + dy * dy <= radius_sq {
                let idx = ((y as u32 * width + x as u32) * 4) as usize;
                rgba[idx..idx + 4].copy_from_slice(&RECORDING_BADGE_RGBA);
            }
        }
    }
}

fn toggle_window(app: &AppHandle) {
    if let Some(window) = app.get_webview_window("main") {
        if window.is_visible().unwrap_or(false) {
            let _ = window.hide();
        } else {
            let _ = window.show();
            let _ = window.unminimize();
            let _ = window.set_focus();
        }
    }
}

fn toggle_clipboard_monitor(app: &AppHandle) {
    let state = app.state::<crate::AppState>();
    let running = state.clipboard_monitor.lock().running;
    let result = if running {
        Ok(crate::commands::clipboard_monitor::stop_clipboard_monitor_inner(state.inner()))
    } else {
        crate::commands::clipboard_monitor::start_clipboard_monitor_inner(app, state.inner(), None)
    };

    match result {
        Ok(status) => {
            let _ = set_clipboard_monitor_checked(app, status.running);
            if let Some(error) = status.last_error.filter(|_| !status.running) {
                let _ = app.emit(
                    "clipboard-monitor:error",
                    serde_json::json!({ "message": error }),
                );
            }
        }
        Err(error) => {
            let _ = set_clipboard_monitor_checked(app, false);
            let _ = app.emit(
                "clipboard-monitor:error",
                serde_json::json!({ "message": error }),
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tray_menu_exposes_clipboard_monitor_as_checkbox() {
        let spec = tray_menu_specs()
            .into_iter()
            .find(|item| item.id == TRAY_CLIPBOARD_MONITOR_ID)
            .expect("clipboard monitor tray item exists");

        assert_eq!(spec.label, "Clipboard Monitor");
        assert_eq!(spec.kind, TrayMenuItemKind::Check);
        assert_eq!(spec.enabled, cfg!(target_os = "macos"));
        assert!(!spec.checked);
    }

    #[test]
    fn clipboard_monitor_tray_item_dispatches_toggle_action() {
        assert_eq!(
            tray_action_for_id(TRAY_CLIPBOARD_MONITOR_ID),
            TrayMenuAction::ToggleClipboardMonitor
        );
    }

    #[test]
    fn tray_menu_uses_dynamic_library_and_mcp_labels() {
        let specs = tray_menu_specs_for_state(TrayMenuState {
            image_count: Some(42),
            mcp_connections: 2,
            clipboard_monitor_checked: true,
        });

        let stats = specs
            .iter()
            .find(|item| item.id == TRAY_STATS_ID)
            .expect("stats tray item exists");
        let mcp = specs
            .iter()
            .find(|item| item.id == TRAY_MCP_STATUS_ID)
            .expect("mcp tray item exists");
        let clipboard = specs
            .iter()
            .find(|item| item.id == TRAY_CLIPBOARD_MONITOR_ID)
            .expect("clipboard monitor tray item exists");

        assert_eq!(stats.label, "Library: 42 images");
        assert_eq!(mcp.label, "MCP: 2 connections");
        assert!(clipboard.checked);
        assert!(!specs.iter().any(|item| item.label == "Loading..."));
        assert!(!specs.iter().any(|item| item.label == "MCP: starting..."));
    }

    #[test]
    fn tray_menu_pluralizes_single_image_and_idle_mcp() {
        let specs = tray_menu_specs_for_state(TrayMenuState {
            image_count: Some(1),
            mcp_connections: 0,
            clipboard_monitor_checked: false,
        });

        let stats = specs
            .iter()
            .find(|item| item.id == TRAY_STATS_ID)
            .expect("stats tray item exists");
        let mcp = specs
            .iter()
            .find(|item| item.id == TRAY_MCP_STATUS_ID)
            .expect("mcp tray item exists");

        assert_eq!(stats.label, "Library: 1 image");
        assert_eq!(mcp.label, "MCP: idle");
    }

    #[test]
    fn recording_badge_draws_half_size_red_circle_on_top_right_of_icon() {
        let width = 16;
        let height = 16;
        let mut rgba = vec![0u8; width * height * 4];

        apply_recording_badge_to_rgba(&mut rgba, width as u32, height as u32);

        let badge_center = ((2 * width + 14) * 4) as usize;
        assert_eq!(&rgba[badge_center..badge_center + 4], &[247, 118, 142, 255]);

        let old_large_badge_pixel = ((4 * width + 9) * 4) as usize;
        assert_eq!(
            &rgba[old_large_badge_pixel..old_large_badge_pixel + 4],
            &[0, 0, 0, 0]
        );

        let untouched_corner = ((14 * width + 1) * 4) as usize;
        assert_eq!(&rgba[untouched_corner..untouched_corner + 4], &[0, 0, 0, 0]);
    }
}
