use crate::db_core::detection::Detection;
use crate::services::{ServiceContext, ServiceError};

pub fn find_similar_images(
    ctx: &ServiceContext,
    image_id: &str,
    top_k: usize,
    model: Option<&str>,
) -> Result<Vec<(String, f32)>, ServiceError> {
    let model_name = model.unwrap_or("clip-vit-b32");
    let all = ctx.db.get_all_embeddings(model_name)?;
    let query = all.iter().find(|(id, _)| id == image_id)
        .ok_or_else(|| ServiceError::NotFound("Image has no embedding".into()))?;
    Ok(ctx.db.find_similar(&query.1, model_name, top_k)?)
}

pub fn get_all_embeddings(
    ctx: &ServiceContext,
    model: Option<&str>,
) -> Result<Vec<(String, Vec<f32>)>, ServiceError> {
    let model_name = model.unwrap_or("clip-vit-b32");
    Ok(ctx.db.get_all_embeddings(model_name)?)
}

pub fn get_embedding_count(
    ctx: &ServiceContext,
    model: Option<&str>,
) -> Result<u32, ServiceError> {
    let model_name = model.unwrap_or("clip-vit-b32");
    Ok(ctx.db.embedding_count(model_name)?)
}

pub fn is_clip_available(ctx: &ServiceContext) -> Result<bool, ServiceError> {
    let engine = ctx.embedding_engine.lock().unwrap();
    Ok(engine.is_model_available())
}

pub fn get_detections(
    ctx: &ServiceContext,
    image_id: &str,
    model: Option<&str>,
) -> Result<Vec<Detection>, ServiceError> {
    Ok(ctx.db.get_detections(image_id, model)?)
}

pub fn search_by_detected_class(
    ctx: &ServiceContext,
    class_name: &str,
    limit: u32,
) -> Result<Vec<(String, f32)>, ServiceError> {
    Ok(ctx.db.search_by_class(class_name, limit)?)
}

pub fn get_detection_count(ctx: &ServiceContext, model: &str) -> Result<u32, ServiceError> {
    Ok(ctx.db.detection_count(model)?)
}

pub fn get_vision_metadata(
    ctx: &ServiceContext,
    image_id: &str,
) -> Result<Vec<(String, String, String)>, ServiceError> {
    Ok(ctx.db.get_vision_metadata(image_id)?)
}

pub fn get_vision_count(ctx: &ServiceContext, source: Option<&str>) -> Result<u32, ServiceError> {
    let src = source.unwrap_or("minicpm-v");
    Ok(ctx.db.count_vision_processed(src)?)
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

    fn make_ctx_parts() -> (Database, MemoryStore, PathBuf, Mutex<EmbeddingEngine>, Mutex<DetectionEngine>, Mutex<DetectionEngine>) {
        let db = Database::open(std::path::Path::new(":memory:")).unwrap();
        let secrets = MemoryStore::new();
        let dir = PathBuf::from("/tmp/imageview-test");
        let mdir = PathBuf::from("/tmp/imageview-test/models");
        (db, secrets, dir, Mutex::new(EmbeddingEngine::new(&mdir)), Mutex::new(DetectionEngine::new_yolo(&mdir)), Mutex::new(DetectionEngine::new_nudenet(&mdir)))
    }

    fn ctx<'a>(db: &'a Database, s: &'a MemoryStore, d: &'a PathBuf, ee: &'a Mutex<EmbeddingEngine>, de: &'a Mutex<DetectionEngine>, se: &'a Mutex<DetectionEngine>) -> ServiceContext<'a> {
        ServiceContext { db, app_data_dir: d, embedding_engine: ee, detection_engine: de, safety_engine: se, secrets: s, app_handle: None }
    }

    #[test]
    fn test_get_detections_empty() {
        let (db, s, d, ee, de, se) = make_ctx_parts();
        let c = ctx(&db, &s, &d, &ee, &de, &se);
        let result = get_detections(&c, "nonexistent_img", None).unwrap();
        assert!(result.is_empty());
    }

    #[test]
    fn test_get_vision_metadata_empty() {
        let (db, s, d, ee, de, se) = make_ctx_parts();
        let c = ctx(&db, &s, &d, &ee, &de, &se);
        let result = get_vision_metadata(&c, "nonexistent_img").unwrap();
        assert!(result.is_empty());
    }

    #[test]
    fn test_get_embedding_count_zero() {
        let (db, s, d, ee, de, se) = make_ctx_parts();
        let c = ctx(&db, &s, &d, &ee, &de, &se);
        assert_eq!(get_embedding_count(&c, None).unwrap(), 0);
    }

    #[test]
    fn test_get_embedding_count_with_model() {
        let (db, s, d, ee, de, se) = make_ctx_parts();
        let c = ctx(&db, &s, &d, &ee, &de, &se);
        assert_eq!(get_embedding_count(&c, Some("clip-vit-b32")).unwrap(), 0);
        assert_eq!(get_embedding_count(&c, Some("gemini-embedding-2")).unwrap(), 0);
    }

    #[test]
    fn test_is_clip_available_initially_false() {
        let (db, s, d, ee, de, se) = make_ctx_parts();
        let c = ctx(&db, &s, &d, &ee, &de, &se);
        assert!(!is_clip_available(&c).unwrap());
    }

    #[test]
    fn test_get_detection_count_zero() {
        let (db, s, d, ee, de, se) = make_ctx_parts();
        let c = ctx(&db, &s, &d, &ee, &de, &se);
        assert_eq!(get_detection_count(&c, "yolov8m").unwrap(), 0);
    }

    #[test]
    fn test_get_vision_count_zero() {
        let (db, s, d, ee, de, se) = make_ctx_parts();
        let c = ctx(&db, &s, &d, &ee, &de, &se);
        assert_eq!(get_vision_count(&c, None).unwrap(), 0);
        assert_eq!(get_vision_count(&c, Some("minicpm-v")).unwrap(), 0);
    }

    #[test]
    fn test_get_all_embeddings_empty() {
        let (db, s, d, ee, de, se) = make_ctx_parts();
        let c = ctx(&db, &s, &d, &ee, &de, &se);
        let result = get_all_embeddings(&c, None).unwrap();
        assert!(result.is_empty());
    }

    #[test]
    fn test_find_similar_no_embedding() {
        let (db, s, d, ee, de, se) = make_ctx_parts();
        let c = ctx(&db, &s, &d, &ee, &de, &se);
        let result = find_similar_images(&c, "no_such_img", 10, None);
        assert!(result.is_err());
        match result.unwrap_err() {
            ServiceError::NotFound(msg) => assert!(msg.contains("no embedding")),
            other => panic!("Expected NotFound, got {:?}", other),
        }
    }

    #[test]
    fn test_search_by_detected_class_empty() {
        let (db, s, d, ee, de, se) = make_ctx_parts();
        let c = ctx(&db, &s, &d, &ee, &de, &se);
        let result = search_by_detected_class(&c, "person", 50).unwrap();
        assert!(result.is_empty());
    }
}
