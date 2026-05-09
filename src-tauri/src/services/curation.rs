use crate::db_core::models::ImageWithFile;
use crate::db_core::smart_collections::SmartCollection;
use crate::services::{ServiceContext, ServiceError};
use crate::services::library::enrich_thumbnails;

pub fn set_rating(ctx: &ServiceContext, image_id: &str, rating: u8) -> Result<(), ServiceError> {
    Ok(ctx.db.set_rating(image_id, rating)?)
}

pub fn set_decision(ctx: &ServiceContext, image_id: &str, decision: &str) -> Result<(), ServiceError> {
    Ok(ctx.db.set_decision(image_id, decision)?)
}

pub fn create_collection(ctx: &ServiceContext, name: &str) -> Result<String, ServiceError> {
    Ok(ctx.db.create_collection(name)?)
}

pub fn list_collections(ctx: &ServiceContext) -> Result<Vec<(String, String, u32)>, ServiceError> {
    Ok(ctx.db.list_collections()?)
}

pub fn add_to_collection(ctx: &ServiceContext, collection_id: &str, image_ids: &[&str]) -> Result<(), ServiceError> {
    Ok(ctx.db.add_to_collection(collection_id, image_ids)?)
}

pub fn list_collection_images(ctx: &ServiceContext, collection_id: &str) -> Result<Vec<ImageWithFile>, ServiceError> {
    let mut images = ctx.db.list_collection_images(collection_id)?;
    enrich_thumbnails(&mut images, ctx.app_data_dir);
    Ok(images)
}

pub fn delete_collection(ctx: &ServiceContext, collection_id: &str) -> Result<(), ServiceError> {
    Ok(ctx.db.delete_collection(collection_id)?)
}

pub fn create_smart_collection(
    ctx: &ServiceContext,
    name: &str,
    filter_json: &str,
    nl_query: Option<&str>,
) -> Result<String, ServiceError> {
    Ok(ctx.db.create_smart_collection(name, filter_json, nl_query, false)?)
}

pub fn list_smart_collections(ctx: &ServiceContext) -> Result<Vec<SmartCollection>, ServiceError> {
    Ok(ctx.db.list_smart_collections()?)
}

pub fn evaluate_smart_collection(ctx: &ServiceContext, filter_json: &str) -> Result<Vec<ImageWithFile>, ServiceError> {
    Ok(ctx.db.evaluate_smart_collection(filter_json)?)
}

pub fn delete_smart_collection(ctx: &ServiceContext, id: &str) -> Result<(), ServiceError> {
    Ok(ctx.db.delete_smart_collection(id)?)
}

pub fn update_smart_collection(
    ctx: &ServiceContext,
    id: &str,
    name: &str,
    filter_json: &str,
    nl_query: Option<&str>,
) -> Result<(), ServiceError> {
    Ok(ctx.db.update_smart_collection(id, name, filter_json, nl_query)?)
}
