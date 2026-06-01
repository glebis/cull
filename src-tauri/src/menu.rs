use serde::Deserialize;
use std::path::PathBuf;
use tauri::menu::{CheckMenuItem, Menu, MenuItem, MenuItemKind, PredefinedMenuItem, Submenu};
use tauri::{AppHandle, Emitter, Manager, Wry};

const CULL_HELP_BOOK_ID: &str = "com.glebkalinin.cull.help";
const CULL_HELP_PAGE: &str = "index.html";
const VIEW_PUBLISH_ID: &str = "view_publish";
const VIEW_EXPORT_ID: &str = "view_export";

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
        "check_update",
        "Check for Update...",
        true,
        None::<&str>,
    )?)?;
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
        "view_preview_display",
        "Preview Display",
        true,
        Some::<&str>("CmdOrCtrl+Shift+P"),
    )?)?;
    view_menu.append(&MenuItem::with_id(
        app,
        "preview_display_move_monitor",
        "Move Preview Display to Display...",
        true,
        None::<&str>,
    )?)?;
    view_menu.append(&MenuItem::with_id(
        app,
        "preview_display_fullscreen",
        "Fullscreen Preview Display",
        true,
        None::<&str>,
    )?)?;
    view_menu.append(&MenuItem::with_id(
        app,
        "preview_display_start_web_stream",
        "Start Preview Display Web Stream",
        true,
        None::<&str>,
    )?)?;
    view_menu.append(&MenuItem::with_id(
        app,
        "preview_display_copy_web_stream_url",
        "Copy Preview Display Web URL",
        false,
        None::<&str>,
    )?)?;
    view_menu.append(&MenuItem::with_id(
        app,
        "preview_display_stop_web_stream",
        "Stop Preview Display Web Stream",
        false,
        None::<&str>,
    )?)?;
    view_menu.append(&CheckMenuItem::with_id(
        app,
        "preview_display_freeze",
        "Freeze Preview Display",
        true,
        false,
        None::<&str>,
    )?)?;
    view_menu.append(&CheckMenuItem::with_id(
        app,
        "preview_display_blank",
        "Blank Preview Display",
        true,
        false,
        None::<&str>,
    )?)?;
    view_menu.append(&PredefinedMenuItem::separator(app)?)?;
    view_menu.append(&CheckMenuItem::with_id(
        app,
        "preview_display_preset_image_only",
        "Image Only",
        true,
        true,
        None::<&str>,
    )?)?;
    view_menu.append(&CheckMenuItem::with_id(
        app,
        "preview_display_preset_client_review",
        "Client Review",
        true,
        false,
        None::<&str>,
    )?)?;
    view_menu.append(&CheckMenuItem::with_id(
        app,
        "preview_display_preset_metadata_review",
        "Metadata Review",
        true,
        false,
        None::<&str>,
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
    help_menu.append(&MenuItem::with_id(
        app,
        "github_wiki",
        "GitHub Wiki",
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
    #[serde(default)]
    static_publishing_enabled: bool,
    #[serde(default)]
    preview_display_frozen: bool,
    #[serde(default)]
    preview_display_blanked: bool,
    #[serde(default = "default_preview_display_mode")]
    preview_display_mode: String,
    #[serde(default)]
    preview_display_web_stream_active: bool,
}

fn default_preview_display_mode() -> String {
    "image_only".to_string()
}

#[tauri::command]
pub async fn update_menu_state(app: AppHandle, state: MenuStatePayload) -> Result<(), String> {
    sync_static_publishing_menu_item(&app, state.static_publishing_enabled)?;

    for (id, mode) in [
        ("view_grid", "grid"),
        ("view_loupe", "loupe"),
        ("view_compare", "compare"),
        ("view_canvas", "canvas"),
        ("view_lineage", "lineage"),
        ("view_embeddings", "embeddings"),
        (VIEW_PUBLISH_ID, "publish"),
        (VIEW_EXPORT_ID, "export"),
    ] {
        set_menu_item_checked(&app, id, state.view_mode == mode)?;
    }

    set_menu_item_checked(&app, "toggle_sidebar", state.sidebar_visible)?;
    set_menu_item_checked(&app, "preview_display_freeze", state.preview_display_frozen)?;
    set_menu_item_checked(&app, "preview_display_blank", state.preview_display_blanked)?;
    set_menu_item_checked(
        &app,
        "preview_display_preset_image_only",
        state.preview_display_mode == "image_only",
    )?;
    set_menu_item_checked(
        &app,
        "preview_display_preset_client_review",
        state.preview_display_mode == "client_review",
    )?;
    set_menu_item_checked(
        &app,
        "preview_display_preset_metadata_review",
        state.preview_display_mode == "metadata_review",
    )?;
    set_menu_item_enabled(
        &app,
        "preview_display_start_web_stream",
        !state.preview_display_web_stream_active,
    )?;
    set_menu_item_enabled(
        &app,
        "preview_display_copy_web_stream_url",
        state.preview_display_web_stream_active,
    )?;
    set_menu_item_enabled(
        &app,
        "preview_display_stop_web_stream",
        state.preview_display_web_stream_active,
    )?;
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

fn sync_static_publishing_menu_item(app: &AppHandle, enabled: bool) -> Result<(), String> {
    let exists = find_menu_item(app, VIEW_PUBLISH_ID)?.is_some();
    if enabled && !exists {
        let item =
            CheckMenuItem::with_id(app, VIEW_PUBLISH_ID, "Publish", true, false, None::<&str>)
                .map_err(|e| format!("Failed to create Publish menu item: {}", e))?;
        insert_menu_item_before(app, VIEW_EXPORT_ID, &item)?;
    } else if !enabled && exists {
        remove_menu_item(app, VIEW_PUBLISH_ID)?;
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

fn insert_menu_item_before(
    app: &AppHandle,
    target_id: &str,
    item: &dyn tauri::menu::IsMenuItem<Wry>,
) -> Result<(), String> {
    let Some(menu) = app.menu() else {
        return Ok(());
    };
    if insert_menu_item_before_in_items(menu.items().map_err(|e| e.to_string())?, target_id, item)?
    {
        Ok(())
    } else {
        Err(format!(
            "Could not find menu item '{}' to insert before",
            target_id
        ))
    }
}

fn insert_menu_item_before_in_items(
    items: Vec<MenuItemKind<Wry>>,
    target_id: &str,
    item: &dyn tauri::menu::IsMenuItem<Wry>,
) -> Result<bool, String> {
    for menu_item in items {
        if let MenuItemKind::Submenu(submenu) = &menu_item {
            let children = submenu.items().map_err(|e| e.to_string())?;
            for (idx, child) in children.iter().enumerate() {
                if child.id().0.as_str() == target_id {
                    submenu
                        .insert(item, idx)
                        .map_err(|e| format!("Failed to insert menu item: {}", e))?;
                    return Ok(true);
                }
            }
            if insert_menu_item_before_in_items(children, target_id, item)? {
                return Ok(true);
            }
        }
    }
    Ok(false)
}

fn remove_menu_item(app: &AppHandle, id: &str) -> Result<(), String> {
    let Some(menu) = app.menu() else {
        return Ok(());
    };
    remove_menu_item_from_items(menu.items().map_err(|e| e.to_string())?, id).map(|_| ())
}

fn remove_menu_item_from_items(items: Vec<MenuItemKind<Wry>>, id: &str) -> Result<bool, String> {
    for menu_item in items {
        if let MenuItemKind::Submenu(submenu) = &menu_item {
            let children = submenu.items().map_err(|e| e.to_string())?;
            for child in &children {
                if child.id().0.as_str() == id {
                    match child {
                        MenuItemKind::MenuItem(item) => submenu.remove(item),
                        MenuItemKind::Check(item) => submenu.remove(item),
                        MenuItemKind::Icon(item) => submenu.remove(item),
                        MenuItemKind::Submenu(item) => submenu.remove(item),
                        MenuItemKind::Predefined(item) => submenu.remove(item),
                    }
                    .map_err(|e| format!("Failed to remove menu item '{}': {}", id, e))?;
                    return Ok(true);
                }
            }
            if remove_menu_item_from_items(children, id)? {
                return Ok(true);
            }
        }
    }
    Ok(false)
}

pub fn handle_menu_event(app: &AppHandle, event: &tauri::menu::MenuEvent) {
    let id = event.id().0.as_str();
    match id {
        "help" => show_cull_help(app),
        "about" | "check_update" | "open_file" | "open_folder" | "settings" | "undo" | "redo"
        | "deselect_all" | "image_share" | "image_open_default" | "image_open_with"
        | "image_reveal" | "image_rename" | "image_move_to" | "image_trash" | "view_grid"
        | "view_compare" | "view_loupe" | "view_canvas" | "view_lineage" | "view_embeddings"
        | "view_publish" | "view_export" | "toggle_sidebar" | "view_preview_display"
        | "preview_display_move_monitor" | "preview_display_fullscreen"
        | "preview_display_start_web_stream" | "preview_display_copy_web_stream_url"
        | "preview_display_stop_web_stream"
        | "preview_display_freeze" | "preview_display_blank"
        | "preview_display_preset_image_only" | "preview_display_preset_client_review"
        | "preview_display_preset_metadata_review" | "zoom_in" | "zoom_out" | "actual_size"
        | "github_wiki" => {
            let _ = app.emit("menu-action", id);
        }
        _ => {}
    }
}

#[cfg(target_os = "macos")]
fn show_cull_help(app: &AppHandle) {
    if let Err(err) = open_cull_help_book(app) {
        crate::safe_eprintln!("[menu] Failed to open Cull User Guide: {}", err);
        let _ = app.emit("menu-action", "help");
    }
}

#[cfg(target_os = "macos")]
fn open_cull_help_book(app: &AppHandle) -> Result<(), String> {
    if objc2::MainThreadMarker::new().is_some() {
        return open_cull_help_book_on_main(app);
    }

    let handle = app.clone();
    let (tx, rx) = std::sync::mpsc::channel();
    app.run_on_main_thread(move || {
        let _ = tx.send(open_cull_help_book_on_main(&handle));
    })
    .map_err(|err| format!("Failed to schedule Cull User Guide open: {}", err))?;

    rx.recv()
        .map_err(|_| "Failed to receive Cull User Guide open result".to_string())?
}

#[cfg(target_os = "macos")]
fn open_cull_help_book_on_main(app: &AppHandle) -> Result<(), String> {
    let bundle_path = cull_app_bundle_path(app)?;
    apple_help::register_help_book(&bundle_path)?;
    apple_help::goto_page(CULL_HELP_BOOK_ID, CULL_HELP_PAGE)
}

#[cfg(target_os = "macos")]
fn cull_app_bundle_path(app: &AppHandle) -> Result<PathBuf, String> {
    let resource_dir = app
        .path()
        .resource_dir()
        .map_err(|err| format!("Failed to resolve app resources: {}", err))?;
    if let Some(bundle_path) = resource_dir
        .parent()
        .and_then(|contents_dir| contents_dir.parent())
        .filter(|path| {
            path.file_name()
                .and_then(|name| name.to_str())
                .is_some_and(|name| name.ends_with(".app"))
        })
    {
        return Ok(bundle_path.to_path_buf());
    }

    Err(format!(
        "Expected app resources to be inside a .app bundle, got '{}'",
        resource_dir.display()
    ))
}

#[cfg(target_os = "macos")]
mod apple_help {
    use core::ffi::c_void;
    use core_foundation::base::{OSStatus, TCFType};
    use core_foundation::string::CFString;
    use core_foundation::url::CFURL;
    use std::path::Path;
    use std::ptr;

    const NO_ERR: OSStatus = 0;

    #[link(name = "Carbon", kind = "framework")]
    extern "C" {
        fn AHRegisterHelpBookWithURL(application_url: *const c_void) -> OSStatus;
        fn AHGotoPage(
            bookname: *const c_void,
            path: *const c_void,
            anchor: *const c_void,
        ) -> OSStatus;
    }

    pub fn register_help_book(app_bundle_path: &Path) -> Result<(), String> {
        let app_bundle_url = CFURL::from_path(app_bundle_path, true).ok_or_else(|| {
            format!(
                "Failed to create file URL for app bundle '{}'",
                app_bundle_path.display()
            )
        })?;
        let status =
            unsafe { AHRegisterHelpBookWithURL(app_bundle_url.as_concrete_TypeRef().cast()) };
        status_to_result(
            status,
            format!(
                "Failed to register Help Book from '{}'",
                app_bundle_path.display()
            ),
        )
    }

    pub fn goto_page(book_id: &str, page: &str) -> Result<(), String> {
        let book = CFString::new(book_id);
        let page = CFString::new(page);
        let status = unsafe {
            AHGotoPage(
                book.as_concrete_TypeRef().cast(),
                page.as_concrete_TypeRef().cast(),
                ptr::null(),
            )
        };
        status_to_result(
            status,
            format!("Failed to open Help Book '{}' page '{}'", book_id, page),
        )
    }

    fn status_to_result(status: OSStatus, context: String) -> Result<(), String> {
        if status == NO_ERR {
            Ok(())
        } else {
            Err(format!("{}: OSStatus {}", context, status))
        }
    }
}

#[cfg(not(target_os = "macos"))]
fn show_cull_help(app: &AppHandle) {
    let _ = app.emit("menu-action", "help");
}
