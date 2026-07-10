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
             WHERE NOT EXISTS (
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
