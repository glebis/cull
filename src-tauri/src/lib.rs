mod commands;
mod db_core;

use std::path::PathBuf;
use std::sync::Mutex;
use tauri::{Manager, Emitter, Listener};
use tauri_plugin_dialog::DialogExt;
use crate::db_core::db::Database;
use crate::db_core::embeddings::EmbeddingEngine;
use crate::commands::deeplink::parse_deep_link;

pub struct AppState {
    pub db: Database,
    pub app_data_dir: PathBuf,
    pub embedding_engine: Mutex<EmbeddingEngine>,
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_deep_link::init())
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

            let model_dir = app_data_dir.join("models");
            let embedding_engine = Mutex::new(EmbeddingEngine::new(&model_dir));

            app.manage(AppState { db, app_data_dir, embedding_engine });

            // Handle deep link URLs that launched the app
            #[cfg(any(target_os = "macos", target_os = "linux", target_os = "windows"))]
            {
                let handle = app.handle().clone();
                app.listen("deep-link://new-url", move |event: tauri::Event| {
                    if let Ok(urls) = serde_json::from_str::<Vec<String>>(event.payload()) {
                        for url in urls {
                            let params = parse_deep_link(&url);
                            let _ = handle.emit("open-with-params", params);
                        }
                    }
                });
            }

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::import::import_folder,
            commands::import::import_files,
            commands::library::list_images,
            commands::library::get_image_count,
            commands::library::get_images_by_ids,
            commands::library::get_iteration_siblings,
            commands::library::list_folders,
            commands::library::list_images_by_folder,
            commands::library::delete_folder,
            commands::library::list_images_filtered,
            commands::selection::set_rating,
            commands::selection::set_decision,
            commands::deeplink::open_with_params,
            commands::collections::create_collection,
            commands::collections::list_collections,
            commands::collections::add_to_collection,
            commands::collections::list_collection_images,
            commands::collections::delete_collection,
            commands::embeddings::generate_embeddings,
            commands::embeddings::get_all_embeddings,
            commands::embeddings::find_similar_images,
            commands::embeddings::download_clip_model,
            commands::embeddings::is_model_available,
            commands::embeddings::get_embedding_count,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
