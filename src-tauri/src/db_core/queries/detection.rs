// Copyright (c) 2026-present Gleb Kalinin. Architecture and design by author.
// Implementation assisted by Claude (Anthropic). See AUTHORSHIP.md.

use crate::db_core::db::Database;
use crate::db_core::models::*;
use rusqlite::{params, Result};

impl Database {
    pub fn store_detections(
        &self,
        image_id: &str,
        model_name: &str,
        detections: &[crate::db_core::detection::Detection],
    ) -> Result<()> {
        let conn = self.conn.lock();
        // Clear previous detections for this image+model
        conn.execute(
            "DELETE FROM detections WHERE image_id = ?1 AND model_name = ?2",
            params![image_id, model_name],
        )?;
        for det in detections {
            conn.execute(
                "INSERT INTO detections (id, image_id, model_name, class_name, confidence, x, y, width, height, created_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
                params![
                    uuid::Uuid::new_v4().to_string(),
                    image_id,
                    model_name,
                    det.class_name,
                    det.confidence,
                    det.x,
                    det.y,
                    det.width,
                    det.height,
                    chrono::Utc::now().to_rfc3339(),
                ],
            )?;
        }
        Ok(())
    }

    pub fn get_detections(
        &self,
        image_id: &str,
        model_name: Option<&str>,
    ) -> Result<Vec<crate::db_core::detection::Detection>> {
        let conn = self.conn.lock();
        let (sql, params_vec): (String, Vec<Box<dyn rusqlite::types::ToSql>>) = if let Some(mn) =
            model_name
        {
            (
                "SELECT class_name, confidence, x, y, width, height FROM detections WHERE image_id = ?1 AND model_name = ?2 ORDER BY confidence DESC".to_string(),
                vec![Box::new(image_id.to_string()), Box::new(mn.to_string())],
            )
        } else {
            (
                "SELECT class_name, confidence, x, y, width, height FROM detections WHERE image_id = ?1 ORDER BY confidence DESC".to_string(),
                vec![Box::new(image_id.to_string())],
            )
        };
        let mut stmt = conn.prepare(&sql)?;
        let params_refs: Vec<&dyn rusqlite::types::ToSql> =
            params_vec.iter().map(|p| p.as_ref()).collect();
        let rows = stmt.query_map(params_refs.as_slice(), |row| {
            Ok(crate::db_core::detection::Detection {
                class_name: row.get(0)?,
                confidence: row.get(1)?,
                x: row.get(2)?,
                y: row.get(3)?,
                width: row.get(4)?,
                height: row.get(5)?,
            })
        })?;
        rows.collect::<Result<Vec<_>>>()
    }

    pub fn search_by_class(&self, class_name: &str, limit: u32) -> Result<Vec<(String, f32)>> {
        let conn = self.conn.lock();
        let mut stmt = conn.prepare(
            "SELECT DISTINCT image_id, MAX(confidence) as max_conf
             FROM detections WHERE class_name = ?1
             GROUP BY image_id ORDER BY max_conf DESC LIMIT ?2",
        )?;
        let rows = stmt.query_map(params![class_name, limit], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, f32>(1)?))
        })?;
        rows.collect::<Result<Vec<_>>>()
    }

    pub fn count_by_class(&self, class_name: &str) -> Result<u32> {
        let conn = self.conn.lock();
        conn.query_row(
            "SELECT COUNT(DISTINCT image_id) FROM detections WHERE class_name = ?1",
            params![class_name],
            |row| row.get::<_, u32>(0),
        )
    }

    pub fn list_images_by_class(
        &self,
        class_name: &str,
        limit: u32,
        offset: u32,
    ) -> Result<Vec<ImageWithFile>> {
        let conn = self.conn.lock();
        let mut stmt = conn.prepare(
            "SELECT i.id, i.sha256_hash, i.width, i.height, i.format, i.file_size,
                    i.created_at, i.imported_at, f.path,
                    s.star_rating, s.color_label, s.decision, i.source_label, i.ai_prompt,
                    i.raw_metadata, f.missing_at
             FROM detections d
             JOIN images i ON i.id = d.image_id
             JOIN image_files f ON f.image_id = i.id AND f.missing_at IS NULL
             LEFT JOIN selections s ON s.image_id = i.id AND s.project_id = '__global__'
             WHERE d.class_name = ?1
             GROUP BY i.id
             ORDER BY MAX(d.confidence) DESC, i.imported_at DESC
             LIMIT ?2 OFFSET ?3",
        )?;
        let rows = stmt.query_map(params![class_name, limit, offset], |row| {
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
                    file_size: row.get(5)?,
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

    pub fn detection_count(&self, model_name: &str) -> Result<u32> {
        let conn = self.conn.lock();
        conn.query_row(
            "SELECT COUNT(DISTINCT image_id) FROM detections WHERE model_name = ?1",
            params![model_name],
            |row| row.get::<_, u32>(0),
        )
    }
}
