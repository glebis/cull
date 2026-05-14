use serde::Deserialize;
use serde_json::Value;

use crate::db_core::models::ImageWithFile;

use super::HeadlessContext;

#[derive(Debug, Deserialize)]
struct ListImagesParams {
    offset: Option<u32>,
    limit: Option<u32>,
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
    Ok(Value::Array(images.iter().map(image_value).collect()))
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

fn image_value(img: &ImageWithFile) -> Value {
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

fn clamp_limit(limit: u32) -> u32 {
    limit.min(100).max(1)
}
