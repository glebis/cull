// Copyright (c) 2025-present Gleb Kalinin. Architecture and design by author.
// Implementation assisted by Claude (Anthropic). See AUTHORSHIP.md.

mod cli;
mod commands;
mod db_core;
mod dictation;
mod export;
mod mcp;
mod menu;
mod services;
mod tray;
mod cloud;
pub mod extensions;
pub mod raw;
mod watcher;

use std::path::PathBuf;
use std::panic::AssertUnwindSafe;
use parking_lot::Mutex;
use tauri::{AppHandle, Manager, Emitter, Listener};
use tauri_plugin_dialog::DialogExt;
use crate::db_core::db::Database;
use crate::db_core::detection::DetectionEngine;
use crate::db_core::embeddings::EmbeddingEngine;
use crate::db_core::secrets::{SecretStore, KeychainStore};
use crate::commands::deeplink::parse_deep_link;

pub struct AppState {
    pub db: Database,
    pub app_data_dir: PathBuf,
    pub embedding_engine: Mutex<EmbeddingEngine>,
    pub detection_engine: Mutex<DetectionEngine>,
    pub safety_engine: Mutex<DetectionEngine>,
    pub secrets: Box<dyn SecretStore>,
    pub jobs: crate::services::jobs::JobRegistry,
    pub action_manager: services::undo::ActionManager,
    pub file_watcher: Mutex<watcher::FileWatcher>,
}

fn install_panic_hook(app: AppHandle) {
    let prev = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        let thread = std::thread::current().name().unwrap_or("unnamed").to_string();
        let msg = if let Some(s) = info.payload().downcast_ref::<&str>() {
            s.to_string()
        } else if let Some(s) = info.payload().downcast_ref::<String>() {
            s.clone()
        } else {
            "unknown panic".to_string()
        };
        let loc = info.location().map(|l| format!("{}:{}", l.file(), l.line()));
        eprintln!("[panic] thread={} location={:?} message={}", thread, loc, msg);
        let _ = app.emit("rust-panic", serde_json::json!({
            "thread": thread, "location": loc, "message": msg
        }));
        prev(info);
    }));
}

pub fn spawn_guarded<F, Fut>(app: AppHandle, task_name: &'static str, fut: F)
where
    F: FnOnce() -> Fut + Send + 'static,
    Fut: std::future::Future<Output = ()> + Send,
{
    tokio::spawn(async move {
        let result = futures_util::FutureExt::catch_unwind(AssertUnwindSafe(fut())).await;
        if let Err(payload) = result {
            let msg = if let Some(s) = payload.downcast_ref::<&str>() {
                s.to_string()
            } else if let Some(s) = payload.downcast_ref::<String>() {
                s.clone()
            } else {
                "unknown panic".to_string()
            };
            eprintln!("[guarded-spawn] task={} panicked: {}", task_name, msg);
            let _ = app.emit("background-task-failed", serde_json::json!({
                "task": task_name, "message": msg, "recoverable": true
            }));
        }
    });
}


fn run_stdio_bridge() {
    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("Failed to create tokio runtime");

    rt.block_on(async {
        let app_data_dir = dirs::data_dir()
            .expect("No data dir")
            .join("com.glebkalinin.cull");

        let sock_path = mcp::socket::socket_path(&app_data_dir);

        let stream = match tokio::net::UnixStream::connect(&sock_path).await {
            Ok(s) => s,
            Err(_) => {
                eprintln!("MCP socket not found, launching app in tray mode...");
                let exe = std::env::current_exe().expect("Can't find own executable");
                std::process::Command::new(&exe)
                    .arg("--tray")
                    .spawn()
                    .expect("Failed to launch app in tray mode");

                let mut attempts = 0;
                loop {
                    tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;
                    match tokio::net::UnixStream::connect(&sock_path).await {
                        Ok(s) => break s,
                        Err(_) if attempts < 20 => { attempts += 1; }
                        Err(e) => {
                            eprintln!("Failed to connect to MCP socket after 10s: {}", e);
                            std::process::exit(1);
                        }
                    }
                }
            }
        };

        let (mut sock_read, mut sock_write) = tokio::io::split(stream);
        let mut stdin = tokio::io::stdin();
        let mut stdout = tokio::io::stdout();

        tokio::select! {
            r = tokio::io::copy(&mut stdin, &mut sock_write) => {
                if let Err(e) = r { eprintln!("stdin->socket: {}", e); }
            }
            r = tokio::io::copy(&mut sock_read, &mut stdout) => {
                if let Err(e) = r { eprintln!("socket->stdout: {}", e); }
            }
        }
    });
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let args = <cli::CliArgs as clap::Parser>::parse();
    let start_hidden = args.tray;

    if args.mcp_stdio {
        run_stdio_bridge();
        return;
    }

    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_deep_link::init())
        .plugin(tauri_plugin_updater::Builder::new().build())
        .plugin(tauri_plugin_process::init())
        .plugin(tauri_plugin_single_instance::init(|app, args, _cwd| {
            eprintln!("[single-instance] Second instance args: {:?}", args);
            if let Some(w) = app.get_webview_window("main") {
                let _ = w.set_focus();
            }
            for arg in &args {
                if arg.starts_with("cull://") {
                    eprintln!("[single-instance] Forwarding deep link: {}", arg);
                    let params = parse_deep_link(arg);
                    let _ = app.emit("open-with-params", params);
                }
            }
        }))
        .setup(move |app| {
            install_panic_hook(app.handle().clone());

            let app_data_dir = app.path().app_data_dir()
                .map_err(|e| format!("failed to get app data dir: {}", e))?;
            std::fs::create_dir_all(&app_data_dir)
                .map_err(|e| format!("failed to create app data dir: {}", e))?;

            let db_path = app_data_dir.join("cull.db");
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

            let secrets = Box::new(KeychainStore::new("cull"));
            let jobs = crate::services::jobs::JobRegistry::default();
            app.manage(AppState { db, app_data_dir, embedding_engine, detection_engine, safety_engine, secrets, jobs, action_manager: services::undo::ActionManager::new(), file_watcher: Mutex::new(watcher::FileWatcher::new()) });

            // Load persisted job history from DB
            {
                let state: tauri::State<'_, AppState> = app.state();
                state.jobs.load_from_db(&state.db);
            }

            // Start file watcher
            {
                let state: tauri::State<'_, AppState> = app.state();
                let roots = state.db.list_library_roots().unwrap_or_default();
                let db_clone = state.db.clone();
                let app_handle_clone = app.handle().clone();
                let data_dir_clone = state.app_data_dir.clone();
                let mut fw = state.file_watcher.lock();
                let module_raw = state.db.get_setting("module_raw")
                    .ok().flatten().map(|v| v == "true").unwrap_or(false);
                fw.module_raw.store(module_raw, std::sync::atomic::Ordering::Relaxed);
                if let Err(e) = fw.start(db_clone, app_handle_clone, roots, data_dir_clone) {
                    eprintln!("[watcher] Failed to start: {}", e);
                }
            }

            // Set up native menu bar
            let handle = app.handle();
            let app_menu = menu::create_menu(handle)?;
            app.set_menu(app_menu)?;
            let menu_handle = app.handle().clone();
            app.on_menu_event(move |_app, event| {
                menu::handle_menu_event(&menu_handle, &event);
            });

            // Start MCP socket server
            mcp::server::start_mcp_server(app.handle().clone());

            // Start MCP HTTP server if enabled via CLI or settings
            {
                let http_enabled = args.mcp_http.is_some() || {
                    let state: tauri::State<'_, AppState> = app.state();
                    state.db.get_setting("mcp_http_enabled")
                        .ok()
                        .flatten()
                        .map(|v| v == "true")
                        .unwrap_or(false)
                };
                if http_enabled {
                    let port = args.mcp_http.flatten().unwrap_or(9847);
                    let host = args.mcp_http_host.clone();
                    mcp::http::start_http_server(app.handle().clone(), host, port);
                }
            }

            // Set up system tray
            tray::setup_tray(app.handle())?;

            // Hide window if --tray mode
            if start_hidden {
                if let Some(w) = app.get_webview_window("main") {
                    let _ = w.hide();
                }
            }

            // Handle deep link URLs that launched the app
            #[cfg(any(target_os = "macos", target_os = "linux", target_os = "windows"))]
            {
                let handle = app.handle().clone();
                app.listen("deep-link://new-url", move |event: tauri::Event| {
                    eprintln!("[deep-link] Rust received deep-link://new-url: {}", event.payload());
                    if let Ok(urls) = serde_json::from_str::<Vec<String>>(event.payload()) {
                        for url in urls {
                            eprintln!("[deep-link] Processing URL: {}", url);
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
            commands::import::regenerate_thumbnails_by_ids,
            commands::import::regenerate_single_thumbnail,
            commands::import::rescan_sources,
            commands::jobs::get_job,
            commands::jobs::list_jobs,
            commands::jobs::cancel_job,
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
            commands::library::check_library_health,
            commands::selection::set_rating,
            commands::selection::set_decision,
            commands::deeplink::open_with_params,
            commands::collections::create_collection,
            commands::collections::list_collections,
            commands::collections::add_to_collection,
            commands::collections::list_collection_images,
            commands::collections::remove_from_collection,
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
            commands::embeddings::delete_api_key,
            commands::embeddings::has_api_key,
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
            commands::lineage::list_lineage_groups,
            commands::lineage::get_lineage_group_images,
            commands::lineage::create_lineage_group_manual,
            commands::lineage::rename_lineage_group,
            commands::lineage::merge_lineage_groups,
            commands::lineage::dissolve_lineage_group,
            commands::lineage::add_to_lineage_group,
            commands::lineage::remove_from_lineage_group,
            commands::lineage::get_batch_images,
            commands::lineage::scan_lineage,
            commands::lineage::get_generation_run,
            // commands::lineage::rescan_sidecars, // TODO: not yet implemented
            commands::mcp::create_mcp_token,
            commands::mcp::list_mcp_tokens,
            commands::mcp::revoke_mcp_token,
            commands::mcp::rotate_mcp_token,
            commands::transform::crop_image,
            commands::transform::rotate_image,
            commands::dictation::start_dictation,
            commands::dictation::stop_dictation,
            commands::undo::undo,
            commands::undo::redo,
            commands::undo::get_undo_status,
            commands::undo::list_undo_history,
            commands::generation::resubmit_prompt,
            commands::generation::estimate_generation_cost,
            commands::sessions::create_session,
            commands::sessions::list_sessions,
            commands::sessions::get_session,
            commands::sessions::delete_session,
            commands::sessions::convert_session_to_collection,
            commands::sessions::validate_session_folder,
            commands::sessions::create_canvas,
            commands::sessions::list_canvases,
            commands::sessions::update_canvas_layout,
            commands::sessions::delete_canvas,
            commands::files::move_image,
            commands::files::rename_image,
            commands::files::create_subfolder,
            commands::raw::backfill_raw_previews,
            commands::privacy::get_data_flow_status,
            commands::privacy::get_api_audit_log,
            commands::privacy::export_audit_log,
        ])
        .build(tauri::generate_context!())
        .expect("error while building tauri application")
        .run(|app, event| {
            // Handle close-to-tray
            if let tauri::RunEvent::WindowEvent {
                event: tauri::WindowEvent::CloseRequested { api, .. },
                label,
                ..
            } = &event {
                if label == "main" {
                    let close_to_tray = app.state::<AppState>().db
                        .get_setting("close_to_tray")
                        .ok()
                        .flatten()
                        .map(|v| v == "true")
                        .unwrap_or(true);

                    if close_to_tray {
                        api.prevent_close();
                        if let Some(w) = app.get_webview_window("main") {
                            let _ = w.hide();
                        }
                    }
                }
            }

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
                    if url.scheme() == "cull" {
                        let params = parse_deep_link(url.as_str());
                        let _ = app.emit("open-with-params", params);
                    }
                }
            }

            // Handle drag-and-drop from Finder
            if let tauri::RunEvent::WindowEvent { event: tauri::WindowEvent::DragDrop(ref drag_event), .. } = event {
                match drag_event {
                    tauri::DragDropEvent::Enter { paths, .. } => {
                        let has_images = paths.iter().any(|p| crate::extensions::is_image_path(p, false));
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
                            .filter(|p| !p.is_dir() && crate::extensions::is_image_path(p, false))
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
