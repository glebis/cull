use serde::Deserialize;
use serde_json::Value;

use crate::db_core::models::{GenerationRun, ImageWithFile};

use super::HeadlessContext;

#[derive(Debug, Deserialize)]
struct ListImagesParams {
    offset: Option<u32>,
    limit: Option<u32>,
}

#[derive(Debug, Deserialize)]
struct ImageIdParams {
    image_id: String,
}

pub fn get_library_stats(ctx: &HeadlessContext) -> Result<Value, String> {
    let image_count = ctx.db.image_count().map_err(|e| e.to_string())?;
    let folders = ctx.db.list_folders().map_err(|e| e.to_string())?;
    let collections = ctx.db.list_collections().map_err(|e| e.to_string())?;
    Ok(serde_json::json!({
        "image_count": image_count,
        "folder_count": folders.len(),
        "collection_count": collections.len(),
    }))
}

pub fn list_images(ctx: &HeadlessContext, params: Value) -> Result<Value, String> {
    let parsed: ListImagesParams =
        serde_json::from_value(params).map_err(|e| format!("Invalid list_images params: {}", e))?;
    let offset = parsed.offset.unwrap_or(0);
    let limit = clamp_limit(parsed.limit.unwrap_or(50));
    let images = ctx
        .db
        .list_images(limit, offset)
        .map_err(|e| e.to_string())?;
    let values = images
        .iter()
        .map(|image| image_value(ctx, image))
        .collect::<Result<Vec<_>, String>>()?;
    Ok(Value::Array(values))
}

pub fn get_image(ctx: &HeadlessContext, params: Value) -> Result<Value, String> {
    let parsed: ImageIdParams =
        serde_json::from_value(params).map_err(|e| format!("Invalid get_image params: {}", e))?;
    let refs = vec![parsed.image_id.as_str()];
    let images = ctx.db.get_images_by_ids(&refs).map_err(|e| e.to_string())?;
    let image = images
        .into_iter()
        .next()
        .ok_or_else(|| format!("Image '{}' not found", parsed.image_id))?;
    image_value(ctx, &image)
}

pub fn get_generation_run(ctx: &HeadlessContext, params: Value) -> Result<Value, String> {
    let parsed: ImageIdParams = serde_json::from_value(params)
        .map_err(|e| format!("Invalid get_generation_run params: {}", e))?;
    ctx.db
        .get_generation_run_for_image(&parsed.image_id)
        .map(|run| {
            run.map_or(Value::Null, |run| {
                serde_json::to_value(run).unwrap_or(Value::Null)
            })
        })
        .map_err(|e| e.to_string())
}

pub fn list_folders(ctx: &HeadlessContext) -> Result<Value, String> {
    let folders = ctx.db.list_folders().map_err(|e| e.to_string())?;
    Ok(Value::Array(
        folders
            .iter()
            .map(|(path, count)| serde_json::json!({ "path": path, "image_count": count }))
            .collect(),
    ))
}

pub fn list_collections(ctx: &HeadlessContext) -> Result<Value, String> {
    let collections = ctx.db.list_collections().map_err(|e| e.to_string())?;
    Ok(Value::Array(
        collections
            .iter()
            .map(|(id, name, count)| {
                serde_json::json!({ "id": id, "name": name, "image_count": count })
            })
            .collect(),
    ))
}

fn image_value(ctx: &HeadlessContext, img: &ImageWithFile) -> Result<Value, String> {
    let generation_run = ctx
        .db
        .get_generation_run_for_image(&img.image.id)
        .map_err(|e| e.to_string())?;
    Ok(image_value_with_generation(img, generation_run.as_ref()))
}

fn image_value_with_generation(
    img: &ImageWithFile,
    generation_run: Option<&GenerationRun>,
) -> Value {
    serde_json::json!({
        "id": img.image.id,
        "path": img.path,
        "width": img.image.width,
        "height": img.image.height,
        "format": img.image.format,
        "file_size": img.image.file_size,
        "source_label": img.source_label,
        "prompt": generation_run
            .and_then(|run| run.prompt.as_ref())
            .or(img.image.ai_prompt.as_ref()),
        "ai_prompt": img.image.ai_prompt.as_ref(),
        "generation": generation_run,
        "rating": img.selection.as_ref().and_then(|s| s.star_rating),
        "decision": img.selection.as_ref().map(|s| &s.decision),
    })
}

fn clamp_limit(limit: u32) -> u32 {
    limit.min(100).max(1)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db_core::models::Image;

    #[test]
    fn image_value_prefers_generation_prompt() {
        let image = image_fixture(Some("legacy prompt"));
        let run = generation_run_fixture(Some("run prompt"));

        let value = image_value_with_generation(&image, Some(&run));

        assert_eq!(value["prompt"], "run prompt");
        assert_eq!(value["ai_prompt"], "legacy prompt");
        assert_eq!(value["generation"]["prompt"], "run prompt");
        assert_eq!(value["source_label"], "openai");
    }

    #[test]
    fn image_value_falls_back_to_ai_prompt() {
        let image = image_fixture(Some("legacy prompt"));
        let run = generation_run_fixture(None);

        let value = image_value_with_generation(&image, Some(&run));

        assert_eq!(value["prompt"], "legacy prompt");
        assert_eq!(value["generation"]["prompt"], serde_json::Value::Null);
    }

    fn image_fixture(ai_prompt: Option<&str>) -> ImageWithFile {
        ImageWithFile {
            image: Image {
                id: "img-1".to_string(),
                sha256_hash: "hash".to_string(),
                width: 1024,
                height: 768,
                format: "png".to_string(),
                file_size: 42,
                created_at: "2026-05-30T12:00:00Z".to_string(),
                imported_at: "2026-05-30T12:01:00Z".to_string(),
                ai_prompt: ai_prompt.map(str::to_string),
                raw_metadata: None,
            },
            path: "/Users/gleb/art/share/image.png".to_string(),
            thumbnail_path: None,
            selection: None,
            source_label: Some("openai".to_string()),
            missing_at: None,
        }
    }

    fn generation_run_fixture(prompt: Option<&str>) -> GenerationRun {
        GenerationRun {
            id: "run-1".to_string(),
            prompt: prompt.map(str::to_string),
            negative_prompt: None,
            provider: Some("openai".to_string()),
            model: Some("gpt-image-1".to_string()),
            settings_json: "{}".to_string(),
            seed: Some("123".to_string()),
            parent_run_id: None,
            source_type: "sidecar".to_string(),
            source_path: None,
            raw_metadata_json: None,
            created_at: Some("2026-05-30T12:00:00Z".to_string()),
            imported_at: "2026-05-30T12:01:00Z".to_string(),
        }
    }
}
