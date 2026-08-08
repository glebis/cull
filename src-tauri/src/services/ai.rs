use crate::db_core::color;
use crate::db_core::db::Database;
use crate::db_core::detection::Detection;
use crate::db_core::models::{
    EmbeddingClusterMembership, EmbeddingClusterName, EmbeddingPage, EmbeddingScope,
    ImageColorMetrics, ImageIdPage, ImagePerceptualHash, ImageQualityMetrics, ImageWithFile,
    NearDuplicateImage, SimilarityGroupSummary, SimilarityGroupingResult,
};
use crate::db_core::perceptual_hash::{self, PHASH_ALGORITHM};
use crate::db_core::quality;
use crate::services::library::enrich_thumbnails;
use crate::services::{Pagination, ServiceContext, ServiceError};
use std::collections::HashSet;

const MAX_EMBEDDING_PAGE_SIZE: u32 = 5000;
const MAX_SIMILAR_IMAGES: usize = 100;
const SIMILARITY_GROUPING_METHOD: &str = "greedy_threshold_v1";

#[derive(Debug, Clone, serde::Deserialize, schemars::JsonSchema)]
pub struct SearchByObjectParams {
    #[schemars(description = "Object class to search for, e.g. 'person', 'car', 'dog'")]
    pub class_name: String,
    #[schemars(description = "Max results (default 50, max 100)")]
    pub limit: Option<u32>,
}

#[derive(Debug, Clone, PartialEq, serde::Serialize)]
pub struct SearchByObjectMatch {
    pub image_id: String,
    pub confidence: f32,
}

#[derive(Debug, Clone, serde::Deserialize, schemars::JsonSchema)]
pub struct FindSimilarParams {
    #[schemars(description = "Image ID to find similar images for")]
    pub image_id: String,
    #[schemars(description = "Number of results to return (default 10, max 100)")]
    pub limit: Option<u32>,
    #[schemars(description = "Embedding model: 'clip-vit-b32' or 'dinov2-vits14'")]
    pub model: Option<String>,
}

#[derive(Debug, Clone, PartialEq, serde::Serialize)]
pub struct FindSimilarMatch {
    pub image_id: String,
    pub similarity: f32,
    pub model: String,
}

pub fn find_similar_in_database(
    db: &Database,
    params: &FindSimilarParams,
) -> Result<Vec<FindSimilarMatch>, ServiceError> {
    let (image_id, limit, model) = validate_find_similar(params)?;
    let results = db
        .find_similar_live(image_id, model, limit)?
        .ok_or_else(|| missing_embedding_error(image_id, model))?;
    Ok(similarity_matches(results, model))
}

pub fn find_similar_in_token_scope_database(
    db: &Database,
    params: &FindSimilarParams,
    folders: &[String],
    collections: &[String],
    tag_norms: &[String],
) -> Result<Vec<FindSimilarMatch>, ServiceError> {
    let (image_id, limit, model) = validate_find_similar(params)?;
    let results = db
        .find_similar_in_token_scope(image_id, model, folders, collections, tag_norms, limit)?
        .ok_or_else(|| missing_embedding_error(image_id, model))?;
    Ok(similarity_matches(results, model))
}

fn validate_find_similar(params: &FindSimilarParams) -> Result<(&str, usize, &str), ServiceError> {
    let image_id = validated_find_similar_image_id(params)?;
    let model = params.model.as_deref().unwrap_or("clip-vit-b32").trim();
    if crate::db_core::embeddings::embedding_model_spec(model).is_none() {
        return Err(ServiceError::InvalidInput(format!(
            "Unsupported embedding model '{model}'"
        )));
    }
    Ok((
        image_id,
        params
            .limit
            .unwrap_or(10)
            .clamp(1, MAX_SIMILAR_IMAGES as u32) as usize,
        model,
    ))
}

pub fn validated_find_similar_image_id(params: &FindSimilarParams) -> Result<&str, ServiceError> {
    let image_id = params.image_id.trim();
    if image_id.is_empty() {
        return Err(ServiceError::InvalidInput(
            "image_id must not be empty".to_string(),
        ));
    }
    Ok(image_id)
}

fn missing_embedding_error(image_id: &str, model: &str) -> ServiceError {
    ServiceError::NotFound(format!(
        "Image '{image_id}' has no '{model}' embedding. Run generate_embeddings first."
    ))
}

fn similarity_matches(results: Vec<(String, f32)>, model: &str) -> Vec<FindSimilarMatch> {
    results
        .into_iter()
        .map(|(image_id, similarity)| FindSimilarMatch {
            image_id,
            similarity,
            model: model.to_string(),
        })
        .collect()
}

pub fn search_by_object_in_database(
    db: &Database,
    params: &SearchByObjectParams,
) -> Result<Vec<SearchByObjectMatch>, ServiceError> {
    let (class_name, limit) = validate_object_search(params)?;
    Ok(object_search_matches(
        db.search_by_class(class_name, limit)?,
    ))
}

pub fn search_by_object_in_scope_database(
    db: &Database,
    params: &SearchByObjectParams,
    folders: &[String],
    collections: &[String],
    tag_norms: &[String],
) -> Result<Vec<SearchByObjectMatch>, ServiceError> {
    let (class_name, limit) = validate_object_search(params)?;
    Ok(object_search_matches(db.search_by_class_in_scope(
        class_name,
        folders,
        collections,
        tag_norms,
        limit,
    )?))
}

fn validate_object_search(params: &SearchByObjectParams) -> Result<(&str, u32), ServiceError> {
    let class_name = params.class_name.trim();
    if class_name.is_empty() {
        return Err(ServiceError::InvalidInput(
            "class_name must not be empty".to_string(),
        ));
    }
    Ok((class_name, params.limit.unwrap_or(50).clamp(1, 100)))
}

fn object_search_matches(results: Vec<(String, f32)>) -> Vec<SearchByObjectMatch> {
    results
        .into_iter()
        .map(|(image_id, confidence)| SearchByObjectMatch {
            image_id,
            confidence,
        })
        .collect()
}

/// Upper bound on the number of embeddings `generate_similarity_groups` will
/// process in a single call. Grouping is O(N^2) pairwise comparisons, so at
/// 25,000 embeddings that's already ~312 million comparisons; beyond this the
/// CPU/memory cost becomes impractical for a single request.
/// `generate_similarity_groups` has no scope parameter, so callers hitting
/// this limit must reduce the number of embedded images in the library, or
/// run grouping on a smaller model set.
const MAX_SIMILARITY_GROUP_EMBEDDINGS: usize = 25_000;

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

pub fn find_similar_images_in_scope(
    ctx: &ServiceContext,
    scope: &EmbeddingScope,
    image_id: &str,
    top_k: usize,
    model: Option<&str>,
) -> Result<Vec<(String, f32)>, ServiceError> {
    let model_name = model.unwrap_or("clip-vit-b32");
    let top_k = top_k.min(MAX_SIMILAR_IMAGES);
    ctx.db
        .find_similar_in_scope(image_id, model_name, scope, top_k)?
        .ok_or_else(|| ServiceError::NotFound("Image has no embedding".into()))
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

pub fn get_scoped_embedding_page(
    ctx: &ServiceContext,
    scope: &EmbeddingScope,
    model: Option<&str>,
    page: Pagination,
) -> Result<EmbeddingPage, ServiceError> {
    let model_name = model.unwrap_or("clip-vit-b32");
    let limit = page.limit.clamp(1, MAX_EMBEDDING_PAGE_SIZE);
    Ok(ctx
        .db
        .get_scoped_embedding_page(model_name, scope, limit, page.offset)?)
}

pub fn list_scoped_image_ids(
    ctx: &ServiceContext,
    scope: &EmbeddingScope,
    page: Pagination,
) -> Result<ImageIdPage, ServiceError> {
    let limit = page.limit.clamp(1, 100);
    Ok(ctx.db.list_scoped_image_ids(scope, limit, page.offset)?)
}

fn validate_embedding_cluster_memberships(
    clusters: &[EmbeddingClusterMembership],
) -> Result<(), ServiceError> {
    if clusters.len() > 16 {
        return Err(ServiceError::InvalidInput(
            "At most 16 embedding clusters can be named at once".to_string(),
        ));
    }
    let total_ids = clusters.iter().try_fold(0usize, |total, cluster| {
        total
            .checked_add(cluster.image_ids.len())
            .ok_or_else(|| ServiceError::InvalidInput("Too many cluster image IDs".to_string()))
    })?;
    if total_ids > 5000 {
        return Err(ServiceError::InvalidInput(
            "At most 5000 projected image IDs can be named at once".to_string(),
        ));
    }
    Ok(())
}

pub fn name_embedding_clusters(
    ctx: &ServiceContext,
    clusters: &[EmbeddingClusterMembership],
) -> Result<Vec<EmbeddingClusterName>, ServiceError> {
    validate_embedding_cluster_memberships(clusters)?;
    Ok(ctx.db.name_embedding_clusters(clusters)?)
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
    let (groups, singleton_images) = build_similarity_groups(
        embeddings,
        threshold,
        min_group_size,
        MAX_SIMILARITY_GROUP_EMBEDDINGS,
    )?;

    Ok(ctx.db.replace_similarity_groups(
        model_name,
        threshold,
        SIMILARITY_GROUPING_METHOD,
        &groups,
        singleton_images,
    )?)
}

type SimilarityGroups = Vec<Vec<(String, f32)>>;

/// Core grouping logic, factored out so the embedding-count cap can be
/// exercised in tests with a small `max_embeddings` value instead of
/// requiring an unwieldy 25k-row fixture.
fn build_similarity_groups(
    embeddings: Vec<(String, Vec<f32>)>,
    threshold: f64,
    min_group_size: usize,
    max_embeddings: usize,
) -> Result<(SimilarityGroups, u32), ServiceError> {
    if embeddings.len() > max_embeddings {
        return Err(ServiceError::InvalidInput(format!(
            "Too many embeddings ({}) for similarity grouping; the pairwise comparison is \
             O(N^2) and is capped at {} per run. Reduce the number of embedded images in the \
             library, or run grouping on a smaller model set.",
            embeddings.len(),
            max_embeddings
        )));
    }

    // Normalize every vector once up front so the O(N^2) inner loop can use
    // a plain dot product instead of recomputing both vectors' norms on
    // every pairwise comparison.
    let normalized = l2_normalize_all(embeddings);

    let mut assigned: HashSet<String> = HashSet::new();
    let mut groups: Vec<Vec<(String, f32)>> = Vec::new();
    let mut singleton_images = 0u32;

    for (seed_id, seed_vector) in &normalized {
        if assigned.contains(seed_id) {
            continue;
        }

        let mut members = vec![(seed_id.clone(), 1.0f32)];
        for (candidate_id, candidate_vector) in &normalized {
            if candidate_id == seed_id || assigned.contains(candidate_id) {
                continue;
            }
            let score = normalized_dot(seed_vector, candidate_vector);
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

    Ok((groups, singleton_images))
}

/// L2-normalizes every vector in place. Zero-norm vectors are left as-is
/// (all zero components): `normalized_dot` against a zero vector always
/// yields 0.0, matching the "no match" behavior of the original
/// `dot / (norm_a * norm_b)` cosine similarity when either norm was zero.
fn l2_normalize_all(mut vectors: Vec<(String, Vec<f32>)>) -> Vec<(String, Vec<f32>)> {
    for (_, v) in vectors.iter_mut() {
        let norm: f32 = v.iter().map(|x| x * x).sum::<f32>().sqrt();
        if norm > 0.0 {
            for x in v.iter_mut() {
                *x /= norm;
            }
        }
    }
    vectors
}

/// Dot product of two already-L2-normalized vectors, equivalent to cosine
/// similarity. Mismatched lengths return 0.0, matching the original
/// `cosine_similarity`'s guard.
fn normalized_dot(a: &[f32], b: &[f32]) -> f32 {
    if a.len() != b.len() {
        return 0.0;
    }
    a.iter().zip(b.iter()).map(|(x, y)| x * y).sum()
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
pub fn count_by_detected_class_with_visibility(
    ctx: &ServiceContext,
    class_name: &str,
    include_rejected: bool,
) -> Result<u32, ServiceError> {
    Ok(ctx
        .db
        .count_by_class_with_visibility(class_name, include_rejected)?)
}

pub fn list_detected_classes(ctx: &ServiceContext) -> Result<Vec<(String, u32)>, ServiceError> {
    Ok(ctx.db.list_detected_classes()?)
}
pub fn list_detected_classes_with_visibility(
    ctx: &ServiceContext,
    include_rejected: bool,
) -> Result<Vec<(String, u32)>, ServiceError> {
    Ok(ctx
        .db
        .list_detected_classes_with_visibility(include_rejected)?)
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
pub fn list_images_by_detected_class_with_visibility(
    ctx: &ServiceContext,
    class_name: &str,
    page: Pagination,
    include_rejected: bool,
) -> Result<Vec<ImageWithFile>, ServiceError> {
    let page = Pagination::clamped(page.offset, page.limit);
    let mut images = ctx.db.list_images_by_class_with_visibility(
        class_name,
        page.limit,
        page.offset,
        include_rejected,
    )?;
    enrich_thumbnails(&mut images, ctx.app_data_dir);
    Ok(images)
}

pub fn get_detection_count(ctx: &ServiceContext, model: &str) -> Result<u32, ServiceError> {
    Ok(ctx.db.detection_count(model)?)
}

pub fn get_pending_detection_ids(
    ctx: &ServiceContext,
    model: &str,
) -> Result<Vec<String>, ServiceError> {
    Ok(ctx.db.list_image_ids_missing_detection(model)?)
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

pub fn get_pending_vision_ids(
    ctx: &ServiceContext,
    source: &str,
) -> Result<Vec<String>, ServiceError> {
    Ok(ctx.db.list_image_ids_missing_vision(source)?)
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

pub fn analyze_image_color_metrics(
    ctx: &ServiceContext,
    image_id: &str,
) -> Result<ImageColorMetrics, ServiceError> {
    let image = crate::services::library::get_image(ctx, image_id)?;
    let ml_path = crate::commands::resolve_image_path_for_ml(&image, ctx.app_data_dir);
    let metrics =
        color::analyze_image_color_metrics(image_id, &ml_path).map_err(ServiceError::Engine)?;
    ctx.db.store_image_color_metrics(&metrics)?;
    Ok(metrics)
}

pub fn get_image_color_metrics(
    ctx: &ServiceContext,
    image_id: &str,
) -> Result<Option<ImageColorMetrics>, ServiceError> {
    Ok(ctx.db.get_image_color_metrics(image_id)?)
}

pub fn get_color_metrics_count(ctx: &ServiceContext) -> Result<u32, ServiceError> {
    Ok(ctx.db.color_metrics_count()?)
}

pub fn list_images_by_color_bucket(
    ctx: &ServiceContext,
    bucket: &str,
    page: Pagination,
) -> Result<Vec<ImageWithFile>, ServiceError> {
    let page = Pagination::clamped(page.offset, page.limit);
    let mut images = ctx
        .db
        .list_images_by_color_bucket(bucket, page.limit, page.offset)?;
    enrich_thumbnails(&mut images, ctx.app_data_dir);
    Ok(images)
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db_core::db::Database;
    use crate::db_core::detection::DetectionEngine;
    use crate::db_core::embeddings::EmbeddingEngine;
    use crate::db_core::secrets::MemoryStore;
    use parking_lot::Mutex;
    use std::path::PathBuf;

    #[test]
    fn cluster_naming_request_bounds_match_projection_limits() {
        let too_many_clusters = (0..17)
            .map(|cluster_id| EmbeddingClusterMembership {
                cluster_id,
                image_ids: Vec::new(),
            })
            .collect::<Vec<_>>();
        assert!(matches!(
            validate_embedding_cluster_memberships(&too_many_clusters),
            Err(ServiceError::InvalidInput(_))
        ));

        let too_many_ids = vec![EmbeddingClusterMembership {
            cluster_id: 0,
            image_ids: (0..5001).map(|index| format!("image-{index}")).collect(),
        }];
        assert!(matches!(
            validate_embedding_cluster_memberships(&too_many_ids),
            Err(ServiceError::InvalidInput(_))
        ));
    }

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
    fn pending_detection_ids_are_exact_model_and_include_zero_result_scans() {
        let (db, s, d, ee, de, se, _tmp) = make_ctx_parts();
        insert_test_image(&db, "a");
        insert_test_image(&db, "b");
        insert_test_image(&db, "c");
        db.mark_image_analysis_complete("a", "detection", "yolo11m")
            .unwrap();
        db.mark_image_analysis_complete("b", "detection", "yolo11s")
            .unwrap();
        let c = ctx(&db, &s, &d, &ee, &de, &se);

        assert_eq!(
            get_pending_detection_ids(&c, "yolo11m").unwrap(),
            vec!["b".to_string(), "c".to_string()],
        );
    }

    #[test]
    fn pending_vision_ids_are_exact_source() {
        let (db, s, d, ee, de, se, _tmp) = make_ctx_parts();
        insert_test_image(&db, "a");
        insert_test_image(&db, "b");
        db.mark_image_analysis_complete("a", "vision", "minicpm-v")
            .unwrap();
        let c = ctx(&db, &s, &d, &ee, &de, &se);

        assert_eq!(
            get_pending_vision_ids(&c, "llava").unwrap(),
            vec!["a".to_string(), "b".to_string()],
        );
        assert_eq!(
            get_pending_vision_ids(&c, "minicpm-v").unwrap(),
            vec!["b".to_string()],
        );
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
    fn find_similar_shared_params_validate_rank_and_model() {
        let (db, _s, _d, _ee, _de, _se, _tmp) = make_ctx_parts();
        for (id, vector) in [
            ("source", vec![1.0, 0.0]),
            ("near", vec![0.8, 0.6]),
            ("far", vec![0.0, 1.0]),
        ] {
            insert_test_image(&db, id);
            db.store_embedding(id, "clip-vit-b32", &vector).unwrap();
        }

        let matches = find_similar_in_database(
            &db,
            &FindSimilarParams {
                image_id: " source ".to_string(),
                limit: Some(10),
                model: None,
            },
        )
        .unwrap();

        assert_eq!(matches.len(), 2);
        assert_eq!(matches[0].image_id, "near");
        assert!((matches[0].similarity - 0.8).abs() < 0.000_001);
        assert_eq!(matches[0].model, "clip-vit-b32");
        assert_eq!(matches[1].image_id, "far");

        let unsupported = find_similar_in_database(
            &db,
            &FindSimilarParams {
                image_id: "source".to_string(),
                limit: None,
                model: Some("unknown-model".to_string()),
            },
        );
        assert!(matches!(unsupported, Err(ServiceError::InvalidInput(_))));
    }

    #[test]
    fn find_similar_shared_params_reject_empty_and_missing_embeddings() {
        let (db, _s, _d, _ee, _de, _se, _tmp) = make_ctx_parts();
        insert_test_image(&db, "without-vector");

        let empty = find_similar_in_database(
            &db,
            &FindSimilarParams {
                image_id: "   ".to_string(),
                limit: None,
                model: None,
            },
        );
        assert!(matches!(empty, Err(ServiceError::InvalidInput(_))));

        let missing = find_similar_in_database(
            &db,
            &FindSimilarParams {
                image_id: "without-vector".to_string(),
                limit: None,
                model: None,
            },
        );
        assert!(matches!(missing, Err(ServiceError::NotFound(_))));
    }

    #[test]
    fn find_similar_scopes_candidates_before_ranking_and_limit() {
        let (db, _s, _d, _ee, _de, _se, _tmp) = make_ctx_parts();
        insert_test_image(&db, "source");
        db.store_embedding("source", "clip-vit-b32", &[1.0, 0.0])
            .unwrap();
        for index in 0..105 {
            let id = format!("outside-{index:03}");
            insert_test_image(&db, &id);
            db.store_embedding(&id, "clip-vit-b32", &[1.0, (index + 1) as f32 / 10_000.0])
                .unwrap();
        }
        insert_test_image(&db, "inside");
        db.store_embedding("inside", "clip-vit-b32", &[0.0, 1.0])
            .unwrap();
        let collection_id = db
            .create_collection_with_images("Allowed", &["source", "inside"])
            .unwrap();

        let matches = find_similar_in_token_scope_database(
            &db,
            &FindSimilarParams {
                image_id: "source".to_string(),
                limit: Some(2),
                model: None,
            },
            &[],
            &[collection_id],
            &[],
        )
        .unwrap();

        assert_eq!(matches.len(), 1);
        assert_eq!(matches[0].image_id, "inside");
        assert!(matches.iter().all(|item| item.image_id != "source"));
    }

    #[test]
    fn find_similar_defaults_to_ten_and_clamps_to_standard_bounds() {
        let (db, _s, _d, _ee, _de, _se, _tmp) = make_ctx_parts();
        insert_test_image(&db, "source");
        db.store_embedding("source", "clip-vit-b32", &[1.0, 0.0])
            .unwrap();
        for index in 0..105 {
            let id = format!("candidate-{index:03}");
            insert_test_image(&db, &id);
            db.store_embedding(&id, "clip-vit-b32", &[1.0, (index + 1) as f32 / 1_000.0])
                .unwrap();
        }

        let run = |limit| {
            find_similar_in_database(
                &db,
                &FindSimilarParams {
                    image_id: "source".to_string(),
                    limit,
                    model: None,
                },
            )
            .unwrap()
        };

        assert_eq!(run(None).len(), 10);
        assert_eq!(run(Some(0)).len(), 1);
        assert_eq!(run(Some(u32::MAX)).len(), 100);
    }

    #[test]
    fn scoped_neighbor_service_clamps_top_k_to_standard_page_bounds() {
        let (db, s, d, ee, de, se, _tmp) = make_ctx_parts();
        insert_test_image(&db, "source");
        db.store_embedding("source", "clip-vit-b32", &[1.0, 0.0])
            .unwrap();
        for index in 0..101 {
            let id = format!("candidate-{index:03}");
            insert_test_image(&db, &id);
            db.store_embedding(&id, "clip-vit-b32", &[1.0, index as f32 / 1_000.0])
                .unwrap();
        }
        let c = ctx(&db, &s, &d, &ee, &de, &se);
        let scope = EmbeddingScope::All {
            include_rejected: false,
        };

        let minimum = find_similar_images_in_scope(&c, &scope, "source", 0, None).unwrap();
        let maximum = find_similar_images_in_scope(&c, &scope, "source", usize::MAX, None).unwrap();

        assert!(minimum.is_empty());
        assert_eq!(maximum.len(), 100);
        assert!(maximum.iter().all(|(id, _)| id != "source"));
    }

    #[test]
    fn test_search_by_detected_class_empty() {
        let (db, s, d, ee, de, se, _tmp) = make_ctx_parts();
        let c = ctx(&db, &s, &d, &ee, &de, &se);
        let result = search_by_detected_class(&c, "person", 50).unwrap();
        assert!(result.is_empty());
    }

    #[test]
    fn search_by_object_returns_ranked_matches_from_shared_params() {
        let (db, _s, _d, _ee, _de, _se, _tmp) = make_ctx_parts();
        insert_test_image(&db, "lower");
        insert_test_image(&db, "higher");
        db.store_detections(
            "lower",
            "yolo11m",
            &[Detection {
                class_name: "person".to_string(),
                confidence: 0.72,
                x: 0.0,
                y: 0.0,
                width: 1.0,
                height: 1.0,
            }],
        )
        .unwrap();
        db.store_detections(
            "higher",
            "yolo11m",
            &[Detection {
                class_name: "person".to_string(),
                confidence: 0.96,
                x: 0.0,
                y: 0.0,
                width: 1.0,
                height: 1.0,
            }],
        )
        .unwrap();

        let matches = search_by_object_in_database(
            &db,
            &SearchByObjectParams {
                class_name: "person".to_string(),
                limit: Some(10),
            },
        )
        .unwrap();

        assert_eq!(matches.len(), 2);
        assert_eq!(matches[0].image_id, "higher");
        assert_eq!(matches[0].confidence, 0.96);
        assert_eq!(matches[1].image_id, "lower");
        assert_eq!(matches[1].confidence, 0.72);
    }

    #[test]
    fn search_by_object_rejects_an_empty_class_name_before_querying() {
        let (db, _s, _d, _ee, _de, _se, _tmp) = make_ctx_parts();

        let error = search_by_object_in_database(
            &db,
            &SearchByObjectParams {
                class_name: "   ".to_string(),
                limit: None,
            },
        )
        .unwrap_err();

        assert!(matches!(error, ServiceError::InvalidInput(_)));
        assert!(error.to_string().contains("class_name"));
    }

    #[test]
    fn search_by_object_filters_candidates_before_applying_the_result_limit() {
        let (db, _s, _d, _ee, _de, _se, _tmp) = make_ctx_parts();
        for index in 0..105 {
            let id = format!("outside-{index:03}");
            insert_test_image(&db, &id);
            db.store_detections(
                &id,
                "yolo11m",
                &[Detection {
                    class_name: "person".to_string(),
                    confidence: 1.0 - index as f32 / 1_000.0,
                    x: 0.0,
                    y: 0.0,
                    width: 1.0,
                    height: 1.0,
                }],
            )
            .unwrap();
        }
        insert_test_image(&db, "inside");
        db.store_detections(
            "inside",
            "yolo11m",
            &[Detection {
                class_name: "person".to_string(),
                confidence: 0.25,
                x: 0.0,
                y: 0.0,
                width: 1.0,
                height: 1.0,
            }],
        )
        .unwrap();
        let collection_id = db
            .create_collection_with_images("Allowed", &["inside"])
            .unwrap();

        let matches = search_by_object_in_scope_database(
            &db,
            &SearchByObjectParams {
                class_name: "person".to_string(),
                limit: Some(2),
            },
            &[],
            &[collection_id],
            &[],
        )
        .unwrap();

        assert_eq!(matches.len(), 1);
        assert_eq!(matches[0].image_id, "inside");
    }

    #[test]
    fn search_by_object_defaults_to_fifty_and_clamps_to_standard_bounds() {
        let (db, _s, _d, _ee, _de, _se, _tmp) = make_ctx_parts();
        for index in 0..105 {
            let id = format!("image-{index:03}");
            insert_test_image(&db, &id);
            db.store_detections(
                &id,
                "yolo11m",
                &[Detection {
                    class_name: "person".to_string(),
                    confidence: index as f32 / 105.0,
                    x: 0.0,
                    y: 0.0,
                    width: 1.0,
                    height: 1.0,
                }],
            )
            .unwrap();
        }

        let default_results = search_by_object_in_database(
            &db,
            &SearchByObjectParams {
                class_name: "person".to_string(),
                limit: None,
            },
        )
        .unwrap();
        let maximum_results = search_by_object_in_database(
            &db,
            &SearchByObjectParams {
                class_name: "person".to_string(),
                limit: Some(u32::MAX),
            },
        )
        .unwrap();
        let minimum_results = search_by_object_in_database(
            &db,
            &SearchByObjectParams {
                class_name: "person".to_string(),
                limit: Some(0),
            },
        )
        .unwrap();

        assert_eq!(default_results.len(), 50);
        assert_eq!(maximum_results.len(), 100);
        assert_eq!(minimum_results.len(), 1);
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
    fn scoped_embedding_page_service_clamps_limit_to_safe_bounds() {
        let (db, s, d, ee, de, se, _tmp) = make_ctx_parts();
        let c = ctx(&db, &s, &d, &ee, &de, &se);
        let scope = EmbeddingScope::All {
            include_rejected: false,
        };

        let minimum = get_scoped_embedding_page(
            &c,
            &scope,
            None,
            Pagination {
                offset: 0,
                limit: 0,
            },
        )
        .unwrap();
        let maximum = get_scoped_embedding_page(
            &c,
            &scope,
            None,
            Pagination {
                offset: 0,
                limit: u32::MAX,
            },
        )
        .unwrap();

        assert_eq!(minimum.limit, 1);
        assert_eq!(maximum.limit, 5_000);
    }

    #[test]
    fn scoped_image_id_service_paginates_large_scopes_at_one_hundred() {
        let (db, s, d, ee, de, se, _tmp) = make_ctx_parts();
        {
            let conn = db.conn.lock();
            conn.execute_batch(
                "WITH RECURSIVE seq(n) AS (
                    SELECT 1 UNION ALL SELECT n + 1 FROM seq WHERE n < 25001
                 )
                 INSERT INTO images (
                    id, sha256_hash, width, height, format, file_size,
                    created_at, imported_at, ai_prompt
                 )
                 SELECT printf('img-%05d', n), printf('hash-%05d', n), 100, 100,
                        'png', 1000, '2026-01-01', '2026-01-01', NULL
                 FROM seq;

                 WITH RECURSIVE seq(n) AS (
                    SELECT 1 UNION ALL SELECT n + 1 FROM seq WHERE n < 25001
                 )
                 INSERT INTO image_files (id, image_id, path, last_seen_at, missing_at)
                 SELECT printf('file-%05d', n), printf('img-%05d', n),
                        printf('/library/img-%05d.png', n), '2026-01-01', NULL
                 FROM seq;",
            )
            .unwrap();
        }
        let c = ctx(&db, &s, &d, &ee, &de, &se);
        let page = list_scoped_image_ids(
            &c,
            &EmbeddingScope::All {
                include_rejected: false,
            },
            Pagination {
                offset: 25_000,
                limit: u32::MAX,
            },
        )
        .unwrap();

        assert_eq!(page.total, 25_001);
        assert_eq!(page.limit, 100);
        assert_eq!(page.ids, vec!["img-25001"]);
        assert!(!page.has_more);
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

    #[test]
    fn test_build_similarity_groups_rejects_too_many_embeddings() {
        let embeddings = vec![
            ("img1".to_string(), vec![1.0, 0.0]),
            ("img2".to_string(), vec![0.0, 1.0]),
            ("img3".to_string(), vec![1.0, 1.0]),
        ];
        // Cap of 2 with 3 embeddings should be rejected before any pairwise
        // comparison work happens.
        let result = build_similarity_groups(embeddings, 0.9, 2, 2);
        match result {
            Err(ServiceError::InvalidInput(msg)) => {
                assert!(msg.contains("Too many embeddings"));
                assert!(msg.contains('3'));
                assert!(msg.contains('2'));
            }
            other => panic!("Expected InvalidInput cap error, got {:?}", other),
        }
    }

    #[test]
    fn test_build_similarity_groups_within_cap_still_works() {
        let embeddings = vec![
            ("img1".to_string(), vec![1.0, 0.0]),
            ("img2".to_string(), vec![0.99, 0.01]),
        ];
        let (groups, singleton_images) = build_similarity_groups(embeddings, 0.9, 2, 2).unwrap();
        assert_eq!(groups.len(), 1);
        assert_eq!(groups[0].len(), 2);
        assert_eq!(singleton_images, 0);
    }

    #[test]
    fn test_build_similarity_groups_zero_vector_is_never_a_match() {
        // A zero-norm embedding must behave like the original
        // dot / (norm_a * norm_b) cosine similarity, which returns 0.0
        // whenever either operand's norm is zero — i.e. it never matches
        // anything, including another zero vector. (Threshold 0.0 would
        // trivially match everything since cosine scores are >= 0.0 here,
        // so use a small positive threshold to make the guard meaningful.)
        let embeddings = vec![
            ("img_zero_a".to_string(), vec![0.0, 0.0]),
            ("img_zero_b".to_string(), vec![0.0, 0.0]),
            ("img_real".to_string(), vec![1.0, 0.0]),
        ];
        let (groups, singleton_images) =
            build_similarity_groups(embeddings, 0.01, 2, MAX_SIMILARITY_GROUP_EMBEDDINGS).unwrap();
        assert!(groups.is_empty());
        assert_eq!(singleton_images, 3);
    }

    #[test]
    fn test_normalized_dot_matches_cosine_for_known_vectors() {
        let normalized = l2_normalize_all(vec![
            ("a".to_string(), vec![3.0, 4.0]),
            ("b".to_string(), vec![3.0, 4.0]),
            ("c".to_string(), vec![4.0, 3.0]),
        ]);
        let a = &normalized[0].1;
        let b = &normalized[1].1;
        let c = &normalized[2].1;
        assert!((normalized_dot(a, b) - 1.0).abs() < 1e-6);
        // cos angle between (3,4) and (4,3): dot=24, norms=5*5=25 => 0.96
        assert!((normalized_dot(a, c) - 0.96).abs() < 1e-5);
    }

    #[test]
    fn test_normalized_dot_mismatched_lengths_returns_zero() {
        assert_eq!(normalized_dot(&[1.0, 0.0], &[1.0, 0.0, 0.0]), 0.0);
    }
}
