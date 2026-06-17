// Copyright (c) 2026-present Gleb Kalinin. Architecture and design by author.
// Implementation assisted by Claude (Anthropic). See AUTHORSHIP.md.

use crate::db_core::db::Database;
use rusqlite::{params, Result};
use serde::Serialize;

#[derive(Debug, Clone, Serialize)]
pub struct AuditLogEntry {
    pub id: String,
    pub timestamp: String,
    pub provider: String,
    pub endpoint: String,
    pub data_type: String,
    pub data_size_bytes: Option<i64>,
    pub prompt_preview: Option<String>,
    pub image_dimensions: Option<String>,
    pub model: Option<String>,
    pub response_status: Option<i32>,
    pub jurisdiction: String,
}

#[expect(
    clippy::too_many_arguments,
    reason = "audit rows intentionally keep provider, endpoint, cost, and jurisdiction explicit at call sites"
)]
pub fn log_api_call(
    db: &Database,
    provider: &str,
    endpoint: &str,
    data_type: &str,
    data_size_bytes: i64,
    prompt_preview: Option<&str>,
    image_dimensions: Option<&str>,
    model: Option<&str>,
    response_status: i32,
    jurisdiction: &str,
) -> Result<()> {
    let conn = db.conn.lock();
    let id = uuid::Uuid::new_v4().to_string();
    let timestamp = chrono::Utc::now().to_rfc3339();
    let preview = prompt_preview.map(|p| if p.len() > 200 { &p[..200] } else { p });
    conn.execute(
        "INSERT INTO api_audit_log (id, timestamp, provider, endpoint, data_type, data_size_bytes, prompt_preview, image_dimensions, model, response_status, jurisdiction)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)",
        params![id, timestamp, provider, endpoint, data_type, data_size_bytes, preview, image_dimensions, model, response_status, jurisdiction],
    )?;
    Ok(())
}

pub fn get_audit_log(db: &Database, limit: u32) -> Result<Vec<AuditLogEntry>> {
    let conn = db.conn.lock();
    let mut stmt = conn.prepare(
        "SELECT id, timestamp, provider, endpoint, data_type, data_size_bytes, prompt_preview, image_dimensions, model, response_status, jurisdiction
         FROM api_audit_log ORDER BY timestamp DESC LIMIT ?1"
    )?;
    let rows = stmt.query_map(params![limit], |row| {
        Ok(AuditLogEntry {
            id: row.get(0)?,
            timestamp: row.get(1)?,
            provider: row.get(2)?,
            endpoint: row.get(3)?,
            data_type: row.get(4)?,
            data_size_bytes: row.get(5)?,
            prompt_preview: row.get(6)?,
            image_dimensions: row.get(7)?,
            model: row.get(8)?,
            response_status: row.get(9)?,
            jurisdiction: row.get(10)?,
        })
    })?;
    rows.collect()
}

pub fn export_audit_log_json(db: &Database) -> Result<String> {
    let entries = get_audit_log(db, 10000)?;
    Ok(serde_json::to_string_pretty(&entries).unwrap_or_else(|_| "[]".to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db_core::db::Database;

    #[test]
    fn test_log_and_retrieve_api_call() {
        let dir = tempfile::tempdir().unwrap();
        let db = Database::open(&dir.path().join("test.db")).unwrap();

        log_api_call(&db, "gemini", "https://generativelanguage.googleapis.com/v1beta/models/gemini-embedding-exp-03-07:embedContent",
            "image", 1200000, None, Some("2048x1536"), Some("gemini-embedding-exp-03-07"), 200, "US - Google LLC").unwrap();

        log_api_call(
            &db,
            "openai",
            "https://api.openai.com/v1/images/generations",
            "prompt",
            450,
            Some("a cat in watercolor style"),
            None,
            Some("gpt-image-2"),
            200,
            "US - OpenAI Inc",
        )
        .unwrap();

        let entries = get_audit_log(&db, 10).unwrap();
        assert_eq!(entries.len(), 2);
        assert_eq!(entries[0].provider, "openai");
        assert_eq!(entries[1].provider, "gemini");
        assert_eq!(
            entries[0].prompt_preview.as_deref(),
            Some("a cat in watercolor style")
        );
        assert_eq!(entries[1].image_dimensions.as_deref(), Some("2048x1536"));
    }

    #[test]
    fn test_audit_log_limit() {
        let dir = tempfile::tempdir().unwrap();
        let db = Database::open(&dir.path().join("test.db")).unwrap();

        for i in 0..5 {
            log_api_call(
                &db,
                "gemini",
                "https://example.com",
                "image",
                i * 100,
                None,
                None,
                None,
                200,
                "US",
            )
            .unwrap();
        }

        let entries = get_audit_log(&db, 3).unwrap();
        assert_eq!(entries.len(), 3);
    }

    #[test]
    fn test_export_audit_log_json() {
        let dir = tempfile::tempdir().unwrap();
        let db = Database::open(&dir.path().join("test.db")).unwrap();

        log_api_call(
            &db,
            "openai",
            "https://api.openai.com/v1/images/generations",
            "prompt",
            100,
            Some("test prompt"),
            None,
            Some("gpt-image-2"),
            200,
            "US - OpenAI Inc",
        )
        .unwrap();

        let json = export_audit_log_json(&db).unwrap();
        let parsed: Vec<serde_json::Value> = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.len(), 1);
        assert_eq!(parsed[0]["provider"], "openai");
    }
}
