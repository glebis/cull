use serde_json::Value;

use super::context::HeadlessContext;

mod catalog;
mod embeddings;
mod export;
mod import;
mod library;
mod quality;

pub const SUPPORTED_TOOLS: &[&str] = &[
    "approve_catalog_values",
    "attach_images_to_catalog_work",
    "create_catalog_field_def",
    "create_catalog_preset",
    "create_catalog_work",
    "deprecate_catalog_field_def",
    "get_catalog_preset",
    "get_catalog_record",
    "get_catalog_suggestion_job",
    "get_generation_run",
    "get_image",
    "get_library_stats",
    "list_images",
    "list_catalog_drafts",
    "list_catalog_fields",
    "list_catalog_presets",
    "list_catalog_values",
    "list_export_presets",
    "list_folders",
    "list_collections",
    "import_folder",
    "import_files",
    "reject_catalog_values",
    "set_catalog_draft_value",
    "set_catalog_draft_values",
    "suggest_catalog_values",
    "update_catalog_preset",
    "get_embedding_model_download_info",
    "download_embedding_model",
    "generate_embeddings",
    "analyze_image_quality",
    "get_image_quality",
    "get_quality_count",
    "export_images",
];

pub fn execute_named_tool(
    ctx: &HeadlessContext,
    tool_name: &str,
    params: Value,
) -> Result<Value, String> {
    match tool_name {
        "approve_catalog_values" => catalog::approve_catalog_values(ctx, params),
        "attach_images_to_catalog_work" => catalog::attach_images_to_catalog_work(ctx, params),
        "create_catalog_field_def" => catalog::create_catalog_field_def(ctx, params),
        "create_catalog_preset" => catalog::create_catalog_preset(ctx, params),
        "create_catalog_work" => catalog::create_catalog_work(ctx, params),
        "deprecate_catalog_field_def" => catalog::deprecate_catalog_field_def(ctx, params),
        "get_library_stats" => library::get_library_stats(ctx),
        "get_catalog_preset" => catalog::get_catalog_preset(ctx, params),
        "get_catalog_record" => catalog::get_catalog_record(ctx, params),
        "get_catalog_suggestion_job" => catalog::get_catalog_suggestion_job(ctx, params),
        "get_generation_run" => library::get_generation_run(ctx, params),
        "get_image" => library::get_image(ctx, params),
        "list_images" => library::list_images(ctx, params),
        "list_catalog_drafts" => catalog::list_catalog_drafts(ctx, params),
        "list_catalog_fields" => catalog::list_catalog_fields(ctx, params),
        "list_catalog_presets" => catalog::list_catalog_presets(ctx),
        "list_catalog_values" => catalog::list_catalog_values(ctx, params),
        "list_folders" => library::list_folders(ctx),
        "list_collections" => library::list_collections(ctx),
        "import_folder" => import::import_folder(ctx, params),
        "import_files" => import::import_files(ctx, params),
        "reject_catalog_values" => catalog::reject_catalog_values(ctx, params),
        "set_catalog_draft_value" => catalog::set_catalog_draft_value(ctx, params),
        "set_catalog_draft_values" => catalog::set_catalog_draft_values(ctx, params),
        "suggest_catalog_values" => catalog::suggest_catalog_values(ctx, params),
        "update_catalog_preset" => catalog::update_catalog_preset(ctx, params),
        "get_embedding_model_download_info" => {
            embeddings::get_embedding_model_download_info(ctx, params)
        }
        "download_embedding_model" => embeddings::download_embedding_model(ctx, params),
        "generate_embeddings" => embeddings::generate_embeddings(ctx, params),
        "analyze_image_quality" => quality::analyze_image_quality(ctx, params),
        "get_image_quality" => quality::get_image_quality(ctx, params),
        "get_quality_count" => quality::get_quality_count(ctx),
        "list_export_presets" => export::list_export_presets(),
        "export_images" => export::export_images(ctx, params),
        other => Err(format!(
            "Unsupported headless tool '{}'. Supported: {}",
            other,
            SUPPORTED_TOOLS.join(", ")
        )),
    }
}
