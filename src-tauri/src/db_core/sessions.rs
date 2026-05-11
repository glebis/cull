use rusqlite::{params, Result};
use super::db::Database;
use super::models::*;

impl Database {
    pub fn create_session(&self, name: &str, folder_path: &str) -> Result<String> {
        let conn = self.conn.lock().unwrap();
        let id = uuid::Uuid::new_v4().to_string();
        conn.execute(
            "INSERT INTO projects (id, name, collection_type, folder_path, created_at)
             VALUES (?1, ?2, 'session', ?3, datetime('now'))",
            params![id, name, folder_path],
        )?;
        Ok(id)
    }

    pub fn list_sessions(&self) -> Result<Vec<Session>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT p.id, p.name, p.description, p.folder_path, p.settings_json, p.created_at,
                    (SELECT COUNT(*) FROM collection_items ci WHERE ci.collection_id = p.id) as image_count
             FROM projects p
             WHERE p.collection_type = 'session'
             ORDER BY p.created_at DESC"
        )?;
        let rows = stmt.query_map([], |row| {
            Ok(Session {
                id: row.get(0)?,
                name: row.get(1)?,
                description: row.get(2)?,
                folder_path: row.get::<_, Option<String>>(3)?.unwrap_or_default(),
                settings_json: row.get(4)?,
                created_at: row.get(5)?,
                image_count: row.get::<_, i64>(6)? as u32,
            })
        })?;
        rows.collect()
    }

    pub fn get_session(&self, id: &str) -> Result<Session> {
        let conn = self.conn.lock().unwrap();
        conn.query_row(
            "SELECT p.id, p.name, p.description, p.folder_path, p.settings_json, p.created_at,
                    (SELECT COUNT(*) FROM collection_items ci WHERE ci.collection_id = p.id) as image_count
             FROM projects p
             WHERE p.id = ?1 AND p.collection_type = 'session'",
            params![id],
            |row| {
                Ok(Session {
                    id: row.get(0)?,
                    name: row.get(1)?,
                    description: row.get(2)?,
                    folder_path: row.get::<_, Option<String>>(3)?.unwrap_or_default(),
                    settings_json: row.get(4)?,
                    created_at: row.get(5)?,
                    image_count: row.get::<_, i64>(6)? as u32,
                })
            }
        )
    }

    pub fn delete_session(&self, id: &str) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute("DELETE FROM canvases WHERE session_id = ?1", params![id])?;
        conn.execute("DELETE FROM collection_items WHERE collection_id = ?1", params![id])?;
        conn.execute("DELETE FROM projects WHERE id = ?1 AND collection_type = 'session'", params![id])?;
        Ok(())
    }

    pub fn convert_session_to_collection(&self, id: &str) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute("DELETE FROM canvases WHERE session_id = ?1", params![id])?;
        conn.execute(
            "UPDATE projects SET collection_type = 'manual', folder_path = NULL, settings_json = NULL
             WHERE id = ?1 AND collection_type = 'session'",
            params![id],
        )?;
        Ok(())
    }

    pub fn create_canvas(&self, session_id: &str, name: &str, canvas_type: &str) -> Result<String> {
        let conn = self.conn.lock().unwrap();
        let id = uuid::Uuid::new_v4().to_string();
        let max_order: i32 = conn.query_row(
            "SELECT COALESCE(MAX(sort_order), -1) FROM canvases WHERE session_id = ?1",
            params![session_id],
            |row| row.get(0),
        )?;
        conn.execute(
            "INSERT INTO canvases (id, session_id, name, canvas_type, layout_json, sort_order, created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, '{}', ?5, datetime('now'), datetime('now'))",
            params![id, session_id, name, canvas_type, max_order + 1],
        )?;
        Ok(id)
    }

    pub fn list_canvases(&self, session_id: &str) -> Result<Vec<Canvas>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT id, session_id, name, canvas_type, layout_json, filter_json, grid_config_json, sort_order, created_at, updated_at
             FROM canvases WHERE session_id = ?1 ORDER BY sort_order"
        )?;
        let rows = stmt.query_map(params![session_id], |row| {
            Ok(Canvas {
                id: row.get(0)?,
                session_id: row.get(1)?,
                name: row.get(2)?,
                canvas_type: row.get(3)?,
                layout_json: row.get(4)?,
                filter_json: row.get(5)?,
                grid_config_json: row.get(6)?,
                sort_order: row.get(7)?,
                created_at: row.get(8)?,
                updated_at: row.get(9)?,
            })
        })?;
        rows.collect()
    }

    pub fn update_canvas_layout(&self, canvas_id: &str, layout_json: &str) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "UPDATE canvases SET layout_json = ?1, updated_at = datetime('now') WHERE id = ?2",
            params![layout_json, canvas_id],
        )?;
        Ok(())
    }

    pub fn delete_canvas(&self, canvas_id: &str) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute("DELETE FROM canvases WHERE id = ?1", params![canvas_id])?;
        Ok(())
    }

    pub fn cleanup_old_batches(&self, max_age_days: u32) -> Result<u32> {
        let conn = self.conn.lock().unwrap();
        let cutoff = format!("-{} days", max_age_days);
        conn.execute(
            "UPDATE images SET import_batch_id = NULL WHERE import_batch_id IN
             (SELECT id FROM import_batches WHERE created_at < datetime('now', ?1))",
            params![cutoff],
        )?;
        let deleted = conn.execute(
            "DELETE FROM import_batches WHERE created_at < datetime('now', ?1)",
            params![cutoff],
        )?;
        Ok(deleted as u32)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_db() -> Database {
        Database::open(std::path::Path::new(":memory:")).unwrap()
    }

    #[test]
    fn test_create_session() {
        let db = test_db();
        let id = db.create_session("Portrait Shoot", "/tmp/sessions/portrait").unwrap();
        assert!(!id.is_empty());
    }

    #[test]
    fn test_list_sessions() {
        let db = test_db();
        db.create_session("Session A", "/tmp/sessions/a").unwrap();
        db.create_session("Session B", "/tmp/sessions/b").unwrap();
        let sessions = db.list_sessions().unwrap();
        assert_eq!(sessions.len(), 2);
        let names: Vec<&str> = sessions.iter().map(|s| s.name.as_str()).collect();
        assert!(names.contains(&"Session A"));
        assert!(names.contains(&"Session B"));
    }

    #[test]
    fn test_get_session() {
        let db = test_db();
        let id = db.create_session("My Session", "/tmp/sessions/my").unwrap();
        let session = db.get_session(&id).unwrap();
        assert_eq!(session.name, "My Session");
        assert_eq!(session.folder_path, "/tmp/sessions/my");
    }

    #[test]
    fn test_delete_session() {
        let db = test_db();
        let id = db.create_session("To Delete", "/tmp/sessions/del").unwrap();
        db.delete_session(&id).unwrap();
        let sessions = db.list_sessions().unwrap();
        assert!(sessions.is_empty());
    }

    #[test]
    fn test_convert_session_to_collection() {
        let db = test_db();
        let id = db.create_session("Convert Me", "/tmp/sessions/conv").unwrap();
        db.create_canvas(&id, "Test Canvas", "manual").unwrap();
        db.convert_session_to_collection(&id).unwrap();
        let cols = db.list_collections().unwrap();
        assert!(cols.iter().any(|(cid, name, _)| cid == &id && name == "Convert Me"));
        let canvases = db.list_canvases(&id).unwrap();
        assert!(canvases.is_empty());
    }

    #[test]
    fn test_create_and_list_canvases() {
        let db = test_db();
        let sid = db.create_session("Canvas Test", "/tmp/sessions/canvas").unwrap();
        db.create_canvas(&sid, "Layout A", "manual").unwrap();
        db.create_canvas(&sid, "Query View", "query").unwrap();
        let canvases = db.list_canvases(&sid).unwrap();
        assert_eq!(canvases.len(), 2);
        assert_eq!(canvases[0].name, "Layout A");
        assert_eq!(canvases[1].name, "Query View");
    }

    #[test]
    fn test_update_canvas_layout() {
        let db = test_db();
        let sid = db.create_session("Layout Test", "/tmp/sessions/layout").unwrap();
        let cid = db.create_canvas(&sid, "My Canvas", "manual").unwrap();
        let layout = r#"{"images":[{"id":"img1","x":10,"y":20}]}"#;
        db.update_canvas_layout(&cid, layout).unwrap();
        let canvases = db.list_canvases(&sid).unwrap();
        assert_eq!(canvases[0].layout_json, layout);
    }

    #[test]
    fn test_delete_canvas() {
        let db = test_db();
        let sid = db.create_session("Del Canvas", "/tmp/sessions/delc").unwrap();
        db.create_canvas(&sid, "To Remove", "manual").unwrap();
        db.delete_canvas(&db.list_canvases(&sid).unwrap()[0].id.clone()).unwrap();
        let canvases = db.list_canvases(&sid).unwrap();
        assert!(canvases.is_empty());
    }

    #[test]
    fn test_cleanup_old_batches() {
        let db = test_db();
        let conn = db.conn.lock().unwrap();
        conn.execute(
            "INSERT INTO import_batches (id, created_at, source, image_count) VALUES ('old1', datetime('now', '-30 days'), 'test', 5)",
            [],
        ).unwrap();
        conn.execute(
            "INSERT INTO import_batches (id, created_at, source, image_count) VALUES ('new1', datetime('now', '-1 day'), 'test', 3)",
            [],
        ).unwrap();
        drop(conn);
        let cleaned = db.cleanup_old_batches(7).unwrap();
        assert_eq!(cleaned, 1);
    }
}
