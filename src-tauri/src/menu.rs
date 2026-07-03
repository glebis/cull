use serde::Deserialize;
use std::path::PathBuf;
use tauri::menu::{
    CheckMenuItem, Menu, MenuItem, MenuItemKind, PredefinedMenuItem, Submenu, HELP_SUBMENU_ID,
};
use tauri::{AppHandle, Emitter, Manager, Wry};

const CULL_HELP_BOOK_ID: &str = "com.glebkalinin.cull.help";
const CULL_HELP_PAGE: &str = "index.html";
const VIEW_PUBLISH_ID: &str = "view_publish";
const VIEW_EXPORT_ID: &str = "view_export";
const AGENT_SKILLS_ID: &str = "agent_skills";
const WINDOW_MENU_ID: &str = "window_menu";
const WINDOW_MENU_FOCUS_PREFIX: &str = "window_focus:";

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
        "import_folder",
        "Import Folder...",
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
    edit_menu.append(&MenuItem::with_id(
        app,
        "undo_history",
        "Action History…",
        true,
        Some::<&str>("CmdOrCtrl+Shift+H"),
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
    view_menu.append(&MenuItem::with_id(
        app,
        "command_palette",
        "Command Palette...",
        true,
        Some::<&str>("CmdOrCtrl+P"),
    )?)?;
    view_menu.append(&PredefinedMenuItem::separator(app)?)?;
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
    view_menu.append(&MenuItem::with_id(
        app,
        "view_tinder",
        "Speed Review",
        true,
        Some::<&str>("CmdOrCtrl+8"),
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
    view_menu.append(&CheckMenuItem::with_id(
        app,
        "view_loupe_histogram",
        "Loupe Histogram",
        true,
        false,
        None::<&str>,
    )?)?;
    view_menu.append(&PredefinedMenuItem::separator(app)?)?;
    let preview_display_menu = Submenu::new(app, "Preview Display", true)?;
    preview_display_menu.append(&MenuItem::with_id(
        app,
        "view_preview_display",
        "Open Preview Display",
        true,
        Some::<&str>("CmdOrCtrl+Shift+D"),
    )?)?;
    preview_display_menu.append(&MenuItem::with_id(
        app,
        "preview_display_move_monitor",
        "Move to Display...",
        true,
        None::<&str>,
    )?)?;
    preview_display_menu.append(&MenuItem::with_id(
        app,
        "preview_display_fullscreen",
        "Fullscreen",
        true,
        None::<&str>,
    )?)?;
    preview_display_menu.append(&CheckMenuItem::with_id(
        app,
        "preview_display_always_on_top",
        "Always on Top",
        true,
        false,
        None::<&str>,
    )?)?;
    preview_display_menu.append(&PredefinedMenuItem::separator(app)?)?;

    let preview_display_web_stream_menu = Submenu::new(app, "Web Stream", true)?;
    preview_display_web_stream_menu.append(&MenuItem::with_id(
        app,
        "preview_display_start_web_stream",
        "Start Local Web Stream",
        true,
        None::<&str>,
    )?)?;
    preview_display_web_stream_menu.append(&MenuItem::with_id(
        app,
        "preview_display_start_lan_web_stream",
        "Start LAN Web Stream",
        true,
        None::<&str>,
    )?)?;
    preview_display_web_stream_menu.append(&MenuItem::with_id(
        app,
        "preview_display_copy_web_stream_url",
        "Copy Web URL",
        false,
        None::<&str>,
    )?)?;
    preview_display_web_stream_menu.append(&MenuItem::with_id(
        app,
        "preview_display_stop_web_stream",
        "Stop Web Stream",
        false,
        None::<&str>,
    )?)?;
    preview_display_menu.append(&preview_display_web_stream_menu)?;
    preview_display_menu.append(&PredefinedMenuItem::separator(app)?)?;
    preview_display_menu.append(&CheckMenuItem::with_id(
        app,
        "preview_display_freeze",
        "Freeze",
        true,
        false,
        None::<&str>,
    )?)?;
    preview_display_menu.append(&CheckMenuItem::with_id(
        app,
        "preview_display_blank",
        "Blank",
        true,
        false,
        None::<&str>,
    )?)?;
    preview_display_menu.append(&PredefinedMenuItem::separator(app)?)?;

    let preview_display_presets_menu = Submenu::new(app, "Presets", true)?;
    preview_display_presets_menu.append(&CheckMenuItem::with_id(
        app,
        "preview_display_preset_image_only",
        "Image Only",
        true,
        true,
        None::<&str>,
    )?)?;
    preview_display_presets_menu.append(&CheckMenuItem::with_id(
        app,
        "preview_display_preset_client_review",
        "Client Review",
        true,
        false,
        None::<&str>,
    )?)?;
    preview_display_presets_menu.append(&CheckMenuItem::with_id(
        app,
        "preview_display_preset_metadata_review",
        "Metadata Review",
        true,
        false,
        None::<&str>,
    )?)?;
    preview_display_menu.append(&preview_display_presets_menu)?;

    let preview_display_fields_menu = Submenu::new(app, "Metadata Fields", true)?;
    preview_display_fields_menu.append(&CheckMenuItem::with_id(
        app,
        "preview_display_field_filename",
        "Show Filename",
        true,
        false,
        None::<&str>,
    )?)?;
    preview_display_fields_menu.append(&CheckMenuItem::with_id(
        app,
        "preview_display_field_rating",
        "Show Rating",
        true,
        false,
        None::<&str>,
    )?)?;
    preview_display_fields_menu.append(&CheckMenuItem::with_id(
        app,
        "preview_display_field_decision",
        "Show Decision",
        true,
        false,
        None::<&str>,
    )?)?;
    preview_display_fields_menu.append(&CheckMenuItem::with_id(
        app,
        "preview_display_field_dimensions",
        "Show Dimensions",
        true,
        false,
        None::<&str>,
    )?)?;
    preview_display_fields_menu.append(&CheckMenuItem::with_id(
        app,
        "preview_display_field_format",
        "Show Format",
        true,
        false,
        None::<&str>,
    )?)?;
    preview_display_fields_menu.append(&CheckMenuItem::with_id(
        app,
        "preview_display_field_source",
        "Show Source",
        true,
        false,
        None::<&str>,
    )?)?;
    preview_display_fields_menu.append(&CheckMenuItem::with_id(
        app,
        "preview_display_field_prompt",
        "Show Prompt",
        true,
        false,
        None::<&str>,
    )?)?;
    preview_display_fields_menu.append(&CheckMenuItem::with_id(
        app,
        "preview_display_field_tags",
        "Show Tags",
        true,
        false,
        None::<&str>,
    )?)?;
    preview_display_fields_menu.append(&CheckMenuItem::with_id(
        app,
        "preview_display_field_histogram",
        "Show Histogram",
        true,
        false,
        None::<&str>,
    )?)?;
    preview_display_menu.append(&preview_display_fields_menu)?;

    let preview_display_info_rail_menu = Submenu::new(app, "Info Rail", true)?;
    preview_display_info_rail_menu.append(&CheckMenuItem::with_id(
        app,
        "preview_display_rail_left",
        "Left",
        true,
        false,
        None::<&str>,
    )?)?;
    preview_display_info_rail_menu.append(&CheckMenuItem::with_id(
        app,
        "preview_display_rail_right",
        "Right",
        true,
        true,
        None::<&str>,
    )?)?;
    preview_display_info_rail_menu.append(&PredefinedMenuItem::separator(app)?)?;
    preview_display_info_rail_menu.append(&CheckMenuItem::with_id(
        app,
        "preview_display_rail_width_narrow",
        "Narrow",
        true,
        false,
        None::<&str>,
    )?)?;
    preview_display_info_rail_menu.append(&CheckMenuItem::with_id(
        app,
        "preview_display_rail_width_medium",
        "Medium",
        true,
        true,
        None::<&str>,
    )?)?;
    preview_display_info_rail_menu.append(&CheckMenuItem::with_id(
        app,
        "preview_display_rail_width_wide",
        "Wide",
        true,
        false,
        None::<&str>,
    )?)?;
    preview_display_info_rail_menu.append(&PredefinedMenuItem::separator(app)?)?;
    preview_display_info_rail_menu.append(&CheckMenuItem::with_id(
        app,
        "preview_display_text_small",
        "Small Text",
        true,
        false,
        None::<&str>,
    )?)?;
    preview_display_info_rail_menu.append(&CheckMenuItem::with_id(
        app,
        "preview_display_text_medium",
        "Medium Text",
        true,
        true,
        None::<&str>,
    )?)?;
    preview_display_info_rail_menu.append(&CheckMenuItem::with_id(
        app,
        "preview_display_text_large",
        "Large Text",
        true,
        false,
        None::<&str>,
    )?)?;
    preview_display_menu.append(&preview_display_info_rail_menu)?;
    view_menu.append(&preview_display_menu)?;
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
    view_menu.append(&MenuItem::with_id(
        app,
        "fit_in",
        "Fit In",
        true,
        None::<&str>,
    )?)?;
    view_menu.append(&PredefinedMenuItem::separator(app)?)?;
    view_menu.append(&PredefinedMenuItem::fullscreen(app, None)?)?;
    menu.append(&view_menu)?;

    // Window menu
    let window_menu = Submenu::with_id(app, WINDOW_MENU_ID, "Window", true)?;
    window_menu.append(&PredefinedMenuItem::minimize(app, None)?)?;
    window_menu.append(&PredefinedMenuItem::maximize(app, Some("Zoom"))?)?;
    window_menu.append(&PredefinedMenuItem::separator(app)?)?;
    window_menu.append(&PredefinedMenuItem::bring_all_to_front(app, None)?)?;
    window_menu.append(&PredefinedMenuItem::separator(app)?)?;
    append_window_menu_items(app, &window_menu)?;
    menu.append(&window_menu)?;

    // Help menu
    let help_menu = Submenu::with_id(app, HELP_SUBMENU_ID, "Help", true)?;
    help_menu.append(&MenuItem::with_id(
        app,
        "help",
        "Cull User Guide",
        true,
        None::<&str>,
    )?)?;
    help_menu.append(&MenuItem::with_id(
        app,
        AGENT_SKILLS_ID,
        "Install Agent Skills...",
        true,
        None::<&str>,
    )?)?;
    help_menu.append(&PredefinedMenuItem::separator(app)?)?;
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
    show_loupe_histogram: bool,
    #[serde(default)]
    preview_display_frozen: bool,
    #[serde(default)]
    preview_display_blanked: bool,
    #[serde(default)]
    preview_display_always_on_top: bool,
    #[serde(default = "default_preview_display_mode")]
    preview_display_mode: String,
    #[serde(default)]
    preview_display_overlay: crate::preview::state::PreviewOverlayConfig,
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
    set_menu_item_checked(&app, "view_loupe_histogram", state.show_loupe_histogram)?;
    set_menu_item_checked(&app, "preview_display_freeze", state.preview_display_frozen)?;
    set_menu_item_checked(&app, "preview_display_blank", state.preview_display_blanked)?;
    set_menu_item_checked(
        &app,
        "preview_display_always_on_top",
        state.preview_display_always_on_top,
    )?;
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
    set_menu_item_checked(
        &app,
        "preview_display_field_filename",
        state.preview_display_overlay.show_filename,
    )?;
    set_menu_item_checked(
        &app,
        "preview_display_field_rating",
        state.preview_display_overlay.show_rating,
    )?;
    set_menu_item_checked(
        &app,
        "preview_display_field_decision",
        state.preview_display_overlay.show_decision,
    )?;
    set_menu_item_checked(
        &app,
        "preview_display_field_dimensions",
        state.preview_display_overlay.show_dimensions,
    )?;
    set_menu_item_checked(
        &app,
        "preview_display_field_format",
        state.preview_display_overlay.show_format,
    )?;
    set_menu_item_checked(
        &app,
        "preview_display_field_source",
        state.preview_display_overlay.show_source,
    )?;
    set_menu_item_checked(
        &app,
        "preview_display_field_prompt",
        state.preview_display_overlay.show_prompt,
    )?;
    set_menu_item_checked(
        &app,
        "preview_display_field_tags",
        state.preview_display_overlay.show_tags,
    )?;
    set_menu_item_checked(
        &app,
        "preview_display_field_histogram",
        state.preview_display_overlay.show_histogram,
    )?;
    set_menu_item_checked(
        &app,
        "preview_display_rail_left",
        state.preview_display_overlay.rail_side == crate::preview::state::PreviewRailSide::Left,
    )?;
    set_menu_item_checked(
        &app,
        "preview_display_rail_right",
        state.preview_display_overlay.rail_side == crate::preview::state::PreviewRailSide::Right,
    )?;
    set_menu_item_checked(
        &app,
        "preview_display_rail_width_narrow",
        state.preview_display_overlay.rail_width == crate::preview::state::PreviewRailWidth::Narrow,
    )?;
    set_menu_item_checked(
        &app,
        "preview_display_rail_width_medium",
        state.preview_display_overlay.rail_width == crate::preview::state::PreviewRailWidth::Medium,
    )?;
    set_menu_item_checked(
        &app,
        "preview_display_rail_width_wide",
        state.preview_display_overlay.rail_width == crate::preview::state::PreviewRailWidth::Wide,
    )?;
    set_menu_item_checked(
        &app,
        "preview_display_text_small",
        state.preview_display_overlay.rail_text_size
            == crate::preview::state::PreviewRailTextSize::Small,
    )?;
    set_menu_item_checked(
        &app,
        "preview_display_text_medium",
        state.preview_display_overlay.rail_text_size
            == crate::preview::state::PreviewRailTextSize::Medium,
    )?;
    set_menu_item_checked(
        &app,
        "preview_display_text_large",
        state.preview_display_overlay.rail_text_size
            == crate::preview::state::PreviewRailTextSize::Large,
    )?;
    set_menu_item_enabled(
        &app,
        "preview_display_start_web_stream",
        !state.preview_display_web_stream_active,
    )?;
    set_menu_item_enabled(
        &app,
        "preview_display_start_lan_web_stream",
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

#[derive(Debug, Clone, PartialEq, Eq)]
struct WindowMenuEntry {
    label: String,
    title: String,
    focused: bool,
}

fn window_menu_item_id(label: &str) -> String {
    format!("{}{}", WINDOW_MENU_FOCUS_PREFIX, label)
}

fn window_label_from_menu_item_id(id: &str) -> Option<&str> {
    id.strip_prefix(WINDOW_MENU_FOCUS_PREFIX)
}

fn window_menu_title(label: &str, title: String) -> String {
    let trimmed = title.trim();
    if trimmed.is_empty() {
        label.to_string()
    } else {
        trimmed.to_string()
    }
}

fn window_menu_rank(label: &str) -> u8 {
    if label == "main" {
        0
    } else if label == crate::preview::window::PREVIEW_DISPLAY_LABEL {
        1
    } else {
        2
    }
}

fn sort_window_menu_entries(entries: &mut [WindowMenuEntry]) {
    entries.sort_by(|a, b| {
        window_menu_rank(&a.label)
            .cmp(&window_menu_rank(&b.label))
            .then_with(|| a.title.to_lowercase().cmp(&b.title.to_lowercase()))
            .then_with(|| a.label.cmp(&b.label))
    });
}

fn current_window_menu_entries(app: &AppHandle) -> Vec<WindowMenuEntry> {
    let mut entries = app
        .webview_windows()
        .into_iter()
        .map(|(label, window)| WindowMenuEntry {
            title: window_menu_title(&label, window.title().unwrap_or_else(|_| label.clone())),
            focused: window.is_focused().unwrap_or(false),
            label,
        })
        .collect::<Vec<_>>();
    sort_window_menu_entries(&mut entries);
    entries
}

fn append_window_menu_items(app: &AppHandle, window_menu: &Submenu<Wry>) -> tauri::Result<()> {
    for entry in current_window_menu_entries(app) {
        let item = CheckMenuItem::with_id(
            app,
            window_menu_item_id(&entry.label),
            entry.title,
            true,
            entry.focused,
            None::<&str>,
        )?;
        window_menu.append(&item)?;
    }
    Ok(())
}

fn remove_window_menu_items(window_menu: &Submenu<Wry>) -> Result<(), String> {
    let children = window_menu.items().map_err(|e| e.to_string())?;
    for child in &children {
        if window_label_from_menu_item_id(child.id().0.as_str()).is_none() {
            continue;
        }
        match child {
            MenuItemKind::MenuItem(item) => window_menu.remove(item),
            MenuItemKind::Check(item) => window_menu.remove(item),
            MenuItemKind::Icon(item) => window_menu.remove(item),
            MenuItemKind::Submenu(item) => window_menu.remove(item),
            MenuItemKind::Predefined(item) => window_menu.remove(item),
        }
        .map_err(|e| format!("Failed to remove Window menu item: {}", e))?;
    }
    Ok(())
}

pub fn refresh_window_menu(app: &AppHandle) -> Result<(), String> {
    let Some(MenuItemKind::Submenu(window_menu)) = find_menu_item(app, WINDOW_MENU_ID)? else {
        return Ok(());
    };
    remove_window_menu_items(&window_menu)?;
    append_window_menu_items(app, &window_menu)
        .map_err(|e| format!("Failed to refresh Window menu: {}", e))
}

fn focus_window_from_menu(app: &AppHandle, label: &str) {
    let Some(window) = app.get_webview_window(label) else {
        let _ = refresh_window_menu(app);
        return;
    };
    let _ = window.show();
    let _ = window.unminimize();
    let _ = window.set_focus();
    let _ = refresh_window_menu(app);
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
    if let Some(label) = window_label_from_menu_item_id(id) {
        focus_window_from_menu(app, label);
        return;
    }
    match id {
        "help" => show_cull_help(app),
        "about"
        | "check_update"
        | "open_file"
        | "import_folder"
        | "open_folder"
        | "settings"
        | "undo"
        | "redo"
        | "deselect_all"
        | "command_palette"
        | "image_share"
        | "image_open_default"
        | "image_open_with"
        | "image_reveal"
        | "image_rename"
        | "image_move_to"
        | "image_trash"
        | "view_grid"
        | "view_compare"
        | "view_loupe"
        | "view_canvas"
        | "view_lineage"
        | "view_embeddings"
        | "view_publish"
        | "view_export"
        | "view_tinder"
        | "toggle_sidebar"
        | "view_loupe_histogram"
        | "view_preview_display"
        | "preview_display_move_monitor"
        | "preview_display_fullscreen"
        | "preview_display_always_on_top"
        | "preview_display_start_web_stream"
        | "preview_display_start_lan_web_stream"
        | "preview_display_copy_web_stream_url"
        | "preview_display_stop_web_stream"
        | "preview_display_freeze"
        | "preview_display_blank"
        | "preview_display_preset_image_only"
        | "preview_display_preset_client_review"
        | "preview_display_preset_metadata_review"
        | "preview_display_field_filename"
        | "preview_display_field_rating"
        | "preview_display_field_decision"
        | "preview_display_field_dimensions"
        | "preview_display_field_format"
        | "preview_display_field_source"
        | "preview_display_field_prompt"
        | "preview_display_field_tags"
        | "preview_display_field_histogram"
        | "preview_display_rail_left"
        | "preview_display_rail_right"
        | "preview_display_rail_width_narrow"
        | "preview_display_rail_width_medium"
        | "preview_display_rail_width_wide"
        | "preview_display_text_small"
        | "preview_display_text_medium"
        | "preview_display_text_large"
        | "zoom_in"
        | "zoom_out"
        | "actual_size"
        | "fit_in"
        | AGENT_SKILLS_ID
        | "github_wiki" => {
            let _ = app.emit("menu-action", id);
        }
        _ => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn window_menu_ids_round_trip_labels() {
        let id = window_menu_item_id("preview-display");

        assert_eq!(id, "window_focus:preview-display");
        assert_eq!(window_label_from_menu_item_id(&id), Some("preview-display"));
        assert_eq!(window_label_from_menu_item_id("view_grid"), None);
    }

    #[test]
    fn window_menu_title_falls_back_to_label() {
        assert_eq!(
            window_menu_title("preview-display", "Cull Preview Display".to_string()),
            "Cull Preview Display"
        );
        assert_eq!(window_menu_title("window-2", "   ".to_string()), "window-2");
    }

    #[test]
    fn window_menu_entries_sort_main_then_preview_then_named_windows() {
        let mut entries = vec![
            WindowMenuEntry {
                label: "window-3".to_string(),
                title: "Zed".to_string(),
                focused: false,
            },
            WindowMenuEntry {
                label: crate::preview::window::PREVIEW_DISPLAY_LABEL.to_string(),
                title: "Cull Preview Display".to_string(),
                focused: false,
            },
            WindowMenuEntry {
                label: "window-2".to_string(),
                title: "Alpha".to_string(),
                focused: false,
            },
            WindowMenuEntry {
                label: "main".to_string(),
                title: "Cull".to_string(),
                focused: true,
            },
        ];

        sort_window_menu_entries(&mut entries);

        assert_eq!(
            entries
                .iter()
                .map(|entry| entry.label.as_str())
                .collect::<Vec<_>>(),
            vec!["main", "preview-display", "window-2", "window-3"]
        );
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
