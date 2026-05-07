mod commands;
mod core;

use std::path::PathBuf;
use tauri::Manager;
use crate::core::db::Database;

pub struct AppState {
    pub db: Database,
    pub app_data_dir: PathBuf,
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .setup(|app| {
            let app_data_dir = app.path().app_data_dir().expect("failed to get app data dir");
            std::fs::create_dir_all(&app_data_dir).ok();

            let db_path = app_data_dir.join("imageview.db");
            let db = Database::open(&db_path).expect("failed to open database");

            app.manage(AppState { db, app_data_dir });
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::import::import_folder,
            commands::import::import_files,
            commands::library::list_images,
            commands::library::get_image_count,
            commands::selection::set_rating,
            commands::selection::set_decision,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
