use std::sync::Mutex;
use chrono::Utc;
use serde::{Deserialize, Serialize};
use uuid::Uuid;
use crate::db_core::db::Database;
use crate::db_core::models::UndoRecord;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UndoStatus {
    pub can_undo: bool,
    pub can_redo: bool,
    pub undo_label: Option<String>,
    pub redo_label: Option<String>,
    pub stack_depth: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ActionResult {
    pub undo_record_id: String,
    pub label: String,
    pub can_undo: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Action {
    SetRating { image_id: String, rating: u8 },
    SetDecision { image_id: String, decision: String },
}

pub struct ActionManager {
    // cursor_seq tracks where we are in the stack.
    // None means we're at the top of the stack (no undone actions).
    // Some(seq) means the record at seq was the last undone action.
    cursor_seq: Mutex<Option<i64>>,
    max_depth: usize,
}

impl ActionManager {
    pub fn new() -> Self {
        Self {
            cursor_seq: Mutex::new(None),
            max_depth: 200,
        }
    }

    pub fn execute(&self, db: &Database, action: Action) -> Result<ActionResult, String> {
        // 1. Read before state — must happen BEFORE locking conn for the transaction
        let (action_type, label, before_json, after_json, affected_ids) = match &action {
            Action::SetRating { image_id, rating } => {
                let sel = db.get_selection_for_image(image_id).map_err(|e| e.to_string())?;
                let before_rating = sel.as_ref().and_then(|s| s.star_rating).unwrap_or(0);
                (
                    "set_rating",
                    format!("Set rating to {}", rating),
                    serde_json::json!({"image_id": image_id, "rating": before_rating}).to_string(),
                    serde_json::json!({"image_id": image_id, "rating": rating}).to_string(),
                    image_id.clone(),
                )
            }
            Action::SetDecision { image_id, decision } => {
                let sel = db.get_selection_for_image(image_id).map_err(|e| e.to_string())?;
                let before_decision = sel
                    .map(|s| s.decision)
                    .unwrap_or_else(|| "undecided".to_string());
                (
                    "set_decision",
                    format!("Set decision to {}", decision),
                    serde_json::json!({"image_id": image_id, "decision": before_decision}).to_string(),
                    serde_json::json!({"image_id": image_id, "decision": decision}).to_string(),
                    image_id.clone(),
                )
            }
        };

        // 2. Lock cursor position, then perform mutation + undo record insert in one transaction
        let mut cursor = self.cursor_seq.lock().unwrap();

        let mut conn = db.conn.lock().unwrap();
        let tx = conn.savepoint().map_err(|e| e.to_string())?;

        // Clear redo branch if cursor is pointing to an undone record
        if let Some(cur_seq) = *cursor {
            tx.execute(
                "DELETE FROM undo_records WHERE seq >= ?1",
                rusqlite::params![cur_seq],
            )
            .map_err(|e| e.to_string())?;
        }

        // Perform the actual mutation
        match &action {
            Action::SetRating { image_id, rating } => {
                tx.execute(
                    "INSERT INTO selections (image_id, project_id, star_rating, decision)
                     VALUES (?1, '__global__', ?2, 'undecided')
                     ON CONFLICT(image_id, project_id)
                     DO UPDATE SET star_rating = ?2",
                    rusqlite::params![image_id, rating],
                )
                .map_err(|e| e.to_string())?;
            }
            Action::SetDecision { image_id, decision } => {
                tx.execute(
                    "INSERT INTO selections (image_id, project_id, decision)
                     VALUES (?1, '__global__', ?2)
                     ON CONFLICT(image_id, project_id)
                     DO UPDATE SET decision = ?2",
                    rusqlite::params![image_id, decision],
                )
                .map_err(|e| e.to_string())?;
            }
        }

        // Insert undo record
        let record_id = Uuid::new_v4().to_string();
        let now = Utc::now().to_rfc3339();
        tx.execute(
            "INSERT INTO undo_records (id, action_type, label, before_json, after_json, affected_image_ids, has_file_backup, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, 0, ?7)",
            rusqlite::params![record_id, action_type, label, before_json, after_json, affected_ids, now],
        )
        .map_err(|e| e.to_string())?;

        tx.commit().map_err(|e| e.to_string())?;

        // Move cursor back to top of stack
        *cursor = None;

        // Release conn lock before pruning (prune_oldest_undo_records takes its own lock)
        drop(conn);

        let _ = db.prune_oldest_undo_records(self.max_depth);

        Ok(ActionResult {
            undo_record_id: record_id,
            label: label.clone(),
            can_undo: true,
        })
    }

    pub fn undo(&self, db: &Database) -> Result<Option<String>, String> {
        let mut cursor = self.cursor_seq.lock().unwrap();

        // Find the record to undo
        let target_seq = match *cursor {
            None => {
                // At top of stack — undo the most recent record
                db.get_max_undo_seq().map_err(|e| e.to_string())?
            }
            Some(cur) => {
                // Find the record just below current cursor
                let conn = db.conn.lock().unwrap();
                let seq: Option<i64> = conn
                    .query_row(
                        "SELECT MAX(seq) FROM undo_records WHERE seq < ?1",
                        rusqlite::params![cur],
                        |row| row.get(0),
                    )
                    .map_err(|e| e.to_string())?;
                drop(conn);
                seq
            }
        };

        let target_seq = match target_seq {
            Some(s) => s,
            None => return Ok(None), // Nothing to undo
        };

        let record = db
            .get_undo_record_by_seq(target_seq)
            .map_err(|e| e.to_string())?;
        let record = match record {
            Some(r) => r,
            None => return Ok(None),
        };

        // Apply the before state
        self.apply_state(db, &record.action_type, &record.before_json)?;

        // Move cursor to target (the record we just undid)
        *cursor = Some(target_seq);

        Ok(Some(record.label))
    }

    pub fn redo(&self, db: &Database) -> Result<Option<String>, String> {
        let mut cursor = self.cursor_seq.lock().unwrap();

        let cur_seq = match *cursor {
            None => return Ok(None), // Already at top, nothing to redo
            Some(s) => s,
        };

        // The record at cursor was undone — redo it
        let record = db
            .get_undo_record_by_seq(cur_seq)
            .map_err(|e| e.to_string())?;
        let record = match record {
            Some(r) => r,
            None => return Ok(None),
        };

        // Apply the after state
        self.apply_state(db, &record.action_type, &record.after_json)?;

        // Move cursor up — find next record above current, or go to None (top)
        let conn = db.conn.lock().unwrap();
        let next_seq: Option<i64> = conn
            .query_row(
                "SELECT MIN(seq) FROM undo_records WHERE seq > ?1",
                rusqlite::params![cur_seq],
                |row| row.get(0),
            )
            .map_err(|e| e.to_string())?;
        drop(conn);

        match next_seq {
            Some(ns) => *cursor = Some(ns),
            None => *cursor = None,
        }

        Ok(Some(record.label))
    }

    pub fn status(&self, db: &Database) -> UndoStatus {
        let cursor = self.cursor_seq.lock().unwrap();
        let total = db.count_undo_records().unwrap_or(0);
        let max_seq = db.get_max_undo_seq().ok().flatten();

        let (can_undo, undo_label) = match *cursor {
            None => {
                // At top — can undo if there are records
                if let Some(ms) = max_seq {
                    let label = db
                        .get_undo_record_by_seq(ms)
                        .ok()
                        .flatten()
                        .map(|r| r.label);
                    (true, label)
                } else {
                    (false, None)
                }
            }
            Some(cur) => {
                // Can undo if there's a record below cursor
                let conn = db.conn.lock().unwrap();
                let below: Option<i64> = conn
                    .query_row(
                        "SELECT MAX(seq) FROM undo_records WHERE seq < ?1",
                        rusqlite::params![cur],
                        |row| row.get(0),
                    )
                    .unwrap_or(None);
                drop(conn);
                if let Some(bs) = below {
                    let label = db
                        .get_undo_record_by_seq(bs)
                        .ok()
                        .flatten()
                        .map(|r| r.label);
                    (true, label)
                } else {
                    (false, None)
                }
            }
        };

        let (can_redo, redo_label) = match *cursor {
            None => (false, None),
            Some(cur) => {
                let label = db
                    .get_undo_record_by_seq(cur)
                    .ok()
                    .flatten()
                    .map(|r| r.label);
                (true, label)
            }
        };

        UndoStatus {
            can_undo,
            can_redo,
            undo_label,
            redo_label,
            stack_depth: total,
        }
    }

    pub fn record_action(
        &self,
        db: &Database,
        action_type: &str,
        label: String,
        before_json: String,
        after_json: String,
        affected_ids: String,
        has_file_backup: bool,
    ) -> Result<ActionResult, String> {
        let mut cursor = self.cursor_seq.lock().unwrap();
        let mut conn = db.conn.lock().unwrap();
        let tx = conn.savepoint().map_err(|e| e.to_string())?;

        if let Some(cur_seq) = *cursor {
            tx.execute(
                "DELETE FROM undo_records WHERE seq >= ?1",
                rusqlite::params![cur_seq],
            )
            .map_err(|e| e.to_string())?;
        }

        let record_id = Uuid::new_v4().to_string();
        let now = Utc::now().to_rfc3339();
        let backup_flag: i32 = if has_file_backup { 1 } else { 0 };
        tx.execute(
            "INSERT INTO undo_records (id, action_type, label, before_json, after_json, affected_image_ids, has_file_backup, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
            rusqlite::params![record_id, action_type, label, before_json, after_json, affected_ids, backup_flag, now],
        )
        .map_err(|e| e.to_string())?;

        tx.commit().map_err(|e| e.to_string())?;
        *cursor = None;
        drop(conn);

        let _ = db.prune_oldest_undo_records(self.max_depth);

        Ok(ActionResult {
            undo_record_id: record_id,
            label: label.clone(),
            can_undo: true,
        })
    }

    pub fn history(&self, db: &Database, limit: u32) -> Vec<UndoRecord> {
        db.list_undo_records(limit).unwrap_or_default()
    }

    fn apply_state(&self, db: &Database, action_type: &str, state_json: &str) -> Result<(), String> {
        let val: serde_json::Value = serde_json::from_str(state_json)
            .map_err(|e| format!("Invalid undo state JSON: {}", e))?;
        match action_type {
            "set_rating" => {
                let image_id = val["image_id"].as_str().ok_or("Missing image_id")?;
                let rating = val["rating"].as_u64().ok_or("Missing rating")? as u8;
                db.set_rating(image_id, rating).map_err(|e| e.to_string())
            }
            "set_decision" => {
                let image_id = val["image_id"].as_str().ok_or("Missing image_id")?;
                let decision = val["decision"].as_str().ok_or("Missing decision")?;
                db.set_decision(image_id, decision).map_err(|e| e.to_string())
            }
            "trash_image" => {
                let path = val["path"].as_str().ok_or("Missing path")?;
                let trashed = val.get("trashed").and_then(|v| v.as_bool()).unwrap_or(false);
                if trashed {
                    // Redo: re-trash the file
                    #[cfg(target_os = "macos")]
                    {
                        std::process::Command::new("osascript")
                            .args(["-e", &format!(
                                "tell application \"Finder\" to delete POSIX file \"{}\"",
                                path.replace('"', "\\\"")
                            )])
                            .output()
                            .map_err(|e| format!("Failed to re-trash: {}", e))?;
                    }
                    Ok(())
                } else {
                    // Undo: restore from Trash to original path
                    let file_path = std::path::Path::new(path);
                    let filename = file_path.file_name()
                        .and_then(|n| n.to_str())
                        .ok_or("Invalid filename in path")?;
                    let trash_path = dirs::home_dir()
                        .ok_or("Cannot find home directory")?
                        .join(".Trash")
                        .join(filename);
                    if trash_path.exists() {
                        std::fs::rename(&trash_path, path)
                            .map_err(|e| format!("Failed to restore from Trash: {}", e))?;
                    } else {
                        return Err(format!("File not found in Trash: {}", filename));
                    }
                    Ok(())
                }
            }
            _ => Err(format!("Unknown action type for undo: {}", action_type)),
        }
    }
}
