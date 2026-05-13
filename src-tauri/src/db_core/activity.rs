use rusqlite::{params, Result};

use super::db::Database;
use super::models::{ActivityContext, ActivityLibrarySummary, NewSessionEvent, SessionEvent};

fn row_to_session_event(row: &rusqlite::Row<'_>) -> Result<SessionEvent> {
    Ok(SessionEvent {
        id: row.get(0)?,
        session_id: row.get(1)?,
        event_type: row.get(2)?,
        actor_type: row.get(3)?,
        actor_id: row.get(4)?,
        subject_type: row.get(5)?,
        subject_id: row.get(6)?,
        payload_json: row.get(7)?,
        created_at: row.get(8)?,
    })
}

impl Database {
    pub fn log_session_event(&self, event: &NewSessionEvent) -> Result<String> {
        let id = uuid::Uuid::new_v4().to_string();
        let now = chrono::Utc::now().to_rfc3339();
        let payload_json = if event.payload_json.trim().is_empty() {
            "{}"
        } else {
            event.payload_json.as_str()
        };
        let conn = self.conn.lock();
        conn.execute(
            "INSERT INTO session_events (
                id, session_id, event_type, actor_type, actor_id,
                subject_type, subject_id, payload_json, created_at
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
            params![
                id,
                event.session_id.as_deref(),
                event.event_type.as_str(),
                event.actor_type.as_str(),
                event.actor_id.as_deref(),
                event.subject_type.as_deref(),
                event.subject_id.as_deref(),
                payload_json,
                now,
            ],
        )?;
        Ok(id)
    }

    pub fn list_session_events(
        &self,
        session_id: Option<&str>,
        limit: u32,
    ) -> Result<Vec<SessionEvent>> {
        let conn = self.conn.lock();
        let limit = limit.clamp(1, 200);
        let sql = if session_id.is_some() {
            "SELECT id, session_id, event_type, actor_type, actor_id, subject_type,
                    subject_id, payload_json, created_at
             FROM session_events
             WHERE session_id = ?1
             ORDER BY created_at DESC
             LIMIT ?2"
        } else {
            "SELECT id, session_id, event_type, actor_type, actor_id, subject_type,
                    subject_id, payload_json, created_at
             FROM session_events
             ORDER BY created_at DESC
             LIMIT ?1"
        };
        let mut stmt = conn.prepare(sql)?;

        let rows = match session_id {
            Some(id) => stmt.query_map(params![id, limit], row_to_session_event)?,
            None => stmt.query_map(params![limit], row_to_session_event)?,
        };
        rows.collect::<Result<Vec<_>>>()
    }

    pub fn get_activity_library_summary(
        &self,
        session_id: Option<&str>,
    ) -> Result<ActivityLibrarySummary> {
        let conn = self.conn.lock();
        let total_images: i64 =
            conn.query_row("SELECT COUNT(*) FROM images", [], |row| row.get(0))?;

        let scoped_images = match session_id {
            Some(id) => conn.query_row(
                "SELECT COUNT(DISTINCT image_id) FROM collection_items WHERE collection_id = ?1",
                params![id],
                |row| row.get::<_, i64>(0),
            )?,
            None => total_images,
        };

        let (rated_images, accepted_images, rejected_images) = match session_id {
            Some(id) => {
                let rated = conn.query_row(
                    "SELECT COUNT(DISTINCT s.image_id)
                     FROM selections s
                     JOIN collection_items ci ON ci.image_id = s.image_id AND ci.collection_id = ?1
                     WHERE s.project_id = '__global__' AND COALESCE(s.star_rating, 0) > 0",
                    params![id],
                    |row| row.get::<_, i64>(0),
                )?;
                let accepted = conn.query_row(
                    "SELECT COUNT(DISTINCT s.image_id)
                     FROM selections s
                     JOIN collection_items ci ON ci.image_id = s.image_id AND ci.collection_id = ?1
                     WHERE s.project_id = '__global__' AND s.decision = 'accept'",
                    params![id],
                    |row| row.get::<_, i64>(0),
                )?;
                let rejected = conn.query_row(
                    "SELECT COUNT(DISTINCT s.image_id)
                     FROM selections s
                     JOIN collection_items ci ON ci.image_id = s.image_id AND ci.collection_id = ?1
                     WHERE s.project_id = '__global__' AND s.decision = 'reject'",
                    params![id],
                    |row| row.get::<_, i64>(0),
                )?;
                (rated, accepted, rejected)
            }
            None => {
                let rated = conn.query_row(
                    "SELECT COUNT(DISTINCT image_id)
                     FROM selections
                     WHERE project_id = '__global__' AND COALESCE(star_rating, 0) > 0",
                    [],
                    |row| row.get::<_, i64>(0),
                )?;
                let accepted = conn.query_row(
                    "SELECT COUNT(DISTINCT image_id)
                     FROM selections
                     WHERE project_id = '__global__' AND decision = 'accept'",
                    [],
                    |row| row.get::<_, i64>(0),
                )?;
                let rejected = conn.query_row(
                    "SELECT COUNT(DISTINCT image_id)
                     FROM selections
                     WHERE project_id = '__global__' AND decision = 'reject'",
                    [],
                    |row| row.get::<_, i64>(0),
                )?;
                (rated, accepted, rejected)
            }
        };

        let import_batches = match session_id {
            Some(id) => conn.query_row(
                "SELECT COUNT(*) FROM import_batches WHERE collection_id = ?1",
                params![id],
                |row| row.get::<_, i64>(0),
            )?,
            None => conn.query_row("SELECT COUNT(*) FROM import_batches", [], |row| {
                row.get::<_, i64>(0)
            })?,
        };

        Ok(ActivityLibrarySummary {
            total_images: total_images as u32,
            scoped_images: scoped_images as u32,
            rated_images: rated_images as u32,
            accepted_images: accepted_images as u32,
            rejected_images: rejected_images as u32,
            import_batches: import_batches as u32,
        })
    }

    pub fn get_activity_context(
        &self,
        session_id: Option<&str>,
        limit: u32,
    ) -> Result<ActivityContext> {
        let session = match session_id {
            Some(id) => Some(self.get_session(id)?),
            None => None,
        };
        let library = self.get_activity_library_summary(session_id)?;
        let recent_events = self.list_session_events(session_id, limit)?;
        Ok(ActivityContext {
            session,
            library,
            recent_events,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_db() -> Database {
        Database::open(std::path::Path::new(":memory:")).unwrap()
    }

    #[test]
    fn test_log_and_list_session_events() {
        let db = test_db();
        let session_id = db.create_session("Activity Test", "/tmp/activity").unwrap();
        db.log_session_event(&NewSessionEvent {
            session_id: Some(session_id.clone()),
            event_type: "note_added".to_string(),
            actor_type: "user".to_string(),
            actor_id: None,
            subject_type: Some("session".to_string()),
            subject_id: Some(session_id.clone()),
            payload_json: serde_json::json!({"body": "test"}).to_string(),
        })
        .unwrap();

        let events = db.list_session_events(Some(&session_id), 10).unwrap();
        assert!(events.iter().any(|event| event.event_type == "note_added"));
        assert!(events
            .iter()
            .all(|event| event.session_id.as_deref() == Some(&session_id)));
    }

    #[test]
    fn test_activity_summary_scopes_to_session_collection_items() {
        let db = test_db();
        let session_id = db.create_session("Scoped", "/tmp/scoped").unwrap();
        let conn = db.conn.lock();
        conn.execute(
            "INSERT INTO images (id, sha256_hash, width, height, format, file_size, created_at, imported_at)
             VALUES ('img1', 'h1', 100, 100, 'png', 1000, '2026-01-01', '2026-01-01')",
            [],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO images (id, sha256_hash, width, height, format, file_size, created_at, imported_at)
             VALUES ('img2', 'h2', 100, 100, 'png', 1000, '2026-01-01', '2026-01-01')",
            [],
        )
        .unwrap();
        drop(conn);
        db.add_to_collection(&session_id, &["img1"]).unwrap();
        db.set_rating("img1", 4).unwrap();
        db.set_decision("img1", "accept").unwrap();

        let summary = db.get_activity_library_summary(Some(&session_id)).unwrap();
        assert_eq!(summary.total_images, 2);
        assert_eq!(summary.scoped_images, 1);
        assert_eq!(summary.rated_images, 1);
        assert_eq!(summary.accepted_images, 1);
        assert_eq!(summary.rejected_images, 0);
    }
}
