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
    let query = all
        .iter()
        .find(|(id, _)| id == image_id)
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

pub fn get_embedding_count(ctx: &ServiceContext, model: Option<&str>) -> Result<u32, ServiceError> {
    let model_name = model.unwrap_or("clip-vit-b32");
    Ok(ctx.db.embedding_count(model_name)?)
}

pub fn is_clip_available(ctx: &ServiceContext) -> Result<bool, ServiceError> {
    let engine = ctx.embedding_engine.lock();
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
    use crate::db_core::detection::DetectionEngine;
    use crate::db_core::embeddings::EmbeddingEngine;
    use crate::db_core::secrets::MemoryStore;
    use parking_lot::Mutex;
    use std::path::PathBuf;

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
        let dir = tmp.path().to_path_buf();
        let mdir = tmp.path().join("models");
        (
            db,
            secrets,
            dir,
            Mutex::new(EmbeddingEngine::new(&mdir)),
            Mutex::new(DetectionEngine::new_yolo(&mdir)),
            Mutex::new(DetectionEngine::new_nudenet(&mdir)),
            tmp,
        )
    }

    fn ctx<'a>(
        db: &'a Database,
        s: &'a MemoryStore,
        d: &'a PathBuf,
        ee: &'a Mutex<EmbeddingEngine>,
        de: &'a Mutex<DetectionEngine>,
        se: &'a Mutex<DetectionEngine>,
    ) -> ServiceContext<'a> {
        ServiceContext {
            db,
            app_data_dir: d,
            embedding_engine: ee,
            detection_engine: de,
            safety_engine: se,
            secrets: s,
            app_handle: None,
        }
    }

    #[test]
    fn test_get_detections_empty() {
        let (db, s, d, ee, de, se, _tmp) = make_ctx_parts();
        let c = ctx(&db, &s, &d, &ee, &de, &se);
        let result = get_detections(&c, "nonexistent_img", None).unwrap();
        assert!(result.is_empty());
    }

    #[test]
    fn test_get_vision_metadata_empty() {
        let (db, s, d, ee, de, se, _tmp) = make_ctx_parts();
        let c = ctx(&db, &s, &d, &ee, &de, &se);
        let result = get_vision_metadata(&c, "nonexistent_img").unwrap();
        assert!(result.is_empty());
    }

    #[test]
    fn test_get_embedding_count_zero() {
        let (db, s, d, ee, de, se, _tmp) = make_ctx_parts();
        let c = ctx(&db, &s, &d, &ee, &de, &se);
        assert_eq!(get_embedding_count(&c, None).unwrap(), 0);
    }

    #[test]
    fn test_get_embedding_count_with_model() {
        let (db, s, d, ee, de, se, _tmp) = make_ctx_parts();
        let c = ctx(&db, &s, &d, &ee, &de, &se);
        assert_eq!(get_embedding_count(&c, Some("clip-vit-b32")).unwrap(), 0);
        assert_eq!(
            get_embedding_count(&c, Some("gemini-embedding-2")).unwrap(),
            0
        );
    }

    #[test]
    fn test_is_clip_available_initially_false() {
        let (db, s, d, ee, de, se, _tmp) = make_ctx_parts();
        let c = ctx(&db, &s, &d, &ee, &de, &se);
        assert!(!is_clip_available(&c).unwrap());
    }

    #[test]
    fn test_get_detection_count_zero() {
        let (db, s, d, ee, de, se, _tmp) = make_ctx_parts();
        let c = ctx(&db, &s, &d, &ee, &de, &se);
        assert_eq!(get_detection_count(&c, "yolov8m").unwrap(), 0);
    }

    #[test]
    fn test_get_vision_count_zero() {
        let (db, s, d, ee, de, se, _tmp) = make_ctx_parts();
        let c = ctx(&db, &s, &d, &ee, &de, &se);
        assert_eq!(get_vision_count(&c, None).unwrap(), 0);
        assert_eq!(get_vision_count(&c, Some("minicpm-v")).unwrap(), 0);
    }

    #[test]
    fn test_get_all_embeddings_empty() {
        let (db, s, d, ee, de, se, _tmp) = make_ctx_parts();
        let c = ctx(&db, &s, &d, &ee, &de, &se);
        let result = get_all_embeddings(&c, None).unwrap();
        assert!(result.is_empty());
    }

    #[test]
    fn test_find_similar_no_embedding() {
        let (db, s, d, ee, de, se, _tmp) = make_ctx_parts();
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
        let (db, s, d, ee, de, se, _tmp) = make_ctx_parts();
        let c = ctx(&db, &s, &d, &ee, &de, &se);
        let result = search_by_detected_class(&c, "person", 50).unwrap();
        assert!(result.is_empty());
    }

    fn insert_test_image(db: &Database, id: &str) {
        let conn = db.conn.lock();
        conn.execute(
            "INSERT INTO images (id, sha256_hash, width, height, format, file_size, created_at, imported_at, ai_prompt) VALUES (?1, ?2, 100, 100, 'png', 1000, '2026-01-01', '2026-01-01', NULL)",
            rusqlite::params![id, format!("hash_{}", id)],
        ).unwrap();
        conn.execute(
            "INSERT INTO image_files (id, image_id, path, last_seen_at) VALUES (?1, ?2, ?3, '2026-01-01')",
            rusqlite::params![format!("f_{}", id), id, format!("/test/{}.png", id)],
        ).unwrap();
    }

    #[test]
    fn test_get_embedding_count_with_data() {
        let (db, s, d, ee, de, se, _tmp) = make_ctx_parts();
        insert_test_image(&db, "img1");
        db.store_embedding("img1", "clip-vit-b32", &vec![0.1, 0.2, 0.3])
            .unwrap();
        let c = ctx(&db, &s, &d, &ee, &de, &se);
        assert_eq!(get_embedding_count(&c, None).unwrap(), 1);
    }

    #[test]
    fn test_get_all_embeddings_with_data() {
        let (db, s, d, ee, de, se, _tmp) = make_ctx_parts();
        insert_test_image(&db, "img1");
        insert_test_image(&db, "img2");
        db.store_embedding("img1", "clip-vit-b32", &vec![0.1, 0.2, 0.3])
            .unwrap();
        db.store_embedding("img2", "clip-vit-b32", &vec![0.4, 0.5, 0.6])
            .unwrap();
        let c = ctx(&db, &s, &d, &ee, &de, &se);
        let embs = get_all_embeddings(&c, None).unwrap();
        assert_eq!(embs.len(), 2);
    }

    #[test]
    fn test_get_detections_with_data() {
        let (db, s, d, ee, de, se, _tmp) = make_ctx_parts();
        insert_test_image(&db, "img1");
        let detections = vec![crate::db_core::detection::Detection {
            class_name: "person".to_string(),
            confidence: 0.95,
            x: 10.0,
            y: 20.0,
            width: 50.0,
            height: 80.0,
        }];
        db.store_detections("img1", "yolov8m", &detections).unwrap();
        let c = ctx(&db, &s, &d, &ee, &de, &se);
        let result = get_detections(&c, "img1", None).unwrap();
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].class_name, "person");
    }

    #[test]
    fn test_search_by_detected_class_with_data() {
        let (db, s, d, ee, de, se, _tmp) = make_ctx_parts();
        insert_test_image(&db, "img1");
        let detections = vec![crate::db_core::detection::Detection {
            class_name: "car".to_string(),
            confidence: 0.88,
            x: 0.0,
            y: 0.0,
            width: 100.0,
            height: 100.0,
        }];
        db.store_detections("img1", "yolov8m", &detections).unwrap();
        let c = ctx(&db, &s, &d, &ee, &de, &se);
        let result = search_by_detected_class(&c, "car", 50).unwrap();
        assert_eq!(result.len(), 1);
        assert!(result[0].1 > 0.8);
    }

    #[test]
    fn test_get_vision_metadata_with_data() {
        let (db, s, d, ee, de, se, _tmp) = make_ctx_parts();
        insert_test_image(&db, "img1");
        let mut fields = std::collections::HashMap::new();
        fields.insert(
            "description".to_string(),
            "A sunset over the ocean".to_string(),
        );
        fields.insert("tags".to_string(), "sunset, ocean, nature".to_string());
        db.store_vision_metadata("img1", "minicpm-v", &fields)
            .unwrap();
        let c = ctx(&db, &s, &d, &ee, &de, &se);
        let result = get_vision_metadata(&c, "img1").unwrap();
        assert_eq!(result.len(), 2);
    }

    #[test]
    fn test_get_vision_count_with_data() {
        let (db, s, d, ee, de, se, _tmp) = make_ctx_parts();
        insert_test_image(&db, "img1");
        let mut fields = std::collections::HashMap::new();
        fields.insert("desc".to_string(), "test".to_string());
        db.store_vision_metadata("img1", "minicpm-v", &fields)
            .unwrap();
        let c = ctx(&db, &s, &d, &ee, &de, &se);
        assert_eq!(get_vision_count(&c, Some("minicpm-v")).unwrap(), 1);
    }
}
