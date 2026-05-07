mod commands;
mod db_core;

use std::path::PathBuf;
use tauri::Manager;
use tauri_plugin_dialog::DialogExt;
use crate::db_core::db::Database;

pub struct AppState {
    pub db: Database,
    pub app_data_dir: PathBuf,
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .setup(|app| {
            let app_data_dir = app.path().app_data_dir()
                .map_err(|e| format!("failed to get app data dir: {}", e))?;
            std::fs::create_dir_all(&app_data_dir)
                .map_err(|e| format!("failed to create app data dir: {}", e))?;

            let db_path = app_data_dir.join("imageview.db");
            let db = match Database::open(&db_path) {
                Ok(db) => db,
                Err(e) => {
                    let msg = format!(
                        "Failed to open database at {}:\n{}\n\nThe database file may be corrupted. \
                         You can delete it and restart to start fresh.",
                        db_path.display(), e
                    );
                    app.dialog()
                        .message(msg)
                        .title("Database Error")
                        .blocking_show();
                    return Err(format!("database open failed: {}", e).into());
                }
            };

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
