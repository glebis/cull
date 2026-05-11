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

pub fn remove_from_collection(ctx: &ServiceContext, collection_id: &str, image_ids: &[&str]) -> Result<(), ServiceError> {
    for id in image_ids {
        ctx.db.remove_from_collection(collection_id, id)?;
    }
    Ok(())
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db_core::db::Database;
    use crate::db_core::secrets::MemoryStore;
    use crate::db_core::embeddings::EmbeddingEngine;
    use crate::db_core::detection::DetectionEngine;
    use std::path::PathBuf;
    use std::sync::Mutex;

    fn make_ctx_parts() -> (Database, MemoryStore, PathBuf, Mutex<EmbeddingEngine>, Mutex<DetectionEngine>, Mutex<DetectionEngine>, tempfile::TempDir) {
        let tmp = tempfile::tempdir().unwrap();
        let db = Database::open(std::path::Path::new(":memory:")).unwrap();
        let secrets = MemoryStore::new();
        let dir = tmp.path().to_path_buf();
        let mdir = tmp.path().join("models");
        (db, secrets, dir, Mutex::new(EmbeddingEngine::new(&mdir)), Mutex::new(DetectionEngine::new_yolo(&mdir)), Mutex::new(DetectionEngine::new_nudenet(&mdir)), tmp)
    }

    fn ctx<'a>(db: &'a Database, s: &'a MemoryStore, d: &'a PathBuf, ee: &'a Mutex<EmbeddingEngine>, de: &'a Mutex<DetectionEngine>, se: &'a Mutex<DetectionEngine>) -> ServiceContext<'a> {
        ServiceContext { db, app_data_dir: d, embedding_engine: ee, detection_engine: de, safety_engine: se, secrets: s, app_handle: None }
    }

    fn insert_img(db: &Database, id: &str) {
        let conn = db.conn.lock().unwrap();
        conn.execute(
            "INSERT INTO images (id, sha256_hash, width, height, format, file_size, created_at, imported_at, ai_prompt) VALUES (?1, ?2, 100, 100, 'png', 1000, '2026-01-01', '2026-01-01', NULL)",
            rusqlite::params![id, format!("h_{}", id)],
        ).unwrap();
        conn.execute(
            "INSERT INTO image_files (id, image_id, path, last_seen_at) VALUES (?1, ?2, ?3, '2026-01-01')",
            rusqlite::params![format!("f_{}", id), id, format!("/test/{}.png", id)],
        ).unwrap();
    }

    #[test]
    fn test_set_rating_and_read() {
        let (db, s, d, ee, de, se, _tmp) = make_ctx_parts();
        insert_img(&db, "r1");
        let c = ctx(&db, &s, &d, &ee, &de, &se);
        set_rating(&c, "r1", 4).unwrap();
        let imgs = c.db.get_images_by_ids(&["r1"]).unwrap();
        assert_eq!(imgs[0].selection.as_ref().unwrap().star_rating, Some(4));
    }

    #[test]
    fn test_set_decision() {
        let (db, s, d, ee, de, se, _tmp) = make_ctx_parts();
        insert_img(&db, "d1");
        let c = ctx(&db, &s, &d, &ee, &de, &se);
        set_decision(&c, "d1", "accept").unwrap();
        let imgs = c.db.get_images_by_ids(&["d1"]).unwrap();
        assert_eq!(imgs[0].selection.as_ref().unwrap().decision, "accept");
    }

    #[test]
    fn test_create_and_list_collections() {
        let (db, s, d, ee, de, se, _tmp) = make_ctx_parts();
        let c = ctx(&db, &s, &d, &ee, &de, &se);
        let id = create_collection(&c, "My Collection").unwrap();
        assert!(!id.is_empty());
        let cols = list_collections(&c).unwrap();
        let found = cols.iter().find(|(cid, _, _)| cid == &id);
        assert!(found.is_some());
        assert_eq!(found.unwrap().1, "My Collection");
    }

    #[test]
    fn test_add_to_and_list_collection_images() {
        let (db, s, d, ee, de, se, _tmp) = make_ctx_parts();
        insert_img(&db, "ci1");
        insert_img(&db, "ci2");
        let c = ctx(&db, &s, &d, &ee, &de, &se);
        let col_id = create_collection(&c, "Test Col").unwrap();
        add_to_collection(&c, &col_id, &["ci1", "ci2"]).unwrap();
        let imgs = list_collection_images(&c, &col_id).unwrap();
        assert_eq!(imgs.len(), 2);
    }

    #[test]
    fn test_delete_collection() {
        let (db, s, d, ee, de, se, _tmp) = make_ctx_parts();
        let c = ctx(&db, &s, &d, &ee, &de, &se);
        let id = create_collection(&c, "To Delete").unwrap();
        delete_collection(&c, &id).unwrap();
        let cols = list_collections(&c).unwrap();
        assert!(cols.iter().all(|(cid, _, _)| cid != &id));
    }

    #[test]
    fn test_create_smart_collection() {
        let (db, s, d, ee, de, se, _tmp) = make_ctx_parts();
        let c = ctx(&db, &s, &d, &ee, &de, &se);
        let filter = r#"{"type":"rating","operator":"gte","value":4}"#;
        let id = create_smart_collection(&c, "Top Rated", filter, Some("4+ stars")).unwrap();
        assert!(!id.is_empty());
        let smarts = list_smart_collections(&c).unwrap();
        assert!(smarts.iter().any(|sc| sc.id == id));
    }

    #[test]
    fn test_delete_smart_collection() {
        let (db, s, d, ee, de, se, _tmp) = make_ctx_parts();
        let c = ctx(&db, &s, &d, &ee, &de, &se);
        let filter = r#"{"type":"rating","operator":"gte","value":3}"#;
        let id = create_smart_collection(&c, "To Remove", filter, None).unwrap();
        delete_smart_collection(&c, &id).unwrap();
        let smarts = list_smart_collections(&c).unwrap();
        assert!(smarts.iter().all(|sc| sc.id != id));
    }
}
