use clap::{Parser, Subcommand};
use serde_json::Value;
use std::path::PathBuf;

mod context;
mod output;
mod tools;

use context::HeadlessContext;

pub fn resolve_launch_path(path: &std::path::Path, cwd: &std::path::Path) -> PathBuf {
    if path.is_absolute() {
        path.to_path_buf()
    } else {
        cwd.join(path)
    }
}

#[derive(Parser, Debug, Clone)]
#[command(name = "cull")]
pub struct CliArgs {
    /// Emit machine-readable JSON for headless commands
    #[arg(long, short = 'j', global = true)]
    pub json: bool,

    /// Use a specific SQLite database instead of the default
    #[arg(long, global = true)]
    pub db: Option<PathBuf>,

    /// Use a specific app data directory for thumbnails/exports
    #[arg(long, global = true)]
    pub app_data_dir: Option<PathBuf>,

    /// Start in tray-only mode (no window)
    #[arg(long)]
    pub tray: bool,

    /// Run as MCP stdio bridge
    #[arg(long)]
    pub mcp_stdio: bool,

    /// Enable MCP HTTP/SSE server on optional port (default: 9847)
    #[arg(long)]
    pub mcp_http: Option<Option<u16>>,

    /// HTTP listen host (default: 127.0.0.1)
    #[arg(long, default_value = "127.0.0.1")]
    pub mcp_http_host: String,

    /// Permit MCP HTTP to bind to a non-loopback host. Use only with scoped tokens.
    #[arg(long)]
    pub mcp_http_allow_remote: bool,

    /// Open an image or import a folder in the GUI
    #[arg(value_name = "PATH")]
    pub launch_path: Option<PathBuf>,

    #[command(subcommand)]
    pub command: Option<CliCommand>,
}

#[derive(Subcommand, Debug, Clone)]
pub enum CliCommand {
    /// Call a supported MCP-named tool with JSON params
    #[command(name = "call_tool")]
    CallTool {
        tool_name: String,
        #[arg(long = "params_json", visible_alias = "params-json")]
        params_json: Option<String>,
        #[arg(long = "params_file", visible_alias = "params-file")]
        params_file: Option<PathBuf>,
    },

    #[command(name = "get_library_stats")]
    GetLibraryStats,

    #[command(name = "list_images")]
    ListImages {
        #[arg(long, default_value_t = 0)]
        offset: u32,
        #[arg(long, default_value_t = 50)]
        limit: u32,
    },

    #[command(name = "list_folders")]
    ListFolders,

    #[command(name = "list_collections")]
    ListCollections,

    #[command(name = "import_folder")]
    ImportFolder {
        #[arg(long = "folder_path", visible_alias = "folder-path")]
        folder_path: String,
    },

    #[command(name = "import_files")]
    ImportFiles {
        #[arg(
            long = "file_paths",
            visible_alias = "file-path",
            value_delimiter = ','
        )]
        file_paths: Vec<String>,
    },

    #[command(name = "list_export_presets")]
    ListExportPresets,

    #[command(name = "export_images")]
    ExportImages {
        #[arg(long = "image_ids", visible_alias = "image-id", value_delimiter = ',')]
        image_ids: Vec<String>,
        #[arg(long = "collection_id", visible_alias = "collection-id")]
        collection_id: Option<String>,
        #[arg(long = "folder_path", visible_alias = "folder-path")]
        folder_path: Option<String>,
        #[arg(long = "output_dir", visible_alias = "output-dir")]
        output_dir: String,
        #[arg(long)]
        format: Option<String>,
        #[arg(long, default_value_t = true)]
        flatten: bool,
        #[arg(long)]
        naming: Option<String>,
    },

    #[command(name = "get_embedding_model_download_info")]
    GetEmbeddingModelDownloadInfo {
        #[arg(long, default_value = "clip-vit-b32")]
        model: String,
    },

    #[command(name = "download_embedding_model")]
    DownloadEmbeddingModel {
        #[arg(long, default_value = "clip-vit-b32")]
        model: String,
    },

    #[command(name = "generate_embeddings")]
    GenerateEmbeddings {
        #[arg(long, default_value = "clip-vit-b32")]
        model: String,
        #[arg(long = "image_ids", visible_alias = "image-id", value_delimiter = ',')]
        image_ids: Vec<String>,
    },

    #[command(name = "analyze_image_quality")]
    AnalyzeImageQuality {
        #[arg(long = "image_ids", visible_alias = "image-id", value_delimiter = ',')]
        image_ids: Vec<String>,
        #[arg(long)]
        all: bool,
    },

    #[command(name = "get_image_quality")]
    GetImageQuality {
        #[arg(long = "image_id", visible_alias = "image-id")]
        image_id: String,
    },

    #[command(name = "get_quality_count")]
    GetQualityCount,

    /// Search images by an object class already detected in the library
    #[command(name = "search_by_object")]
    SearchByObject {
        #[arg(long = "class_name", visible_alias = "class-name")]
        class_name: String,
        #[arg(long)]
        limit: Option<u32>,
    },

    /// Set a 0-5 star rating on a library image
    #[command(name = "set_rating")]
    SetRating {
        #[arg(long = "image_id", visible_alias = "image-id")]
        image_id: String,
        #[arg(long, value_parser = clap::value_parser!(u8).range(0..=5))]
        rating: u8,
    },
}

pub fn run_headless_if_requested(args: &CliArgs) -> Option<i32> {
    args.command.as_ref()?;
    Some(match execute_headless(args) {
        Ok(value) => {
            output::print_success(args.json, &value);
            0
        }
        Err(message) => {
            output::print_error(args.json, &message);
            1
        }
    })
}

fn execute_headless(args: &CliArgs) -> Result<Value, String> {
    let ctx = HeadlessContext::from_args(args)?;

    match args.command.as_ref().expect("checked by caller") {
        CliCommand::CallTool {
            tool_name,
            params_json,
            params_file,
        } => {
            let params = output::load_params(params_json.as_deref(), params_file.as_deref())?;
            tools::execute_named_tool(&ctx, tool_name, params)
        }
        CliCommand::GetLibraryStats => {
            tools::execute_named_tool(&ctx, "get_library_stats", serde_json::json!({}))
        }
        CliCommand::ListImages { offset, limit } => tools::execute_named_tool(
            &ctx,
            "list_images",
            serde_json::json!({ "offset": offset, "limit": limit }),
        ),
        CliCommand::ListFolders => {
            tools::execute_named_tool(&ctx, "list_folders", serde_json::json!({}))
        }
        CliCommand::ListCollections => {
            tools::execute_named_tool(&ctx, "list_collections", serde_json::json!({}))
        }
        CliCommand::ImportFolder { folder_path } => tools::execute_named_tool(
            &ctx,
            "import_folder",
            serde_json::json!({ "folder_path": folder_path }),
        ),
        CliCommand::ImportFiles { file_paths } => tools::execute_named_tool(
            &ctx,
            "import_files",
            serde_json::json!({ "file_paths": file_paths }),
        ),
        CliCommand::ListExportPresets => {
            tools::execute_named_tool(&ctx, "list_export_presets", serde_json::json!({}))
        }
        CliCommand::ExportImages {
            image_ids,
            collection_id,
            folder_path,
            output_dir,
            format,
            flatten,
            naming,
        } => tools::execute_named_tool(
            &ctx,
            "export_images",
            serde_json::json!({
                "image_ids": if image_ids.is_empty() { None::<Vec<String>> } else { Some(image_ids.clone()) },
                "collection_id": collection_id,
                "folder_path": folder_path,
                "output_dir": output_dir,
                "format": format,
                "flatten": flatten,
                "naming": naming,
            }),
        ),
        CliCommand::GetEmbeddingModelDownloadInfo { model } => tools::execute_named_tool(
            &ctx,
            "get_embedding_model_download_info",
            serde_json::json!({ "model": model }),
        ),
        CliCommand::DownloadEmbeddingModel { model } => tools::execute_named_tool(
            &ctx,
            "download_embedding_model",
            serde_json::json!({ "model": model }),
        ),
        CliCommand::GenerateEmbeddings { model, image_ids } => tools::execute_named_tool(
            &ctx,
            "generate_embeddings",
            serde_json::json!({ "model": model, "image_ids": image_ids }),
        ),
        CliCommand::AnalyzeImageQuality { image_ids, all } => tools::execute_named_tool(
            &ctx,
            "analyze_image_quality",
            serde_json::json!({
                "image_ids": if image_ids.is_empty() { None::<Vec<String>> } else { Some(image_ids.clone()) },
                "all": all,
            }),
        ),
        CliCommand::GetImageQuality { image_id } => tools::execute_named_tool(
            &ctx,
            "get_image_quality",
            serde_json::json!({ "image_id": image_id }),
        ),
        CliCommand::GetQualityCount => {
            tools::execute_named_tool(&ctx, "get_quality_count", serde_json::json!({}))
        }
        CliCommand::SearchByObject { class_name, limit } => tools::execute_named_tool(
            &ctx,
            "search_by_object",
            serde_json::json!({ "class_name": class_name, "limit": limit }),
        ),
        CliCommand::SetRating { image_id, rating } => tools::execute_named_tool(
            &ctx,
            "set_rating",
            serde_json::json!({ "image_id": image_id, "rating": rating }),
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::Parser;

    #[test]
    fn test_default_args() {
        let args = CliArgs::try_parse_from(["cull"]).unwrap();
        assert!(!args.tray);
        assert!(!args.mcp_stdio);
        assert!(args.mcp_http.is_none());
        assert_eq!(args.mcp_http_host, "127.0.0.1");
        assert!(!args.mcp_http_allow_remote);
    }

    #[test]
    fn test_tray_flag() {
        let args = CliArgs::try_parse_from(["cull", "--tray"]).unwrap();
        assert!(args.tray);
        assert!(!args.mcp_stdio);
    }

    #[test]
    fn test_mcp_stdio_flag() {
        let args = CliArgs::try_parse_from(["cull", "--mcp-stdio"]).unwrap();
        assert!(args.mcp_stdio);
        assert!(!args.tray);
    }

    #[test]
    fn test_mcp_http_no_port() {
        let args = CliArgs::try_parse_from(["cull", "--mcp-http"]).unwrap();
        assert!(args.mcp_http.is_some());
        assert_eq!(args.mcp_http.unwrap(), None);
    }

    #[test]
    fn test_mcp_http_with_port() {
        let args = CliArgs::try_parse_from(["cull", "--mcp-http", "8080"]).unwrap();
        assert_eq!(args.mcp_http, Some(Some(8080)));
    }

    #[test]
    fn test_mcp_http_host_custom() {
        let args = CliArgs::try_parse_from(["cull", "--mcp-http-host", "0.0.0.0"]).unwrap();
        assert_eq!(args.mcp_http_host, "0.0.0.0");
        assert!(!args.mcp_http_allow_remote);
    }

    #[test]
    fn test_mcp_http_allow_remote_flag_is_explicit() {
        let args = CliArgs::try_parse_from([
            "cull",
            "--mcp-http",
            "--mcp-http-host",
            "0.0.0.0",
            "--mcp-http-allow-remote",
        ])
        .unwrap();
        assert_eq!(args.mcp_http_host, "0.0.0.0");
        assert!(args.mcp_http_allow_remote);
    }

    #[test]
    fn test_combined_flags() {
        let args = CliArgs::try_parse_from([
            "cull",
            "--tray",
            "--mcp-http",
            "9847",
            "--mcp-http-host",
            "0.0.0.0",
            "--mcp-http-allow-remote",
        ])
        .unwrap();
        assert!(args.tray);
        assert_eq!(args.mcp_http, Some(Some(9847)));
        assert_eq!(args.mcp_http_host, "0.0.0.0");
        assert!(args.mcp_http_allow_remote);
    }

    #[test]
    fn test_unknown_flag_errors() {
        let result = CliArgs::try_parse_from(["cull", "--bogus"]);
        assert!(result.is_err());
    }

    #[test]
    fn test_positional_folder_path_is_a_gui_launch_target() {
        let args = CliArgs::try_parse_from(["cull", "/tmp/photos"]).unwrap();
        assert_eq!(args.launch_path, Some(PathBuf::from("/tmp/photos")));
        assert!(args.command.is_none());
    }

    #[test]
    fn test_option_directory_is_not_mistaken_for_a_gui_launch_target() {
        let args = CliArgs::try_parse_from(["cull", "--app-data-dir", "/tmp/cull-data"]).unwrap();
        assert!(args.launch_path.is_none());
    }

    #[test]
    fn test_relative_launch_path_resolves_against_invoking_working_directory() {
        assert_eq!(
            resolve_launch_path(
                PathBuf::from("photos").as_path(),
                PathBuf::from("/caller").as_path()
            ),
            PathBuf::from("/caller/photos")
        );
    }

    #[test]
    fn test_import_folder_subcommand() {
        let args = CliArgs::try_parse_from([
            "cull",
            "--json",
            "import_folder",
            "--folder_path",
            "/tmp/in",
        ])
        .unwrap();
        assert!(args.json);
        match args.command {
            Some(CliCommand::ImportFolder { folder_path }) => assert_eq!(folder_path, "/tmp/in"),
            other => panic!("expected import_folder command, got {:?}", other),
        }
    }

    #[test]
    fn test_call_tool_subcommand() {
        let args = CliArgs::try_parse_from([
            "cull",
            "call_tool",
            "import_folder",
            "--params_json",
            r#"{"folder_path":"/tmp/in"}"#,
        ])
        .unwrap();
        match args.command {
            Some(CliCommand::CallTool {
                tool_name,
                params_json,
                ..
            }) => {
                assert_eq!(tool_name, "import_folder");
                assert_eq!(
                    params_json,
                    Some(r#"{"folder_path":"/tmp/in"}"#.to_string())
                );
            }
            other => panic!("expected call_tool command, got {:?}", other),
        }
    }

    #[test]
    fn test_download_embedding_model_subcommand() {
        let args = CliArgs::try_parse_from([
            "cull",
            "--json",
            "download_embedding_model",
            "--model",
            "dinov2-vits14",
        ])
        .unwrap();
        match args.command {
            Some(CliCommand::DownloadEmbeddingModel { model }) => {
                assert_eq!(model, "dinov2-vits14")
            }
            other => panic!("expected download_embedding_model command, got {:?}", other),
        }
    }

    #[test]
    fn test_generate_embeddings_subcommand_accepts_model() {
        let args = CliArgs::try_parse_from([
            "cull",
            "generate_embeddings",
            "--model",
            "dinov2-vits14",
            "--image_ids",
            "img1,img2",
        ])
        .unwrap();
        match args.command {
            Some(CliCommand::GenerateEmbeddings { model, image_ids }) => {
                assert_eq!(model, "dinov2-vits14");
                assert_eq!(image_ids, vec!["img1".to_string(), "img2".to_string()]);
            }
            other => panic!("expected generate_embeddings command, got {:?}", other),
        }
    }

    #[test]
    fn test_analyze_image_quality_subcommand_accepts_all() {
        let args = CliArgs::try_parse_from(["cull", "analyze_image_quality", "--all"]).unwrap();
        match args.command {
            Some(CliCommand::AnalyzeImageQuality { image_ids, all }) => {
                assert!(all);
                assert!(image_ids.is_empty());
            }
            other => panic!("expected analyze_image_quality command, got {:?}", other),
        }
    }

    #[test]
    fn test_analyze_image_quality_subcommand_accepts_ids() {
        let args =
            CliArgs::try_parse_from(["cull", "analyze_image_quality", "--image_ids", "img1,img2"])
                .unwrap();
        match args.command {
            Some(CliCommand::AnalyzeImageQuality { image_ids, all }) => {
                assert!(!all);
                assert_eq!(image_ids, vec!["img1".to_string(), "img2".to_string()]);
            }
            other => panic!("expected analyze_image_quality command, got {:?}", other),
        }
    }

    #[test]
    fn test_set_rating_subcommand_accepts_id_and_rating() {
        let args =
            CliArgs::try_parse_from(["cull", "set_rating", "--image_id", "img1", "--rating", "4"])
                .unwrap();
        match args.command {
            Some(CliCommand::SetRating { image_id, rating }) => {
                assert_eq!(image_id, "img1");
                assert_eq!(rating, 4);
            }
            other => panic!("expected set_rating command, got {:?}", other),
        }
    }

    #[test]
    fn test_set_rating_subcommand_rejects_out_of_range_rating() {
        assert!(CliArgs::try_parse_from([
            "cull",
            "set_rating",
            "--image_id",
            "img1",
            "--rating",
            "6",
        ])
        .is_err());
    }

    #[test]
    fn test_search_by_object_subcommand_accepts_mcp_field_names() {
        let args = CliArgs::try_parse_from([
            "cull",
            "search_by_object",
            "--class_name",
            "person",
            "--limit",
            "12",
        ])
        .unwrap();

        match args.command {
            Some(CliCommand::SearchByObject { class_name, limit }) => {
                assert_eq!(class_name, "person");
                assert_eq!(limit, Some(12));
            }
            other => panic!("expected search_by_object command, got {:?}", other),
        }
    }

    #[test]
    fn test_search_by_object_typed_dispatch_reads_temporary_database() {
        let tmp = tempfile::tempdir().unwrap();
        let db_path = tmp.path().join("cull.db");
        let db = crate::db_core::db::Database::open(&db_path).unwrap();
        db.conn
            .lock()
            .execute(
                "INSERT INTO images (id, sha256_hash, width, height, format, file_size, created_at, imported_at)
                 VALUES ('img1', 'hash-img1', 100, 100, 'png', 1000, '2026-01-01', '2026-01-01')",
                [],
            )
            .unwrap();
        db.store_detections(
            "img1",
            "yolo11m",
            &[crate::db_core::detection::Detection {
                class_name: "person".to_string(),
                confidence: 0.88,
                x: 0.0,
                y: 0.0,
                width: 1.0,
                height: 1.0,
            }],
        )
        .unwrap();
        drop(db);

        let args = CliArgs::try_parse_from([
            "cull",
            "--db",
            db_path.to_str().unwrap(),
            "--app-data-dir",
            tmp.path().to_str().unwrap(),
            "search_by_object",
            "--class_name",
            "person",
        ])
        .unwrap();

        let result = execute_headless(&args).unwrap();
        let matches = result.as_array().unwrap();
        assert_eq!(matches.len(), 1);
        assert_eq!(matches[0]["image_id"], "img1");
        assert!((matches[0]["confidence"].as_f64().unwrap() - 0.88).abs() < 0.000_001);
    }

    #[test]
    fn test_set_rating_typed_dispatch_updates_temporary_database() {
        let tmp = tempfile::tempdir().unwrap();
        let db_path = tmp.path().join("cull.db");
        let db = crate::db_core::db::Database::open(&db_path).unwrap();
        db.conn.lock().execute(
            "INSERT INTO images (id, sha256_hash, width, height, format, file_size, created_at, imported_at)
             VALUES ('img1', 'hash-img1', 100, 100, 'png', 1000, '2026-01-01', '2026-01-01')",
            [],
        ).unwrap();
        drop(db);

        let args = CliArgs::try_parse_from([
            "cull",
            "--db",
            db_path.to_str().unwrap(),
            "--app-data-dir",
            tmp.path().to_str().unwrap(),
            "set_rating",
            "--image_id",
            "img1",
            "--rating",
            "5",
        ])
        .unwrap();

        let result = execute_headless(&args).unwrap();
        assert_eq!(result["image_id"], "img1");
        assert_eq!(result["rating"], 5);
        let db = crate::db_core::db::Database::open(&db_path).unwrap();
        assert_eq!(
            db.get_selection_for_image("img1")
                .unwrap()
                .unwrap()
                .star_rating,
            Some(5)
        );
    }
}
