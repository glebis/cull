use serde_json::Value;

use super::context::HeadlessContext;

mod catalog;
mod curation;
mod embeddings;
mod export;
mod import;
mod library;
mod quality;
mod search;

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
    "find_similar",
    "reject_catalog_values",
    "set_rating",
    "search_by_object",
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
        "list_images" => library::list_images(ctx, params),
        "list_catalog_drafts" => catalog::list_catalog_drafts(ctx, params),
        "list_catalog_fields" => catalog::list_catalog_fields(ctx, params),
        "list_catalog_presets" => catalog::list_catalog_presets(ctx),
        "list_catalog_values" => catalog::list_catalog_values(ctx, params),
        "list_folders" => library::list_folders(ctx),
        "list_collections" => library::list_collections(ctx),
        "import_folder" => import::import_folder(ctx, params),
        "import_files" => import::import_files(ctx, params),
        "find_similar" => search::find_similar(ctx, params),
        "reject_catalog_values" => catalog::reject_catalog_values(ctx, params),
        "set_rating" => curation::set_rating(ctx, params),
        "search_by_object" => search::search_by_object(ctx, params),
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db_core::db::Database;

    #[test]
    fn named_search_by_object_dispatches_with_mcp_shaped_params() {
        let tmp = tempfile::tempdir().unwrap();
        let db = Database::open(std::path::Path::new(":memory:")).unwrap();
        for (id, confidence) in [("lower", 0.61), ("higher", 0.94)] {
            db.conn
                .lock()
                .execute(
                    "INSERT INTO images (id, sha256_hash, width, height, format, file_size, created_at, imported_at)
                     VALUES (?1, ?2, 100, 100, 'png', 1000, '2026-01-01', '2026-01-01')",
                    rusqlite::params![id, format!("hash-{id}")],
                )
                .unwrap();
            db.store_detections(
                id,
                "yolo11m",
                &[crate::db_core::detection::Detection {
                    class_name: "person".to_string(),
                    confidence,
                    x: 0.0,
                    y: 0.0,
                    width: 1.0,
                    height: 1.0,
                }],
            )
            .unwrap();
        }
        let ctx = HeadlessContext {
            db,
            app_data_dir: tmp.path().to_path_buf(),
        };

        let result = execute_named_tool(
            &ctx,
            "search_by_object",
            serde_json::json!({ "class_name": "person", "limit": 10 }),
        )
        .unwrap();

        let matches = result.as_array().unwrap();
        assert_eq!(matches.len(), 2);
        assert_eq!(matches[0]["image_id"], "higher");
        assert!((matches[0]["confidence"].as_f64().unwrap() - 0.94).abs() < 0.000_001);
        assert_eq!(matches[1]["image_id"], "lower");
        assert!((matches[1]["confidence"].as_f64().unwrap() - 0.61).abs() < 0.000_001);
    }

    #[test]
    fn named_find_similar_dispatches_with_mcp_shaped_params() {
        let tmp = tempfile::tempdir().unwrap();
        let db = Database::open(std::path::Path::new(":memory:")).unwrap();
        for (id, vector) in [
            ("source", vec![1.0, 0.0]),
            ("near", vec![0.8, 0.6]),
            ("far", vec![0.0, 1.0]),
        ] {
            db.conn
                .lock()
                .execute(
                    "INSERT INTO images (id, sha256_hash, width, height, format, file_size, created_at, imported_at)
                     VALUES (?1, ?2, 100, 100, 'png', 1000, '2026-01-01', '2026-01-01')",
                    rusqlite::params![id, format!("hash-{id}")],
                )
                .unwrap();
            db.conn
                .lock()
                .execute(
                    "INSERT INTO image_files (id, image_id, path, last_seen_at)
                     VALUES (?1, ?2, ?3, '2026-01-01')",
                    rusqlite::params![format!("file-{id}"), id, format!("/test/{id}.png")],
                )
                .unwrap();
            db.store_embedding(id, "clip-vit-b32", &vector).unwrap();
        }
        let ctx = HeadlessContext {
            db,
            app_data_dir: tmp.path().to_path_buf(),
        };

        let result = execute_named_tool(
            &ctx,
            "find_similar",
            serde_json::json!({ "image_id": "source", "limit": 2 }),
        )
        .unwrap();

        let matches = result.as_array().unwrap();
        assert_eq!(matches.len(), 2);
        assert_eq!(matches[0]["image_id"], "near");
        assert_eq!(matches[0]["model"], "clip-vit-b32");
        assert!(
            matches[0]["similarity"].as_f64().unwrap() > matches[1]["similarity"].as_f64().unwrap()
        );
    }
}
