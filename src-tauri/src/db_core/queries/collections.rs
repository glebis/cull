// Copyright (c) 2026-present Gleb Kalinin. Architecture and design by author.
// Implementation assisted by Claude (Anthropic). See AUTHORSHIP.md.

use crate::db_core::db::{row_u64, Database};
use crate::db_core::models::*;
use crate::db_core::visibility::RejectedVisibility;
use rusqlite::{params, Result};

impl Database {
    pub fn create_collection(&self, name: &str) -> Result<String> {
        let id = uuid::Uuid::new_v4().to_string();
        let now = chrono::Utc::now().to_rfc3339();
        let conn = self.conn.lock();
        conn.execute(
            "INSERT INTO projects (id, name, description, created_at) VALUES (?1, ?2, NULL, ?3)",
            params![id, name, now],
        )?;
        Ok(id)
    }

    pub fn create_collection_with_images(&self, name: &str, image_ids: &[&str]) -> Result<String> {
        let id = uuid::Uuid::new_v4().to_string();
        let now = chrono::Utc::now().to_rfc3339();
        let mut conn = self.conn.lock();
        let tx = conn.transaction()?;
        tx.execute(
            "INSERT INTO projects (id, name, description, created_at) VALUES (?1, ?2, NULL, ?3)",
            params![id, name, now],
        )?;
        for (position, image_id) in image_ids.iter().enumerate() {
            let inserted = tx.execute(
                "INSERT INTO collection_items (collection_id, image_id, position)
                 SELECT ?1, id, ?3 FROM images WHERE id = ?2",
                params![id, image_id, position as i64],
            )?;
            if inserted != 1 {
                return Err(rusqlite::Error::QueryReturnedNoRows);
            }
        }
        tx.commit()?;
        Ok(id)
    }

    pub fn list_collections(&self) -> Result<Vec<(String, String, u32)>> {
        self.list_collections_with_visibility(true)
    }

    pub fn list_collections_with_visibility(
        &self,
        include_rejected: bool,
    ) -> Result<Vec<(String, String, u32)>> {
        let conn = self.read_connection();
        let sql = format!(
            "SELECT p.id, p.name, COUNT(DISTINCT f.image_id) as cnt
             FROM projects p
             LEFT JOIN collection_items ci ON ci.collection_id = p.id
             LEFT JOIN images i ON i.id = ci.image_id
             LEFT JOIN selections s ON s.image_id = i.id AND s.project_id = '__global__'
             LEFT JOIN image_files f ON f.image_id = i.id AND f.missing_at IS NULL AND {}
             WHERE (p.collection_type IS NULL OR p.collection_type = 'manual')
             GROUP BY p.id
             ORDER BY p.created_at DESC",
            RejectedVisibility::from_include_rejected(include_rejected).sql_predicate()
        );
        let mut stmt = conn.prepare(&sql)?;
        let rows = stmt.query_map([], |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)))?;
        rows.collect::<Result<Vec<_>>>()
    }

    pub fn rename_collection(&self, collection_id: &str, name: &str) -> Result<()> {
        let conn = self.conn.lock();
        conn.execute(
            "UPDATE projects
             SET name = ?2
             WHERE id = ?1 AND (collection_type IS NULL OR collection_type = 'manual')",
            params![collection_id, name],
        )?;
        Ok(())
    }

    pub fn add_to_collection(&self, collection_id: &str, image_ids: &[&str]) -> Result<()> {
        let conn = self.conn.lock();
        let max_pos: i64 = conn.query_row(
            "SELECT COALESCE(MAX(position), -1) FROM collection_items WHERE collection_id = ?1",
            params![collection_id],
            |row| row.get(0),
        )?;
        for (i, id) in image_ids.iter().enumerate() {
            conn.execute(
                "INSERT OR IGNORE INTO collection_items (collection_id, image_id, position) VALUES (?1, ?2, ?3)",
                params![collection_id, id, max_pos + 1 + i as i64],
            )?;
        }
        Ok(())
    }

    pub fn remove_from_collection(&self, collection_id: &str, image_id: &str) -> Result<()> {
        let conn = self.conn.lock();
        conn.execute(
            "DELETE FROM collection_items WHERE collection_id = ?1 AND image_id = ?2",
            params![collection_id, image_id],
        )?;
        Ok(())
    }

    pub fn image_collection_ids(&self, image_id: &str) -> Result<Vec<String>> {
        let conn = self.read_connection();
        let mut stmt =
            conn.prepare("SELECT collection_id FROM collection_items WHERE image_id = ?1")?;
        let rows = stmt.query_map(params![image_id], |row| row.get::<_, String>(0))?;
        rows.collect::<Result<Vec<_>>>()
    }

    pub fn list_collection_images(&self, collection_id: &str) -> Result<Vec<ImageWithFile>> {
        self.list_collection_images_with_visibility(collection_id, true)
    }

    pub fn list_collection_images_with_visibility(
        &self,
        collection_id: &str,
        include_rejected: bool,
    ) -> Result<Vec<ImageWithFile>> {
        let conn = self.read_connection();
        let sql = format!(
            "SELECT i.id, i.sha256_hash, i.width, i.height, i.format, i.file_size,
                    i.created_at, i.imported_at, f.path,
                    s.star_rating, s.color_label, s.decision, i.source_label, i.ai_prompt,
                    i.raw_metadata, f.missing_at
             FROM collection_items ci
             JOIN images i ON i.id = ci.image_id
             JOIN image_files f ON f.image_id = i.id AND f.missing_at IS NULL
             LEFT JOIN selections s ON s.image_id = i.id AND s.project_id = '__global__'
             WHERE ci.collection_id = ?1 AND {}
             GROUP BY i.id
             ORDER BY ci.position ASC",
            RejectedVisibility::from_include_rejected(include_rejected).sql_predicate()
        );
        let mut stmt = conn.prepare(&sql)?;
        let rows = stmt.query_map(params![collection_id], |row| {
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

    pub fn list_collection_images_page(
        &self,
        collection_id: &str,
        limit: u32,
        offset: u32,
    ) -> Result<Vec<ImageWithFile>> {
        self.list_collection_images_page_with_visibility(collection_id, limit, offset, true)
    }

    pub fn list_collection_images_page_with_visibility(
        &self,
        collection_id: &str,
        limit: u32,
        offset: u32,
        include_rejected: bool,
    ) -> Result<Vec<ImageWithFile>> {
        let conn = self.read_connection();
        let sql = format!(
            "SELECT i.id, i.sha256_hash, i.width, i.height, i.format, i.file_size,
                    i.created_at, i.imported_at, f.path,
                    s.star_rating, s.color_label, s.decision, i.source_label, i.ai_prompt,
                    i.raw_metadata, f.missing_at
             FROM collection_items ci
             JOIN images i ON i.id = ci.image_id
             JOIN image_files f ON f.image_id = i.id AND f.missing_at IS NULL
             LEFT JOIN selections s ON s.image_id = i.id AND s.project_id = '__global__'
             WHERE ci.collection_id = ?1 AND {}
             GROUP BY i.id
             ORDER BY ci.position ASC
             LIMIT ?2 OFFSET ?3",
            RejectedVisibility::from_include_rejected(include_rejected).sql_predicate()
        );
        let mut stmt = conn.prepare(&sql)?;
        let rows = stmt.query_map(params![collection_id, limit, offset], |row| {
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

    pub fn delete_collection(&self, collection_id: &str) -> Result<()> {
        let conn = self.conn.lock();
        conn.execute(
            "DELETE FROM collection_items WHERE collection_id = ?1",
            params![collection_id],
        )?;
        conn.execute("DELETE FROM projects WHERE id = ?1", params![collection_id])?;
        Ok(())
    }

    pub fn set_collection_settings_json(
        &self,
        collection_id: &str,
        settings_json: &str,
    ) -> Result<()> {
        let conn = self.conn.lock();
        conn.execute(
            "UPDATE projects SET settings_json = ?2 WHERE id = ?1",
            params![collection_id, settings_json],
        )?;
        Ok(())
    }

    pub fn get_collection_settings_json(&self, collection_id: &str) -> Result<Option<String>> {
        let conn = self.read_connection();
        let mut stmt = conn.prepare("SELECT settings_json FROM projects WHERE id = ?1")?;
        let mut rows = stmt.query_map(params![collection_id], |row| row.get(0))?;
        match rows.next() {
            Some(Ok(value)) => Ok(value),
            Some(Err(err)) => Err(err),
            None => Ok(None),
        }
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
            "INSERT INTO images (id, sha256_hash, width, height, format, file_size, created_at, imported_at, ai_prompt)
             VALUES (?1, ?2, 100, 100, 'png', 1000, '2026-01-01', '2026-01-01', NULL)",
            params![id, format!("hash_{id}")],
        )
        .unwrap();
    }

    #[test]
    fn create_collection_with_images_rolls_back_everything_when_membership_fails() {
        let db = open_test_db();
        insert_test_image(&db, "valid-image");

        let result =
            db.create_collection_with_images("Atomic selection", &["valid-image", "missing-image"]);
        assert!(result.is_err());

        let conn = db.conn.lock();
        let project_count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM projects WHERE name = 'Atomic selection'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        let item_count: i64 = conn
            .query_row("SELECT COUNT(*) FROM collection_items", [], |row| {
                row.get(0)
            })
            .unwrap();
        assert_eq!(project_count, 0);
        assert_eq!(item_count, 0);
    }

    #[test]
    fn create_collection_with_images_commits_name_membership_and_order_together() {
        let db = open_test_db();
        insert_test_image(&db, "first-image");
        insert_test_image(&db, "second-image");

        let collection_id = db
            .create_collection_with_images("Map selection", &["second-image", "first-image"])
            .unwrap();

        let conn = db.conn.lock();
        let name: String = conn
            .query_row(
                "SELECT name FROM projects WHERE id = ?1",
                params![collection_id],
                |row| row.get(0),
            )
            .unwrap();
        let mut stmt = conn
            .prepare(
                "SELECT image_id FROM collection_items WHERE collection_id = ?1 ORDER BY position",
            )
            .unwrap();
        let image_ids = stmt
            .query_map(params![collection_id], |row| row.get::<_, String>(0))
            .unwrap()
            .collect::<Result<Vec<_>>>()
            .unwrap();
        assert_eq!(name, "Map selection");
        assert_eq!(image_ids, vec!["second-image", "first-image"]);
    }
}
