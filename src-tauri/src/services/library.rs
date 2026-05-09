use crate::db_core::models::ImageWithFile;
use crate::db_core::thumbnails;
use crate::services::{Pagination, ServiceContext, ServiceError};
use std::path::PathBuf;

pub fn enrich_thumbnails(images: &mut [ImageWithFile], app_data_dir: &PathBuf) {
    for img in images.iter_mut() {
        let thumb = thumbnails::thumbnail_path(app_data_dir, &img.image.id);
        if thumb.exists() {
            img.thumbnail_path = Some(thumb.to_string_lossy().to_string());
        }
    }
}

pub fn list_images(
    ctx: &ServiceContext,
    page: Pagination,
) -> Result<Vec<ImageWithFile>, ServiceError> {
    let page = Pagination::clamped(page.offset, page.limit);
    let mut images = ctx.db.list_images(page.limit, page.offset)?;
    enrich_thumbnails(&mut images, ctx.app_data_dir);
    Ok(images)
}

pub fn list_images_by_folder(
    ctx: &ServiceContext,
    folder: &str,
    page: Pagination,
) -> Result<Vec<ImageWithFile>, ServiceError> {
    let page = Pagination::clamped(page.offset, page.limit);
    let mut images = ctx.db.list_images_by_folder(folder, page.limit, page.offset)?;
    enrich_thumbnails(&mut images, ctx.app_data_dir);
    Ok(images)
}

pub fn list_images_filtered(
    ctx: &ServiceContext,
    min_width: Option<u32>,
    min_height: Option<u32>,
    page: Pagination,
) -> Result<Vec<ImageWithFile>, ServiceError> {
    let page = Pagination::clamped(page.offset, page.limit);
    let mut images = ctx.db.list_images_filtered(min_width, min_height, page.limit, page.offset)?;
    enrich_thumbnails(&mut images, ctx.app_data_dir);
    Ok(images)
}

pub fn get_images_by_ids(
    ctx: &ServiceContext,
    image_ids: &[&str],
) -> Result<Vec<ImageWithFile>, ServiceError> {
    let mut images = ctx.db.get_images_by_ids(image_ids)?;
    enrich_thumbnails(&mut images, ctx.app_data_dir);
    Ok(images)
}

pub fn get_image(
    ctx: &ServiceContext,
    image_id: &str,
) -> Result<ImageWithFile, ServiceError> {
    let id_refs = vec![image_id];
    let mut images = ctx.db.get_images_by_ids(&id_refs)?;
    enrich_thumbnails(&mut images, ctx.app_data_dir);
    images.into_iter().next()
        .ok_or_else(|| ServiceError::NotFound(format!("Image '{}'", image_id)))
}

pub fn list_folders(ctx: &ServiceContext) -> Result<Vec<(String, u32)>, ServiceError> {
    Ok(ctx.db.list_folders()?)
}

pub fn get_image_count(ctx: &ServiceContext) -> Result<u32, ServiceError> {
    Ok(ctx.db.image_count()?)
}

pub fn get_iteration_siblings(
    ctx: &ServiceContext,
    parent_id: &str,
) -> Result<Vec<ImageWithFile>, ServiceError> {
    let mut images = ctx.db.get_iteration_siblings(parent_id)?;
    enrich_thumbnails(&mut images, ctx.app_data_dir);
    Ok(images)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_pagination_clamped() {
        let p = Pagination::clamped(0, 200);
        assert_eq!(p.limit, 100);
        assert_eq!(p.offset, 0);

        let p = Pagination::clamped(10, 0);
        assert_eq!(p.limit, 1);

        let p = Pagination::clamped(5, 50);
        assert_eq!(p.limit, 50);
        assert_eq!(p.offset, 5);
    }
}
