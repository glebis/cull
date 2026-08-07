// Copyright (c) 2026-present Gleb Kalinin. Architecture and design by author.
// Implementation assisted by OpenAI Codex. See AUTHORSHIP.md.

use crate::db_core::db::Database;
use rusqlite::{params, Result};

impl Database {
    pub fn mark_image_analysis_complete(
        &self,
        image_id: &str,
        analysis_kind: &str,
        model_name: &str,
    ) -> Result<()> {
        let conn = self.conn.lock();
        conn.execute(
            "INSERT INTO image_analysis_status (image_id, analysis_kind, model_name, completed_at)
             VALUES (?1, ?2, ?3, ?4)
             ON CONFLICT(image_id, analysis_kind, model_name) DO UPDATE SET
                completed_at = excluded.completed_at",
            params![
                image_id,
                analysis_kind,
                model_name,
                chrono::Utc::now().to_rfc3339()
            ],
        )?;
        Ok(())
    }

    pub fn list_image_ids_missing_detection(&self, model_name: &str) -> Result<Vec<String>> {
        self.list_image_ids_missing_analysis("detection", model_name)
    }

    pub fn list_image_ids_missing_vision(&self, model_name: &str) -> Result<Vec<String>> {
        self.list_image_ids_missing_analysis("vision", model_name)
    }

    fn list_image_ids_missing_analysis(
        &self,
        analysis_kind: &str,
        model_name: &str,
    ) -> Result<Vec<String>> {
        let conn = self.conn.lock();
        let mut stmt = conn.prepare(
            "SELECT i.id
             FROM images i
             WHERE EXISTS (
                SELECT 1
                FROM image_files f
                WHERE f.image_id = i.id
                  AND f.missing_at IS NULL
             )
             AND NOT EXISTS (
                SELECT 1
                FROM image_analysis_status s
                WHERE s.image_id = i.id
                  AND s.analysis_kind = ?1
                  AND s.model_name = ?2
             )
             ORDER BY i.imported_at, i.id",
        )?;
        let rows = stmt.query_map(params![analysis_kind, model_name], |row| row.get(0))?;
        rows.collect::<Result<Vec<_>>>()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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

    fn insert_image_file(db: &Database, image_id: &str, missing_at: Option<&str>) {
        let conn = db.conn.lock();
        conn.execute(
            "INSERT INTO image_files (id, image_id, path, last_seen_at, missing_at)
             VALUES (?1, ?2, ?3, '2026-01-01', ?4)",
            params![
                format!("file-{image_id}"),
                image_id,
                format!("/test/{image_id}.png"),
                missing_at,
            ],
        )
        .unwrap();
    }

    #[test]
    fn pending_analysis_excludes_images_without_a_live_file() {
        let db = Database::open(std::path::Path::new(":memory:")).unwrap();

        insert_image(&db, "live");
        insert_image_file(&db, "live", None);

        insert_image(&db, "missing");
        insert_image_file(&db, "missing", Some("2026-01-02"));

        insert_image(&db, "without-file");

        assert_eq!(
            db.list_image_ids_missing_detection("yolo11m").unwrap(),
            vec!["live".to_string()],
        );
        assert_eq!(
            db.list_image_ids_missing_vision("minicpm-v").unwrap(),
            vec!["live".to_string()],
        );
    }
}
