// Copyright (c) 2026-present Gleb Kalinin. Architecture and design by author.
// Implementation assisted by Claude (Anthropic). See AUTHORSHIP.md.

mod cli;
mod cloud;
mod commands;
#[cfg(test)]
mod config_contract;
mod db_core;
mod dictation;
pub mod exchange;
mod export;
pub mod extensions;
mod logging;
mod mcp;
mod menu;
mod plugins;
pub mod preview;
pub mod raw;
mod services;
mod tray;
mod watcher;

/// Test-only surface for integration tests (which see only the public API).
/// Gated behind the `test-support` Cargo feature so it widens nothing in normal
/// builds. See `tests/compat_golden.rs`.
#[cfg(feature = "test-support")]
pub mod test_support {
    pub use crate::db_core::db::Database;
}

use crate::commands::deeplink::{
    emit_open_params, open_params_for_drag_drop_paths, open_params_for_file_paths,
    open_params_for_urls, parse_deep_link,
};
use crate::db_core::db::Database;
use crate::db_core::detection::DetectionEngine;
use crate::db_core::embeddings::EmbeddingEngine;
use crate::db_core::secrets::{KeychainStore, SecretStore};
use crate::preview::state::PreviewStateStore;
use crate::preview::web_stream::PreviewWebStreamController;
use parking_lot::Mutex;
use std::panic::AssertUnwindSafe;
use std::path::PathBuf;
use tauri::{AppHandle, Emitter, Listener, Manager};
use tauri_plugin_dialog::DialogExt;

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
    pub clipboard_monitor: Mutex<services::clipboard_monitor::ClipboardMonitorState>,
    pub static_publish_server: Mutex<commands::static_publishing::StaticPublishServerState>,
    pub preview_state: PreviewStateStore,
    pub preview_web_stream: PreviewWebStreamController,
    pub agent_snapshots: Mutex<services::agent_snapshots::AgentSnapshotRegistry>,
    pub agent_snapshot_requests: Mutex<
        std::collections::HashMap<
            String,
            tokio::sync::oneshot::Sender<services::agent_snapshots::AgentSnapshotPackage>,
        >,
    >,
}

fn install_panic_hook(app: AppHandle) {
    let prev = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        let thread = std::thread::current()
            .name()
            .unwrap_or("unnamed")
            .to_string();
        let msg = if let Some(s) = info.payload().downcast_ref::<&str>() {
            s.to_string()
        } else if let Some(s) = info.payload().downcast_ref::<String>() {
            s.clone()
        } else {
            "unknown panic".to_string()
        };
        let loc = info
            .location()
            .map(|l| format!("{}:{}", l.file(), l.line()));
        crate::safe_eprintln!(
            "[panic] thread={} location={:?} message={}",
            thread,
            loc,
            msg
        );
        let _ = app.emit(
            "rust-panic",
            serde_json::json!({
                "thread": thread, "location": loc, "message": msg
            }),
        );
        let _ = std::panic::catch_unwind(AssertUnwindSafe(|| prev(info)));
    }));
}

pub fn spawn_guarded<F, Fut>(app: AppHandle, task_name: &'static str, fut: F)
where
    F: FnOnce() -> Fut + Send + 'static,
    Fut: std::future::Future<Output = ()> + Send,
{
    tauri::async_runtime::spawn(async move {
        let result = futures_util::FutureExt::catch_unwind(AssertUnwindSafe(fut())).await;
        if let Err(payload) = result {
            let msg = if let Some(s) = payload.downcast_ref::<&str>() {
                s.to_string()
            } else if let Some(s) = payload.downcast_ref::<String>() {
                s.clone()
            } else {
                "unknown panic".to_string()
            };
            crate::safe_eprintln!("[guarded-spawn] task={} panicked: {}", task_name, msg);
            let _ = app.emit(
                "background-task-failed",
                serde_json::json!({
                    "task": task_name, "message": msg, "recoverable": true
                }),
            );
        }
    });
}

pub(crate) fn reveal_main_window(app: &AppHandle) {
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.show();
        let _ = window.unminimize();
        let _ = window.set_focus();
    }
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
                crate::safe_eprintln!("MCP socket not found, launching app in tray mode...");
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
                        Err(_) if attempts < 20 => {
                            attempts += 1;
                        }
                        Err(e) => {
                            crate::safe_eprintln!(
                                "Failed to connect to MCP socket after 10s: {}",
                                e
                            );
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
                if let Err(e) = r { crate::safe_eprintln!("stdin->socket: {}", e); }
            }
            r = tokio::io::copy(&mut sock_read, &mut stdout) => {
                if let Err(e) = r { crate::safe_eprintln!("socket->stdout: {}", e); }
            }
        }
    });
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let args = <cli::CliArgs as clap::Parser>::parse();
    let start_hidden = args.tray;

    if let Some(code) = cli::run_headless_if_requested(&args) {
        std::process::exit(code);
    }

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
            crate::safe_eprintln!("[single-instance] Second instance args: {:?}", args);
            reveal_main_window(app);
            let mut file_paths = Vec::new();
            for arg in &args {
                if arg.starts_with("cull://") {
                    crate::safe_eprintln!("[single-instance] Forwarding deep link: {}", arg);
                    match parse_deep_link(arg) {
                        Ok(params) => { let _ = emit_open_params(app, params); }
                        Err(e) => crate::safe_eprintln!("[single-instance] Deep link rejected: {}", e),
                    }
                } else {
                    let path = PathBuf::from(arg);
                    if path.is_file() && crate::extensions::is_image_path(&path, false) {
                        file_paths.push(path.to_string_lossy().into_owned());
                    }
                }
            }
            if let Some(params) = open_params_for_file_paths(file_paths) {
                let _ = emit_open_params(app, params);
            }
        }))
        .setup(move |app| {
            install_panic_hook(app.handle().clone());

            let app_data_dir = app
                .path()
                .app_data_dir()
                .map_err(|e| format!("failed to get app data dir: {}", e))?;
            std::fs::create_dir_all(&app_data_dir)
                .map_err(|e| format!("failed to create app data dir: {}", e))?;

            let db_path = app_data_dir.join("cull.db");
            let db = match Database::open(&db_path) {
                Ok(db) => db,
                Err(e) => {
                    let msg = format!(
                        "Failed to open database at {}:\n{}\n\nCull did not modify the database. \
                         Back up this file before attempting repair, then run an integrity check or restore a known-good backup.",
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
            app.manage(AppState {
                db,
                app_data_dir,
                embedding_engine,
                detection_engine,
                safety_engine,
                secrets,
                jobs,
                action_manager: services::undo::ActionManager::new(),
                file_watcher: Mutex::new(watcher::FileWatcher::new()),
                clipboard_monitor: Mutex::new(
                    services::clipboard_monitor::ClipboardMonitorState::default(),
                ),
                static_publish_server: Mutex::new(
                    commands::static_publishing::StaticPublishServerState::default(),
                ),
                preview_state: PreviewStateStore::default(),
                preview_web_stream: PreviewWebStreamController::default(),
                agent_snapshots: Mutex::new(services::agent_snapshots::AgentSnapshotRegistry::default()),
                agent_snapshot_requests: Mutex::new(std::collections::HashMap::new()),
            });

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
                let module_raw = crate::db_core::import::is_module_raw_enabled(&state.db);
                fw.module_raw
                    .store(module_raw, std::sync::atomic::Ordering::Relaxed);
                if let Err(e) = fw.start(db_clone, app_handle_clone, roots, data_dir_clone) {
                    crate::safe_eprintln!("[watcher] Failed to start: {}", e);
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
                    state
                        .db
                        .get_setting("mcp_http_enabled")
                        .ok()
                        .flatten()
                        .map(|v| v == "true")
                        .unwrap_or(false)
                };
                if http_enabled {
                    let state: tauri::State<'_, AppState> = app.state();
                    let saved_port = state
                        .db
                        .get_setting("mcp_http_port")
                        .ok()
                        .flatten()
                        .and_then(|v| v.parse::<u16>().ok());
                    let saved_host = state.db.get_setting("mcp_http_host").ok().flatten();
                    let allow_remote = args.mcp_http_allow_remote
                        || state
                            .db
                            .get_setting("mcp_http_allow_remote")
                            .ok()
                            .flatten()
                            .map(|v| v == "true")
                            .unwrap_or(false);
                    let port = args.mcp_http.flatten().or(saved_port).unwrap_or(9847);
                    let host = if args.mcp_http_host != "127.0.0.1" {
                        args.mcp_http_host.clone()
                    } else {
                        saved_host.unwrap_or_else(|| args.mcp_http_host.clone())
                    };
                    mcp::http::start_http_server(app.handle().clone(), host, port, allow_remote);
                }
            }

            // Set up system tray
            tray::setup_tray(app.handle())?;

            // Apply persisted app icon after tray creation so the window, Dock, and tray stay in sync.
            {
                let state: tauri::State<'_, AppState> = app.state();
                let icon_variant = state
                    .db
                    .get_setting("app_icon_variant")
                    .ok()
                    .flatten()
                    .unwrap_or_else(|| commands::window::DEFAULT_ICON_VARIANT.to_string());
                if let Err(e) =
                    commands::window::apply_app_icon_variant_to_app(app.handle(), &icon_variant)
                {
                    crate::safe_eprintln!("[icon] Failed to apply app icon variant: {}", e);
                }
            }

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
                    crate::safe_eprintln!(
                        "[deep-link] Rust received deep-link://new-url: {}",
                        event.payload()
                    );
                    if let Ok(urls) = serde_json::from_str::<Vec<String>>(event.payload()) {
                        for params in open_params_for_urls(&urls) {
                            let _ = emit_open_params(&handle, params);
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
            commands::deeplink::drain_pending_open_params,
            commands::deeplink::open_deep_link_urls,
            commands::jobs::get_job,
            commands::jobs::list_jobs,
            commands::jobs::cancel_job,
            commands::jobs::pause_job,
            commands::jobs::resume_job,
            commands::media::list_media_assets,
            commands::media::get_media_asset,
            commands::media::get_media_asset_for_image,
            commands::media::list_media_files,
            commands::media::list_pdf_pages,
            commands::catalog::list_catalog_presets,
            commands::catalog::get_catalog_preset,
            commands::catalog::list_catalog_fields,
            commands::catalog::create_catalog_field_def,
            commands::catalog::deprecate_catalog_field_def,
            commands::catalog::create_catalog_preset,
            commands::catalog::update_catalog_preset,
            commands::catalog::create_catalog_work,
            commands::catalog::attach_images_to_catalog_work,
            commands::catalog::list_catalog_values,
            commands::catalog::list_catalog_drafts,
            commands::catalog::get_catalog_record,
            commands::catalog::set_catalog_draft_value,
            commands::catalog::set_catalog_draft_values,
            commands::catalog::suggest_catalog_values,
            commands::catalog::get_catalog_suggestion_job,
            commands::catalog::approve_catalog_values,
            commands::catalog::reject_catalog_values,
            commands::clipboard_monitor::get_clipboard_monitor_status,
            commands::clipboard_monitor::start_clipboard_monitor,
            commands::clipboard_monitor::stop_clipboard_monitor,
            commands::clipboard_monitor::set_clipboard_monitor_capture_dir,
            commands::clipboard_monitor::set_clipboard_monitor_capture_existing_on_start,
            commands::clipboard_monitor::move_clipboard_capture_folder,
            commands::clipboard_monitor::publish_clipboard_collection,
            commands::library::list_images,
            commands::library::get_image_count,
            commands::library::list_image_ids,
            commands::library::get_images_by_ids,
            commands::library::get_image_by_path,
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
            commands::selection::set_client_feedback,
            commands::selection::get_client_feedback,
            commands::selection::list_client_feedback,
            commands::deeplink::open_with_params,
            commands::collections::create_collection,
            commands::collections::list_collections,
            commands::collections::add_to_collection,
            commands::collections::list_collection_images,
            commands::collections::remove_from_collection,
            commands::collections::delete_collection,
            commands::embeddings::generate_embeddings,
            commands::embeddings::generate_model_embeddings,
            commands::embeddings::get_embedding_page,
            commands::embeddings::find_similar_images,
            commands::embeddings::generate_similarity_groups,
            commands::embeddings::list_similarity_groups,
            commands::embeddings::list_similarity_group_images,
            commands::embeddings::get_clip_model_download_info,
            commands::embeddings::get_embedding_model_download_info,
            commands::embeddings::list_embedding_providers,
            commands::embeddings::check_ollama_embedding,
            commands::embeddings::get_ollama_embedding_config,
            commands::embeddings::set_ollama_embedding_config,
            commands::embeddings::download_clip_model,
            commands::embeddings::download_embedding_model,
            commands::embeddings::is_model_available,
            commands::embeddings::is_embedding_model_available,
            commands::embeddings::get_embedding_count,
            commands::embeddings::set_api_key,
            commands::embeddings::validate_api_key,
            commands::embeddings::delete_api_key,
            commands::embeddings::has_api_key,
            commands::embeddings::generate_gemini_embeddings,
            commands::window::create_window,
            commands::window::list_windows,
            commands::window::rename_window,
            commands::window::send_to_window,
            commands::window::apply_app_icon_variant,
            commands::detection::download_yolo_model,
            commands::detection::download_nudenet_model,
            commands::detection::detect_objects,
            commands::detection::detect_nsfw,
            commands::detection::get_detections,
            commands::detection::search_by_detected_class,
            commands::detection::count_by_detected_class,
            commands::detection::list_images_by_detected_class,
            commands::detection::is_yolo_available,
            commands::detection::is_nudenet_available,
            commands::detection::get_detection_count,
            commands::quality::analyze_image_quality,
            commands::quality::get_image_quality,
            commands::quality::get_quality_count,
            commands::color::analyze_image_colors,
            commands::color::get_image_color_metrics,
            commands::color::get_color_metrics_count,
            commands::color::list_images_by_color_bucket,
            commands::perceptual_hash::analyze_perceptual_hashes,
            commands::perceptual_hash::get_image_perceptual_hash,
            commands::perceptual_hash::get_perceptual_hash_count,
            commands::perceptual_hash::find_near_duplicates_by_phash,
            commands::tags::backfill_image_tags,
            commands::tags::list_image_tags,
            commands::tags::list_tags,
            commands::smart_collections::create_smart_collection,
            commands::smart_collections::list_smart_collections,
            commands::smart_collections::evaluate_smart_collection,
            commands::smart_collections::count_smart_collection,
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
            commands::ocr::start_ocr_batch,
            commands::export::create_export_manifest,
            commands::export::validate_export_manifest,
            commands::export::apply_export_patches,
            commands::export::list_export_presets,
            commands::export::export_images_to_folder,
            commands::export::get_export_asset,
            commands::export::save_export_image,
            commands::export::save_png_to_path,
            commands::export::save_text_to_path,
            commands::export::assemble_export_pdf,
            commands::exchange::export_cull_exchange,
            commands::exchange::preview_cull_exchange_import,
            commands::exchange::import_cull_exchange,
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
            commands::lineage::rescan_sidecars,
            commands::plugins::plugin_invoke,
            commands::plugins::load_installed_plugins,
            commands::plugins::fetch_plugin_registry,
            commands::plugins::install_plugin,
            commands::plugins::uninstall_plugin,
            commands::plugins::list_installed_plugin_info,
            commands::mcp::create_mcp_token,
            commands::mcp::list_mcp_tokens,
            commands::mcp::revoke_mcp_token,
            commands::mcp::rotate_mcp_token,
            commands::mcp::get_mcp_audit_log,
            commands::static_publishing::export_static_publish_package,
            commands::static_publishing::serve_static_publish_package,
            commands::static_publishing::stop_static_publish_server,
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
            commands::sessions::list_session_events,
            commands::sessions::get_activity_context,
            commands::sessions::get_session,
            commands::sessions::delete_session,
            commands::sessions::convert_session_to_collection,
            commands::sessions::validate_session_folder,
            commands::sessions::create_canvas,
            commands::sessions::list_canvases,
            commands::sessions::update_canvas_layout,
            commands::sessions::delete_canvas,
            commands::files::copy_image_to_clipboard,
            commands::files::paste_image_from_clipboard,
            commands::files::move_image,
            commands::files::rename_image,
            commands::files::create_subfolder,
            commands::files::share_images,
            commands::files::open_images_with_application,
            commands::files::list_open_with_applications,
            commands::agent_snapshots::capture_agent_window_snapshot,
            commands::agent_snapshots::complete_agent_view_snapshot,
            commands::agent_snapshots::get_last_agent_view_snapshot,
            commands::agent_snapshots::request_agent_view_snapshot,
            menu::update_menu_state,
            commands::raw::backfill_raw_previews,
            commands::privacy::get_data_flow_status,
            commands::privacy::get_api_audit_log,
            commands::privacy::export_audit_log,
            commands::preview::get_preview_state,
            commands::preview::update_preview_state,
            commands::preview::open_preview_display,
            commands::preview::set_preview_display_always_on_top,
            commands::preview::list_preview_display_monitors,
            commands::preview::place_preview_display,
            commands::preview::start_preview_display_web_stream,
            commands::preview::stop_preview_display_web_stream,
            commands::preview::get_preview_display_web_stream_status,
            commands::preview::get_image_histogram,
            commands::diagnostics::record_asset_load_event,
            commands::diagnostics::get_asset_load_events,
        ])
        .build(tauri::generate_context!())
        .expect("error while building tauri application")
        .run(|app, event| {
            // Handle close-to-tray
            if let tauri::RunEvent::WindowEvent {
                event: tauri::WindowEvent::CloseRequested { api, .. },
                label,
                ..
            } = &event
            {
                if label == "main" {
                    let close_to_tray = app
                        .state::<AppState>()
                        .db
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

            #[cfg(target_os = "macos")]
            if let tauri::RunEvent::Reopen { .. } = &event {
                reveal_main_window(app);
            }

            // Handle files opened via Finder "Open With"
            #[cfg(target_os = "macos")]
            if let tauri::RunEvent::Opened { urls } = &event {
                reveal_main_window(app);
                let file_paths: Vec<String> = urls
                    .iter()
                    .filter_map(|url| {
                        if url.scheme() == "file" {
                            url.to_file_path()
                                .ok()
                                .map(|p| p.to_string_lossy().into_owned())
                        } else {
                            None
                        }
                    })
                    .collect();

                if let Some(params) = open_params_for_file_paths(file_paths) {
                    let _ = emit_open_params(app, params);
                }

                for url in urls {
                    if url.scheme() == "cull" {
                        match parse_deep_link(url.as_str()) {
                            Ok(params) => { let _ = emit_open_params(app, params); }
                            Err(e) => crate::safe_eprintln!("[deep-link] Deep link rejected: {}", e),
                        }
                    }
                }
            }

            // Handle drag-and-drop from Finder
            if let tauri::RunEvent::WindowEvent {
                event: tauri::WindowEvent::DragDrop(ref drag_event),
                ..
            } = event
            {
                match drag_event {
                    tauri::DragDropEvent::Enter { paths, .. } => {
                        if !open_params_for_drag_drop_paths(paths).is_empty() {
                            let _ = app.emit("drag-hover", true);
                        }
                    }
                    tauri::DragDropEvent::Leave => {
                        let _ = app.emit("drag-hover", false);
                    }
                    tauri::DragDropEvent::Drop { paths, .. } => {
                        let _ = app.emit("drag-hover", false);

                        for params in open_params_for_drag_drop_paths(paths) {
                            let _ = emit_open_params(app, params);
                        }
                    }
                    _ => {}
                }
            }

            if let tauri::RunEvent::WindowEvent {
                event:
                    tauri::WindowEvent::Destroyed | tauri::WindowEvent::Focused(_),
                ..
            } = &event
            {
                let _ = menu::refresh_window_menu(app);
            }

            drop(event);
        });
}
