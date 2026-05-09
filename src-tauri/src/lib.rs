mod commands;
mod db_core;
mod export;
mod menu;

use std::path::PathBuf;
use std::sync::Mutex;
use tauri::{Manager, Emitter, Listener};
use tauri_plugin_dialog::DialogExt;
use crate::db_core::db::Database;
use crate::db_core::detection::DetectionEngine;
use crate::db_core::embeddings::EmbeddingEngine;
use crate::commands::deeplink::parse_deep_link;

pub struct AppState {
    pub db: Database,
    pub app_data_dir: PathBuf,
    pub embedding_engine: Mutex<EmbeddingEngine>,
    pub detection_engine: Mutex<DetectionEngine>,
    pub safety_engine: Mutex<DetectionEngine>,
}

const IMAGE_EXTENSIONS: &[&str] = &[
    "jpg", "jpeg", "png", "gif", "webp", "bmp", "tiff", "tif",
    "heic", "heif", "avif", "svg", "ico",
    "cr2", "cr3", "nef", "arw", "dng", "orf", "raf", "rw2", "psd",
];

fn is_image_path(path: &std::path::Path) -> bool {
    path.extension()
        .and_then(|ext| ext.to_str())
        .map(|ext| IMAGE_EXTENSIONS.contains(&ext.to_lowercase().as_str()))
        .unwrap_or(false)
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_deep_link::init())
        .plugin(tauri_plugin_updater::Builder::new().build())
        .plugin(tauri_plugin_process::init())
        .plugin(tauri_plugin_single_instance::init(|app, args, _cwd| {
            if let Some(w) = app.get_webview_window("main") {
                let _ = w.set_focus();
            }
            for arg in &args {
                if arg.starts_with("imageview://") {
                    let params = parse_deep_link(arg);
                    let _ = app.emit("open-with-params", params);
                }
            }
        }))
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
            let detection_engine = Mutex::new(DetectionEngine::new_yolo(&model_dir));
            let safety_engine = Mutex::new(DetectionEngine::new_nudenet(&model_dir));

            app.manage(AppState { db, app_data_dir, embedding_engine, detection_engine, safety_engine });

            // Set up native menu bar
            let handle = app.handle();
            let app_menu = menu::create_menu(handle)?;
            app.set_menu(app_menu)?;
            let menu_handle = app.handle().clone();
            app.on_menu_event(move |_app, event| {
                menu::handle_menu_event(&menu_handle, &event);
            });

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
            commands::import::regenerate_thumbnails,
            commands::library::list_images,
            commands::library::get_image_count,
            commands::library::get_images_by_ids,
            commands::library::get_iteration_siblings,
            commands::library::list_folders,
            commands::library::list_images_by_folder,
            commands::library::delete_folder,
            commands::library::list_images_filtered,
            commands::library::trash_images,
            commands::library::delete_images_permanently,
            commands::library::get_app_setting,
            commands::library::set_app_setting,
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
            commands::embeddings::set_api_key,
            commands::embeddings::get_api_key,
            commands::embeddings::validate_api_key,
            commands::embeddings::generate_gemini_embeddings,
            commands::window::create_window,
            commands::window::list_windows,
            commands::window::rename_window,
            commands::window::send_to_window,
            commands::detection::download_yolo_model,
            commands::detection::download_nudenet_model,
            commands::detection::detect_objects,
            commands::detection::detect_nsfw,
            commands::detection::get_detections,
            commands::detection::search_by_detected_class,
            commands::detection::is_yolo_available,
            commands::detection::is_nudenet_available,
            commands::detection::get_detection_count,
            commands::smart_collections::create_smart_collection,
            commands::smart_collections::list_smart_collections,
            commands::smart_collections::evaluate_smart_collection,
            commands::smart_collections::delete_smart_collection,
            commands::smart_collections::update_smart_collection,
            commands::smart_collections::parse_nl_query,
            commands::smart_collections::backfill_image_metadata,
            commands::vision::check_ollama,
            commands::vision::set_ollama_config,
            commands::vision::get_ollama_config,
            commands::vision::analyze_images,
            commands::vision::get_vision_metadata,
            commands::vision::get_vision_count,
            commands::export::create_export_manifest,
            commands::export::validate_export_manifest,
            commands::export::apply_export_patches,
            commands::export::list_export_presets,
            commands::export::get_export_asset,
            commands::export::save_export_image,
            commands::export::assemble_export_pdf,
        ])
        .build(tauri::generate_context!())
        .expect("error while building tauri application")
        .run(|app, event| {
            // Handle files opened via Finder "Open With"
            #[cfg(target_os = "macos")]
            if let tauri::RunEvent::Opened { urls } = &event {
                let file_paths: Vec<String> = urls
                    .iter()
                    .filter_map(|url| {
                        if url.scheme() == "file" {
                            url.to_file_path().ok().map(|p| p.to_string_lossy().into_owned())
                        } else {
                            None
                        }
                    })
                    .collect();

                if !file_paths.is_empty() {
                    let params = crate::commands::deeplink::OpenParams {
                        path: if file_paths.len() == 1 { Some(file_paths[0].clone()) } else { None },
                        paths: if file_paths.len() > 1 { Some(file_paths) } else { None },
                        folder: None,
                        view: Some("loupe".to_string()),
                        size: None,
                        zoom: None,
                        fullscreen: None,
                        focus: None,
                        gap: None,
                    };
                    let _ = app.emit("open-with-params", params);
                }

                for url in urls {
                    if url.scheme() == "imageview" {
                        let params = parse_deep_link(url.as_str());
                        let _ = app.emit("open-with-params", params);
                    }
                }
            }

            // Handle drag-and-drop from Finder
            if let tauri::RunEvent::WindowEvent { event: tauri::WindowEvent::DragDrop(ref drag_event), .. } = event {
                match drag_event {
                    tauri::DragDropEvent::Enter { paths, .. } => {
                        let has_images = paths.iter().any(|p| is_image_path(p));
                        let has_dirs = paths.iter().any(|p| p.is_dir());
                        if has_images || has_dirs {
                            let _ = app.emit("drag-hover", true);
                        }
                    }
                    tauri::DragDropEvent::Leave => {
                        let _ = app.emit("drag-hover", false);
                    }
                    tauri::DragDropEvent::Drop { paths, .. } => {
                        let _ = app.emit("drag-hover", false);

                        let dirs: Vec<&PathBuf> = paths.iter().filter(|p| p.is_dir()).collect();
                        let files: Vec<String> = paths.iter()
                            .filter(|p| !p.is_dir() && is_image_path(p))
                            .map(|p| p.to_string_lossy().into_owned())
                            .collect();

                        if dirs.len() == 1 && files.is_empty() {
                            let params = crate::commands::deeplink::OpenParams {
                                path: None,
                                paths: None,
                                folder: Some(dirs[0].to_string_lossy().into_owned()),
                                view: Some("grid".to_string()),
                                size: None,
                                zoom: None,
                                fullscreen: None,
                                focus: None,
                                gap: None,
                            };
                            let _ = app.emit("open-with-params", params);
                        } else if !files.is_empty() {
                            let params = crate::commands::deeplink::OpenParams {
                                path: if files.len() == 1 { Some(files[0].clone()) } else { None },
                                paths: if files.len() > 1 { Some(files.clone()) } else { None },
                                folder: None,
                                view: Some(if files.len() == 1 { "loupe" } else { "grid" }.to_string()),
                                size: None,
                                zoom: None,
                                fullscreen: None,
                                focus: None,
                                gap: None,
                            };
                            let _ = app.emit("open-with-params", params);
                        }

                        for dir in &dirs {
                            if !files.is_empty() || dirs.len() > 1 {
                                let params = crate::commands::deeplink::OpenParams {
                                    path: None,
                                    paths: None,
                                    folder: Some(dir.to_string_lossy().into_owned()),
                                    view: None,
                                    size: None,
                                    zoom: None,
                                    fullscreen: None,
                                    focus: None,
                                    gap: None,
                                };
                                let _ = app.emit("open-with-params", params);
                            }
                        }
                    }
                    _ => {}
                }
            }

            drop(event);
        });
}
