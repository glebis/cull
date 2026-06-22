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
        let conn = self.conn.lock();
        let mut stmt =
            conn.prepare("SELECT image_id, vector, dims FROM embeddings WHERE model_name = ?1")?;
        let rows = stmt.query_map(params![model_name], |row| {
            let image_id: String = row.get(0)?;
            let bytes: Vec<u8> = row.get(1)?;
            let _dims: u32 = row.get(2)?;
            let vector = decode_embedding_bytes(&bytes);
            Ok((image_id, vector))
        })?;
        rows.collect::<Result<Vec<_>>>()
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

        let conn = self.conn.lock();
        let mut stmt =
            conn.prepare("SELECT image_id, vector FROM embeddings WHERE model_name = ?1")?;
        let rows = stmt.query_map(params![model_name], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, Vec<u8>>(1)?))
        })?;

        let mut scores: Vec<(String, f32)> = Vec::with_capacity(top_k);
        for row in rows {
            let (id, bytes) = row?;
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
