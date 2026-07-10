// Copyright (c) 2026-present Gleb Kalinin. Architecture and design by author.
// Implementation assisted by Claude (Anthropic). See AUTHORSHIP.md.

use crate::db_core::db::{row_u64, sql_usize, Database};
use crate::db_core::models::*;
use crate::db_core::perceptual_hash::hamming_distance_parts;
use crate::db_core::tags::{normalize_tag_name, split_tag_list};
use rusqlite::{params, OptionalExtension, Result};
use std::collections::{HashMap, HashSet};

impl Database {
    pub fn get_setting(&self, key: &str) -> Result<Option<String>> {
        let conn = self.conn.lock();
        let mut stmt = conn.prepare("SELECT value FROM app_settings WHERE key = ?1")?;
        let mut rows = stmt.query_map(params![key], |row| row.get(0))?;
        match rows.next() {
            Some(Ok(val)) => Ok(Some(val)),
            _ => Ok(None),
        }
    }

    pub fn set_setting(&self, key: &str, value: &str) -> Result<()> {
        let conn = self.conn.lock();
        conn.execute(
            "INSERT OR REPLACE INTO app_settings (key, value) VALUES (?1, ?2)",
            params![key, value],
        )?;
        Ok(())
    }

    pub fn set_client_feedback(
        &self,
        image_id: &str,
        favorite: bool,
        comment: Option<&str>,
    ) -> Result<()> {
        let conn = self.conn.lock();
        let now = chrono::Utc::now().to_rfc3339();
        conn.execute(
            "INSERT INTO client_feedback (image_id, favorite, comment, updated_at)
             VALUES (?1, ?2, ?3, ?4)
             ON CONFLICT(image_id) DO UPDATE SET
                favorite = excluded.favorite,
                comment = excluded.comment,
                updated_at = excluded.updated_at",
            params![image_id, favorite as i64, comment, now],
        )?;
        Ok(())
    }

    pub fn get_client_feedback(&self, image_id: &str) -> Result<Option<ClientFeedback>> {
        let conn = self.conn.lock();
        let mut stmt = conn.prepare(
            "SELECT image_id, favorite, comment, updated_at FROM client_feedback WHERE image_id = ?1",
        )?;
        let mut rows = stmt.query_map(params![image_id], |row| {
            let favorite: i64 = row.get(1)?;
            Ok(ClientFeedback {
                image_id: row.get(0)?,
                favorite: favorite != 0,
                comment: row.get(2)?,
                updated_at: row.get(3)?,
            })
        })?;

        match rows.next() {
            Some(row) => Ok(Some(row?)),
            None => Ok(None),
        }
    }

    pub fn list_client_feedback(&self) -> Result<Vec<ClientFeedback>> {
        let conn = self.conn.lock();
        let mut stmt = conn.prepare(
            "SELECT image_id, favorite, comment, updated_at FROM client_feedback ORDER BY updated_at DESC",
        )?;
        let rows = stmt.query_map([], |row| {
            let favorite: i64 = row.get(1)?;
            Ok(ClientFeedback {
                image_id: row.get(0)?,
                favorite: favorite != 0,
                comment: row.get(2)?,
                updated_at: row.get(3)?,
            })
        })?;
        rows.collect()
    }

    pub fn set_plugin_grants(&self, plugin_id: &str, capabilities: &[String]) -> Result<()> {
        let mut conn = self.conn.lock();
        let tx = conn.transaction()?;
        let now = chrono::Utc::now().to_rfc3339();
        tx.execute(
            "DELETE FROM plugin_grants WHERE plugin_id = ?1",
            params![plugin_id],
        )?;
        for capability in capabilities {
            tx.execute(
                "INSERT OR REPLACE INTO plugin_grants (plugin_id, capability, granted_at)
                 VALUES (?1, ?2, ?3)",
                params![plugin_id, capability, now],
            )?;
        }
        tx.commit()
    }

    pub fn granted_plugin_capabilities(&self, plugin_id: &str) -> Result<Vec<String>> {
        let conn = self.conn.lock();
        let mut stmt = conn.prepare(
            "SELECT capability FROM plugin_grants WHERE plugin_id = ?1 ORDER BY capability",
        )?;
        let rows = stmt.query_map(params![plugin_id], |row| row.get(0))?;
        rows.collect()
    }

    pub fn store_image_quality_metrics(&self, metrics: &ImageQualityMetrics) -> Result<()> {
        let conn = self.conn.lock();
        conn.execute(
            "INSERT OR REPLACE INTO image_quality_metrics (
                image_id, analyzer_version, focus_score, blur_score, exposure_score,
                clipped_shadow_pct, clipped_highlight_pct, mean_luma, contrast, analyzed_at
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
            params![
                &metrics.image_id,
                &metrics.analyzer_version,
                metrics.focus_score,
                metrics.blur_score,
                metrics.exposure_score,
                metrics.clipped_shadow_pct,
                metrics.clipped_highlight_pct,
                metrics.mean_luma,
                metrics.contrast,
                &metrics.analyzed_at,
            ],
        )?;
        Ok(())
    }

    pub fn get_image_quality_metrics(&self, image_id: &str) -> Result<Option<ImageQualityMetrics>> {
        let conn = self.conn.lock();
        conn.query_row(
            "SELECT image_id, analyzer_version, focus_score, blur_score, exposure_score,
                    clipped_shadow_pct, clipped_highlight_pct, mean_luma, contrast, analyzed_at
             FROM image_quality_metrics
             WHERE image_id = ?1",
            params![image_id],
            |row| {
                Ok(ImageQualityMetrics {
                    image_id: row.get(0)?,
                    analyzer_version: row.get(1)?,
                    focus_score: row.get(2)?,
                    blur_score: row.get(3)?,
                    exposure_score: row.get(4)?,
                    clipped_shadow_pct: row.get(5)?,
                    clipped_highlight_pct: row.get(6)?,
                    mean_luma: row.get(7)?,
                    contrast: row.get(8)?,
                    analyzed_at: row.get(9)?,
                })
            },
        )
        .optional()
    }

    pub fn quality_metrics_count(&self) -> Result<u32> {
        let conn = self.conn.lock();
        conn.query_row("SELECT COUNT(*) FROM image_quality_metrics", [], |row| {
            row.get(0)
        })
    }

    pub fn store_image_color_metrics(&self, metrics: &ImageColorMetrics) -> Result<()> {
        let palette_json =
            serde_json::to_string(&metrics.palette).unwrap_or_else(|_| "[]".to_string());
        let conn = self.conn.lock();
        conn.execute(
            "INSERT OR REPLACE INTO image_color_metrics (
                image_id, analyzer_version, dominant_hex, palette_json, dominant_hue_bucket,
                mean_luma, mean_saturation, colorfulness, contrast, analyzed_at
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
            params![
                &metrics.image_id,
                &metrics.analyzer_version,
                &metrics.dominant_hex,
                &palette_json,
                &metrics.dominant_hue_bucket,
                metrics.mean_luma,
                metrics.mean_saturation,
                metrics.colorfulness,
                metrics.contrast,
                &metrics.analyzed_at,
            ],
        )?;
        Ok(())
    }

    pub fn get_image_color_metrics(&self, image_id: &str) -> Result<Option<ImageColorMetrics>> {
        let conn = self.conn.lock();
        conn.query_row(
            "SELECT image_id, analyzer_version, dominant_hex, palette_json, dominant_hue_bucket,
                    mean_luma, mean_saturation, colorfulness, contrast, analyzed_at
             FROM image_color_metrics
             WHERE image_id = ?1",
            params![image_id],
            |row| {
                let palette_json: String = row.get(3)?;
                let palette = serde_json::from_str::<Vec<ImagePaletteColor>>(&palette_json)
                    .unwrap_or_default();
                Ok(ImageColorMetrics {
                    image_id: row.get(0)?,
                    analyzer_version: row.get(1)?,
                    dominant_hex: row.get(2)?,
                    palette,
                    dominant_hue_bucket: row.get(4)?,
                    mean_luma: row.get(5)?,
                    mean_saturation: row.get(6)?,
                    colorfulness: row.get(7)?,
                    contrast: row.get(8)?,
                    analyzed_at: row.get(9)?,
                })
            },
        )
        .optional()
    }

    pub fn color_metrics_count(&self) -> Result<u32> {
        let conn = self.conn.lock();
        conn.query_row("SELECT COUNT(*) FROM image_color_metrics", [], |row| {
            row.get(0)
        })
    }

    pub fn list_images_by_color_bucket(
        &self,
        bucket: &str,
        limit: u32,
        offset: u32,
    ) -> Result<Vec<ImageWithFile>> {
        let conn = self.conn.lock();
        let mut stmt = conn.prepare(
            "SELECT i.id, i.sha256_hash, i.width, i.height, i.format, i.file_size,
                    i.created_at, i.imported_at, f.path,
                    s.star_rating, s.color_label, s.decision, i.source_label, i.ai_prompt,
                    i.raw_metadata, f.missing_at
             FROM image_color_metrics cm
             JOIN images i ON i.id = cm.image_id
             JOIN image_files f ON f.image_id = i.id AND f.missing_at IS NULL
             LEFT JOIN selections s ON s.image_id = i.id AND s.project_id = '__global__'
             WHERE cm.dominant_hue_bucket = ?1
             GROUP BY i.id
             ORDER BY cm.colorfulness DESC, i.imported_at DESC
             LIMIT ?2 OFFSET ?3",
        )?;
        let rows = stmt.query_map(params![bucket, limit, offset], |row| {
            let star: Option<u8> = row.get(9)?;
            let color: Option<String> = row.get(10)?;
            let decision: Option<String> = row.get(11)?;
            let selection =
                Selection::from_nullable_parts(row.get(0)?, None, star, color, decision);
            Ok(ImageWithFile {
                image: Image {
                    id: row.get(0)?,
                    sha256_hash: row.get(1)?,
                    width: row.get(2)?,
                    height: row.get(3)?,
                    format: row.get(4)?,
                    file_size: row_u64(row, 5)?,
                    created_at: row.get(6)?,
                    imported_at: row.get(7)?,
                    ai_prompt: row.get(13)?,
                    raw_metadata: row.get(14)?,
                },
                path: row.get(8)?,
                thumbnail_path: None,
                selection,
                source_label: row.get(12)?,
                missing_at: row.get(15)?,
            })
        })?;
        rows.collect::<Result<Vec<_>>>()
    }

    pub fn add_image_tag(
        &self,
        image_id: &str,
        name: &str,
        tag_type: &str,
        source: &str,
        confidence: Option<f64>,
    ) -> Result<bool> {
        let Some(normalized_name) = normalize_tag_name(name) else {
            return Ok(false);
        };

        let display_name = name.trim();
        let now = chrono::Utc::now().to_rfc3339();
        let tag_id = format!("tag_{}", uuid::Uuid::new_v4().to_string().replace('-', ""));
        let conn = self.conn.lock();

        conn.execute(
            "INSERT OR IGNORE INTO tags (id, name, normalized_name, tag_type, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5)",
            params![&tag_id, display_name, &normalized_name, tag_type, &now],
        )?;

        let resolved_tag_id: String = conn.query_row(
            "SELECT id FROM tags WHERE normalized_name = ?1",
            params![&normalized_name],
            |row| row.get(0),
        )?;

        let inserted = conn.execute(
            "INSERT OR IGNORE INTO image_tags (image_id, tag_id, source, confidence, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5)",
            params![image_id, resolved_tag_id, source, confidence, &now],
        )?;
        Ok(inserted > 0)
    }

    pub fn list_image_tags(&self, image_id: &str) -> Result<Vec<ImageTag>> {
        let conn = self.conn.lock();
        let mut stmt = conn.prepare(
            "SELECT t.id, it.image_id, t.name, t.normalized_name, t.tag_type,
                    it.source, it.confidence, it.created_at
             FROM image_tags it
             JOIN tags t ON t.id = it.tag_id
             WHERE it.image_id = ?1
             ORDER BY t.tag_type ASC, t.name ASC, it.source ASC",
        )?;
        let rows = stmt.query_map(params![image_id], |row| {
            Ok(ImageTag {
                id: row.get(0)?,
                image_id: row.get(1)?,
                name: row.get(2)?,
                normalized_name: row.get(3)?,
                tag_type: row.get(4)?,
                source: row.get(5)?,
                confidence: row.get(6)?,
                created_at: row.get(7)?,
            })
        })?;
        rows.collect::<Result<Vec<_>>>()
    }

    pub fn list_tags(&self, limit: u32, offset: u32) -> Result<Vec<TagSummary>> {
        let conn = self.conn.lock();
        let mut stmt = conn.prepare(
            "SELECT t.id, t.name, t.normalized_name, t.tag_type,
                    COUNT(DISTINCT it.image_id) AS image_count
             FROM tags t
             LEFT JOIN image_tags it ON it.tag_id = t.id
             GROUP BY t.id
             ORDER BY image_count DESC, t.name ASC
             LIMIT ?1 OFFSET ?2",
        )?;
        let rows = stmt.query_map(params![limit, offset], |row| {
            Ok(TagSummary {
                id: row.get(0)?,
                name: row.get(1)?,
                normalized_name: row.get(2)?,
                tag_type: row.get(3)?,
                image_count: row.get(4)?,
            })
        })?;
        rows.collect::<Result<Vec<_>>>()
    }

    pub fn tag_count(&self) -> Result<u32> {
        let conn = self.conn.lock();
        conn.query_row("SELECT COUNT(*) FROM tags", [], |row| row.get(0))
    }

    pub fn backfill_image_tags(&self) -> Result<TagBackfillResult> {
        let before_count = self.tag_count()?;
        let mut candidates: Vec<(String, String, String, String, Option<f64>)> = Vec::new();

        {
            let conn = self.conn.lock();

            let mut stmt = conn.prepare(
                "SELECT id, format, orientation, source_label
                 FROM images",
            )?;
            let rows = stmt.query_map([], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, Option<String>>(2)?,
                    row.get::<_, Option<String>>(3)?,
                ))
            })?;
            for row in rows {
                let (image_id, format, orientation, source_label) = row?;
                candidates.push((
                    image_id.clone(),
                    format.to_lowercase(),
                    "format".to_string(),
                    "file:format".to_string(),
                    None,
                ));
                if let Some(orientation) = orientation {
                    candidates.push((
                        image_id.clone(),
                        orientation,
                        "metadata".to_string(),
                        "file:orientation".to_string(),
                        None,
                    ));
                }
                if let Some(source_label) = source_label {
                    candidates.push((
                        image_id,
                        source_label,
                        "source".to_string(),
                        "source_detection".to_string(),
                        None,
                    ));
                }
            }
            drop(stmt);

            let mut stmt = conn.prepare(
                "SELECT i.id, g.provider, g.model
                 FROM images i
                 JOIN generation_runs g ON g.id = i.generation_run_id",
            )?;
            let rows = stmt.query_map([], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, Option<String>>(1)?,
                    row.get::<_, Option<String>>(2)?,
                ))
            })?;
            for row in rows {
                let (image_id, provider, model) = row?;
                if let Some(provider) = provider {
                    candidates.push((
                        image_id.clone(),
                        provider,
                        "generation".to_string(),
                        "generation:provider".to_string(),
                        None,
                    ));
                }
                if let Some(model) = model {
                    candidates.push((
                        image_id,
                        model,
                        "generation".to_string(),
                        "generation:model".to_string(),
                        None,
                    ));
                }
            }
            drop(stmt);

            let mut stmt = conn.prepare(
                "SELECT image_id, key, value, source
                 FROM image_metadata",
            )?;
            let rows = stmt.query_map([], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, String>(3)?,
                ))
            })?;
            for row in rows {
                let (image_id, key, value, source) = row?;
                let key_lower = key.to_lowercase();
                let (tag_type, values) = match key_lower.as_str() {
                    "tags" | "keywords" => ("vision", split_tag_list(&value)),
                    "objects" | "object" => ("object", split_tag_list(&value)),
                    "dominant_colors" | "colors" | "color_palette" => {
                        ("color", split_tag_list(&value))
                    }
                    "scene_type" | "mood" | "indoor_outdoor" | "time_of_day" | "activity"
                    | "image_quality" | "style" | "subject" => {
                        ("vision", vec![value.trim().to_string()])
                    }
                    _ => continue,
                };

                for value in values {
                    candidates.push((
                        image_id.clone(),
                        value,
                        tag_type.to_string(),
                        format!("metadata:{}:{}", source, key_lower),
                        None,
                    ));
                }
            }
            drop(stmt);

            let mut stmt = conn.prepare(
                "SELECT image_id, class_name, model_name, MAX(confidence)
                 FROM detections
                 WHERE confidence >= 0.35
                 GROUP BY image_id, class_name, model_name",
            )?;
            let rows = stmt.query_map([], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, f64>(3)?,
                ))
            })?;
            for row in rows {
                let (image_id, class_name, model_name, confidence) = row?;
                candidates.push((
                    image_id,
                    class_name,
                    "object".to_string(),
                    format!("detection:{}", model_name),
                    Some(confidence),
                ));
            }
        }

        let mut image_ids = HashSet::new();
        let mut image_tags_created = 0u32;
        for (image_id, name, tag_type, source, confidence) in candidates {
            image_ids.insert(image_id.clone());
            if self.add_image_tag(&image_id, &name, &tag_type, &source, confidence)? {
                image_tags_created += 1;
            }
        }

        let after_count = self.tag_count()?;
        Ok(TagBackfillResult {
            images_processed: image_ids.len() as u32,
            tags_created: after_count.saturating_sub(before_count),
            image_tags_created,
        })
    }

    pub fn store_image_perceptual_hash(&self, hash: &ImagePerceptualHash) -> Result<()> {
        let conn = self.conn.lock();
        conn.execute(
            "INSERT OR REPLACE INTO image_perceptual_hashes (
                image_id, algorithm, hash_hi, hash_lo, band0, band1, band2, band3, analyzed_at
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
            params![
                &hash.image_id,
                &hash.algorithm,
                hash.hash_hi,
                hash.hash_lo,
                hash.band0,
                hash.band1,
                hash.band2,
                hash.band3,
                &hash.analyzed_at,
            ],
        )?;
        Ok(())
    }

    pub fn get_image_perceptual_hash(
        &self,
        image_id: &str,
        algorithm: &str,
    ) -> Result<Option<ImagePerceptualHash>> {
        let conn = self.conn.lock();
        conn.query_row(
            "SELECT image_id, algorithm, hash_hi, hash_lo, band0, band1, band2, band3, analyzed_at
             FROM image_perceptual_hashes
             WHERE image_id = ?1 AND algorithm = ?2",
            params![image_id, algorithm],
            |row| {
                Ok(ImagePerceptualHash {
                    image_id: row.get(0)?,
                    algorithm: row.get(1)?,
                    hash_hi: row.get(2)?,
                    hash_lo: row.get(3)?,
                    band0: row.get(4)?,
                    band1: row.get(5)?,
                    band2: row.get(6)?,
                    band3: row.get(7)?,
                    analyzed_at: row.get(8)?,
                })
            },
        )
        .optional()
    }

    pub fn perceptual_hash_count(&self, algorithm: &str) -> Result<u32> {
        let conn = self.conn.lock();
        conn.query_row(
            "SELECT COUNT(*) FROM image_perceptual_hashes WHERE algorithm = ?1",
            params![algorithm],
            |row| row.get(0),
        )
    }

    pub fn find_near_duplicates_by_phash(
        &self,
        image_id: &str,
        algorithm: &str,
        max_distance: u32,
        limit: u32,
    ) -> Result<Vec<NearDuplicateImage>> {
        let Some(base) = self.get_image_perceptual_hash(image_id, algorithm)? else {
            return Ok(vec![]);
        };

        let mut candidate_distances: Vec<(String, u32)> = {
            let conn = self.conn.lock();
            let mut stmt = conn.prepare(
                "SELECT image_id, hash_hi, hash_lo
                 FROM image_perceptual_hashes
                 WHERE algorithm = ?1
                   AND image_id != ?2
                   AND (band0 = ?3 OR band1 = ?4 OR band2 = ?5 OR band3 = ?6)",
            )?;
            let rows = stmt.query_map(
                params![algorithm, image_id, base.band0, base.band1, base.band2, base.band3],
                |row| {
                    let candidate_id: String = row.get(0)?;
                    let hash_hi: i64 = row.get(1)?;
                    let hash_lo: i64 = row.get(2)?;
                    let distance =
                        hamming_distance_parts(base.hash_hi, base.hash_lo, hash_hi, hash_lo);
                    Ok((candidate_id, distance))
                },
            )?;
            rows.collect::<Result<Vec<_>>>()?
        };

        candidate_distances.retain(|(_, distance)| *distance <= max_distance);
        candidate_distances.sort_by(|a, b| a.1.cmp(&b.1).then_with(|| a.0.cmp(&b.0)));
        candidate_distances.truncate(limit as usize);

        let ids: Vec<String> = candidate_distances
            .iter()
            .map(|(id, _)| id.clone())
            .collect();
        let id_refs: Vec<&str> = ids.iter().map(String::as_str).collect();
        let images = self.get_images_by_ids(&id_refs)?;
        let mut images_by_id: HashMap<String, ImageWithFile> = images
            .into_iter()
            .map(|image| (image.image.id.clone(), image))
            .collect();

        Ok(candidate_distances
            .into_iter()
            .filter_map(|(id, distance)| {
                images_by_id.remove(&id).map(|image| NearDuplicateImage {
                    image,
                    algorithm: algorithm.to_string(),
                    distance,
                })
            })
            .collect())
    }

    pub fn replace_similarity_groups(
        &self,
        model_name: &str,
        threshold: f64,
        method: &str,
        groups: &[Vec<(String, f32)>],
        singleton_images: u32,
    ) -> Result<SimilarityGroupingResult> {
        let mut conn = self.conn.lock();
        let tx = conn.transaction()?;

        tx.execute(
            "DELETE FROM image_similarity_groups WHERE model_name = ?1 AND method = ?2",
            params![model_name, method],
        )?;

        let now = chrono::Utc::now().to_rfc3339();
        let mut images_grouped = 0u32;
        for group in groups {
            if group.is_empty() {
                continue;
            }
            let group_id = format!("sg_{}", uuid::Uuid::new_v4().to_string().replace('-', ""));
            let representative_image_id = group.first().map(|(id, _)| id.as_str());
            tx.execute(
                "INSERT INTO image_similarity_groups (
                    id, model_name, threshold, method, representative_image_id,
                    image_count, created_at, updated_at
                 ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
                params![
                    &group_id,
                    model_name,
                    threshold,
                    method,
                    representative_image_id,
                    group.len() as u32,
                    &now,
                    &now,
                ],
            )?;

            for (rank, (image_id, score)) in group.iter().enumerate() {
                tx.execute(
                    "INSERT INTO image_similarity_group_items (
                        group_id, image_id, score_to_representative, rank
                     ) VALUES (?1, ?2, ?3, ?4)",
                    params![&group_id, image_id, *score as f64, rank as u32],
                )?;
            }
            images_grouped += group.len() as u32;
        }

        tx.commit()?;
        Ok(SimilarityGroupingResult {
            model_name: model_name.to_string(),
            threshold,
            method: method.to_string(),
            groups_created: groups.len() as u32,
            images_grouped,
            singleton_images,
        })
    }

    pub fn list_similarity_groups(
        &self,
        limit: u32,
        offset: u32,
    ) -> Result<Vec<SimilarityGroupSummary>> {
        let conn = self.conn.lock();
        let mut stmt = conn.prepare(
            "SELECT id, model_name, threshold, method, representative_image_id,
                    image_count, created_at, updated_at
             FROM image_similarity_groups
             ORDER BY image_count DESC, updated_at DESC
             LIMIT ?1 OFFSET ?2",
        )?;
        let rows = stmt.query_map(params![limit, offset], |row| {
            Ok(SimilarityGroupSummary {
                id: row.get(0)?,
                model_name: row.get(1)?,
                threshold: row.get(2)?,
                method: row.get(3)?,
                representative_image_id: row.get(4)?,
                image_count: row.get(5)?,
                created_at: row.get(6)?,
                updated_at: row.get(7)?,
            })
        })?;
        rows.collect::<Result<Vec<_>>>()
    }

    pub fn list_similarity_group_images(&self, group_id: &str) -> Result<Vec<ImageWithFile>> {
        let conn = self.conn.lock();
        let mut stmt = conn.prepare(
            "SELECT i.id, i.sha256_hash, i.width, i.height, i.format, i.file_size,
                    i.created_at, i.imported_at, f.path,
                    s.star_rating, s.color_label, s.decision, i.source_label, i.ai_prompt,
                    i.raw_metadata, f.missing_at
             FROM image_similarity_group_items gi
             JOIN images i ON i.id = gi.image_id
             JOIN image_files f ON f.image_id = i.id AND f.missing_at IS NULL
             LEFT JOIN selections s ON s.image_id = i.id AND s.project_id = '__global__'
             WHERE gi.group_id = ?1
             GROUP BY i.id
             ORDER BY gi.rank ASC",
        )?;
        let rows = stmt.query_map(params![group_id], |row| {
            let star: Option<u8> = row.get(9)?;
            let color: Option<String> = row.get(10)?;
            let decision: Option<String> = row.get(11)?;
            let selection =
                Selection::from_nullable_parts(row.get(0)?, None, star, color, decision);
            Ok(ImageWithFile {
                image: Image {
                    id: row.get(0)?,
                    sha256_hash: row.get(1)?,
                    width: row.get(2)?,
                    height: row.get(3)?,
                    format: row.get(4)?,
                    file_size: row_u64(row, 5)?,
                    created_at: row.get(6)?,
                    imported_at: row.get(7)?,
                    ai_prompt: row.get(13)?,
                    raw_metadata: row.get(14)?,
                },
                path: row.get(8)?,
                thumbnail_path: None,
                selection,
                source_label: row.get(12)?,
                missing_at: row.get(15)?,
            })
        })?;
        rows.collect::<Result<Vec<_>>>()
    }

    pub fn store_vision_metadata(
        &self,
        image_id: &str,
        source: &str,
        fields: &std::collections::HashMap<String, String>,
    ) -> Result<()> {
        let conn = self.conn.lock();
        for (key, value) in fields {
            conn.execute(
                "INSERT OR REPLACE INTO image_metadata (image_id, key, value, source) VALUES (?1, ?2, ?3, ?4)",
                params![image_id, key, value, source],
            )?;
        }
        Ok(())
    }

    pub fn delete_image_metadata_source(&self, image_id: &str, source: &str) -> Result<u32> {
        let conn = self.conn.lock();
        let deleted = conn.execute(
            "DELETE FROM image_metadata WHERE image_id = ?1 AND source = ?2",
            params![image_id, source],
        )?;
        Ok(deleted as u32)
    }

    pub fn image_has_metadata_source(&self, image_id: &str, source: &str) -> Result<bool> {
        let conn = self.conn.lock();
        conn.query_row(
            "SELECT EXISTS (
                SELECT 1 FROM image_metadata
                WHERE image_id = ?1 AND source = ?2
             )",
            params![image_id, source],
            |row| row.get::<_, bool>(0),
        )
    }

    pub fn get_vision_metadata(&self, image_id: &str) -> Result<Vec<(String, String, String)>> {
        let conn = self.conn.lock();
        let mut stmt = conn.prepare(
            "SELECT key, value, source FROM image_metadata WHERE image_id = ?1 ORDER BY key",
        )?;
        let rows = stmt.query_map(params![image_id], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
            ))
        })?;
        rows.collect::<Result<Vec<_>>>()
    }

    pub fn count_vision_processed(&self, source: &str) -> Result<u32> {
        let conn = self.conn.lock();
        conn.query_row(
            "SELECT COUNT(DISTINCT image_id) FROM image_metadata WHERE source = ?1",
            params![source],
            |row| row.get::<_, u32>(0),
        )
    }

    pub fn insert_generation_run(&self, run: &GenerationRun) -> Result<()> {
        let conn = self.conn.lock();
        conn.execute(
            "INSERT OR IGNORE INTO generation_runs (id, prompt, negative_prompt, provider, model, settings_json, seed, parent_run_id, source_type, source_path, raw_metadata_json, created_at, imported_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13)",
            rusqlite::params![run.id, run.prompt, run.negative_prompt, run.provider, run.model, run.settings_json, run.seed, run.parent_run_id, run.source_type, run.source_path, run.raw_metadata_json, run.created_at, run.imported_at],
        )?;
        Ok(())
    }

    pub fn link_image_to_run(&self, image_id: &str, run_id: &str) -> Result<()> {
        let conn = self.conn.lock();
        conn.execute(
            "UPDATE images
             SET generation_run_id = ?1,
                 ai_prompt = COALESCE((SELECT prompt FROM generation_runs WHERE id = ?1), ai_prompt)
             WHERE id = ?2",
            rusqlite::params![run_id, image_id],
        )?;
        Ok(())
    }

    pub fn get_generation_run_for_image(&self, image_id: &str) -> Result<Option<GenerationRun>> {
        let conn = self.conn.lock();
        let mut stmt = conn.prepare(
            "SELECT g.id, g.prompt, g.negative_prompt, g.provider, g.model, g.settings_json, g.seed, g.parent_run_id, g.source_type, g.source_path, g.raw_metadata_json, g.created_at, g.imported_at
             FROM generation_runs g
             JOIN images i ON i.generation_run_id = g.id
             WHERE i.id = ?1"
        )?;
        let run = stmt
            .query_row(rusqlite::params![image_id], |row| {
                Ok(GenerationRun {
                    id: row.get(0)?,
                    prompt: row.get(1)?,
                    negative_prompt: row.get(2)?,
                    provider: row.get(3)?,
                    model: row.get(4)?,
                    settings_json: row.get(5)?,
                    seed: row.get(6)?,
                    parent_run_id: row.get(7)?,
                    source_type: row.get(8)?,
                    source_path: row.get(9)?,
                    raw_metadata_json: row.get(10)?,
                    created_at: row.get(11)?,
                    imported_at: row.get(12)?,
                })
            })
            .optional()?;
        Ok(run)
    }

    pub fn get_images_without_generation_run(&self) -> Result<Vec<(String, String)>> {
        let conn = self.conn.lock();
        let mut stmt = conn.prepare(
            "SELECT i.id, f.path
             FROM images i
             JOIN image_files f ON f.image_id = i.id AND f.missing_at IS NULL
             WHERE i.generation_run_id IS NULL",
        )?;
        let rows = stmt.query_map([], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
        })?;
        let mut results = Vec::new();
        for row in rows {
            results.push(row?);
        }
        Ok(results)
    }

    pub fn get_undo_record_by_seq(&self, seq: i64) -> Result<Option<UndoRecord>> {
        let conn = self.conn.lock();
        let mut stmt = conn.prepare(
            "SELECT seq, id, action_type, label, before_json, after_json, affected_image_ids, group_id, has_file_backup, created_at
             FROM undo_records WHERE seq = ?1"
        )?;
        stmt.query_row(params![seq], |row| {
            Ok(UndoRecord {
                seq: row.get(0)?,
                id: row.get(1)?,
                action_type: row.get(2)?,
                label: row.get(3)?,
                before_json: row.get(4)?,
                after_json: row.get(5)?,
                affected_image_ids: row.get(6)?,
                group_id: row.get(7)?,
                has_file_backup: row.get::<_, i32>(8)? != 0,
                created_at: row.get(9)?,
            })
        })
        .optional()
    }

    pub fn get_max_undo_seq(&self) -> Result<Option<i64>> {
        let conn = self.conn.lock();
        conn.query_row("SELECT MAX(seq) FROM undo_records", [], |row| row.get(0))
    }

    pub fn count_undo_records(&self) -> Result<i64> {
        let conn = self.conn.lock();
        conn.query_row("SELECT COUNT(*) FROM undo_records", [], |row| row.get(0))
    }

    pub fn list_undo_records(&self, limit: u32) -> Result<Vec<UndoRecord>> {
        let conn = self.conn.lock();
        let mut stmt = conn.prepare(
            "SELECT seq, id, action_type, label, before_json, after_json, affected_image_ids, group_id, has_file_backup, created_at
             FROM undo_records ORDER BY seq DESC LIMIT ?1"
        )?;
        let rows = stmt.query_map(params![limit], |row| {
            Ok(UndoRecord {
                seq: row.get(0)?,
                id: row.get(1)?,
                action_type: row.get(2)?,
                label: row.get(3)?,
                before_json: row.get(4)?,
                after_json: row.get(5)?,
                affected_image_ids: row.get(6)?,
                group_id: row.get(7)?,
                has_file_backup: row.get::<_, i32>(8)? != 0,
                created_at: row.get(9)?,
            })
        })?;
        rows.collect::<Result<Vec<_>>>()
    }

    pub fn prune_oldest_undo_records(&self, keep_count: usize) -> Result<()> {
        let conn = self.conn.lock();
        conn.execute(
            "DELETE FROM undo_records WHERE seq NOT IN (
                SELECT seq FROM undo_records ORDER BY seq DESC LIMIT ?1
            )",
            params![sql_usize(keep_count)?],
        )?;
        Ok(())
    }
}
