use tauri::menu::{Menu, MenuItem, PredefinedMenuItem, Submenu, AboutMetadata};
use tauri::{AppHandle, Emitter, Wry};

pub fn create_menu(app: &AppHandle) -> tauri::Result<Menu<Wry>> {
    let menu = Menu::new(app)?;

    // App menu
    let app_menu = Submenu::new(app, "ImageView", true)?;
    app_menu.append(&PredefinedMenuItem::about(app, Some("About ImageView"), Some(AboutMetadata {
        name: Some("ImageView".to_string()),
        version: Some("0.1.0".to_string()),
        ..Default::default()
    }))?)?;
    app_menu.append(&PredefinedMenuItem::separator(app)?)?;
    app_menu.append(&MenuItem::with_id(app, "settings", "Settings...", true, Some::<&str>("CmdOrCtrl+,"))?)?;
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
    file_menu.append(&MenuItem::with_id(app, "open_file", "Open File...", true, Some::<&str>("CmdOrCtrl+O"))?)?;
    file_menu.append(&MenuItem::with_id(app, "open_folder", "Open Folder...", true, Some::<&str>("CmdOrCtrl+Shift+O"))?)?;
    file_menu.append(&PredefinedMenuItem::separator(app)?)?;
    file_menu.append(&PredefinedMenuItem::close_window(app, None)?)?;
    menu.append(&file_menu)?;

    // Edit menu
    let edit_menu = Submenu::new(app, "Edit", true)?;
    edit_menu.append(&PredefinedMenuItem::undo(app, None)?)?;
    edit_menu.append(&PredefinedMenuItem::redo(app, None)?)?;
    edit_menu.append(&PredefinedMenuItem::separator(app)?)?;
    edit_menu.append(&PredefinedMenuItem::cut(app, None)?)?;
    edit_menu.append(&PredefinedMenuItem::copy(app, None)?)?;
    edit_menu.append(&PredefinedMenuItem::paste(app, None)?)?;
    edit_menu.append(&PredefinedMenuItem::separator(app)?)?;
    edit_menu.append(&PredefinedMenuItem::select_all(app, None)?)?;
    edit_menu.append(&MenuItem::with_id(app, "deselect_all", "Deselect All", true, Some::<&str>("CmdOrCtrl+Shift+A"))?)?;
    menu.append(&edit_menu)?;

    // View menu
    let view_menu = Submenu::new(app, "View", true)?;
    view_menu.append(&MenuItem::with_id(app, "view_grid", "Grid", true, Some::<&str>("CmdOrCtrl+1"))?)?;
    view_menu.append(&MenuItem::with_id(app, "view_compare", "Compare", true, Some::<&str>("CmdOrCtrl+2"))?)?;
    view_menu.append(&MenuItem::with_id(app, "view_loupe", "Loupe", true, Some::<&str>("CmdOrCtrl+3"))?)?;
    view_menu.append(&MenuItem::with_id(app, "view_canvas", "Canvas", true, Some::<&str>("CmdOrCtrl+4"))?)?;
    view_menu.append(&MenuItem::with_id(app, "view_lineage", "Lineage", true, Some::<&str>("CmdOrCtrl+5"))?)?;
    view_menu.append(&MenuItem::with_id(app, "view_embeddings", "Embedding Explorer", true, Some::<&str>("CmdOrCtrl+6"))?)?;
    view_menu.append(&MenuItem::with_id(app, "view_export", "Export", true, Some::<&str>("CmdOrCtrl+7"))?)?;
    view_menu.append(&PredefinedMenuItem::separator(app)?)?;
    view_menu.append(&MenuItem::with_id(app, "toggle_sidebar", "Toggle Sidebar", true, Some::<&str>("CmdOrCtrl+B"))?)?;
    view_menu.append(&PredefinedMenuItem::separator(app)?)?;
    view_menu.append(&MenuItem::with_id(app, "zoom_in", "Zoom In", true, Some::<&str>("CmdOrCtrl+Plus"))?)?;
    view_menu.append(&MenuItem::with_id(app, "zoom_out", "Zoom Out", true, Some::<&str>("CmdOrCtrl+Minus"))?)?;
    view_menu.append(&MenuItem::with_id(app, "actual_size", "Actual Size", true, Some::<&str>("CmdOrCtrl+0"))?)?;
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
    help_menu.append(&MenuItem::with_id(app, "help", "ImageView Help", true, None::<&str>)?)?;
    menu.append(&help_menu)?;

    Ok(menu)
}

pub fn handle_menu_event(app: &AppHandle, event: &tauri::menu::MenuEvent) {
    let id = event.id().0.as_str();
    match id {
        "open_file" | "open_folder" | "settings"
        | "deselect_all"
        | "view_grid" | "view_compare" | "view_loupe" | "view_canvas"
        | "view_lineage" | "view_embeddings" | "view_export"
        | "toggle_sidebar" | "zoom_in" | "zoom_out" | "actual_size"
        | "help" => {
            let _ = app.emit("menu-action", id);
        }
        _ => {}
    }
}
