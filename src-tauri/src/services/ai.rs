use crate::db_core::detection::Detection;
use crate::db_core::models::{
    EmbeddingPage, ImagePerceptualHash, ImageQualityMetrics, ImageWithFile, NearDuplicateImage,
    SimilarityGroupSummary, SimilarityGroupingResult,
};
use crate::db_core::perceptual_hash::{self, PHASH_ALGORITHM};
use crate::db_core::quality;
use crate::services::library::enrich_thumbnails;
use crate::services::{Pagination, ServiceContext, ServiceError};
use std::collections::HashSet;

const MAX_EMBEDDING_PAGE_SIZE: u32 = 5000;
const SIMILARITY_GROUPING_METHOD: &str = "greedy_threshold_v1";

pub fn find_similar_images(
    ctx: &ServiceContext,
    image_id: &str,
    top_k: usize,
    model: Option<&str>,
) -> Result<Vec<(String, f32)>, ServiceError> {
    let model_name = model.unwrap_or("clip-vit-b32");
    let query = ctx
        .db
        .get_embedding_vector(image_id, model_name)?
        .ok_or_else(|| ServiceError::NotFound("Image has no embedding".into()))?;
    Ok(ctx.db.find_similar(&query, model_name, top_k)?)
}

pub fn get_all_embeddings(
    ctx: &ServiceContext,
    model: Option<&str>,
) -> Result<Vec<(String, Vec<f32>)>, ServiceError> {
    let model_name = model.unwrap_or("clip-vit-b32");
    Ok(ctx.db.get_all_embeddings(model_name)?)
}

pub fn get_embedding_page(
    ctx: &ServiceContext,
    model: Option<&str>,
    page: Pagination,
) -> Result<EmbeddingPage, ServiceError> {
    let model_name = model.unwrap_or("clip-vit-b32");
    let limit = page.limit.clamp(1, MAX_EMBEDDING_PAGE_SIZE);
    Ok(ctx.db.get_embedding_page(model_name, limit, page.offset)?)
}

pub fn get_embedding_count(ctx: &ServiceContext, model: Option<&str>) -> Result<u32, ServiceError> {
    let model_name = model.unwrap_or("clip-vit-b32");
    Ok(ctx.db.embedding_count(model_name)?)
}

pub fn generate_similarity_groups(
    ctx: &ServiceContext,
    model: Option<&str>,
    threshold: f64,
    min_group_size: u32,
) -> Result<SimilarityGroupingResult, ServiceError> {
    if !(0.0..=1.0).contains(&threshold) {
        return Err(ServiceError::InvalidInput(
            "Similarity threshold must be between 0.0 and 1.0".to_string(),
        ));
    }

    let model_name = model.unwrap_or("clip-vit-b32");
    let min_group_size = min_group_size.max(2) as usize;
    let embeddings = ctx.db.get_all_embeddings(model_name)?;
    let mut assigned: HashSet<String> = HashSet::new();
    let mut groups: Vec<Vec<(String, f32)>> = Vec::new();
    let mut singleton_images = 0u32;

    for (seed_id, seed_vector) in &embeddings {
        if assigned.contains(seed_id) {
            continue;
        }

        let mut members = vec![(seed_id.clone(), 1.0f32)];
        for (candidate_id, candidate_vector) in &embeddings {
            if candidate_id == seed_id || assigned.contains(candidate_id) {
                continue;
            }
            let score = cosine_similarity(seed_vector, candidate_vector);
            if score >= threshold as f32 {
                members.push((candidate_id.clone(), score));
            }
        }

        if members.len() >= min_group_size {
            members.sort_by(|a, b| {
                b.1.partial_cmp(&a.1)
                    .unwrap_or(std::cmp::Ordering::Equal)
                    .then_with(|| a.0.cmp(&b.0))
            });
            for (image_id, _) in &members {
                assigned.insert(image_id.clone());
            }
            groups.push(members);
        } else {
            assigned.insert(seed_id.clone());
            singleton_images += 1;
        }
    }

    groups.sort_by(|a, b| b.len().cmp(&a.len()).then_with(|| a[0].0.cmp(&b[0].0)));

    Ok(ctx.db.replace_similarity_groups(
        model_name,
        threshold,
        SIMILARITY_GROUPING_METHOD,
        &groups,
        singleton_images,
    )?)
}

pub fn list_similarity_groups(
    ctx: &ServiceContext,
    page: Pagination,
) -> Result<Vec<SimilarityGroupSummary>, ServiceError> {
    let page = Pagination::clamped(page.offset, page.limit);
    Ok(ctx.db.list_similarity_groups(page.limit, page.offset)?)
}

pub fn list_similarity_group_images(
    ctx: &ServiceContext,
    group_id: &str,
) -> Result<Vec<ImageWithFile>, ServiceError> {
    let mut images = ctx.db.list_similarity_group_images(group_id)?;
    enrich_thumbnails(&mut images, ctx.app_data_dir);
    Ok(images)
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

pub fn count_by_detected_class(
    ctx: &ServiceContext,
    class_name: &str,
) -> Result<u32, ServiceError> {
    Ok(ctx.db.count_by_class(class_name)?)
}

pub fn list_images_by_detected_class(
    ctx: &ServiceContext,
    class_name: &str,
    page: Pagination,
) -> Result<Vec<ImageWithFile>, ServiceError> {
    let page = Pagination::clamped(page.offset, page.limit);
    let mut images = ctx
        .db
        .list_images_by_class(class_name, page.limit, page.offset)?;
    enrich_thumbnails(&mut images, ctx.app_data_dir);
    Ok(images)
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

pub fn analyze_image_quality(
    ctx: &ServiceContext,
    image_id: &str,
) -> Result<ImageQualityMetrics, ServiceError> {
    let image = crate::services::library::get_image(ctx, image_id)?;
    let ml_path = crate::commands::resolve_image_path_for_ml(&image, ctx.app_data_dir);
    let metrics =
        quality::analyze_image_quality(image_id, &ml_path).map_err(ServiceError::Engine)?;
    ctx.db.store_image_quality_metrics(&metrics)?;
    Ok(metrics)
}

pub fn get_image_quality(
    ctx: &ServiceContext,
    image_id: &str,
) -> Result<Option<ImageQualityMetrics>, ServiceError> {
    Ok(ctx.db.get_image_quality_metrics(image_id)?)
}

pub fn get_quality_count(ctx: &ServiceContext) -> Result<u32, ServiceError> {
    Ok(ctx.db.quality_metrics_count()?)
}

pub fn analyze_image_perceptual_hash(
    ctx: &ServiceContext,
    image_id: &str,
) -> Result<ImagePerceptualHash, ServiceError> {
    let image = crate::services::library::get_image(ctx, image_id)?;
    let ml_path = crate::commands::resolve_image_path_for_ml(&image, ctx.app_data_dir);
    let hash = perceptual_hash::analyze_image_perceptual_hash(image_id, &ml_path)
        .map_err(ServiceError::Engine)?;
    ctx.db.store_image_perceptual_hash(&hash)?;
    Ok(hash)
}

pub fn get_image_perceptual_hash(
    ctx: &ServiceContext,
    image_id: &str,
    algorithm: Option<&str>,
) -> Result<Option<ImagePerceptualHash>, ServiceError> {
    Ok(ctx
        .db
        .get_image_perceptual_hash(image_id, algorithm.unwrap_or(PHASH_ALGORITHM))?)
}

pub fn get_perceptual_hash_count(
    ctx: &ServiceContext,
    algorithm: Option<&str>,
) -> Result<u32, ServiceError> {
    Ok(ctx
        .db
        .perceptual_hash_count(algorithm.unwrap_or(PHASH_ALGORITHM))?)
}

pub fn find_near_duplicates_by_phash(
    ctx: &ServiceContext,
    image_id: &str,
    max_distance: u32,
    limit: u32,
    algorithm: Option<&str>,
) -> Result<Vec<NearDuplicateImage>, ServiceError> {
    let mut duplicates = ctx.db.find_near_duplicates_by_phash(
        image_id,
        algorithm.unwrap_or(PHASH_ALGORITHM),
        max_distance,
        limit,
    )?;
    let mut images: Vec<ImageWithFile> = duplicates.iter().map(|d| d.image.clone()).collect();
    enrich_thumbnails(&mut images, ctx.app_data_dir);
    for (duplicate, image) in duplicates.iter_mut().zip(images) {
        duplicate.image = image;
    }
    Ok(duplicates)
}

fn cosine_similarity(a: &[f32], b: &[f32]) -> f32 {
    if a.len() != b.len() {
        return 0.0;
    }
    let dot: f32 = a.iter().zip(b.iter()).map(|(x, y)| x * y).sum();
    let norm_a: f32 = a.iter().map(|x| x * x).sum::<f32>().sqrt();
    let norm_b: f32 = b.iter().map(|x| x * x).sum::<f32>().sqrt();
    if norm_a == 0.0 || norm_b == 0.0 {
        0.0
    } else {
        dot / (norm_a * norm_b)
    }
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
    fn test_get_embedding_page_returns_flat_limited_vectors() {
        let (db, s, d, ee, de, se, _tmp) = make_ctx_parts();
        insert_test_image(&db, "img1");
        insert_test_image(&db, "img2");
        insert_test_image(&db, "img3");
        db.store_embedding("img1", "clip-vit-b32", &vec![0.1, 0.2])
            .unwrap();
        db.store_embedding("img2", "clip-vit-b32", &vec![0.3, 0.4])
            .unwrap();
        db.store_embedding("img3", "clip-vit-b32", &vec![0.5, 0.6])
            .unwrap();
        let c = ctx(&db, &s, &d, &ee, &de, &se);

        let page = get_embedding_page(
            &c,
            None,
            Pagination {
                offset: 1,
                limit: 1,
            },
        )
        .unwrap();

        assert_eq!(page.ids, vec!["img2".to_string()]);
        assert_eq!(page.vectors, vec![0.3, 0.4]);
        assert_eq!(page.dims, 2);
        assert_eq!(page.total, 3);
        assert_eq!(page.offset, 1);
        assert_eq!(page.limit, 1);
        assert!(page.has_more);
    }

    #[test]
    fn test_generate_similarity_groups_from_embeddings() {
        let (db, s, d, ee, de, se, _tmp) = make_ctx_parts();
        insert_test_image(&db, "img1");
        insert_test_image(&db, "img2");
        insert_test_image(&db, "img3");
        db.store_embedding("img1", "clip-vit-b32", &vec![1.0, 0.0])
            .unwrap();
        db.store_embedding("img2", "clip-vit-b32", &vec![0.99, 0.01])
            .unwrap();
        db.store_embedding("img3", "clip-vit-b32", &vec![0.0, 1.0])
            .unwrap();

        let c = ctx(&db, &s, &d, &ee, &de, &se);
        let result = generate_similarity_groups(&c, None, 0.95, 2).unwrap();
        assert_eq!(result.groups_created, 1);
        assert_eq!(result.images_grouped, 2);
        assert_eq!(result.singleton_images, 1);

        let groups = list_similarity_groups(
            &c,
            Pagination {
                offset: 0,
                limit: 10,
            },
        )
        .unwrap();
        assert_eq!(groups.len(), 1);
        let images = list_similarity_group_images(&c, &groups[0].id).unwrap();
        let ids: Vec<&str> = images.iter().map(|img| img.image.id.as_str()).collect();
        assert_eq!(ids, vec!["img1", "img2"]);
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
