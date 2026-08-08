// Copyright (c) 2026-present Gleb Kalinin. Architecture and design by author.
// Implementation assisted by Claude (Anthropic). See AUTHORSHIP.md.

use crate::db_core::db::Database;
use crate::db_core::db::{cosine_similarity, decode_embedding_bytes};

use crate::db_core::models::*;
use crate::db_core::smart_collections::FilterNode;
use crate::db_core::visibility::RejectedVisibility;
use rusqlite::types::Value;
use rusqlite::{params, OptionalExtension, Result, ToSql};
use std::collections::HashSet;

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

fn to_param_refs(params: &[Value]) -> Vec<&dyn ToSql> {
    params.iter().map(|param| param as &dyn ToSql).collect()
}

impl Database {
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
            let mut stmt =
                conn.prepare("SELECT image_id, vector FROM embeddings WHERE model_name = ?1")?;
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
            "SELECT COUNT(*) FROM embeddings WHERE model_name = ?1",
            params![model_name],
            |row| row.get(0),
        )?;
        let mut stmt = conn.prepare(
            "SELECT image_id, vector, dims
             FROM embeddings
             WHERE model_name = ?1
             ORDER BY image_id
             LIMIT ?2 OFFSET ?3",
        )?;
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
             WHERE e.model_name = ? AND ({scope_clause}) AND {}",
            visibility.sql_predicate()
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
             WHERE ({scope_clause}) AND {}",
            visibility.sql_predicate()
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
            let mut stmt =
                conn.prepare("SELECT image_id, vector FROM embeddings WHERE model_name = ?1")?;
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
                   AND ({scope_clause}) AND {}",
                visibility.sql_predicate()
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
            "SELECT COUNT(*) FROM embeddings WHERE model_name = ?1",
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
}
