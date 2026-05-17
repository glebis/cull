use serde_json::Value;

use super::context::HeadlessContext;

mod embeddings;
mod export;
mod import;
mod library;

pub const SUPPORTED_TOOLS: &[&str] = &[
    "get_library_stats",
    "list_images",
    "list_folders",
    "list_collections",
    "import_folder",
    "import_files",
    "get_embedding_model_download_info",
    "download_embedding_model",
    "generate_embeddings",
    "list_export_presets",
    "export_images",
];

pub fn execute_named_tool(
    ctx: &HeadlessContext,
    tool_name: &str,
    params: Value,
) -> Result<Value, String> {
    match tool_name {
        "get_library_stats" => library::get_library_stats(ctx),
        "list_images" => library::list_images(ctx, params),
        "list_folders" => library::list_folders(ctx),
        "list_collections" => library::list_collections(ctx),
        "import_folder" => import::import_folder(ctx, params),
        "import_files" => import::import_files(ctx, params),
        "get_embedding_model_download_info" => {
            embeddings::get_embedding_model_download_info(ctx, params)
        }
        "download_embedding_model" => embeddings::download_embedding_model(ctx, params),
        "generate_embeddings" => embeddings::generate_embeddings(ctx, params),
        "list_export_presets" => export::list_export_presets(),
        "export_images" => export::export_images(ctx, params),
        other => Err(format!(
            "Unsupported headless tool '{}'. Supported: {}",
            other,
            SUPPORTED_TOOLS.join(", ")
        )),
    }
}
