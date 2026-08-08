// Copyright (c) 2026-present Gleb Kalinin. Architecture and design by author.
// Implementation assisted by Claude (Anthropic). See AUTHORSHIP.md.

use crate::db_core::db::{row_u64, Database};
use crate::db_core::models::*;
use crate::db_core::queries::images::image_scope_filter;
use crate::db_core::visibility::RejectedVisibility;
use rusqlite::types::Value;
use rusqlite::{params, Result};

impl Database {
    pub fn store_detections(
        &self,
        image_id: &str,
        model_name: &str,
        detections: &[crate::db_core::detection::Detection],
    ) -> Result<()> {
        let mut conn = self.conn.lock();
        let tx = conn.transaction()?;
        // Clear previous detections for this image+model
        tx.execute(
            "DELETE FROM detections WHERE image_id = ?1 AND model_name = ?2",
            params![image_id, model_name],
        )?;
        for det in detections {
            tx.execute(
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
        tx.execute(
            "INSERT INTO image_analysis_status (image_id, analysis_kind, model_name, completed_at)
             VALUES (?1, 'detection', ?2, ?3)
             ON CONFLICT(image_id, analysis_kind, model_name) DO UPDATE SET
                completed_at = excluded.completed_at",
            params![image_id, model_name, chrono::Utc::now().to_rfc3339()],
        )?;
        tx.commit()
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
        let conn = self.read_connection();
        let mut stmt = conn.prepare(
            "SELECT DISTINCT image_id, MAX(confidence) as max_conf
             FROM detections WHERE class_name = ?1
             GROUP BY image_id ORDER BY max_conf DESC, image_id ASC LIMIT ?2",
        )?;
        let rows = stmt.query_map(params![class_name, limit], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, f32>(1)?))
        })?;
        rows.collect::<Result<Vec<_>>>()
    }

    pub fn search_by_class_in_scope(
        &self,
        class_name: &str,
        folders: &[String],
        collections: &[String],
        tag_norms: &[String],
        limit: u32,
    ) -> Result<Vec<(String, f32)>> {
        let Some((scope_filter, scope_args)) = image_scope_filter(folders, collections, tag_norms)
        else {
            return Ok(Vec::new());
        };
        let sql = format!(
            "SELECT d.image_id, MAX(d.confidence) AS max_conf
             FROM detections d
             JOIN images i ON i.id = d.image_id
             JOIN image_files f ON f.image_id = i.id AND f.missing_at IS NULL
             WHERE d.class_name = ? AND ({scope_filter})
             GROUP BY d.image_id
             ORDER BY max_conf DESC, d.image_id ASC
             LIMIT ?"
        );
        let mut args = Vec::with_capacity(scope_args.len() + 2);
        args.push(Value::Text(class_name.to_string()));
        args.extend(scope_args);
        args.push(Value::Integer(limit as i64));

        let conn = self.read_connection();
        let mut stmt = conn.prepare(&sql)?;
        let rows = stmt.query_map(rusqlite::params_from_iter(args), |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, f32>(1)?))
        })?;
        rows.collect::<Result<Vec<_>>>()
    }

    pub fn count_by_class(&self, class_name: &str) -> Result<u32> {
        self.count_by_class_with_visibility(class_name, true)
    }

    pub fn count_by_class_with_visibility(
        &self,
        class_name: &str,
        include_rejected: bool,
    ) -> Result<u32> {
        let conn = self.conn.lock();
        let sql = format!(
            "SELECT COUNT(DISTINCT d.image_id) FROM detections d
             JOIN images i ON i.id = d.image_id
             JOIN image_files f ON f.image_id = i.id AND f.missing_at IS NULL
             LEFT JOIN selections s ON s.image_id = i.id AND s.project_id = '__global__'
             WHERE d.class_name = ?1 AND {}",
            RejectedVisibility::from_include_rejected(include_rejected).sql_predicate()
        );
        conn.query_row(&sql, params![class_name], |row| row.get::<_, u32>(0))
    }

    pub fn list_detected_classes(&self) -> Result<Vec<(String, u32)>> {
        self.list_detected_classes_with_visibility(true)
    }

    pub fn list_detected_classes_with_visibility(
        &self,
        include_rejected: bool,
    ) -> Result<Vec<(String, u32)>> {
        let conn = self.conn.lock();
        let sql = format!(
            "SELECT d.class_name, COUNT(DISTINCT d.image_id) AS image_count
             FROM detections d
             JOIN images i ON i.id = d.image_id
             JOIN image_files f ON f.image_id = d.image_id AND f.missing_at IS NULL
             LEFT JOIN selections s ON s.image_id = i.id AND s.project_id = '__global__'
             WHERE d.model_name GLOB 'yolo*' AND {}
             GROUP BY d.class_name
             ORDER BY image_count DESC, d.class_name ASC",
            RejectedVisibility::from_include_rejected(include_rejected).sql_predicate()
        );
        let mut stmt = conn.prepare(&sql)?;
        let rows = stmt.query_map([], |row| Ok((row.get(0)?, row.get(1)?)))?;
        rows.collect::<Result<Vec<_>>>()
    }

    pub fn list_images_by_class(
        &self,
        class_name: &str,
        limit: u32,
        offset: u32,
    ) -> Result<Vec<ImageWithFile>> {
        self.list_images_by_class_with_visibility(class_name, limit, offset, true)
    }

    pub fn list_images_by_class_with_visibility(
        &self,
        class_name: &str,
        limit: u32,
        offset: u32,
        include_rejected: bool,
    ) -> Result<Vec<ImageWithFile>> {
        let conn = self.conn.lock();
        let sql = format!(
            "SELECT i.id, i.sha256_hash, i.width, i.height, i.format, i.file_size,
                    i.created_at, i.imported_at, f.path,
                    s.star_rating, s.color_label, s.decision, i.source_label, i.ai_prompt,
                    i.raw_metadata, f.missing_at
             FROM detections d
             JOIN images i ON i.id = d.image_id
             JOIN image_files f ON f.image_id = i.id AND f.missing_at IS NULL
             LEFT JOIN selections s ON s.image_id = i.id AND s.project_id = '__global__'
             WHERE d.class_name = ?1 AND {}
             GROUP BY i.id
             ORDER BY MAX(d.confidence) DESC, i.imported_at DESC
             LIMIT ?2 OFFSET ?3",
            RejectedVisibility::from_include_rejected(include_rejected).sql_predicate()
        );
        let mut stmt = conn.prepare(&sql)?;
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

    pub fn detection_count(&self, model_name: &str) -> Result<u32> {
        let conn = self.conn.lock();
        conn.query_row(
            "SELECT COUNT(DISTINCT image_id) FROM detections WHERE model_name = ?1",
            params![model_name],
            |row| row.get::<_, u32>(0),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db_core::detection::Detection;

    fn insert_image(db: &Database, id: &str) {
        let conn = db.conn.lock();
        conn.execute(
            "INSERT INTO images
                (id, sha256_hash, width, height, format, file_size, created_at, imported_at)
             VALUES (?1, ?2, 100, 100, 'png', 100, '2026-01-01', '2026-01-01')",
            params![id, format!("hash-{id}")],
        )
        .unwrap();
    }

    fn insert_file(db: &Database, id: &str, image_id: &str, missing_at: Option<&str>) {
        let conn = db.conn.lock();
        conn.execute(
            "INSERT INTO image_files (id, image_id, path, last_seen_at, missing_at)
             VALUES (?1, ?2, ?3, '2026-01-01', ?4)",
            params![id, image_id, format!("/test/{id}.png"), missing_at],
        )
        .unwrap();
    }

    fn detection(class_name: &str, confidence: f32) -> Detection {
        Detection {
            class_name: class_name.to_string(),
            confidence,
            x: 0.0,
            y: 0.0,
            width: 1.0,
            height: 1.0,
        }
    }

    #[test]
    fn list_detected_classes_counts_distinct_live_images_and_sorts_stably() {
        let db = Database::open(std::path::Path::new(":memory:")).unwrap();

        insert_image(&db, "live-a");
        insert_file(&db, "file-a-1", "live-a", None);
        insert_file(&db, "file-a-2", "live-a", None);
        db.store_detections(
            "live-a",
            "yolo11m",
            &[
                detection("truck", 0.9),
                detection("truck", 0.8),
                detection("bus", 0.7),
            ],
        )
        .unwrap();
        db.store_detections("live-a", "nudenet", &[detection("EXPOSED_BREAST_F", 0.95)])
            .unwrap();

        insert_image(&db, "live-b");
        insert_file(&db, "file-b", "live-b", None);
        db.store_detections(
            "live-b",
            "yolo11m",
            &[detection("truck", 0.9), detection("airplane", 0.8)],
        )
        .unwrap();

        insert_image(&db, "live-c");
        insert_file(&db, "file-c", "live-c", None);
        db.store_detections("live-c", "yolo11m", &[detection("bus", 0.9)])
            .unwrap();

        insert_image(&db, "historical-yolo");
        insert_file(&db, "file-historical-yolo", "historical-yolo", None);
        db.store_detections("historical-yolo", "yolov8m", &[detection("dog", 0.9)])
            .unwrap();

        insert_image(&db, "missing");
        insert_file(&db, "file-missing", "missing", Some("2026-01-02"));
        db.store_detections("missing", "yolo11m", &[detection("zebra", 0.9)])
            .unwrap();

        insert_image(&db, "without-file");
        db.store_detections("without-file", "yolo11m", &[detection("elephant", 0.9)])
            .unwrap();

        assert_eq!(
            db.list_detected_classes().unwrap(),
            vec![
                ("bus".to_string(), 2),
                ("truck".to_string(), 2),
                ("airplane".to_string(), 1),
                ("dog".to_string(), 1),
            ],
        );
    }
}
