use tauri::{
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum TrayMenuItemKind {
    Item,
    Check,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct TrayMenuItemSpec {
    id: &'static str,
    label: &'static str,
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
    let menu = build_tray_menu(app, false)?;

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

pub fn set_clipboard_monitor_checked(app: &AppHandle, checked: bool) -> Result<(), String> {
    let Some(tray) = app.tray_by_id(TRAY_ID) else {
        return Ok(());
    };
    let menu =
        build_tray_menu(app, checked).map_err(|e| format!("Failed to rebuild tray menu: {}", e))?;
    tray.set_menu(Some(menu))
        .map_err(|e| format!("Failed to update tray menu: {}", e))
}

fn build_tray_menu(app: &AppHandle, clipboard_monitor_checked: bool) -> tauri::Result<Menu<Wry>> {
    let show_hide = MenuItem::with_id(app, SHOW_HIDE_ID, "Show Window", true, None::<&str>)?;
    let sep1 = PredefinedMenuItem::separator(app)?;
    let clipboard_monitor = CheckMenuItem::with_id(
        app,
        TRAY_CLIPBOARD_MONITOR_ID,
        "Clipboard Monitor",
        cfg!(target_os = "macos"),
        clipboard_monitor_checked,
        None::<&str>,
    )?;
    let stats = MenuItem::with_id(app, TRAY_STATS_ID, "Loading...", false, None::<&str>)?;
    let mcp_status = MenuItem::with_id(
        app,
        TRAY_MCP_STATUS_ID,
        "MCP: starting...",
        false,
        None::<&str>,
    )?;
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

fn tray_menu_specs() -> Vec<TrayMenuItemSpec> {
    vec![
        TrayMenuItemSpec {
            id: SHOW_HIDE_ID,
            label: "Show Window",
            kind: TrayMenuItemKind::Item,
            enabled: true,
            checked: false,
        },
        TrayMenuItemSpec {
            id: TRAY_CLIPBOARD_MONITOR_ID,
            label: "Clipboard Monitor",
            kind: TrayMenuItemKind::Check,
            enabled: cfg!(target_os = "macos"),
            checked: false,
        },
        TrayMenuItemSpec {
            id: TRAY_STATS_ID,
            label: "Loading...",
            kind: TrayMenuItemKind::Item,
            enabled: false,
            checked: false,
        },
        TrayMenuItemSpec {
            id: TRAY_MCP_STATUS_ID,
            label: "MCP: starting...",
            kind: TrayMenuItemKind::Item,
            enabled: false,
            checked: false,
        },
        TrayMenuItemSpec {
            id: QUIT_APP_ID,
            label: "Quit Cull",
            kind: TrayMenuItemKind::Item,
            enabled: true,
            checked: false,
        },
    ]
}

fn tray_action_for_id(id: &str) -> TrayMenuAction {
    match id {
        SHOW_HIDE_ID => TrayMenuAction::ToggleWindow,
        TRAY_CLIPBOARD_MONITOR_ID => TrayMenuAction::ToggleClipboardMonitor,
        QUIT_APP_ID => TrayMenuAction::Quit,
        _ => TrayMenuAction::None,
    }
}

fn toggle_window(app: &AppHandle) {
    if let Some(window) = app.get_webview_window("main") {
        if window.is_visible().unwrap_or(false) {
            let _ = window.hide();
        } else {
            let _ = window.show();
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
}
