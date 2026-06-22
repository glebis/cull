// Copyright (c) 2026-present Gleb Kalinin. Architecture and design by author.
// Implementation assisted by Claude (Anthropic). See AUTHORSHIP.md.

use crate::db_core::db::Database;
use crate::db_core::models::*;
use rusqlite::{params, OptionalExtension, Result};

impl Database {
    pub fn set_rating(&self, image_id: &str, rating: u8) -> Result<()> {
        let conn = self.conn.lock();
        conn.execute(
            "INSERT INTO selections (image_id, project_id, star_rating, decision)
             VALUES (?1, '__global__', ?2, 'undecided')
             ON CONFLICT(image_id, project_id)
             DO UPDATE SET star_rating = ?2, decision = COALESCE(decision, 'undecided')",
            params![image_id, rating],
        )?;
        Ok(())
    }

    pub fn set_decision(&self, image_id: &str, decision: &str) -> Result<()> {
        let conn = self.conn.lock();
        conn.execute(
            "INSERT INTO selections (image_id, project_id, decision)
             VALUES (?1, '__global__', ?2)
             ON CONFLICT(image_id, project_id)
             DO UPDATE SET decision = ?2",
            params![image_id, decision],
        )?;
        Ok(())
    }

    pub fn get_selection_for_image(&self, image_id: &str) -> Result<Option<Selection>> {
        let conn = self.conn.lock();
        let mut stmt = conn.prepare(
            "SELECT image_id, project_id, star_rating, color_label, decision
             FROM selections WHERE image_id = ?1 AND project_id = '__global__'",
        )?;
        stmt.query_row(params![image_id], |row| {
            Ok(Selection {
                image_id: row.get(0)?,
                project_id: row.get(1)?,
                star_rating: row.get(2)?,
                color_label: row.get(3)?,
                decision: row.get(4).unwrap_or_else(|_| "undecided".to_string()),
            })
        })
        .optional()
    }
}
