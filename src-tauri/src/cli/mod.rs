use clap::{Parser, Subcommand};
use serde_json::Value;
use std::path::PathBuf;

mod context;
mod output;
mod tools;

use context::HeadlessContext;

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
