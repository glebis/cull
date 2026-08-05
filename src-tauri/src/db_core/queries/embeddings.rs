// Copyright (c) 2026-present Gleb Kalinin. Architecture and design by author.
// Implementation assisted by Claude (Anthropic). See AUTHORSHIP.md.

use crate::db_core::db::Database;
use crate::db_core::db::{cosine_similarity, decode_embedding_bytes};

use crate::db_core::models::*;
use rusqlite::{params, OptionalExtension, Result};

impl Database {
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
}
