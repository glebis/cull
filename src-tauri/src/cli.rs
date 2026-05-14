use clap::{Parser, Subcommand};
use serde::Deserialize;
use serde_json::Value;
use std::path::{Path, PathBuf};

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
}

#[derive(Debug, Deserialize)]
struct ListImagesParams {
    offset: Option<u32>,
    limit: Option<u32>,
}

pub fn run_headless_if_requested(args: &CliArgs) -> Option<i32> {
    args.command.as_ref()?;
    Some(match execute_headless(args) {
        Ok(value) => {
            print_success(args.json, &value);
            0
        }
        Err(message) => {
            print_error(args.json, &message);
            1
        }
    })
}

fn execute_headless(args: &CliArgs) -> Result<Value, String> {
    let app_data_dir = resolve_app_data_dir(args)?;
    std::fs::create_dir_all(&app_data_dir).map_err(|e| {
        format!(
            "Failed to create app data dir '{}': {}",
            app_data_dir.display(),
            e
        )
    })?;
    let db_path = args
        .db
        .clone()
        .unwrap_or_else(|| app_data_dir.join("cull.db"));
    let db = crate::db_core::db::Database::open(&db_path).map_err(|e| e.to_string())?;

    match args.command.as_ref().expect("checked by caller") {
        CliCommand::CallTool {
            tool_name,
            params_json,
            params_file,
        } => {
            let params = load_params(params_json.as_deref(), params_file.as_deref())?;
            execute_named_tool(&db, &app_data_dir, tool_name, params)
        }
        CliCommand::GetLibraryStats => execute_named_tool(
            &db,
            &app_data_dir,
            "get_library_stats",
            serde_json::json!({}),
        ),
        CliCommand::ListImages { offset, limit } => execute_named_tool(
            &db,
            &app_data_dir,
            "list_images",
            serde_json::json!({ "offset": offset, "limit": limit }),
        ),
        CliCommand::ListFolders => {
            execute_named_tool(&db, &app_data_dir, "list_folders", serde_json::json!({}))
        }
        CliCommand::ListCollections => execute_named_tool(
            &db,
            &app_data_dir,
            "list_collections",
            serde_json::json!({}),
        ),
        CliCommand::ImportFolder { folder_path } => execute_named_tool(
            &db,
            &app_data_dir,
            "import_folder",
            serde_json::json!({ "folder_path": folder_path }),
        ),
        CliCommand::ImportFiles { file_paths } => execute_named_tool(
            &db,
            &app_data_dir,
            "import_files",
            serde_json::json!({ "file_paths": file_paths }),
        ),
        CliCommand::ListExportPresets => execute_named_tool(
            &db,
            &app_data_dir,
            "list_export_presets",
            serde_json::json!({}),
        ),
        CliCommand::ExportImages {
            image_ids,
            collection_id,
            folder_path,
            output_dir,
            format,
            flatten,
            naming,
        } => execute_named_tool(
            &db,
            &app_data_dir,
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
    }
}

fn execute_named_tool(
    db: &crate::db_core::db::Database,
    app_data_dir: &Path,
    tool_name: &str,
    params: Value,
) -> Result<Value, String> {
    match tool_name {
        "get_library_stats" => {
            let image_count = db.image_count().map_err(|e| e.to_string())?;
            let folders = db.list_folders().map_err(|e| e.to_string())?;
            let collections = db.list_collections().map_err(|e| e.to_string())?;
            Ok(serde_json::json!({
                "image_count": image_count,
                "folder_count": folders.len(),
                "collection_count": collections.len(),
            }))
        }
        "list_images" => {
            let parsed: ListImagesParams = serde_json::from_value(params)
                .map_err(|e| format!("Invalid list_images params: {}", e))?;
            let offset = parsed.offset.unwrap_or(0);
            let limit = clamp_limit(parsed.limit.unwrap_or(50));
            let images = db.list_images(limit, offset).map_err(|e| e.to_string())?;
            Ok(Value::Array(images.iter().map(image_value).collect()))
        }
        "list_folders" => {
            let folders = db.list_folders().map_err(|e| e.to_string())?;
            Ok(Value::Array(
                folders
                    .iter()
                    .map(|(path, count)| serde_json::json!({ "path": path, "image_count": count }))
                    .collect(),
            ))
        }
        "list_collections" => {
            let collections = db.list_collections().map_err(|e| e.to_string())?;
            Ok(Value::Array(
                collections
                    .iter()
                    .map(|(id, name, count)| {
                        serde_json::json!({ "id": id, "name": name, "image_count": count })
                    })
                    .collect(),
            ))
        }
        "import_folder" => {
            let parsed: crate::services::import::ImportFolderParams =
                serde_json::from_value(params)
                    .map_err(|e| format!("Invalid import_folder params: {}", e))?;
            let result = crate::services::import::import_folder(db, app_data_dir, parsed)?;
            serde_json::to_value(result).map_err(|e| e.to_string())
        }
        "import_files" => {
            let parsed: crate::services::import::ImportFilesParams = serde_json::from_value(params)
                .map_err(|e| format!("Invalid import_files params: {}", e))?;
            let result = crate::services::import::import_files(db, app_data_dir, parsed)?;
            serde_json::to_value(result).map_err(|e| e.to_string())
        }
        "list_export_presets" => {
            serde_json::to_value(crate::services::export::list_presets()).map_err(|e| e.to_string())
        }
        "export_images" => {
            let parsed: crate::services::export::ExportImagesParams =
                serde_json::from_value(params)
                    .map_err(|e| format!("Invalid export_images params: {}", e))?;
            let result = crate::services::export::export_images(db, app_data_dir, parsed)?;
            serde_json::to_value(result).map_err(|e| e.to_string())
        }
        other => Err(format!(
            "Unsupported headless tool '{}'. Supported: get_library_stats, list_images, list_folders, list_collections, import_folder, import_files, list_export_presets, export_images",
            other
        )),
    }
}

fn image_value(img: &crate::db_core::models::ImageWithFile) -> Value {
    serde_json::json!({
        "id": img.image.id,
        "path": img.path,
        "width": img.image.width,
        "height": img.image.height,
        "format": img.image.format,
        "file_size": img.image.file_size,
        "rating": img.selection.as_ref().and_then(|s| s.star_rating),
        "decision": img.selection.as_ref().map(|s| &s.decision),
    })
}

fn load_params(params_json: Option<&str>, params_file: Option<&Path>) -> Result<Value, String> {
    match (params_json, params_file) {
        (Some(_), Some(_)) => Err("Use params_json or params_file, not both".to_string()),
        (Some(raw), None) => serde_json::from_str(raw).map_err(|e| format!("Invalid JSON: {}", e)),
        (None, Some(path)) => {
            let raw = std::fs::read_to_string(path)
                .map_err(|e| format!("Failed to read '{}': {}", path.display(), e))?;
            serde_json::from_str(&raw).map_err(|e| format!("Invalid JSON: {}", e))
        }
        (None, None) => Ok(serde_json::json!({})),
    }
}

fn resolve_app_data_dir(args: &CliArgs) -> Result<PathBuf, String> {
    if let Some(dir) = args.app_data_dir.clone() {
        return Ok(dir);
    }
    dirs::data_dir()
        .map(|dir| dir.join("com.glebkalinin.cull"))
        .ok_or_else(|| "No data dir is available on this system".to_string())
}

fn clamp_limit(limit: u32) -> u32 {
    limit.min(100).max(1)
}

fn print_success(json: bool, value: &Value) {
    if json {
        println!(
            "{}",
            serde_json::to_string(value).unwrap_or_else(|_| "{}".to_string())
        );
    } else {
        println!(
            "{}",
            serde_json::to_string_pretty(value).unwrap_or_else(|_| "{}".to_string())
        );
    }
}

fn print_error(json: bool, message: &str) {
    if json {
        println!(
            "{}",
            serde_json::json!({"event": "error", "message": message}).to_string()
        );
    } else {
        eprintln!("{}", message);
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
        ])
        .unwrap();
        assert!(args.tray);
        assert_eq!(args.mcp_http, Some(Some(9847)));
        assert_eq!(args.mcp_http_host, "0.0.0.0");
    }

    #[test]
    fn test_unknown_flag_errors() {
        let result = CliArgs::try_parse_from(["cull", "--bogus"]);
        assert!(result.is_err());
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
}
