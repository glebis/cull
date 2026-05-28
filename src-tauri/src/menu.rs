use serde::Deserialize;
use tauri::menu::{CheckMenuItem, Menu, MenuItem, MenuItemKind, PredefinedMenuItem, Submenu};
use tauri::{AppHandle, Emitter, Wry};

pub fn create_menu(app: &AppHandle) -> tauri::Result<Menu<Wry>> {
    let menu = Menu::new(app)?;

    // App menu
    let app_menu = Submenu::new(app, "Cull", true)?;
    app_menu.append(&MenuItem::with_id(
        app,
        "about",
        "About Cull",
        true,
        None::<&str>,
    )?)?;
    app_menu.append(&PredefinedMenuItem::separator(app)?)?;
    app_menu.append(&MenuItem::with_id(
        app,
        "settings",
        "Settings...",
        true,
        Some::<&str>("CmdOrCtrl+,"),
    )?)?;
    app_menu.append(&PredefinedMenuItem::separator(app)?)?;
    app_menu.append(&PredefinedMenuItem::services(app, None)?)?;
    app_menu.append(&PredefinedMenuItem::separator(app)?)?;
    app_menu.append(&PredefinedMenuItem::hide(app, None)?)?;
    app_menu.append(&PredefinedMenuItem::hide_others(app, None)?)?;
    app_menu.append(&PredefinedMenuItem::show_all(app, None)?)?;
    app_menu.append(&PredefinedMenuItem::separator(app)?)?;
    app_menu.append(&PredefinedMenuItem::quit(app, None)?)?;
    menu.append(&app_menu)?;

    // File menu
    let file_menu = Submenu::new(app, "File", true)?;
    file_menu.append(&MenuItem::with_id(
        app,
        "open_file",
        "Open File...",
        true,
        Some::<&str>("CmdOrCtrl+O"),
    )?)?;
    file_menu.append(&MenuItem::with_id(
        app,
        "open_folder",
        "Open Folder...",
        true,
        Some::<&str>("CmdOrCtrl+Shift+O"),
    )?)?;
    file_menu.append(&PredefinedMenuItem::separator(app)?)?;
    file_menu.append(&PredefinedMenuItem::close_window(app, None)?)?;
    menu.append(&file_menu)?;

    // Edit menu
    let edit_menu = Submenu::new(app, "Edit", true)?;
    edit_menu.append(&MenuItem::with_id(
        app,
        "undo",
        "Undo",
        true,
        Some::<&str>("CmdOrCtrl+Z"),
    )?)?;
    edit_menu.append(&MenuItem::with_id(
        app,
        "redo",
        "Redo",
        true,
        Some::<&str>("CmdOrCtrl+Shift+Z"),
    )?)?;
    edit_menu.append(&PredefinedMenuItem::separator(app)?)?;
    edit_menu.append(&PredefinedMenuItem::cut(app, None)?)?;
    edit_menu.append(&PredefinedMenuItem::copy(app, None)?)?;
    edit_menu.append(&PredefinedMenuItem::paste(app, None)?)?;
    edit_menu.append(&PredefinedMenuItem::separator(app)?)?;
    edit_menu.append(&PredefinedMenuItem::select_all(app, None)?)?;
    edit_menu.append(&MenuItem::with_id(
        app,
        "deselect_all",
        "Deselect All",
        false,
        Some::<&str>("CmdOrCtrl+Shift+A"),
    )?)?;
    menu.append(&edit_menu)?;

    // Image menu
    let image_menu = Submenu::new(app, "Image", true)?;
    image_menu.append(&MenuItem::with_id(
        app,
        "image_share",
        "Share...",
        false,
        None::<&str>,
    )?)?;
    image_menu.append(&MenuItem::with_id(
        app,
        "image_open_default",
        "Open in Default App",
        false,
        None::<&str>,
    )?)?;
    image_menu.append(&MenuItem::with_id(
        app,
        "image_open_with",
        "Open With...",
        false,
        None::<&str>,
    )?)?;
    image_menu.append(&PredefinedMenuItem::separator(app)?)?;
    image_menu.append(&MenuItem::with_id(
        app,
        "image_reveal",
        "Reveal in Finder",
        false,
        None::<&str>,
    )?)?;
    image_menu.append(&MenuItem::with_id(
        app,
        "image_rename",
        "Rename...",
        false,
        None::<&str>,
    )?)?;
    image_menu.append(&MenuItem::with_id(
        app,
        "image_move_to",
        "Move to Folder...",
        false,
        None::<&str>,
    )?)?;
    image_menu.append(&PredefinedMenuItem::separator(app)?)?;
    image_menu.append(&MenuItem::with_id(
        app,
        "image_trash",
        "Move to Trash",
        false,
        None::<&str>,
    )?)?;
    menu.append(&image_menu)?;

    // View menu
    let view_menu = Submenu::new(app, "View", true)?;
    view_menu.append(&CheckMenuItem::with_id(
        app,
        "view_grid",
        "Grid",
        true,
        true,
        Some::<&str>("CmdOrCtrl+1"),
    )?)?;
    view_menu.append(&CheckMenuItem::with_id(
        app,
        "view_loupe",
        "Loupe",
        true,
        false,
        Some::<&str>("CmdOrCtrl+2"),
    )?)?;
    view_menu.append(&CheckMenuItem::with_id(
        app,
        "view_compare",
        "Compare",
        true,
        false,
        Some::<&str>("CmdOrCtrl+3"),
    )?)?;
    view_menu.append(&CheckMenuItem::with_id(
        app,
        "view_canvas",
        "Canvas",
        true,
        false,
        Some::<&str>("CmdOrCtrl+4"),
    )?)?;
    view_menu.append(&CheckMenuItem::with_id(
        app,
        "view_lineage",
        "Lineage",
        true,
        false,
        Some::<&str>("CmdOrCtrl+5"),
    )?)?;
    view_menu.append(&CheckMenuItem::with_id(
        app,
        "view_embeddings",
        "Embedding Explorer",
        true,
        false,
        Some::<&str>("CmdOrCtrl+6"),
    )?)?;
    view_menu.append(&CheckMenuItem::with_id(
        app,
        "view_export",
        "Export",
        true,
        false,
        Some::<&str>("CmdOrCtrl+7"),
    )?)?;
    view_menu.append(&PredefinedMenuItem::separator(app)?)?;
    view_menu.append(&CheckMenuItem::with_id(
        app,
        "toggle_sidebar",
        "Toggle Sidebar",
        true,
        true,
        Some::<&str>("CmdOrCtrl+B"),
    )?)?;
    view_menu.append(&PredefinedMenuItem::separator(app)?)?;
    view_menu.append(&MenuItem::with_id(
        app,
        "zoom_in",
        "Zoom In",
        true,
        Some::<&str>("CmdOrCtrl+="),
    )?)?;
    view_menu.append(&MenuItem::with_id(
        app,
        "zoom_out",
        "Zoom Out",
        true,
        Some::<&str>("CmdOrCtrl+Minus"),
    )?)?;
    view_menu.append(&MenuItem::with_id(
        app,
        "actual_size",
        "Actual Size",
        true,
        Some::<&str>("CmdOrCtrl+0"),
    )?)?;
    view_menu.append(&PredefinedMenuItem::separator(app)?)?;
    view_menu.append(&PredefinedMenuItem::fullscreen(app, None)?)?;
    menu.append(&view_menu)?;

    // Window menu
    let window_menu = Submenu::new(app, "Window", true)?;
    window_menu.append(&PredefinedMenuItem::minimize(app, None)?)?;
    window_menu.append(&PredefinedMenuItem::maximize(app, Some("Zoom"))?)?;
    window_menu.append(&PredefinedMenuItem::separator(app)?)?;
    window_menu.append(&PredefinedMenuItem::bring_all_to_front(app, None)?)?;
    menu.append(&window_menu)?;

    // Help menu
    let help_menu = Submenu::new(app, "Help", true)?;
    help_menu.append(&MenuItem::with_id(
        app,
        "help",
        "Cull User Guide",
        true,
        None::<&str>,
    )?)?;
    menu.append(&help_menu)?;

    Ok(menu)
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MenuStatePayload {
    view_mode: String,
    sidebar_visible: bool,
    has_focused_image: bool,
    selected_count: usize,
}

#[tauri::command]
pub async fn update_menu_state(app: AppHandle, state: MenuStatePayload) -> Result<(), String> {
    for (id, mode) in [
        ("view_grid", "grid"),
        ("view_loupe", "loupe"),
        ("view_compare", "compare"),
        ("view_canvas", "canvas"),
        ("view_lineage", "lineage"),
        ("view_embeddings", "embeddings"),
        ("view_export", "export"),
    ] {
        set_menu_item_checked(&app, id, state.view_mode == mode)?;
    }

    set_menu_item_checked(&app, "toggle_sidebar", state.sidebar_visible)?;
    set_menu_item_enabled(&app, "deselect_all", state.selected_count > 0)?;

    for id in [
        "image_share",
        "image_open_default",
        "image_open_with",
        "image_reveal",
        "image_rename",
        "image_move_to",
        "image_trash",
    ] {
        set_menu_item_enabled(&app, id, state.has_focused_image)?;
    }

    Ok(())
}

fn set_menu_item_enabled(app: &AppHandle, id: &str, enabled: bool) -> Result<(), String> {
    if let Some(item) = find_menu_item(app, id)? {
        match item {
            MenuItemKind::MenuItem(item) => item.set_enabled(enabled),
            MenuItemKind::Check(item) => item.set_enabled(enabled),
            MenuItemKind::Icon(item) => item.set_enabled(enabled),
            MenuItemKind::Submenu(item) => item.set_enabled(enabled),
            MenuItemKind::Predefined(_) => Ok(()),
        }
        .map_err(|e| format!("Failed to update menu item '{}': {}", id, e))?;
    }
    Ok(())
}

fn set_menu_item_checked(app: &AppHandle, id: &str, checked: bool) -> Result<(), String> {
    if let Some(item) = find_menu_item(app, id)? {
        if let MenuItemKind::Check(item) = item {
            item.set_checked(checked)
                .map_err(|e| format!("Failed to check menu item '{}': {}", id, e))?;
        }
    }
    Ok(())
}

fn find_menu_item(app: &AppHandle, id: &str) -> Result<Option<MenuItemKind<Wry>>, String> {
    let Some(menu) = app.menu() else {
        return Ok(None);
    };
    find_menu_item_in_items(menu.items().map_err(|e| e.to_string())?, id)
}

fn find_menu_item_in_items(
    items: Vec<MenuItemKind<Wry>>,
    id: &str,
) -> Result<Option<MenuItemKind<Wry>>, String> {
    for item in items {
        if item.id().0.as_str() == id {
            return Ok(Some(item));
        }
        if let MenuItemKind::Submenu(submenu) = &item {
            if let Some(found) =
                find_menu_item_in_items(submenu.items().map_err(|e| e.to_string())?, id)?
            {
                return Ok(Some(found));
            }
        }
    }
    Ok(None)
}

pub fn handle_menu_event(app: &AppHandle, event: &tauri::menu::MenuEvent) {
    let id = event.id().0.as_str();
    match id {
        "help" => show_cull_help(app),
        "about" | "open_file" | "open_folder" | "settings" | "undo" | "redo" | "deselect_all"
        | "image_share" | "image_open_default" | "image_open_with" | "image_reveal"
        | "image_rename" | "image_move_to" | "image_trash" | "view_grid" | "view_compare"
        | "view_loupe" | "view_canvas" | "view_lineage" | "view_embeddings" | "view_export"
        | "toggle_sidebar" | "zoom_in" | "zoom_out" | "actual_size" => {
            let _ = app.emit("menu-action", id);
        }
        _ => {}
    }
}

#[cfg(target_os = "macos")]
fn show_cull_help(app: &AppHandle) {
    if objc2::MainThreadMarker::new().is_some() {
        show_cull_help_on_main();
        return;
    }

    let _ = app.run_on_main_thread(show_cull_help_on_main);
}

#[cfg(target_os = "macos")]
fn show_cull_help_on_main() {
    use objc2_app_kit::NSApplication;

    let Some(mtm) = objc2::MainThreadMarker::new() else {
        return;
    };
    let application = NSApplication::sharedApplication(mtm);
    unsafe {
        application.showHelp(None);
    }
}

#[cfg(not(target_os = "macos"))]
fn show_cull_help(app: &AppHandle) {
    let _ = app.emit("menu-action", "help");
}
