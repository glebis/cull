# Sessions Architecture Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add file-system-based session workspaces to ImageView, with session-scoped canvases, file copy/move with hash validation, and a top-level session switcher in the sidebar.

**Architecture:** Sessions reuse the `projects` table with `collection_type = 'session'`. Each session maps to a folder on disk (`~/ImageView/Sessions/<name>/`). A new `canvases` table stores multiple canvases per session. The UI gets a session switcher dropdown above the sidebar that scopes all navigation to the active session.

**Tech Stack:** Rust (rusqlite, Tauri 2 commands), Svelte 5 (runes, stores), SQLite WAL mode

**Spec:** `docs/superpowers/specs/2026-05-10-sessions-architecture-design.md`

---

## File Map

| File | Action | Responsibility |
|------|--------|---------------|
| `src-tauri/src/db_core/db.rs` | Modify | Add `migrate_sessions()`, canvases table, session columns, indexes |
| `src-tauri/src/db_core/models.rs` | Modify | Add `Session`, `Canvas` structs |
| `src-tauri/src/db_core/sessions.rs` | Create | Session DB operations (CRUD, conversion, batch cleanup) |
| `src-tauri/src/db_core/mod.rs` | Modify | Add `pub mod sessions;` |
| `src-tauri/src/services/sessions.rs` | Create | Session service layer (file ops, hash validation, lifecycle) |
| `src-tauri/src/services/mod.rs` | Modify | Add `pub mod sessions;` |
| `src-tauri/src/commands/sessions.rs` | Create | Tauri IPC commands for sessions |
| `src-tauri/src/commands/mod.rs` | Modify | Add `pub mod sessions;` |
| `src-tauri/src/lib.rs` | Modify | Register session commands in `invoke_handler` |
| `src/lib/api.ts` | Modify | Add session + canvas API functions |
| `src/lib/stores.ts` | Modify | Add session stores (`activeSession`, `sessionCanvases`) |
| `src/lib/persistence.ts` | Modify | Add `activeSessionId` to persisted state |
| `src/lib/components/SessionSwitcher.svelte` | Create | Dropdown with search above sidebar |
| `src/lib/components/Sidebar.svelte` | Modify | Session-scoped navigation, canvases section |

---

## Task 1: Schema Migration & Models

**Files:**
- Modify: `src-tauri/src/db_core/db.rs:127-160` (add `migrate_sessions` call)
- Modify: `src-tauri/src/db_core/models.rs`
- Test: inline `#[cfg(test)]` in `db.rs`

- [ ] **Step 1: Write failing test for session migration**

Add to `src-tauri/src/db_core/db.rs` at the end of the file:

```rust
#[cfg(test)]
mod session_tests {
    use super::*;

    #[test]
    fn test_session_migration_creates_canvases_table() {
        let db = Database::open(std::path::Path::new(":memory:")).unwrap();
        let conn = db.conn.lock().unwrap();
        let count: i64 = conn.query_row(
            "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name='canvases'",
            [], |row| row.get(0)
        ).unwrap();
        assert_eq!(count, 1, "canvases table should exist after migration");
    }

    #[test]
    fn test_session_migration_adds_project_columns() {
        let db = Database::open(std::path::Path::new(":memory:")).unwrap();
        let conn = db.conn.lock().unwrap();
        // Check folder_path column exists
        let mut stmt = conn.prepare("SELECT folder_path FROM projects LIMIT 0").unwrap();
        drop(stmt);
        // Check owning_session_id column exists
        stmt = conn.prepare("SELECT owning_session_id FROM projects LIMIT 0").unwrap();
        drop(stmt);
        // Check settings_json column exists
        stmt = conn.prepare("SELECT settings_json FROM projects LIMIT 0").unwrap();
        drop(stmt);
    }

    #[test]
    fn test_session_indexes_exist() {
        let db = Database::open(std::path::Path::new(":memory:")).unwrap();
        let conn = db.conn.lock().unwrap();
        let indexes: Vec<String> = conn.prepare(
            "SELECT name FROM sqlite_master WHERE type='index'"
        ).unwrap()
        .query_map([], |row| row.get(0)).unwrap()
        .filter_map(|r| r.ok())
        .collect();
        assert!(indexes.contains(&"idx_canvases_session".to_string()));
        assert!(indexes.contains(&"idx_collection_items_image".to_string()));
        assert!(indexes.contains(&"idx_selections_project".to_string()));
        assert!(indexes.contains(&"idx_embeddings_image".to_string()));
        assert!(indexes.contains(&"idx_images_import_batch".to_string()));
    }
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cd src-tauri && cargo test session_tests -- --nocapture`
Expected: FAIL — `canvases` table doesn't exist, columns don't exist

- [ ] **Step 3: Add model structs**

Add to `src-tauri/src/db_core/models.rs`:

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Session {
    pub id: String,
    pub name: String,
    pub description: Option<String>,
    pub folder_path: String,
    pub settings_json: Option<String>,
    pub created_at: String,
    pub image_count: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Canvas {
    pub id: String,
    pub session_id: String,
    pub name: String,
    pub canvas_type: String,
    pub layout_json: String,
    pub filter_json: Option<String>,
    pub grid_config_json: Option<String>,
    pub sort_order: i32,
    pub created_at: String,
    pub updated_at: String,
}
```

- [ ] **Step 4: Add `migrate_sessions` to `db.rs`**

Add this method to `impl Database` in `src-tauri/src/db_core/db.rs`, after `migrate_generation_runs`:

```rust
fn migrate_sessions(&self) -> Result<()> {
    let conn = self.conn.lock().unwrap();

    // Add session columns to projects table
    let project_columns = vec![
        ("folder_path", "TEXT"),
        ("owning_session_id", "TEXT REFERENCES projects(id)"),
        ("settings_json", "TEXT"),
    ];
    for (name, typ) in &project_columns {
        let sql = format!("ALTER TABLE projects ADD COLUMN {} {}", name, typ);
        match conn.execute(&sql, []) {
            Ok(_) => {},
            Err(e) if e.to_string().contains("duplicate column") => {},
            Err(e) => return Err(e),
        }
    }

    // Create canvases table
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS canvases (
            id TEXT PRIMARY KEY,
            session_id TEXT NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
            name TEXT NOT NULL,
            canvas_type TEXT NOT NULL DEFAULT 'manual'
                CHECK (canvas_type IN ('manual', 'query')),
            layout_json TEXT NOT NULL DEFAULT '{}',
            filter_json TEXT,
            grid_config_json TEXT,
            sort_order INTEGER DEFAULT 0,
            created_at TEXT NOT NULL,
            updated_at TEXT NOT NULL
        );
        CREATE INDEX IF NOT EXISTS idx_canvases_session ON canvases(session_id);"
    )?;

    // Add missing indexes
    conn.execute_batch(
        "CREATE INDEX IF NOT EXISTS idx_collection_items_image ON collection_items(image_id);
         CREATE INDEX IF NOT EXISTS idx_selections_project ON selections(project_id);
         CREATE INDEX IF NOT EXISTS idx_embeddings_image ON embeddings(image_id);
         CREATE INDEX IF NOT EXISTS idx_images_import_batch ON images(import_batch_id);"
    )?;

    Ok(())
}
```

- [ ] **Step 5: Call `migrate_sessions` from `run_migrations`**

In `src-tauri/src/db_core/db.rs`, add to `run_migrations()` after `self.migrate_generation_runs()?;`:

```rust
self.migrate_sessions()?;
```

- [ ] **Step 6: Run tests to verify they pass**

Run: `cd src-tauri && cargo test session_tests -- --nocapture`
Expected: All 3 tests PASS

- [ ] **Step 7: Commit**

```bash
git add src-tauri/src/db_core/db.rs src-tauri/src/db_core/models.rs
git commit -m "feat(sessions): add schema migration, canvases table, and model structs"
```

---

## Task 2: Session DB Operations

**Files:**
- Create: `src-tauri/src/db_core/sessions.rs`
- Modify: `src-tauri/src/db_core/mod.rs`
- Test: inline `#[cfg(test)]` in `sessions.rs`

**Depends on:** Task 1 (schema must exist)

- [ ] **Step 1: Create `src-tauri/src/db_core/mod.rs` entry**

Add to `src-tauri/src/db_core/mod.rs`:

```rust
pub mod sessions;
```

- [ ] **Step 2: Write failing tests for session CRUD**

Create `src-tauri/src/db_core/sessions.rs`:

```rust
use rusqlite::{params, Result};
use super::db::Database;
use super::models::*;

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
        assert_eq!(sessions[0].name, "Session A");
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
        // Add a canvas to verify it gets deleted
        db.create_canvas(&id, "Test Canvas", "manual").unwrap();
        db.convert_session_to_collection(&id).unwrap();
        // Should now be a manual collection
        let cols = db.list_collections().unwrap();
        assert!(cols.iter().any(|(cid, name, _)| cid == &id && name == "Convert Me"));
        // Canvases should be deleted
        let canvases = db.list_canvases(&id).unwrap();
        assert!(canvases.is_empty());
    }

    #[test]
    fn test_create_and_list_canvases() {
        let db = test_db();
        let sid = db.create_session("Canvas Test", "/tmp/sessions/canvas").unwrap();
        let c1 = db.create_canvas(&sid, "Layout A", "manual").unwrap();
        let c2 = db.create_canvas(&sid, "Query View", "query").unwrap();
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
        let cid = db.create_canvas(&sid, "To Remove", "manual").unwrap();
        db.delete_canvas(&cid).unwrap();
        let canvases = db.list_canvases(&sid).unwrap();
        assert!(canvases.is_empty());
    }

    #[test]
    fn test_cleanup_old_batches() {
        let db = test_db();
        let conn = db.conn.lock().unwrap();
        // Insert an old batch (30 days ago)
        conn.execute(
            "INSERT INTO import_batches (id, created_at, source, image_count) VALUES ('old1', datetime('now', '-30 days'), 'test', 5)",
            [],
        ).unwrap();
        // Insert a recent batch (1 day ago)
        conn.execute(
            "INSERT INTO import_batches (id, created_at, source, image_count) VALUES ('new1', datetime('now', '-1 day'), 'test', 3)",
            [],
        ).unwrap();
        drop(conn);
        let cleaned = db.cleanup_old_batches(7).unwrap();
        assert_eq!(cleaned, 1);
    }
}
```

- [ ] **Step 3: Run tests to verify they fail**

Run: `cd src-tauri && cargo test db_core::sessions -- --nocapture`
Expected: FAIL — methods don't exist

- [ ] **Step 4: Implement session DB operations**

Add implementations above the `#[cfg(test)]` block in `src-tauri/src/db_core/sessions.rs`:

```rust
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
                folder_path: row.get(3)?,
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
                    folder_path: row.get(3)?,
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

    pub fn add_images_to_session(&self, session_id: &str, image_ids: &[&str]) -> Result<()> {
        self.add_to_collection(session_id, image_ids)
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
        // Clear batch references on images first
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
    // ... tests from Step 2 ...
}
```

- [ ] **Step 5: Run tests to verify they pass**

Run: `cd src-tauri && cargo test db_core::sessions -- --nocapture`
Expected: All 8 tests PASS

- [ ] **Step 6: Commit**

```bash
git add src-tauri/src/db_core/sessions.rs src-tauri/src/db_core/mod.rs
git commit -m "feat(sessions): add session and canvas DB operations with batch cleanup"
```

---

## Task 3: Session Service Layer & File Operations

**Files:**
- Create: `src-tauri/src/services/sessions.rs`
- Modify: `src-tauri/src/services/mod.rs`
- Test: inline `#[cfg(test)]` in `sessions.rs`

**Depends on:** Task 2 (DB operations must exist)

- [ ] **Step 1: Add module to `src-tauri/src/services/mod.rs`**

```rust
pub mod sessions;
```

- [ ] **Step 2: Write failing tests for session service**

Create `src-tauri/src/services/sessions.rs`:

```rust
use std::path::{Path, PathBuf};
use crate::db_core::db::Database;
use crate::db_core::models::*;
use crate::services::{ServiceContext, ServiceError};
use crate::services::library::enrich_thumbnails;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db_core::db::Database;
    use crate::db_core::secrets::MemoryStore;
    use crate::db_core::embeddings::EmbeddingEngine;
    use crate::db_core::detection::DetectionEngine;
    use std::sync::Mutex;

    fn make_ctx_parts() -> (Database, MemoryStore, PathBuf, Mutex<EmbeddingEngine>, Mutex<DetectionEngine>, Mutex<DetectionEngine>, tempfile::TempDir) {
        let tmp = tempfile::tempdir().unwrap();
        let db = Database::open(Path::new(":memory:")).unwrap();
        let secrets = MemoryStore::new();
        let dir = tmp.path().to_path_buf();
        let mdir = tmp.path().join("models");
        (db, secrets, dir, Mutex::new(EmbeddingEngine::new(&mdir)), Mutex::new(DetectionEngine::new_yolo(&mdir)), Mutex::new(DetectionEngine::new_nudenet(&mdir)), tmp)
    }

    fn ctx<'a>(db: &'a Database, s: &'a MemoryStore, d: &'a PathBuf, ee: &'a Mutex<EmbeddingEngine>, de: &'a Mutex<DetectionEngine>, se: &'a Mutex<DetectionEngine>) -> ServiceContext<'a> {
        ServiceContext { db, app_data_dir: d, embedding_engine: ee, detection_engine: de, safety_engine: se, secrets: s, app_handle: None }
    }

    #[test]
    fn test_sanitize_folder_name() {
        assert_eq!(sanitize_folder_name("Portrait Shoot"), "portrait-shoot");
        assert_eq!(sanitize_folder_name("hello/world:test"), "hello-world-test");
        assert_eq!(sanitize_folder_name("  spaces  "), "spaces");
    }

    #[test]
    fn test_create_session_creates_folders() {
        let (db, s, d, ee, de, se, tmp) = make_ctx_parts();
        let c = ctx(&db, &s, &d, &ee, &de, &se);
        let sessions_root = tmp.path().join("Sessions");
        let session = create_session(&c, "Test Shoot", &sessions_root).unwrap();
        let folder = Path::new(&session.folder_path);
        assert!(folder.exists());
        assert!(folder.join("Imports").exists());
        assert!(folder.join("Selects").exists());
        assert!(folder.join("Exports").exists());
    }

    #[test]
    fn test_validate_file_hash() {
        let tmp = tempfile::tempdir().unwrap();
        let src = tmp.path().join("test.txt");
        std::fs::write(&src, b"hello world").unwrap();
        let expected_hash = compute_sha256(&src).unwrap();
        assert!(validate_file_hash(&src, &expected_hash).unwrap());
        assert!(!validate_file_hash(&src, "wrong_hash").unwrap());
    }

    #[test]
    fn test_copy_file_to_session() {
        let tmp = tempfile::tempdir().unwrap();
        let src = tmp.path().join("original.png");
        std::fs::write(&src, b"fake image data").unwrap();
        let dest_dir = tmp.path().join("session/Imports");
        std::fs::create_dir_all(&dest_dir).unwrap();
        let dest = copy_file_to_session(&src, &dest_dir).unwrap();
        assert!(dest.exists());
        assert_eq!(std::fs::read(&dest).unwrap(), b"fake image data");
        // Original still exists
        assert!(src.exists());
    }

    #[test]
    fn test_move_file_to_session() {
        let tmp = tempfile::tempdir().unwrap();
        let src = tmp.path().join("original.png");
        std::fs::write(&src, b"fake image data").unwrap();
        let dest_dir = tmp.path().join("session/Imports");
        std::fs::create_dir_all(&dest_dir).unwrap();
        let dest = move_file_to_session(&src, &dest_dir).unwrap();
        assert!(dest.exists());
        assert!(!src.exists()); // Original removed
    }
}
```

- [ ] **Step 3: Run tests to verify they fail**

Run: `cd src-tauri && cargo test services::sessions -- --nocapture`
Expected: FAIL — functions don't exist

- [ ] **Step 4: Implement session service functions**

Add implementations above the `#[cfg(test)]` block in `src-tauri/src/services/sessions.rs`:

```rust
use std::path::{Path, PathBuf};
use crate::db_core::db::Database;
use crate::db_core::models::*;
use crate::services::{ServiceContext, ServiceError};
use crate::services::library::enrich_thumbnails;

pub fn sanitize_folder_name(name: &str) -> String {
    name.trim()
        .to_lowercase()
        .chars()
        .map(|c| if c.is_alphanumeric() || c == '-' || c == '_' { c } else { '-' })
        .collect::<String>()
        .split('-')
        .filter(|s| !s.is_empty())
        .collect::<Vec<&str>>()
        .join("-")
}

pub fn compute_sha256(path: &Path) -> Result<String, ServiceError> {
    use sha2::{Sha256, Digest};
    let mut file = std::fs::File::open(path)?;
    let mut hasher = Sha256::new();
    std::io::copy(&mut file, &mut hasher)?;
    Ok(format!("{:x}", hasher.finalize()))
}

pub fn validate_file_hash(path: &Path, expected_hash: &str) -> Result<bool, ServiceError> {
    let actual = compute_sha256(path)?;
    Ok(actual == expected_hash)
}

pub fn create_session(ctx: &ServiceContext, name: &str, sessions_root: &Path) -> Result<Session, ServiceError> {
    let date = chrono::Local::now().format("%Y-%m-%d").to_string();
    let folder_name = format!("{}-{}", date, sanitize_folder_name(name));
    let folder_path = sessions_root.join(&folder_name);

    std::fs::create_dir_all(folder_path.join("Imports"))?;
    std::fs::create_dir_all(folder_path.join("Selects"))?;
    std::fs::create_dir_all(folder_path.join("Exports"))?;

    let folder_str = folder_path.to_string_lossy().to_string();
    let id = ctx.db.create_session(name, &folder_str)?;
    ctx.db.get_session(&id).map_err(ServiceError::from)
}

pub fn copy_file_to_session(source: &Path, dest_dir: &Path) -> Result<PathBuf, ServiceError> {
    let filename = source.file_name()
        .ok_or_else(|| ServiceError::InvalidInput("No filename".into()))?;
    let dest = dest_dir.join(filename);
    std::fs::copy(source, &dest)?;
    Ok(dest)
}

pub fn move_file_to_session(source: &Path, dest_dir: &Path) -> Result<PathBuf, ServiceError> {
    let filename = source.file_name()
        .ok_or_else(|| ServiceError::InvalidInput("No filename".into()))?;
    let dest = dest_dir.join(filename);
    // Try rename first (fast, same filesystem), fall back to copy+delete
    if std::fs::rename(source, &dest).is_err() {
        std::fs::copy(source, &dest)?;
        std::fs::remove_file(source)?;
    }
    Ok(dest)
}

pub fn list_sessions(ctx: &ServiceContext) -> Result<Vec<Session>, ServiceError> {
    Ok(ctx.db.list_sessions()?)
}

pub fn get_session(ctx: &ServiceContext, id: &str) -> Result<Session, ServiceError> {
    Ok(ctx.db.get_session(id)?)
}

pub fn delete_session(ctx: &ServiceContext, id: &str, delete_files: bool) -> Result<(), ServiceError> {
    if delete_files {
        let session = ctx.db.get_session(id)?;
        let folder = Path::new(&session.folder_path);
        if folder.exists() {
            std::fs::remove_dir_all(folder)?;
        }
    }
    Ok(ctx.db.delete_session(id)?)
}

pub fn convert_session_to_collection(ctx: &ServiceContext, id: &str) -> Result<(), ServiceError> {
    Ok(ctx.db.convert_session_to_collection(id)?)
}

pub fn validate_session_folder(ctx: &ServiceContext, id: &str) -> Result<bool, ServiceError> {
    let session = ctx.db.get_session(id)?;
    Ok(Path::new(&session.folder_path).exists())
}

pub fn create_canvas(ctx: &ServiceContext, session_id: &str, name: &str, canvas_type: &str) -> Result<Canvas, ServiceError> {
    if canvas_type != "manual" && canvas_type != "query" {
        return Err(ServiceError::InvalidInput(format!("Invalid canvas type: {}", canvas_type)));
    }
    let id = ctx.db.create_canvas(session_id, name, canvas_type)?;
    let canvases = ctx.db.list_canvases(session_id)?;
    canvases.into_iter().find(|c| c.id == id)
        .ok_or_else(|| ServiceError::NotFound("Canvas not found after creation".into()))
}

pub fn list_canvases(ctx: &ServiceContext, session_id: &str) -> Result<Vec<Canvas>, ServiceError> {
    Ok(ctx.db.list_canvases(session_id)?)
}

pub fn update_canvas_layout(ctx: &ServiceContext, canvas_id: &str, layout_json: &str) -> Result<(), ServiceError> {
    Ok(ctx.db.update_canvas_layout(canvas_id, layout_json)?)
}

pub fn delete_canvas(ctx: &ServiceContext, canvas_id: &str) -> Result<(), ServiceError> {
    Ok(ctx.db.delete_canvas(canvas_id)?)
}

pub fn cleanup_old_batches(ctx: &ServiceContext) -> Result<u32, ServiceError> {
    let max_age = ctx.db.get_setting("batch_cleanup_days")
        .ok()
        .flatten()
        .and_then(|v| v.parse::<u32>().ok())
        .unwrap_or(7);
    Ok(ctx.db.cleanup_old_batches(max_age)?)
}
```

- [ ] **Step 5: Run tests to verify they pass**

Run: `cd src-tauri && cargo test services::sessions -- --nocapture`
Expected: All 5 tests PASS

- [ ] **Step 6: Commit**

```bash
git add src-tauri/src/services/sessions.rs src-tauri/src/services/mod.rs
git commit -m "feat(sessions): add session service layer with file copy/move and hash validation"
```

---

## Task 4: Tauri Commands & API Layer

**Files:**
- Create: `src-tauri/src/commands/sessions.rs`
- Modify: `src-tauri/src/commands/mod.rs`
- Modify: `src-tauri/src/lib.rs` (register commands)
- Modify: `src/lib/api.ts` (add TS functions)
- Modify: `src/lib/stores.ts` (add session stores)

**Depends on:** Task 3 (service layer must exist)

- [ ] **Step 1: Create Tauri commands**

Create `src-tauri/src/commands/sessions.rs`:

```rust
use tauri::State;
use crate::AppState;
use crate::db_core::models::{Session, Canvas};
use crate::services::ServiceContext;
use crate::services::sessions as svc;

#[tauri::command]
pub async fn create_session(state: State<'_, AppState>, name: String) -> Result<Session, String> {
    let ctx = ServiceContext::from_app_state(&state, None);
    let sessions_root = state.app_data_dir.parent()
        .unwrap_or(&state.app_data_dir)
        .join("ImageView")
        .join("Sessions");
    // Check for custom sessions root in settings
    let root = ctx.db.get_setting("sessions_root")
        .ok()
        .flatten()
        .map(|p| std::path::PathBuf::from(p))
        .unwrap_or(sessions_root);
    svc::create_session(&ctx, &name, &root).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn list_sessions(state: State<'_, AppState>) -> Result<Vec<Session>, String> {
    let ctx = ServiceContext::from_app_state(&state, None);
    svc::list_sessions(&ctx).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn get_session(state: State<'_, AppState>, session_id: String) -> Result<Session, String> {
    let ctx = ServiceContext::from_app_state(&state, None);
    svc::get_session(&ctx, &session_id).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn delete_session(state: State<'_, AppState>, session_id: String, delete_files: bool) -> Result<(), String> {
    let ctx = ServiceContext::from_app_state(&state, None);
    svc::delete_session(&ctx, &session_id, delete_files).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn convert_session_to_collection(state: State<'_, AppState>, session_id: String) -> Result<(), String> {
    let ctx = ServiceContext::from_app_state(&state, None);
    svc::convert_session_to_collection(&ctx, &session_id).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn validate_session_folder(state: State<'_, AppState>, session_id: String) -> Result<bool, String> {
    let ctx = ServiceContext::from_app_state(&state, None);
    svc::validate_session_folder(&ctx, &session_id).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn create_canvas(state: State<'_, AppState>, session_id: String, name: String, canvas_type: String) -> Result<Canvas, String> {
    let ctx = ServiceContext::from_app_state(&state, None);
    svc::create_canvas(&ctx, &session_id, &name, &canvas_type).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn list_canvases(state: State<'_, AppState>, session_id: String) -> Result<Vec<Canvas>, String> {
    let ctx = ServiceContext::from_app_state(&state, None);
    svc::list_canvases(&ctx, &session_id).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn update_canvas_layout(state: State<'_, AppState>, canvas_id: String, layout_json: String) -> Result<(), String> {
    let ctx = ServiceContext::from_app_state(&state, None);
    svc::update_canvas_layout(&ctx, &canvas_id, &layout_json).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn delete_canvas(state: State<'_, AppState>, canvas_id: String) -> Result<(), String> {
    let ctx = ServiceContext::from_app_state(&state, None);
    svc::delete_canvas(&ctx, &canvas_id).map_err(|e| e.to_string())
}
```

- [ ] **Step 2: Register commands**

Add to `src-tauri/src/commands/mod.rs`:

```rust
pub mod sessions;
```

Add to `src-tauri/src/lib.rs` in the `invoke_handler` macro, after the lineage commands:

```rust
commands::sessions::create_session,
commands::sessions::list_sessions,
commands::sessions::get_session,
commands::sessions::delete_session,
commands::sessions::convert_session_to_collection,
commands::sessions::validate_session_folder,
commands::sessions::create_canvas,
commands::sessions::list_canvases,
commands::sessions::update_canvas_layout,
commands::sessions::delete_canvas,
```

- [ ] **Step 3: Verify Rust compiles**

Run: `cd src-tauri && cargo check`
Expected: No errors

- [ ] **Step 4: Add TypeScript API functions**

Add to `src/lib/api.ts`, after the existing interface definitions:

```typescript
export interface Session {
    id: string;
    name: string;
    description: string | null;
    folder_path: string;
    settings_json: string | null;
    created_at: string;
    image_count: number;
}

export interface Canvas {
    id: string;
    session_id: string;
    name: string;
    canvas_type: 'manual' | 'query';
    layout_json: string;
    filter_json: string | null;
    grid_config_json: string | null;
    sort_order: number;
    created_at: string;
    updated_at: string;
}
```

Add API functions at the end of `src/lib/api.ts`:

```typescript
// Sessions
export async function createSession(name: string): Promise<Session> {
    return invoke('create_session', { name });
}
export async function listSessions(): Promise<Session[]> {
    return invoke('list_sessions');
}
export async function getSession(sessionId: string): Promise<Session> {
    return invoke('get_session', { sessionId });
}
export async function deleteSession(sessionId: string, deleteFiles: boolean): Promise<void> {
    return invoke('delete_session', { sessionId, deleteFiles });
}
export async function convertSessionToCollection(sessionId: string): Promise<void> {
    return invoke('convert_session_to_collection', { sessionId });
}
export async function validateSessionFolder(sessionId: string): Promise<boolean> {
    return invoke('validate_session_folder', { sessionId });
}

// Canvases
export async function createCanvas(sessionId: string, name: string, canvasType: string): Promise<Canvas> {
    return invoke('create_canvas', { sessionId, name, canvasType });
}
export async function listCanvases(sessionId: string): Promise<Canvas[]> {
    return invoke('list_canvases', { sessionId });
}
export async function updateCanvasLayout(canvasId: string, layoutJson: string): Promise<void> {
    return invoke('update_canvas_layout', { canvasId, layoutJson });
}
export async function deleteCanvas(canvasId: string): Promise<void> {
    return invoke('delete_canvas', { canvasId });
}
```

- [ ] **Step 5: Add session stores**

Add to `src/lib/stores.ts` after the existing smart collection stores:

```typescript
import type { Session, Canvas } from './api';

// Sessions
export const sessions = writable<Session[]>([]);
export const activeSession = writable<Session | null>(null);
export const sessionCanvases = writable<Canvas[]>([]);
```

- [ ] **Step 6: Add session to persistence**

In `src/lib/persistence.ts`, add `activeSessionId` to the `PersistedState` interface:

```typescript
activeSessionId: string | null;
```

Add to `saveAppState()`:

```typescript
activeSessionId: get(activeSession)?.id ?? null,
```

Add to `restoreAppStateBeforeImages()` after other restores:

```typescript
// activeSessionId is restored after images load — handled by App.svelte
```

Bump `SCHEMA_VERSION` to `2`.

- [ ] **Step 7: Verify frontend compiles**

Run: `npx svelte-check`
Expected: No errors

- [ ] **Step 8: Commit**

```bash
git add src-tauri/src/commands/sessions.rs src-tauri/src/commands/mod.rs src-tauri/src/lib.rs src/lib/api.ts src/lib/stores.ts src/lib/persistence.ts
git commit -m "feat(sessions): add Tauri commands, TypeScript API, and session stores"
```

---

## Task 5: Session Switcher UI & Sidebar Integration

**Files:**
- Create: `src/lib/components/SessionSwitcher.svelte`
- Modify: `src/lib/components/Sidebar.svelte`

**Depends on:** Task 4 (API + stores must exist)

- [ ] **Step 1: Create SessionSwitcher component**

Create `src/lib/components/SessionSwitcher.svelte`:

```svelte
<script lang="ts">
    import { sessions, activeSession, sessionCanvases, showToast } from '$lib/stores';
    import { listSessions, createSession, listCanvases, validateSessionFolder } from '$lib/api';
    import { onMount } from 'svelte';

    let open = $state(false);
    let search = $state('');
    let creating = $state(false);
    let newName = $state('');

    let filtered = $derived(
        $sessions.filter(s =>
            s.name.toLowerCase().includes(search.toLowerCase())
        )
    );

    onMount(async () => {
        try {
            const s = await listSessions();
            sessions.set(s);
        } catch (e) {
            console.error('Failed to load sessions:', e);
        }
    });

    async function selectSession(session: typeof $sessions[0] | null) {
        if (session) {
            const valid = await validateSessionFolder(session.id);
            if (!valid) {
                showToast('Session folder missing — files may be unavailable', { type: 'warning' });
            }
            const canvases = await listCanvases(session.id);
            sessionCanvases.set(canvases);
        } else {
            sessionCanvases.set([]);
        }
        activeSession.set(session);
        open = false;
        search = '';
    }

    async function handleCreate() {
        if (!newName.trim()) return;
        try {
            const session = await createSession(newName.trim());
            sessions.update(s => [session, ...s]);
            await selectSession(session);
            showToast(`Session "${session.name}" created`, { type: 'success' });
        } catch (e) {
            showToast(`Failed to create session: ${e}`, { type: 'error' });
        }
        creating = false;
        newName = '';
    }
</script>

<div class="session-switcher">
    <button class="session-toggle" onclick={() => open = !open}>
        <span class="session-label">
            {$activeSession?.name ?? 'All Images'}
        </span>
        <span class="chevron" class:open>▾</span>
    </button>

    {#if open}
        <div class="session-dropdown">
            <input
                class="session-search"
                type="text"
                placeholder="Search sessions..."
                bind:value={search}
            />

            <button
                class="session-item"
                class:active={!$activeSession}
                onclick={() => selectSession(null)}
            >
                All Images
            </button>

            {#each filtered as session}
                <button
                    class="session-item"
                    class:active={$activeSession?.id === session.id}
                    onclick={() => selectSession(session)}
                >
                    <span class="session-name">{session.name}</span>
                    <span class="count">{session.image_count}</span>
                </button>
            {/each}

            {#if creating}
                <div class="session-create-form">
                    <input
                        class="session-search"
                        type="text"
                        placeholder="Session name..."
                        bind:value={newName}
                        onkeydown={(e) => e.key === 'Enter' && handleCreate()}
                    />
                    <button class="create-btn" onclick={handleCreate}>Create</button>
                </div>
            {:else}
                <button class="session-item new-session" onclick={() => creating = true}>
                    + New Session
                </button>
            {/if}
        </div>
    {/if}
</div>

<style>
    .session-switcher {
        position: relative;
        padding: var(--spacing);
        border-bottom: 1px solid var(--border);
    }
    .session-toggle {
        display: flex;
        align-items: center;
        justify-content: space-between;
        width: 100%;
        padding: 6px 8px;
        background: var(--surface);
        border: 1px solid var(--border);
        border-radius: var(--radius);
        color: var(--text);
        cursor: pointer;
        font: inherit;
    }
    .session-toggle:hover {
        border-color: var(--blue);
    }
    .session-label {
        overflow: hidden;
        text-overflow: ellipsis;
        white-space: nowrap;
    }
    .chevron {
        transition: transform 0.15s;
        font-size: 10px;
        color: var(--text-secondary);
    }
    .chevron.open {
        transform: rotate(180deg);
    }
    .session-dropdown {
        position: absolute;
        top: 100%;
        left: var(--spacing);
        right: var(--spacing);
        background: var(--surface);
        border: 1px solid var(--border);
        border-radius: var(--radius);
        z-index: 100;
        max-height: 300px;
        overflow-y: auto;
    }
    .session-search {
        width: 100%;
        padding: 6px 8px;
        background: var(--bg);
        border: none;
        border-bottom: 1px solid var(--border);
        color: var(--text);
        font: inherit;
        outline: none;
        box-sizing: border-box;
    }
    .session-item {
        display: flex;
        align-items: center;
        justify-content: space-between;
        width: 100%;
        padding: 6px 8px;
        background: none;
        border: none;
        color: var(--text);
        cursor: pointer;
        font: inherit;
        text-align: left;
    }
    .session-item:hover {
        background: var(--bg);
    }
    .session-item.active {
        color: var(--blue);
    }
    .session-name {
        overflow: hidden;
        text-overflow: ellipsis;
        white-space: nowrap;
    }
    .count {
        color: var(--text-secondary);
        font-size: 11px;
        flex-shrink: 0;
    }
    .new-session {
        color: var(--blue);
        border-top: 1px solid var(--border);
    }
    .session-create-form {
        display: flex;
        gap: 4px;
        padding: 4px;
        border-top: 1px solid var(--border);
    }
    .create-btn {
        padding: 4px 8px;
        background: var(--blue);
        border: none;
        border-radius: var(--radius);
        color: var(--bg);
        cursor: pointer;
        font: inherit;
        white-space: nowrap;
    }
</style>
```

- [ ] **Step 2: Add SessionSwitcher to Sidebar**

In `src/lib/components/Sidebar.svelte`, add the import at the top of the `<script>` block:

```typescript
import SessionSwitcher from './SessionSwitcher.svelte';
```

Add the component at the very top of the template, before the first sidebar section:

```svelte
<SessionSwitcher />
```

- [ ] **Step 3: Add session-scoped canvases section to Sidebar**

In `src/lib/components/Sidebar.svelte`, add to the `<script>` block imports:

```typescript
import { activeSession, sessionCanvases } from '$lib/stores';
import { deleteCanvas as deleteCanvasApi, createCanvas } from '$lib/api';
```

Add a canvases section in the template, after SessionSwitcher and before folders:

```svelte
{#if $activeSession}
    <div class="section">
        <div class="section-header">
            <span>Canvases</span>
            <button class="section-action" onclick={async () => {
                const canvas = await createCanvas($activeSession.id, 'New Canvas', 'manual');
                sessionCanvases.update(c => [...c, canvas]);
            }}>+</button>
        </div>
        {#each $sessionCanvases as canvas}
            <button class="section-item">
                <span>{canvas.name}</span>
                <span class="count">{canvas.canvas_type}</span>
            </button>
        {/each}
    </div>
{/if}
```

- [ ] **Step 4: Verify dev server renders correctly**

Run: `npm run dev`
Open the app. The session switcher should appear above the sidebar sections showing "All Images". Clicking it should open a dropdown with "New Session" option.

- [ ] **Step 5: Commit**

```bash
git add src/lib/components/SessionSwitcher.svelte src/lib/components/Sidebar.svelte
git commit -m "feat(sessions): add session switcher UI and sidebar canvas section"
```

---

## Known Gaps (deferred to follow-up tasks)

1. **Import into session:** The existing `import_folder` and `import_files` commands in `src-tauri/src/commands/import.rs` need an optional `session_id` parameter. When provided, files should be copied/moved to the session's `Imports/` folder instead of their original location, and `collection_items` entries should be created to link images to the session. This is a modification to existing commands, not a new command.

2. **Smart collection → Session materialization:** Requires evaluating a smart collection's filter, then running the session import flow for each result. Can be built as a service function that composes `evaluate_smart_collection` + `create_session` + import loop.

3. **Import batch cleanup on app launch:** The `cleanup_old_batches` function exists in the service layer but needs to be called during app startup in `src-tauri/src/lib.rs` setup closure.

4. **Settings UI:** The Sessions settings section (sessions root folder, default import mode, batch cleanup interval) is not included in this plan. Use the existing `app_settings` table via `get_app_setting`/`set_app_setting`.

---

## Task Dependencies

```
Task 1 (Schema) ──► Task 2 (DB Ops) ──► Task 3 (Service) ──► Task 4 (Commands/API) ──► Task 5 (UI)
```

Tasks 1-3 are backend-only and can potentially be merged into a single agent. Task 4 bridges backend and frontend. Task 5 is frontend-only.

For parallel execution with multiple agents:
- **Agent A:** Tasks 1 + 2 (schema + DB operations)
- **Agent B:** Task 3 (service layer, starts after Agent A completes)
- **Agent C:** Task 5 (UI — can stub the API calls and build components in parallel)
- **Agent D:** Task 4 (wiring — runs after Agent A+B, merges with Agent C)
