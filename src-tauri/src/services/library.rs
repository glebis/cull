use crate::db_core::models::ImageWithFile;
use crate::db_core::thumbnails;
use crate::services::{Pagination, ServiceContext, ServiceError};
use std::path::Path;
#[cfg(test)]
use std::path::PathBuf;

pub fn enrich_thumbnails(images: &mut [ImageWithFile], app_data_dir: &Path) {
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
    let mut images = ctx
        .db
        .list_images_by_folder(folder, page.limit, page.offset)?;
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
    let mut images = ctx
        .db
        .list_images_filtered(min_width, min_height, page.limit, page.offset)?;
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

pub fn get_image(ctx: &ServiceContext, image_id: &str) -> Result<ImageWithFile, ServiceError> {
    let id_refs = vec![image_id];
    let mut images = ctx.db.get_images_by_ids(&id_refs)?;
    enrich_thumbnails(&mut images, ctx.app_data_dir);
    images
        .into_iter()
        .next()
        .ok_or_else(|| ServiceError::NotFound(format!("Image '{}'", image_id)))
}

pub fn get_image_by_path(
    ctx: &ServiceContext,
    path: &str,
) -> Result<Option<ImageWithFile>, ServiceError> {
    let Some(file) = ctx.db.get_image_file_by_path(path)? else {
        return Ok(None);
    };
    let id_refs = vec![file.image_id.as_str()];
    let mut images = ctx.db.get_images_by_ids(&id_refs)?;
    enrich_thumbnails(&mut images, ctx.app_data_dir);
    Ok(images.into_iter().next())
}

pub fn list_folders(ctx: &ServiceContext) -> Result<Vec<(String, u32)>, ServiceError> {
    Ok(ctx.db.list_folders()?)
}

pub fn get_image_count(ctx: &ServiceContext) -> Result<u32, ServiceError> {
    Ok(ctx.db.image_count()?)
}

pub fn list_image_ids(ctx: &ServiceContext) -> Result<Vec<String>, ServiceError> {
    Ok(ctx.db.list_image_ids()?)
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
    use crate::db_core::db::Database;
    use crate::db_core::detection::DetectionEngine;
    use crate::db_core::embeddings::EmbeddingEngine;
    use crate::db_core::secrets::MemoryStore;
    use parking_lot::Mutex;

    fn make_ctx_parts() -> (
        Database,
        MemoryStore,
        PathBuf,
        Mutex<EmbeddingEngine>,
        Mutex<DetectionEngine>,
        Mutex<DetectionEngine>,
        tempfile::TempDir,
    ) {
        let tmp = tempfile::tempdir().unwrap();
        let db = Database::open(std::path::Path::new(":memory:")).unwrap();
        let secrets = MemoryStore::new();
        let app_data_dir = tmp.path().to_path_buf();
        let model_dir = tmp.path().join("models");
        let ee = Mutex::new(EmbeddingEngine::new(&model_dir));
        let de = Mutex::new(DetectionEngine::new_yolo(&model_dir));
        let se = Mutex::new(DetectionEngine::new_nudenet(&model_dir));
        (db, secrets, app_data_dir, ee, de, se, tmp)
    }

    fn make_ctx<'a>(
        db: &'a Database,
        secrets: &'a MemoryStore,
        app_data_dir: &'a PathBuf,
        ee: &'a Mutex<EmbeddingEngine>,
        de: &'a Mutex<DetectionEngine>,
        se: &'a Mutex<DetectionEngine>,
    ) -> crate::services::ServiceContext<'a> {
        crate::services::ServiceContext {
            db,
            app_data_dir,
            embedding_engine: ee,
            detection_engine: de,
            safety_engine: se,
            secrets,
            app_handle: None,
        }
    }

    fn insert_test_image(db: &Database, id: &str, path: &str) {
        let conn = db.conn.lock();
        conn.execute(
            "INSERT INTO images (id, sha256_hash, width, height, format, file_size, created_at, imported_at, ai_prompt)
             VALUES (?1, ?2, 100, 100, 'png', 1000, '2026-01-01', '2026-01-01', NULL)",
            rusqlite::params![id, format!("hash_{}", id)],
        ).unwrap();
        conn.execute(
            "INSERT INTO image_files (id, image_id, path, last_seen_at)
             VALUES (?1, ?2, ?3, '2026-01-01')",
            rusqlite::params![format!("tf_{}", id), id, path],
        )
        .unwrap();
    }

    #[test]
    fn test_pagination_clamped() {
        let p = Pagination::clamped(0, 300);
        assert_eq!(p.limit, 250);
        assert_eq!(p.offset, 0);

        let p = Pagination::clamped(10, 0);
        assert_eq!(p.limit, 1);

        let p = Pagination::clamped(5, 50);
        assert_eq!(p.limit, 50);
        assert_eq!(p.offset, 5);
    }

    #[test]
    fn test_list_images_empty_db() {
        let (db, sec, dir, ee, de, se, _tmp) = make_ctx_parts();
        let ctx = make_ctx(&db, &sec, &dir, &ee, &de, &se);
        let result = list_images(&ctx, Pagination::default()).unwrap();
        assert!(result.is_empty());
    }

    #[test]
    fn test_list_images_returns_inserted() {
        let (db, sec, dir, ee, de, se, _tmp) = make_ctx_parts();
        insert_test_image(&db, "img_1", "/photos/a.png");
        insert_test_image(&db, "img_2", "/photos/b.png");
        let ctx = make_ctx(&db, &sec, &dir, &ee, &de, &se);
        let result = list_images(&ctx, Pagination::default()).unwrap();
        assert_eq!(result.len(), 2);
    }

    #[test]
    fn test_get_image_not_found() {
        let (db, sec, dir, ee, de, se, _tmp) = make_ctx_parts();
        let ctx = make_ctx(&db, &sec, &dir, &ee, &de, &se);
        let result = get_image(&ctx, "nonexistent");
        assert!(result.is_err());
        match result.unwrap_err() {
            ServiceError::NotFound(msg) => assert!(msg.contains("nonexistent")),
            other => panic!("Expected NotFound, got {:?}", other),
        }
    }

    #[test]
    fn test_get_image_found() {
        let (db, sec, dir, ee, de, se, _tmp) = make_ctx_parts();
        insert_test_image(&db, "img_x", "/photos/x.png");
        let ctx = make_ctx(&db, &sec, &dir, &ee, &de, &se);
        let img = get_image(&ctx, "img_x").unwrap();
        assert_eq!(img.image.id, "img_x");
        assert_eq!(img.path, "/photos/x.png");
    }

    #[test]
    fn test_list_folders_empty() {
        let (db, sec, dir, ee, de, se, _tmp) = make_ctx_parts();
        let ctx = make_ctx(&db, &sec, &dir, &ee, &de, &se);
        let result = list_folders(&ctx).unwrap();
        assert!(result.is_empty());
    }

    #[test]
    fn test_get_image_count_zero() {
        let (db, sec, dir, ee, de, se, _tmp) = make_ctx_parts();
        let ctx = make_ctx(&db, &sec, &dir, &ee, &de, &se);
        assert_eq!(get_image_count(&ctx).unwrap(), 0);
    }

    #[test]
    fn test_get_image_count_with_images() {
        let (db, sec, dir, ee, de, se, _tmp) = make_ctx_parts();
        insert_test_image(&db, "c1", "/p/1.png");
        insert_test_image(&db, "c2", "/p/2.png");
        insert_test_image(&db, "c3", "/p/3.png");
        let ctx = make_ctx(&db, &sec, &dir, &ee, &de, &se);
        assert_eq!(get_image_count(&ctx).unwrap(), 3);
    }

    #[test]
    fn test_enrich_thumbnails_no_thumbs() {
        let dir = PathBuf::from("/nonexistent/path");
        let mut images = vec![];
        enrich_thumbnails(&mut images, &dir);
        assert!(images.is_empty());
    }

    #[test]
    fn test_get_images_by_ids() {
        let (db, sec, dir, ee, de, se, _tmp) = make_ctx_parts();
        insert_test_image(&db, "id_a", "/a.png");
        insert_test_image(&db, "id_b", "/b.png");
        insert_test_image(&db, "id_c", "/c.png");
        let ctx = make_ctx(&db, &sec, &dir, &ee, &de, &se);
        let result = get_images_by_ids(&ctx, &["id_a", "id_c"]).unwrap();
        assert_eq!(result.len(), 2);
        let ids: Vec<&str> = result.iter().map(|i| i.image.id.as_str()).collect();
        assert!(ids.contains(&"id_a"));
        assert!(ids.contains(&"id_c"));
    }

    #[test]
    fn test_list_images_pagination() {
        let (db, sec, dir, ee, de, se, _tmp) = make_ctx_parts();
        for i in 0..10 {
            insert_test_image(&db, &format!("pg_{}", i), &format!("/p/{}.png", i));
        }
        let ctx = make_ctx(&db, &sec, &dir, &ee, &de, &se);
        let page1 = list_images(&ctx, Pagination::clamped(0, 3)).unwrap();
        assert_eq!(page1.len(), 3);
        let page2 = list_images(&ctx, Pagination::clamped(3, 3)).unwrap();
        assert_eq!(page2.len(), 3);
        assert_ne!(page1[0].image.id, page2[0].image.id);
    }
}
