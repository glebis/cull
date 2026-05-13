// Copyright (c) 2025-present Gleb Kalinin. Architecture and design by author.
// Implementation assisted by Claude (Anthropic). See AUTHORSHIP.md.

use crate::db_core::db::Database;
use rusqlite::{params, Result};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NewAssetLoadEvent {
    pub view: String,
    pub image_id: Option<String>,
    pub asset_kind: String,
    pub image_format: Option<String>,
    pub fallback_used: bool,
    pub fallback_succeeded: Option<bool>,
    pub path_basename: Option<String>,
    pub path_hash: Option<String>,
    pub error_kind: String,
    pub details_json: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AssetLoadEvent {
    pub id: String,
    pub created_at: String,
    pub view: String,
    pub image_id: Option<String>,
    pub asset_kind: String,
    pub image_format: Option<String>,
    pub fallback_used: bool,
    pub fallback_succeeded: Option<bool>,
    pub path_basename: Option<String>,
    pub path_hash: Option<String>,
    pub error_kind: String,
    pub details_json: Option<String>,
}

fn required_value(value: String, max_len: usize, fallback: &str) -> String {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return fallback.to_string();
    }
    trimmed.chars().take(max_len).collect()
}

fn optional_value(value: Option<String>, max_len: usize) -> Option<String> {
    value.and_then(|v| {
        let trimmed = v.trim();
        if trimmed.is_empty() {
            None
        } else {
            Some(trimmed.chars().take(max_len).collect())
        }
    })
}

fn bool_to_int(value: bool) -> i64 {
    if value {
        1
    } else {
        0
    }
}

fn int_to_bool(value: i64) -> bool {
    value != 0
}

pub fn log_asset_load_event(db: &Database, event: NewAssetLoadEvent) -> Result<AssetLoadEvent> {
    let entry = AssetLoadEvent {
        id: uuid::Uuid::new_v4().to_string(),
        created_at: chrono::Utc::now().to_rfc3339(),
        view: required_value(event.view, 32, "unknown"),
        image_id: optional_value(event.image_id, 128),
        asset_kind: required_value(event.asset_kind, 32, "unknown"),
        image_format: optional_value(event.image_format, 32),
        fallback_used: event.fallback_used,
        fallback_succeeded: event.fallback_succeeded,
        path_basename: optional_value(event.path_basename, 255),
        path_hash: optional_value(event.path_hash, 128),
        error_kind: required_value(event.error_kind, 64, "unknown"),
        details_json: optional_value(event.details_json, 2048),
    };

    let conn = db.conn.lock();
    conn.execute(
        "INSERT INTO asset_load_events (
            id, created_at, view, image_id, asset_kind, image_format,
            fallback_used, fallback_succeeded, path_basename, path_hash,
            error_kind, details_json
        ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)",
        params![
            entry.id,
            entry.created_at,
            entry.view,
            entry.image_id,
            entry.asset_kind,
            entry.image_format,
            bool_to_int(entry.fallback_used),
            entry.fallback_succeeded.map(bool_to_int),
            entry.path_basename,
            entry.path_hash,
            entry.error_kind,
            entry.details_json,
        ],
    )?;

    Ok(entry)
}

pub fn get_asset_load_events(db: &Database, limit: u32) -> Result<Vec<AssetLoadEvent>> {
    let conn = db.conn.lock();
    let limit = limit.clamp(1, 500);
    let mut stmt = conn.prepare(
        "SELECT id, created_at, view, image_id, asset_kind, image_format,
            fallback_used, fallback_succeeded, path_basename, path_hash,
            error_kind, details_json
         FROM asset_load_events
         ORDER BY seq DESC
         LIMIT ?1",
    )?;
    let rows = stmt.query_map(params![limit], |row| {
        let fallback_used: i64 = row.get(6)?;
        let fallback_succeeded: Option<i64> = row.get(7)?;
        Ok(AssetLoadEvent {
            id: row.get(0)?,
            created_at: row.get(1)?,
            view: row.get(2)?,
            image_id: row.get(3)?,
            asset_kind: row.get(4)?,
            image_format: row.get(5)?,
            fallback_used: int_to_bool(fallback_used),
            fallback_succeeded: fallback_succeeded.map(int_to_bool),
            path_basename: row.get(8)?,
            path_hash: row.get(9)?,
            error_kind: row.get(10)?,
            details_json: row.get(11)?,
        })
    })?;
    rows.collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db_core::db::Database;
    use std::path::Path;

    fn test_db() -> Database {
        Database::open(Path::new(":memory:")).unwrap()
    }

    #[test]
    fn records_asset_load_event_without_full_path() {
        let db = test_db();

        let event = log_asset_load_event(
            &db,
            NewAssetLoadEvent {
                view: "loupe".to_string(),
                image_id: Some("img-1".to_string()),
                asset_kind: "source".to_string(),
                image_format: Some("png".to_string()),
                fallback_used: true,
                fallback_succeeded: None,
                path_basename: Some("ig_abc.png".to_string()),
                path_hash: Some("path-123".to_string()),
                error_kind: "img_onerror".to_string(),
                details_json: Some("{\"phase\":\"source\"}".to_string()),
            },
        )
        .unwrap();

        assert_eq!(event.view, "loupe");
        assert_eq!(event.image_id.as_deref(), Some("img-1"));
        assert_eq!(event.path_basename.as_deref(), Some("ig_abc.png"));
        assert_eq!(event.path_hash.as_deref(), Some("path-123"));
        assert_eq!(event.fallback_used, true);
        assert_eq!(event.fallback_succeeded, None);

        let serialized = serde_json::to_string(&event).unwrap();
        assert!(!serialized.contains("/Users/"));
        assert!(!serialized.contains("Application Support"));
    }

    #[test]
    fn lists_recent_asset_load_events_with_limit() {
        let db = test_db();

        for i in 0..5 {
            log_asset_load_event(
                &db,
                NewAssetLoadEvent {
                    view: "thumbnail".to_string(),
                    image_id: Some(format!("img-{i}")),
                    asset_kind: "thumbnail".to_string(),
                    image_format: Some("jpg".to_string()),
                    fallback_used: false,
                    fallback_succeeded: Some(false),
                    path_basename: Some(format!("img-{i}.jpg")),
                    path_hash: Some(format!("hash-{i}")),
                    error_kind: "img_onerror".to_string(),
                    details_json: None,
                },
            )
            .unwrap();
        }

        let events = get_asset_load_events(&db, 3).unwrap();
        assert_eq!(events.len(), 3);
        assert_eq!(events[0].image_id.as_deref(), Some("img-4"));
        assert_eq!(events[1].image_id.as_deref(), Some("img-3"));
        assert_eq!(events[2].image_id.as_deref(), Some("img-2"));
    }
}
