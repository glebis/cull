// Copyright (c) 2026-present Gleb Kalinin. Architecture and design by author.
// Implementation assisted by Claude (Anthropic). See AUTHORSHIP.md.

use crate::db_core::db::Database;
use crate::db_core::db::{cosine_similarity, decode_embedding_bytes};

use crate::db_core::models::*;
use crate::db_core::queries::images::image_scope_filter;
use crate::db_core::referenced_sources::NORMAL_LIBRARY_FILE_PREDICATE;
use crate::db_core::smart_collections::FilterNode;
use crate::db_core::tags::normalize_tag_name;
use crate::db_core::visibility::RejectedVisibility;
use rusqlite::types::Value;
use rusqlite::{params, OptionalExtension, Result, ToSql};
use std::collections::{HashMap, HashSet};

fn title_case_label(value: &str) -> String {
    normalize_tag_name(value)
        .unwrap_or_else(|| value.trim().to_lowercase())
        .split('-')
        .filter(|part| !part.is_empty())
        .map(|part| {
            let mut chars = part.chars();
            match chars.next() {
                Some(first) => first.to_uppercase().collect::<String>() + chars.as_str(),
                None => String::new(),
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

fn filename_candidates(path: &str) -> Vec<String> {
    const STOP_WORDS: &[&str] = &[
        "copy",
        "dsc",
        "export",
        "final",
        "frame",
        "generated",
        "image",
        "img",
        "output",
        "photo",
        "screenshot",
        "untitled",
    ];
    let stem = std::path::Path::new(path)
        .file_stem()
        .and_then(|value| value.to_str())
        .unwrap_or_default();
    stem.split(|character: char| !character.is_alphabetic())
        .map(str::to_lowercase)
        .filter(|token| token.chars().count() >= 3 && !STOP_WORDS.contains(&token.as_str()))
        .collect()
}

fn source_priority(source: &str) -> u8 {
    match source {
        "tag" => 3,
        "yolo" => 2,
        "filename" => 1,
        _ => 0,
    }
}

fn scoped_where_clause(scope: &EmbeddingScope) -> Result<(String, Vec<Value>, RejectedVisibility)> {
    let default_visibility = RejectedVisibility::from_include_rejected(scope.include_rejected());

    match scope {
        EmbeddingScope::All { .. } => Ok(("1=1".to_string(), vec![], default_visibility)),
        EmbeddingScope::Folder { path, min_size, .. } => {
            let folder = path.trim_end_matches('/');
            let prefix = if folder.is_empty() {
                "/".to_string()
            } else {
                format!("{folder}/")
            };
            Ok((
                "(f.path = ? OR substr(f.path, 1, ?) COLLATE BINARY = ? COLLATE BINARY)
                 AND i.width >= ? AND i.height >= ?"
                    .to_string(),
                vec![
                    Value::Text(path.to_string()),
                    Value::Integer(prefix.chars().count() as i64),
                    Value::Text(prefix),
                    Value::Integer(*min_size as i64),
                    Value::Integer(*min_size as i64),
                ],
                default_visibility,
            ))
        }
        EmbeddingScope::Filtered { min_size, .. } => Ok((
            "i.width >= ? AND i.height >= ?".to_string(),
            vec![
                Value::Integer(*min_size as i64),
                Value::Integer(*min_size as i64),
            ],
            default_visibility,
        )),
        EmbeddingScope::Collection { id, .. } => Ok((
            "EXISTS (
                SELECT 1 FROM collection_items ci
                WHERE ci.image_id = i.id AND ci.collection_id = ?
             )"
            .to_string(),
            vec![Value::Text(id.clone())],
            default_visibility,
        )),
        EmbeddingScope::Smart {
            id: _, filter_json, ..
        } => {
            let filter: FilterNode = serde_json::from_str(filter_json)
                .map_err(|error| rusqlite::Error::InvalidParameterName(error.to_string()))?;
            let (where_clause, params) = filter
                .to_sql_clause()
                .map_err(rusqlite::Error::InvalidParameterName)?;
            let visibility = default_visibility.for_filter(&filter);
            Ok((where_clause, params, visibility))
        }
        EmbeddingScope::DetectedClass { class_name, .. } => Ok((
            "EXISTS (
                SELECT 1 FROM detections d
                WHERE d.image_id = i.id AND d.class_name = ?
             )"
            .to_string(),
            vec![Value::Text(class_name.clone())],
            default_visibility,
        )),
        EmbeddingScope::ImportBatch { batch_id, .. } => Ok((
            "i.import_batch_id = ?".to_string(),
            vec![Value::Text(batch_id.clone())],
            default_visibility,
        )),
    }
}

fn membership_predicate_for_embedding_scope(scope: &EmbeddingScope) -> &'static str {
    // A collection is an explicit user-curated surface, so it may retain an
    // image whose only available file belongs to a referenced source. Every
    // other EmbeddingScope is an automatic library view.
    match scope {
        EmbeddingScope::Collection { .. } => "1 = 1",
        _ => NORMAL_LIBRARY_FILE_PREDICATE,
    }
}

fn membership_predicate_for_token_scope(collections: &[String]) -> (String, Vec<Value>) {
    if collections.is_empty() {
        return (NORMAL_LIBRARY_FILE_PREDICATE.to_string(), Vec::new());
    }

    let placeholders = vec!["?"; collections.len()].join(",");
    (
        format!(
            "({NORMAL_LIBRARY_FILE_PREDICATE} OR i.id IN (
                SELECT image_id FROM collection_items WHERE collection_id IN ({placeholders})
             ))"
        ),
        collections.iter().cloned().map(Value::Text).collect(),
    )
}

fn to_param_refs(params: &[Value]) -> Vec<&dyn ToSql> {
    params.iter().map(|param| param as &dyn ToSql).collect()
}

impl Database {
    pub fn name_embedding_clusters(
        &self,
        clusters: &[EmbeddingClusterMembership],
    ) -> Result<Vec<EmbeddingClusterName>> {
        let conn = self.read_connection();
        let mut result = Vec::with_capacity(clusters.len());

        for cluster in clusters {
            let mut unique_ids = Vec::with_capacity(cluster.image_ids.len());
            let mut seen = HashSet::with_capacity(cluster.image_ids.len());
            for image_id in &cluster.image_ids {
                if seen.insert(image_id.as_str()) {
                    unique_ids.push(image_id.clone());
                }
            }
            if unique_ids.is_empty() {
                continue;
            }

            // Each image contributes at most once to a candidate. The weight
            // records the strongest available evidence for that image/name.
            let mut by_image: HashMap<String, HashMap<String, (String, String, u32)>> =
                HashMap::new();
            for chunk in unique_ids.chunks(400) {
                let placeholders = (0..chunk.len()).map(|_| "?").collect::<Vec<_>>().join(",");
                let values = chunk.iter().cloned().map(Value::Text).collect::<Vec<_>>();

                let tag_sql = format!(
                    "SELECT it.image_id, t.name, t.tag_type, it.source
                     FROM image_tags it
                     JOIN tags t ON t.id = it.tag_id
                     WHERE it.image_id IN ({placeholders})"
                );
                let mut stmt = conn.prepare(&tag_sql)?;
                let rows = stmt.query_map(to_param_refs(&values).as_slice(), |row| {
                    Ok((
                        row.get::<_, String>(0)?,
                        row.get::<_, String>(1)?,
                        row.get::<_, String>(2)?,
                        row.get::<_, String>(3)?,
                    ))
                })?;
                for row in rows {
                    let (image_id, name, tag_type, source) = row?;
                    let Some(key) = normalize_tag_name(&name) else {
                        continue;
                    };
                    let weight = if source == "manual" || tag_type == "user" {
                        6
                    } else if tag_type == "object" {
                        5
                    } else {
                        4
                    };
                    let candidates = by_image.entry(image_id).or_default();
                    let next = (title_case_label(&key), "tag".to_string(), weight);
                    if candidates
                        .get(&key)
                        .is_none_or(|current| current.2 < weight)
                    {
                        candidates.insert(key, next);
                    }
                }

                let detection_sql = format!(
                    "SELECT image_id, class_name, MAX(confidence)
                     FROM detections
                     WHERE image_id IN ({placeholders})
                       AND model_name GLOB 'yolo*' AND confidence >= 0.35
                     GROUP BY image_id, class_name"
                );
                let mut stmt = conn.prepare(&detection_sql)?;
                let rows = stmt.query_map(to_param_refs(&values).as_slice(), |row| {
                    Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
                })?;
                for row in rows {
                    let (image_id, name) = row?;
                    let Some(key) = normalize_tag_name(&name) else {
                        continue;
                    };
                    let candidates = by_image.entry(image_id).or_default();
                    if candidates.get(&key).is_none_or(|current| current.2 < 5) {
                        candidates.insert(key, (title_case_label(&name), "yolo".to_string(), 5));
                    }
                }

                let file_sql = format!(
                    "SELECT image_id, path FROM image_files
                     WHERE image_id IN ({placeholders}) AND missing_at IS NULL
                     ORDER BY image_id, path"
                );
                let mut stmt = conn.prepare(&file_sql)?;
                let rows = stmt.query_map(to_param_refs(&values).as_slice(), |row| {
                    Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
                })?;
                for row in rows {
                    let (image_id, path) = row?;
                    let candidates = by_image.entry(image_id).or_default();
                    for (index, key) in filename_candidates(&path).into_iter().enumerate() {
                        let weight = if index == 0 { 2 } else { 1 };
                        candidates.entry(key.clone()).or_insert_with(|| {
                            (title_case_label(&key), "filename".to_string(), weight)
                        });
                    }
                }
            }

            let mut scores: HashMap<String, (String, String, u32, u32, u32)> = HashMap::new();
            for candidates in by_image.values() {
                for (key, (label, source, weight)) in candidates {
                    let score = scores
                        .entry(key.clone())
                        .or_insert_with(|| (label.clone(), source.clone(), 0, 0, 0));
                    score.2 += weight;
                    score.3 += 1;
                    if *weight > score.4
                        || (*weight == score.4
                            && source_priority(source) > source_priority(&score.1))
                    {
                        score.0 = label.clone();
                        score.1 = source.clone();
                        score.4 = *weight;
                    }
                }
            }

            let best = scores.into_iter().max_by(
                |(a_key, (_, _, a_score, a_coverage, _)),
                 (b_key, (_, _, b_score, b_coverage, _))| {
                    a_score
                        .cmp(b_score)
                        .then_with(|| a_coverage.cmp(b_coverage))
                        .then_with(|| b_key.cmp(a_key))
                },
            );
            if let Some((_, (label, source, _, _, _))) = best {
                result.push(EmbeddingClusterName {
                    cluster_id: cluster.cluster_id,
                    label,
                    source,
                });
            }
        }

        result.sort_by_key(|item| item.cluster_id);
        Ok(result)
    }

    /// Returns the first occurrence of each requested image ID that does not
    /// already have an embedding for `model_name`.
    pub fn image_ids_without_embedding(
        &self,
        image_ids: &[String],
        model_name: &str,
    ) -> Result<Vec<String>> {
        if image_ids.is_empty() {
            return Ok(Vec::new());
        }

        let mut unique_ids = Vec::with_capacity(image_ids.len());
        let mut seen = HashSet::with_capacity(image_ids.len());
        for image_id in image_ids {
            if seen.insert(image_id.as_str()) {
                unique_ids.push(image_id.clone());
            }
        }

        let conn = self.read_connection();
        let mut existing = HashSet::new();
        for chunk in unique_ids.chunks(500) {
            let placeholders = (0..chunk.len()).map(|_| "?").collect::<Vec<_>>().join(",");
            let sql = format!(
                "SELECT image_id FROM embeddings WHERE model_name = ? AND image_id IN ({placeholders})"
            );
            let mut values = Vec::with_capacity(chunk.len() + 1);
            values.push(Value::Text(model_name.to_string()));
            values.extend(chunk.iter().cloned().map(Value::Text));
            let mut stmt = conn.prepare(&sql)?;
            let rows = stmt.query_map(to_param_refs(&values).as_slice(), |row| row.get(0))?;
            existing.extend(rows.collect::<Result<Vec<String>>>()?);
        }

        Ok(unique_ids
            .into_iter()
            .filter(|image_id| !existing.contains(image_id))
            .collect())
    }

    pub fn store_embedding(&self, image_id: &str, model_name: &str, vector: &[f32]) -> Result<()> {
        self.store_embedding_with_model_run(image_id, model_name, vector, None)
            .map(|_| ())
    }

    pub fn store_embedding_with_model_run(
        &self,
        image_id: &str,
        model_name: &str,
        vector: &[f32],
        model_run_id: Option<&str>,
    ) -> Result<String> {
        let conn = self.conn.lock();
        let bytes: Vec<u8> = vector.iter().flat_map(|f| f.to_le_bytes()).collect();
        let embedding_id = uuid::Uuid::new_v4().to_string();
        conn.execute(
            "INSERT OR REPLACE INTO embeddings (id, image_id, model_name, model_run_id, vector, dims, dtype, normalized, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, 'float32', 1, ?7)",
            params![
                embedding_id,
                image_id,
                model_name,
                model_run_id,
                bytes,
                vector.len() as u32,
                chrono::Utc::now().to_rfc3339(),
            ],
        )?;
        Ok(embedding_id)
    }

    pub fn get_all_embeddings(&self, model_name: &str) -> Result<Vec<(String, Vec<f32>)>> {
        // As in `find_similar`, only run the query under the lock; decoding
        // the (potentially large) blob-to-f32 conversion happens afterwards
        // so the lock isn't held for the whole table scan.
        let raw_rows: Vec<(String, Vec<u8>)> = {
            let conn = self.conn.lock();
            let mut stmt = conn.prepare(&format!(
                "SELECT DISTINCT e.image_id, e.vector
                 FROM embeddings e
                 JOIN images i ON i.id = e.image_id
                 JOIN image_files f ON f.image_id = i.id AND f.missing_at IS NULL
                 WHERE e.model_name = ?1 AND {NORMAL_LIBRARY_FILE_PREDICATE}"
            ))?;
            let rows = stmt.query_map(params![model_name], |row| {
                Ok((row.get::<_, String>(0)?, row.get::<_, Vec<u8>>(1)?))
            })?;
            rows.collect::<Result<Vec<_>>>()?
        };

        Ok(raw_rows
            .into_iter()
            .map(|(image_id, bytes)| (image_id, decode_embedding_bytes(&bytes)))
            .collect())
    }

    pub fn get_embedding_page(
        &self,
        model_name: &str,
        limit: u32,
        offset: u32,
    ) -> Result<EmbeddingPage> {
        let conn = self.conn.lock();
        let total: u32 = conn.query_row(
            &format!(
                "SELECT COUNT(DISTINCT e.image_id)
                 FROM embeddings e
                 JOIN images i ON i.id = e.image_id
                 JOIN image_files f ON f.image_id = i.id AND f.missing_at IS NULL
                 WHERE e.model_name = ?1 AND {NORMAL_LIBRARY_FILE_PREDICATE}"
            ),
            params![model_name],
            |row| row.get(0),
        )?;
        let mut stmt = conn.prepare(&format!(
            "SELECT DISTINCT e.image_id, e.vector, e.dims
             FROM embeddings e
             JOIN images i ON i.id = e.image_id
             JOIN image_files f ON f.image_id = i.id AND f.missing_at IS NULL
             WHERE e.model_name = ?1 AND {NORMAL_LIBRARY_FILE_PREDICATE}
             ORDER BY e.image_id
             LIMIT ?2 OFFSET ?3"
        ))?;
        let rows = stmt.query_map(params![model_name, limit, offset], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, Vec<u8>>(1)?,
                row.get::<_, u32>(2)?,
            ))
        })?;

        let mut ids = Vec::new();
        let mut vectors = Vec::new();
        let mut dims = 0;
        for row in rows {
            let (image_id, bytes, row_dims) = row?;
            if dims == 0 {
                dims = row_dims;
            }
            ids.push(image_id);
            vectors.extend(decode_embedding_bytes(&bytes));
        }
        let returned = ids.len() as u32;
        Ok(EmbeddingPage {
            ids,
            vectors,
            dims,
            total,
            offset,
            limit,
            has_more: offset.saturating_add(returned) < total,
        })
    }

    /// Returns embeddings from the requested live library scope. Both the
    /// count and the page use the same read snapshot; raw blobs are collected
    /// before releasing the connection and decoded afterwards.
    pub fn get_scoped_embedding_page(
        &self,
        model_name: &str,
        scope: &EmbeddingScope,
        limit: u32,
        offset: u32,
    ) -> Result<EmbeddingPage> {
        let (scope_clause, scope_params, visibility) = scoped_where_clause(scope)?;
        let from_and_where = format!(
            "FROM embeddings e
             JOIN images i ON i.id = e.image_id
             JOIN image_files f ON f.image_id = i.id AND f.missing_at IS NULL
             LEFT JOIN selections s ON s.image_id = i.id AND s.project_id = '__global__'
             LEFT JOIN image_quality_metrics qm ON qm.image_id = i.id
             LEFT JOIN image_color_metrics cm ON cm.image_id = i.id
             LEFT JOIN image_similarity_group_items sgi ON sgi.image_id = i.id
             WHERE e.model_name = ? AND ({scope_clause}) AND {} AND {}",
            visibility.sql_predicate(),
            membership_predicate_for_embedding_scope(scope)
        );

        let mut query_params = vec![Value::Text(model_name.to_string())];
        query_params.extend(scope_params);

        let (total, raw_rows): (u32, Vec<(String, Vec<u8>, u32)>) = {
            let mut conn = self.read_connection();
            let transaction = conn.transaction()?;
            let count_sql = format!("SELECT COUNT(DISTINCT e.image_id) {from_and_where}");
            let total = transaction.query_row(
                &count_sql,
                to_param_refs(&query_params).as_slice(),
                |row| row.get(0),
            )?;

            let page_sql = format!(
                "SELECT DISTINCT e.image_id, e.vector, e.dims
                 {from_and_where}
                 ORDER BY e.image_id
                 LIMIT ? OFFSET ?"
            );
            let mut page_params = query_params;
            page_params.push(Value::Integer(limit as i64));
            page_params.push(Value::Integer(offset as i64));
            let raw_rows = {
                let mut stmt = transaction.prepare(&page_sql)?;
                let rows = stmt.query_map(to_param_refs(&page_params).as_slice(), |row| {
                    Ok((
                        row.get::<_, String>(0)?,
                        row.get::<_, Vec<u8>>(1)?,
                        row.get::<_, u32>(2)?,
                    ))
                })?;
                rows.collect::<Result<Vec<_>>>()?
            };
            transaction.commit()?;
            (total, raw_rows)
        };

        let dims = raw_rows.first().map(|row| row.2).unwrap_or(0);
        let returned = raw_rows.len() as u32;
        let mut ids = Vec::with_capacity(raw_rows.len());
        let mut vectors = Vec::with_capacity(raw_rows.len().saturating_mul(dims as usize));
        for (image_id, bytes, _) in raw_rows {
            ids.push(image_id);
            vectors.extend(decode_embedding_bytes(&bytes));
        }

        Ok(EmbeddingPage {
            ids,
            vectors,
            dims,
            total,
            offset,
            limit,
            has_more: offset.saturating_add(returned) < total,
        })
    }

    /// Lists live image IDs in a scope for embedding generation. This query
    /// deliberately does not join `embeddings`: images without a vector must
    /// remain eligible for generation.
    pub fn list_scoped_image_ids(
        &self,
        scope: &EmbeddingScope,
        limit: u32,
        offset: u32,
    ) -> Result<ImageIdPage> {
        let (scope_clause, scope_params, visibility) = scoped_where_clause(scope)?;
        let from_and_where = format!(
            "FROM images i
             JOIN image_files f ON f.image_id = i.id AND f.missing_at IS NULL
             LEFT JOIN selections s ON s.image_id = i.id AND s.project_id = '__global__'
             LEFT JOIN image_quality_metrics qm ON qm.image_id = i.id
             LEFT JOIN image_color_metrics cm ON cm.image_id = i.id
             LEFT JOIN image_similarity_group_items sgi ON sgi.image_id = i.id
             WHERE ({scope_clause}) AND {} AND {}",
            visibility.sql_predicate(),
            membership_predicate_for_embedding_scope(scope)
        );
        let (total, ids): (u32, Vec<String>) = {
            let mut conn = self.read_connection();
            let transaction = conn.transaction()?;
            let count_sql = format!("SELECT COUNT(DISTINCT i.id) {from_and_where}");
            let total = transaction.query_row(
                &count_sql,
                to_param_refs(&scope_params).as_slice(),
                |row| row.get(0),
            )?;
            let sql = format!(
                "SELECT DISTINCT i.id
                 {from_and_where}
                 ORDER BY i.id
                 LIMIT ? OFFSET ?"
            );
            let mut page_params = scope_params;
            page_params.push(Value::Integer(limit as i64));
            page_params.push(Value::Integer(offset as i64));
            let ids = {
                let mut stmt = transaction.prepare(&sql)?;
                let rows =
                    stmt.query_map(to_param_refs(&page_params).as_slice(), |row| row.get(0))?;
                rows.collect::<Result<Vec<_>>>()?
            };
            transaction.commit()?;
            (total, ids)
        };
        let returned = ids.len() as u32;
        Ok(ImageIdPage {
            ids,
            total,
            offset,
            limit,
            has_more: offset.saturating_add(returned) < total,
        })
    }

    pub fn get_embedding_vector(
        &self,
        image_id: &str,
        model_name: &str,
    ) -> Result<Option<Vec<f32>>> {
        let conn = self.conn.lock();
        conn.query_row(
            "SELECT vector FROM embeddings WHERE image_id = ?1 AND model_name = ?2",
            params![image_id, model_name],
            |row| row.get::<_, Vec<u8>>(0),
        )
        .optional()
        .map(|maybe_bytes| maybe_bytes.map(|bytes| decode_embedding_bytes(&bytes)))
    }

    pub fn find_similar(
        &self,
        vector: &[f32],
        model_name: &str,
        top_k: usize,
    ) -> Result<Vec<(String, f32)>> {
        if top_k == 0 {
            return Ok(Vec::new());
        }

        // Only hold the connection lock long enough to run the query and
        // collect the raw rows. Decoding embedding blobs and computing
        // cosine similarity for every row can be slow for large libraries,
        // and doing that work under the lock would block all other
        // database access (including UI reads) for the duration.
        let raw_rows: Vec<(String, Vec<u8>)> = {
            let conn = self.conn.lock();
            let mut stmt = conn.prepare(&format!(
                "SELECT DISTINCT e.image_id, e.vector
                 FROM embeddings e
                 JOIN images i ON i.id = e.image_id
                 JOIN image_files f ON f.image_id = i.id AND f.missing_at IS NULL
                 WHERE e.model_name = ?1 AND {NORMAL_LIBRARY_FILE_PREDICATE}"
            ))?;
            let rows = stmt.query_map(params![model_name], |row| {
                Ok((row.get::<_, String>(0)?, row.get::<_, Vec<u8>>(1)?))
            })?;
            rows.collect::<Result<Vec<_>>>()?
        };

        let mut scores: Vec<(String, f32)> = Vec::with_capacity(top_k);
        for (id, bytes) in raw_rows {
            let emb = decode_embedding_bytes(&bytes);
            let score = cosine_similarity(vector, &emb);
            if scores.len() < top_k {
                scores.push((id, score));
            } else if let Some((min_idx, (_, min_score))) = scores
                .iter()
                .enumerate()
                .min_by(|(_, a), (_, b)| a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal))
            {
                if score > *min_score {
                    scores[min_idx] = (id, score);
                }
            }
        }
        scores.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
        Ok(scores)
    }

    /// Finds nearest neighbors among the live images admitted by `scope`.
    /// The scope is applied in SQL before vector decoding and scoring, and
    /// the source image is always excluded from the candidate set.
    pub fn find_similar_in_scope(
        &self,
        image_id: &str,
        model_name: &str,
        scope: &EmbeddingScope,
        top_k: usize,
    ) -> Result<Option<Vec<(String, f32)>>> {
        let (scope_clause, scope_params, visibility) = scoped_where_clause(scope)?;
        let scope_clause = format!(
            "({scope_clause}) AND {}",
            membership_predicate_for_embedding_scope(scope)
        );
        self.find_similar_with_candidate_filter(
            image_id,
            model_name,
            top_k,
            &scope_clause,
            scope_params,
            visibility.sql_predicate(),
        )
    }

    /// Finds nearest live library neighbors without an authorization scope.
    /// The source image is excluded and ties are ordered by image ID.
    pub fn find_similar_live(
        &self,
        image_id: &str,
        model_name: &str,
        top_k: usize,
    ) -> Result<Option<Vec<(String, f32)>>> {
        self.find_similar_with_candidate_filter(
            image_id,
            model_name,
            top_k,
            NORMAL_LIBRARY_FILE_PREDICATE,
            Vec::new(),
            "1 = 1",
        )
    }

    /// Finds nearest live neighbors admitted by the MCP token-scope union.
    /// Scope filtering happens in SQL before vector decoding and ranking.
    pub fn find_similar_in_token_scope(
        &self,
        image_id: &str,
        model_name: &str,
        folders: &[String],
        collections: &[String],
        tag_norms: &[String],
        top_k: usize,
    ) -> Result<Option<Vec<(String, f32)>>> {
        let Some((scope_clause, mut scope_params)) =
            image_scope_filter(folders, collections, tag_norms)
        else {
            return Ok(Some(Vec::new()));
        };
        let (membership_predicate, membership_params) =
            membership_predicate_for_token_scope(collections);
        scope_params.extend(membership_params);
        self.find_similar_with_candidate_filter(
            image_id,
            model_name,
            top_k,
            &format!("({scope_clause}) AND ({membership_predicate})"),
            scope_params,
            "1 = 1",
        )
    }

    fn find_similar_with_candidate_filter(
        &self,
        image_id: &str,
        model_name: &str,
        top_k: usize,
        scope_clause: &str,
        scope_params: Vec<Value>,
        visibility_predicate: &str,
    ) -> Result<Option<Vec<(String, f32)>>> {
        let (source_bytes, raw_candidates): (Option<Vec<u8>>, Vec<(String, Vec<u8>)>) = {
            let mut conn = self.read_connection();
            let transaction = conn.transaction()?;
            let source_bytes = transaction
                .query_row(
                    "SELECT vector FROM embeddings WHERE image_id = ?1 AND model_name = ?2",
                    params![image_id, model_name],
                    |row| row.get(0),
                )
                .optional()?;

            let candidate_sql = format!(
                "SELECT DISTINCT e.image_id, e.vector
                 FROM embeddings e
                 JOIN images i ON i.id = e.image_id
                 JOIN image_files f ON f.image_id = i.id AND f.missing_at IS NULL
                 LEFT JOIN selections s ON s.image_id = i.id AND s.project_id = '__global__'
                 LEFT JOIN image_quality_metrics qm ON qm.image_id = i.id
                 LEFT JOIN image_color_metrics cm ON cm.image_id = i.id
                 LEFT JOIN image_similarity_group_items sgi ON sgi.image_id = i.id
                 WHERE e.model_name = ? AND e.image_id != ?
                   AND ({scope_clause}) AND {visibility_predicate}"
            );
            let mut candidate_params = vec![
                Value::Text(model_name.to_string()),
                Value::Text(image_id.to_string()),
            ];
            candidate_params.extend(scope_params);
            let raw_candidates = {
                let mut stmt = transaction.prepare(&candidate_sql)?;
                let rows = stmt.query_map(to_param_refs(&candidate_params).as_slice(), |row| {
                    Ok((row.get::<_, String>(0)?, row.get::<_, Vec<u8>>(1)?))
                })?;
                rows.collect::<Result<Vec<_>>>()?
            };
            transaction.commit()?;
            (source_bytes, raw_candidates)
        };

        let Some(source_bytes) = source_bytes else {
            return Ok(None);
        };
        if top_k == 0 {
            return Ok(Some(Vec::new()));
        }

        let source = decode_embedding_bytes(&source_bytes);
        let mut scores: Vec<(String, f32)> = raw_candidates
            .into_iter()
            .map(|(id, bytes)| {
                let candidate = decode_embedding_bytes(&bytes);
                (id, cosine_similarity(&source, &candidate))
            })
            .collect();
        scores.sort_by(|a, b| {
            b.1.partial_cmp(&a.1)
                .unwrap_or(std::cmp::Ordering::Equal)
                .then_with(|| a.0.cmp(&b.0))
        });
        scores.truncate(top_k);
        Ok(Some(scores))
    }

    pub fn embedding_count(&self, model_name: &str) -> Result<u32> {
        let conn = self.conn.lock();
        conn.query_row(
            &format!(
                "SELECT COUNT(DISTINCT e.image_id)
                 FROM embeddings e
                 JOIN images i ON i.id = e.image_id
                 JOIN image_files f ON f.image_id = i.id AND f.missing_at IS NULL
                 WHERE e.model_name = ?1 AND {NORMAL_LIBRARY_FILE_PREDICATE}"
            ),
            params![model_name],
            |row| row.get(0),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn open_test_db() -> Database {
        Database::open(std::path::Path::new(":memory:")).unwrap()
    }

    fn insert_test_image(db: &Database, id: &str) {
        let conn = db.conn.lock();
        conn.execute(
            "INSERT INTO images (id, sha256_hash, width, height, format, file_size, created_at, imported_at, ai_prompt) VALUES (?1, ?2, 100, 100, 'png', 1000, '2026-01-01', '2026-01-01', NULL)",
            params![id, format!("hash_{}", id)],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO image_files (id, image_id, path, last_seen_at, missing_at)
             VALUES (?1, ?2, ?3, '2026-01-01', NULL)",
            params![format!("file_{id}"), id, format!("/library/{id}.png")],
        )
        .unwrap();
    }

    fn referenced_source() -> ReferencedSource {
        ReferencedSource {
            id: "source-1".into(),
            platform_volume_id: Some("volume-uuid-1".into()),
            display_name: "UNTITLED".into(),
            last_mount_path: Some("/Volumes/UNTITLED".into()),
            source_kind: ReferencedSourceKind::SdCard,
            capacity_bytes: Some(64_000_000_000),
            recursive_default: false,
            settings_json: "{}".into(),
            last_seen_at: "2026-09-01T10:00:00Z".into(),
            offline_at: None,
        }
    }

    fn insert_scoped_image(
        db: &Database,
        id: &str,
        path: &str,
        width: u32,
        height: u32,
        import_batch_id: Option<&str>,
    ) {
        let conn = db.conn.lock();
        conn.execute(
            "INSERT INTO images (
                id, sha256_hash, width, height, format, file_size, created_at,
                imported_at, ai_prompt, import_batch_id
             ) VALUES (?1, ?2, ?3, ?4, 'png', 1000, '2026-01-01',
                       '2026-01-01', NULL, ?5)",
            params![id, format!("hash_{id}"), width, height, import_batch_id],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO image_files (id, image_id, path, last_seen_at, missing_at)
             VALUES (?1, ?2, ?3, '2026-01-01', NULL)",
            params![format!("file_{id}"), id, path],
        )
        .unwrap();
    }

    fn mark_as_browsed_file(db: &Database, file_id: &str) {
        db.conn
            .lock()
            .execute(
                "UPDATE image_files SET library_member = 0 WHERE id = ?1",
                params![file_id],
            )
            .unwrap();
    }

    fn reject_image(db: &Database, id: &str) {
        db.conn
            .lock()
            .execute(
                "INSERT INTO selections (image_id, project_id, decision)
                 VALUES (?1, '__global__', 'reject')",
                [id],
            )
            .unwrap();
    }

    fn ids_for_scope(db: &Database, scope: &EmbeddingScope) -> Vec<String> {
        db.list_scoped_image_ids(scope, 25_000, 0).unwrap().ids
    }

    /// Straightforward reference implementation of top-k cosine similarity,
    /// used to check that the lock-scoped `find_similar` returns identical
    /// results (same scores, same ordering, same tie handling).
    fn reference_find_similar(
        vector: &[f32],
        rows: &[(String, Vec<f32>)],
        top_k: usize,
    ) -> Vec<(String, f32)> {
        let mut scores: Vec<(String, f32)> = rows
            .iter()
            .map(|(id, emb)| (id.clone(), cosine_similarity(vector, emb)))
            .collect();
        scores.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
        scores.truncate(top_k);
        scores
    }

    #[test]
    fn find_similar_matches_reference_implementation() {
        let db = open_test_db();
        let model = "clip-vit-b32";
        let fixture: Vec<(&str, Vec<f32>)> = vec![
            ("img1", vec![1.0, 0.0, 0.0]),
            ("img2", vec![0.9, 0.1, 0.0]),
            ("img3", vec![0.0, 1.0, 0.0]),
            ("img4", vec![0.0, 0.0, 1.0]),
            ("img5", vec![0.5, 0.5, 0.0]),
            ("img6", vec![-1.0, 0.0, 0.0]),
        ];
        for (id, vec) in &fixture {
            insert_test_image(&db, id);
            db.store_embedding(id, model, vec).unwrap();
        }

        let query = vec![1.0, 0.0, 0.0];
        let top_k = 3;

        let actual = db.find_similar(&query, model, top_k).unwrap();

        let reference_rows: Vec<(String, Vec<f32>)> = fixture
            .iter()
            .map(|(id, v)| (id.to_string(), v.clone()))
            .collect();
        let expected = reference_find_similar(&query, &reference_rows, top_k);

        assert_eq!(actual.len(), expected.len());
        assert_eq!(actual, expected);
        assert_eq!(actual[0].0, "img1");
    }

    #[test]
    fn find_similar_respects_top_k_zero() {
        let db = open_test_db();
        insert_test_image(&db, "img1");
        db.store_embedding("img1", "clip-vit-b32", &[1.0, 0.0])
            .unwrap();
        let result = db.find_similar(&[1.0, 0.0], "clip-vit-b32", 0).unwrap();
        assert!(result.is_empty());
    }

    #[test]
    fn find_similar_filters_by_model_name() {
        let db = open_test_db();
        insert_test_image(&db, "img1");
        db.store_embedding("img1", "model-a", &[1.0, 0.0]).unwrap();
        let result = db.find_similar(&[1.0, 0.0], "model-b", 10).unwrap();
        assert!(result.is_empty());
    }

    #[test]
    fn scoped_neighbors_filter_candidates_before_scoring_and_exclude_source() {
        let db = open_test_db();
        for (id, path, vector) in [
            ("source", "/selected/source.png", vec![1.0, 0.0]),
            ("in-close", "/selected/close.png", vec![0.8, 0.6]),
            ("in-far", "/selected/far.png", vec![0.0, 1.0]),
            ("outside-closest", "/other/closest.png", vec![0.999, 0.001]),
        ] {
            insert_scoped_image(&db, id, path, 500, 500, None);
            db.store_embedding(id, "model", &vector).unwrap();
        }
        let scope = EmbeddingScope::Folder {
            path: "/selected".to_string(),
            min_size: 0,
            include_rejected: false,
        };

        let neighbors = db
            .find_similar_in_scope("source", "model", &scope, 2)
            .unwrap()
            .expect("source embedding exists");

        assert_eq!(neighbors.len(), 2);
        assert_eq!(neighbors[0].0, "in-close");
        assert!((neighbors[0].1 - 0.8).abs() < 0.000_001);
        assert_eq!(neighbors[1], ("in-far".to_string(), 0.0));
        assert!(neighbors.iter().all(|(id, _)| id != "source"));
        assert!(neighbors.iter().all(|(id, _)| id != "outside-closest"));
    }

    #[test]
    fn live_neighbors_exclude_source_missing_files_and_tiebreak_by_id() {
        let db = open_test_db();
        for (id, path) in [
            ("source", "/test/source.png"),
            ("beta", "/test/beta.png"),
            ("alpha", "/test/alpha.png"),
            ("missing", "/test/missing.png"),
        ] {
            insert_scoped_image(&db, id, path, 100, 100, None);
            db.store_embedding(id, "clip-vit-b32", &[1.0, 0.0]).unwrap();
        }
        db.conn
            .lock()
            .execute(
                "UPDATE image_files SET missing_at = '2026-08-08' WHERE image_id = 'missing'",
                [],
            )
            .unwrap();

        let neighbors = db
            .find_similar_live("source", "clip-vit-b32", 10)
            .unwrap()
            .expect("source embedding exists");

        assert_eq!(
            neighbors
                .iter()
                .map(|item| item.0.as_str())
                .collect::<Vec<_>>(),
            vec!["alpha", "beta"]
        );
    }

    #[test]
    fn image_ids_without_embedding_preserves_order_dedupes_and_is_model_specific() {
        let db = open_test_db();
        for id in ["has-selected", "other-model", "missing", "missing-two"] {
            insert_test_image(&db, id);
        }
        db.store_embedding("has-selected", "selected-model", &[1.0, 0.0])
            .unwrap();
        db.store_embedding("other-model", "different-model", &[0.0, 1.0])
            .unwrap();

        let requested = vec![
            "has-selected".to_string(),
            "missing".to_string(),
            "other-model".to_string(),
            "missing".to_string(),
            "missing-two".to_string(),
        ];

        let missing = db
            .image_ids_without_embedding(&requested, "selected-model")
            .unwrap();

        assert_eq!(missing, vec!["missing", "other-model", "missing-two"]);

        let large_request = (0..505)
            .map(|index| format!("unembedded-{index:03}"))
            .collect::<Vec<_>>();
        assert_eq!(
            db.image_ids_without_embedding(&large_request, "selected-model")
                .unwrap(),
            large_request
        );
    }

    #[test]
    fn get_all_embeddings_decodes_all_rows_for_model() {
        let db = open_test_db();
        insert_test_image(&db, "img1");
        insert_test_image(&db, "img2");
        db.store_embedding("img1", "clip-vit-b32", &[0.1, 0.2, 0.3])
            .unwrap();
        db.store_embedding("img2", "clip-vit-b32", &[0.4, 0.5, 0.6])
            .unwrap();

        let mut result = db.get_all_embeddings("clip-vit-b32").unwrap();
        result.sort_by(|a, b| a.0.cmp(&b.0));

        assert_eq!(
            result,
            vec![
                ("img1".to_string(), vec![0.1, 0.2, 0.3]),
                ("img2".to_string(), vec![0.4, 0.5, 0.6]),
            ]
        );
    }

    #[test]
    fn scoped_embedding_folder_filters_before_pagination_and_keeps_vectors_aligned() {
        let db = open_test_db();
        for (id, path, width, vector) in [
            ("a-small", "/art/set_%/a.png", 100, vec![1.0, 1.5]),
            ("b-first", "/art/set_%/b.png", 500, vec![2.0, 2.5]),
            ("c-second", "/art/set_%/deep/c.png", 600, vec![3.0, 3.5]),
            ("d-prefix", "/art/set_%up/d.png", 700, vec![4.0, 4.5]),
            ("e-wild", "/art/setXA/e.png", 800, vec![5.0, 5.5]),
        ] {
            insert_scoped_image(&db, id, path, width, width, None);
            db.store_embedding(id, "model", &vector).unwrap();
        }
        insert_scoped_image(&db, "z-no-embedding", "/art/set_%/z.png", 900, 900, None);
        db.conn
            .lock()
            .execute(
                "INSERT INTO image_files (id, image_id, path, last_seen_at, missing_at)
                 VALUES ('file-b-copy', 'b-first', '/art/set_%/b-copy.png', '2026-01-01', NULL)",
                [],
            )
            .unwrap();

        let scope = EmbeddingScope::Folder {
            path: "/art/set_%".to_string(),
            min_size: 400,
            include_rejected: false,
        };
        let first = db.get_scoped_embedding_page("model", &scope, 1, 0).unwrap();
        let second = db.get_scoped_embedding_page("model", &scope, 1, 1).unwrap();

        assert_eq!(first.ids, vec!["b-first"]);
        assert_eq!(first.vectors, vec![2.0, 2.5]);
        assert_eq!(first.dims, 2);
        assert_eq!(first.total, 2);
        assert!(first.has_more);
        assert_eq!(second.ids, vec!["c-second"]);
        assert_eq!(second.vectors, vec![3.0, 3.5]);
        assert_eq!(second.total, 2);
        assert!(!second.has_more);

        assert_eq!(
            ids_for_scope(&db, &scope),
            vec!["b-first", "c-second", "z-no-embedding"]
        );
    }

    #[test]
    fn scoped_image_ids_match_collection_detected_batch_and_all_visibility() {
        let db = open_test_db();
        for id in ["a-keep", "b-reject", "c-other", "d-missing"] {
            insert_scoped_image(
                &db,
                id,
                &format!("/library/{id}.png"),
                500,
                500,
                (id != "c-other").then_some("batch-1"),
            );
        }
        db.conn
            .lock()
            .execute(
                "UPDATE image_files SET missing_at = '2026-02-01' WHERE image_id = 'd-missing'",
                [],
            )
            .unwrap();
        reject_image(&db, "b-reject");
        {
            let conn = db.conn.lock();
            conn.execute(
                "INSERT INTO projects (id, name, collection_type, created_at)
                 VALUES ('collection-1', 'One', 'manual', '2026-01-01')",
                [],
            )
            .unwrap();
            for (position, id) in ["a-keep", "b-reject", "d-missing"].iter().enumerate() {
                conn.execute(
                    "INSERT INTO collection_items (collection_id, image_id, position)
                     VALUES ('collection-1', ?1, ?2)",
                    params![id, position as i64],
                )
                .unwrap();
            }
            for (det_id, id) in ["a-keep", "a-keep", "b-reject", "c-other"]
                .iter()
                .enumerate()
            {
                conn.execute(
                    "INSERT INTO detections (
                        id, image_id, model_name, class_name, confidence,
                        x, y, width, height, created_at
                     ) VALUES (?1, ?2, 'yolo', 'person', 0.9, 0, 0, 1, 1, '2026-01-01')",
                    params![format!("det-{det_id}"), id],
                )
                .unwrap();
            }
        }

        let hidden_collection = EmbeddingScope::Collection {
            id: "collection-1".to_string(),
            include_rejected: false,
        };
        let shown_collection = EmbeddingScope::Collection {
            id: "collection-1".to_string(),
            include_rejected: true,
        };
        let detected = EmbeddingScope::DetectedClass {
            class_name: "person".to_string(),
            include_rejected: false,
        };
        let batch = EmbeddingScope::ImportBatch {
            batch_id: "batch-1".to_string(),
            include_rejected: false,
        };
        let all_hidden = EmbeddingScope::All {
            include_rejected: false,
        };

        assert_eq!(ids_for_scope(&db, &hidden_collection), vec!["a-keep"]);
        assert_eq!(
            ids_for_scope(&db, &shown_collection),
            vec!["a-keep", "b-reject"]
        );
        assert_eq!(ids_for_scope(&db, &detected), vec!["a-keep", "c-other"]);
        assert_eq!(ids_for_scope(&db, &batch), vec!["a-keep"]);
        assert_eq!(ids_for_scope(&db, &all_hidden), vec!["a-keep", "c-other"]);
    }

    #[test]
    fn automatic_embedding_scopes_exclude_referenced_only_images_but_collections_keep_them() {
        let db = open_test_db();
        db.upsert_referenced_source(&referenced_source()).unwrap();

        insert_scoped_image(
            &db,
            "referenced-only",
            "/Volumes/UNTITLED/DCIM/only.jpg",
            500,
            500,
            Some("batch-1"),
        );
        db.attach_referenced_file("source-1", "file_referenced-only", "DCIM/only.jpg")
            .unwrap();
        mark_as_browsed_file(&db, "file_referenced-only");
        db.store_embedding("referenced-only", "model", &[1.0, 0.0])
            .unwrap();

        insert_scoped_image(
            &db,
            "mixed",
            "/Volumes/UNTITLED/DCIM/mixed.jpg",
            500,
            500,
            Some("batch-1"),
        );
        db.attach_referenced_file("source-1", "file_mixed", "DCIM/mixed.jpg")
            .unwrap();
        mark_as_browsed_file(&db, "file_mixed");
        {
            let conn = db.conn.lock();
            conn.execute(
                "INSERT INTO image_files (id, image_id, path, last_seen_at, missing_at)
                 VALUES ('mixed-normal-file', 'mixed', '/Pictures/mixed.jpg', '2026-09-01', NULL)",
                [],
            )
            .unwrap();
        }
        db.store_embedding("mixed", "model", &[0.8, 0.6]).unwrap();

        {
            let conn = db.conn.lock();
            conn.execute(
                "INSERT INTO projects (id, name, collection_type, created_at)
                 VALUES ('collection-1', 'Keep referenced', 'manual', '2026-09-01')",
                [],
            )
            .unwrap();
            conn.execute(
                "INSERT INTO collection_items (collection_id, image_id, position)
                 VALUES ('collection-1', 'referenced-only', 0)",
                [],
            )
            .unwrap();
        }

        let all = EmbeddingScope::All {
            include_rejected: true,
        };
        let collection = EmbeddingScope::Collection {
            id: "collection-1".into(),
            include_rejected: true,
        };

        assert_eq!(ids_for_scope(&db, &all), vec!["mixed"]);
        assert_eq!(
            db.get_scoped_embedding_page("model", &all, 20, 0)
                .unwrap()
                .ids,
            vec!["mixed"]
        );
        assert_eq!(ids_for_scope(&db, &collection), vec!["referenced-only"]);
        assert_eq!(
            db.get_scoped_embedding_page("model", &collection, 20, 0)
                .unwrap()
                .ids,
            vec!["referenced-only"]
        );
        assert_eq!(
            db.get_all_embeddings("model")
                .unwrap()
                .into_iter()
                .map(|(id, _)| id)
                .collect::<Vec<_>>(),
            vec!["mixed"]
        );
        assert_eq!(
            db.get_embedding_page("model", 20, 0).unwrap().ids,
            vec!["mixed"]
        );
        assert_eq!(db.embedding_count("model").unwrap(), 1);
    }

    #[test]
    fn automatic_similarity_scopes_exclude_referenced_only_candidates() {
        let db = open_test_db();
        db.upsert_referenced_source(&referenced_source()).unwrap();

        insert_scoped_image(&db, "source", "/Pictures/source.jpg", 500, 500, None);
        db.store_embedding("source", "model", &[1.0, 0.0]).unwrap();

        insert_scoped_image(
            &db,
            "referenced-only",
            "/Volumes/UNTITLED/DCIM/only.jpg",
            500,
            500,
            None,
        );
        db.attach_referenced_file("source-1", "file_referenced-only", "DCIM/only.jpg")
            .unwrap();
        mark_as_browsed_file(&db, "file_referenced-only");
        db.store_embedding("referenced-only", "model", &[0.999, 0.001])
            .unwrap();

        insert_scoped_image(
            &db,
            "mixed",
            "/Volumes/UNTITLED/DCIM/mixed.jpg",
            500,
            500,
            None,
        );
        db.attach_referenced_file("source-1", "file_mixed", "DCIM/mixed.jpg")
            .unwrap();
        mark_as_browsed_file(&db, "file_mixed");
        {
            let conn = db.conn.lock();
            conn.execute(
                "INSERT INTO image_files (id, image_id, path, last_seen_at, missing_at)
                 VALUES ('mixed-normal-file', 'mixed', '/Pictures/mixed.jpg', '2026-09-01', NULL)",
                [],
            )
            .unwrap();
        }
        db.store_embedding("mixed", "model", &[0.8, 0.6]).unwrap();

        let all = EmbeddingScope::All {
            include_rejected: true,
        };
        let similarity = db.find_similar(&[0.6, 0.8], "model", 1).unwrap();
        assert_eq!(similarity[0].0, "mixed");
        assert_eq!(
            db.find_similar_live("source", "model", 20)
                .unwrap()
                .unwrap(),
            vec![("mixed".to_string(), 0.8)]
        );
        assert_eq!(
            db.find_similar_in_scope("source", "model", &all, 20)
                .unwrap()
                .unwrap(),
            vec![("mixed".to_string(), 0.8)]
        );
        assert_eq!(
            db.find_similar_in_token_scope("source", "model", &["/".into()], &[], &[], 20)
                .unwrap()
                .unwrap(),
            vec![("mixed".to_string(), 0.8)]
        );

        {
            let conn = db.conn.lock();
            conn.execute(
                "INSERT INTO projects (id, name, collection_type, created_at)
                 VALUES ('collection-1', 'Keep referenced', 'manual', '2026-09-01')",
                [],
            )
            .unwrap();
            conn.execute(
                "INSERT INTO collection_items (collection_id, image_id, position)
                 VALUES ('collection-1', 'referenced-only', 0)",
                [],
            )
            .unwrap();
        }
        let collection_neighbors = db
            .find_similar_in_token_scope("source", "model", &[], &["collection-1".into()], &[], 20)
            .unwrap()
            .unwrap();
        assert_eq!(collection_neighbors.len(), 1);
        assert_eq!(collection_neighbors[0].0, "referenced-only");
    }

    #[test]
    fn scoped_smart_and_filtered_apply_min_size_and_rejected_filter_semantics() {
        let db = open_test_db();
        for (id, size) in [("a-small", 200), ("b-large", 900), ("c-reject", 950)] {
            insert_scoped_image(&db, id, &format!("/library/{id}.png"), size, size, None);
        }
        reject_image(&db, "c-reject");

        let filtered = EmbeddingScope::Filtered {
            min_size: 800,
            include_rejected: false,
        };
        assert_eq!(ids_for_scope(&db, &filtered), vec!["b-large"]);

        let large_filter = r#"{"type":"rule","field":"width","op":"gte","value":800}"#;
        let smart = EmbeddingScope::Smart {
            id: "smart-large".to_string(),
            filter_json: large_filter.to_string(),
            include_rejected: false,
        };
        assert_eq!(ids_for_scope(&db, &smart), vec!["b-large"]);

        let rejected_filter = r#"{"type":"rule","field":"decision","op":"eq","value":"reject"}"#;
        let smart_rejected = EmbeddingScope::Smart {
            id: "smart-rejected".to_string(),
            filter_json: rejected_filter.to_string(),
            include_rejected: false,
        };
        assert_eq!(ids_for_scope(&db, &smart_rejected), vec!["c-reject"]);
    }

    #[test]
    fn cluster_names_prioritize_shared_tags_then_yolo_then_filenames() {
        let db = open_test_db();
        for (id, path) in [
            ("tag-a", "/library/sunset_001.png"),
            ("tag-b", "/library/sunset_002.png"),
            ("dog-a", "/library/frame_101.png"),
            ("dog-b", "/library/frame_102.png"),
            ("file-a", "/library/mountain_mist_01.png"),
            ("file-b", "/library/mountain_mist_02.png"),
        ] {
            insert_scoped_image(&db, id, path, 500, 500, None);
        }
        db.add_image_tag("tag-a", "Golden Hour", "user", "manual", None)
            .unwrap();
        db.add_image_tag("tag-b", "Golden Hour", "user", "manual", None)
            .unwrap();
        db.conn
            .lock()
            .execute(
                "INSERT INTO detections (
                    id, image_id, model_name, class_name, confidence,
                    x, y, width, height, created_at
                 ) VALUES ('det-tag', 'tag-a', 'yolo11m', 'golden_hour', 0.9, 0, 0, 1, 1, '2026-01-01')",
                [],
            )
            .unwrap();
        for (index, id) in ["dog-a", "dog-b"].iter().enumerate() {
            db.conn
                .lock()
                .execute(
                    "INSERT INTO detections (
                        id, image_id, model_name, class_name, confidence,
                        x, y, width, height, created_at
                     ) VALUES (?1, ?2, 'yolo11m', 'dog', 0.9, 0, 0, 1, 1, '2026-01-01')",
                    params![format!("det-{index}"), id],
                )
                .unwrap();
        }

        let names = db
            .name_embedding_clusters(&[
                EmbeddingClusterMembership {
                    cluster_id: 0,
                    image_ids: vec!["tag-a".into(), "tag-b".into()],
                },
                EmbeddingClusterMembership {
                    cluster_id: 1,
                    image_ids: vec!["dog-a".into(), "dog-b".into()],
                },
                EmbeddingClusterMembership {
                    cluster_id: 2,
                    image_ids: vec!["file-a".into(), "file-b".into()],
                },
            ])
            .unwrap();

        assert_eq!(
            names,
            vec![
                EmbeddingClusterName {
                    cluster_id: 0,
                    label: "Golden Hour".into(),
                    source: "tag".into(),
                },
                EmbeddingClusterName {
                    cluster_id: 1,
                    label: "Dog".into(),
                    source: "yolo".into(),
                },
                EmbeddingClusterName {
                    cluster_id: 2,
                    label: "Mountain".into(),
                    source: "filename".into(),
                },
            ]
        );
    }

    #[test]
    fn cluster_names_dedupe_ids_and_break_equal_scores_alphabetically() {
        let db = open_test_db();
        insert_scoped_image(&db, "apple", "/library/apple_01.png", 500, 500, None);
        insert_scoped_image(&db, "zebra", "/library/zebra_01.png", 500, 500, None);

        let first = db
            .name_embedding_clusters(&[EmbeddingClusterMembership {
                cluster_id: 7,
                image_ids: vec!["zebra".into(), "apple".into(), "apple".into()],
            }])
            .unwrap();
        let second = db
            .name_embedding_clusters(&[EmbeddingClusterMembership {
                cluster_id: 7,
                image_ids: vec!["apple".into(), "zebra".into()],
            }])
            .unwrap();

        assert_eq!(first, second);
        assert_eq!(first[0].label, "Apple");
        assert_eq!(first[0].source, "filename");
    }

    #[test]
    fn cluster_names_canonicalize_equal_weight_tag_and_yolo_evidence() {
        let db = open_test_db();
        insert_scoped_image(&db, "tagged", "/library/a.png", 500, 500, None);
        insert_scoped_image(&db, "detected", "/library/b.png", 500, 500, None);
        db.add_image_tag(
            "tagged",
            "golden_hour",
            "object",
            "metadata:vision",
            Some(0.8),
        )
        .unwrap();
        db.conn
            .lock()
            .execute(
                "INSERT INTO detections (
                    id, image_id, model_name, class_name, confidence,
                    x, y, width, height, created_at
                 ) VALUES ('det-mixed', 'detected', 'yolo11m', 'golden hour', 0.9, 0, 0, 1, 1, '2026-01-01')",
                [],
            )
            .unwrap();

        let names = db
            .name_embedding_clusters(&[EmbeddingClusterMembership {
                cluster_id: 3,
                image_ids: vec!["detected".into(), "tagged".into()],
            }])
            .unwrap();

        assert_eq!(names[0].label, "Golden Hour");
        assert_eq!(names[0].source, "tag");
    }
}
