# Clipboard Monitor Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Build Cull's Clipboard Monitor workflow: explicit clipboard monitoring creates a focused Grid collection, saves copied images as real files in a movable capture folder, imports them, publishes the collection, copies the URL, and exposes the workflow to MCP agents.

**Architecture:** Add a Rust clipboard monitor service with a testable core and a small platform reader boundary. The first native reader is macOS `NSPasteboard`; unsupported platforms return explicit status. Frontend controls live in the sidebar and reuse existing collection scope, static publishing, and MCP surfaces rather than creating a separate workflow.

**Tech Stack:** Tauri 2, Rust, Svelte 5 runes, SQLite via rusqlite, existing static publishing command module, existing MCP rmcp tools.

---

## File Map

- Create `src-tauri/src/services/clipboard_monitor.rs`: platform-neutral monitor types, filename helpers, capture folder resolution, fake-reader-testable capture logic, publish result storage.
- Create `src-tauri/src/services/clipboard_monitor_macos.rs`: macOS `NSPasteboard` reader behind `#[cfg(target_os = "macos")]`.
- Modify `src-tauri/src/services/mod.rs`: export clipboard monitor modules.
- Modify `src-tauri/src/db_core/db.rs`: collection settings helpers and image-file path helpers for capture-folder moves.
- Create `src-tauri/src/commands/clipboard_monitor.rs`: Tauri commands.
- Modify `src-tauri/src/commands/mod.rs`: export command module.
- Modify `src-tauri/src/lib.rs`: add monitor state to `AppState`, initialize it, register commands.
- Modify `src-tauri/permissions/app-read.toml`: allow status command.
- Modify `src-tauri/permissions/app-file-access.toml`: allow start/stop/capture-folder commands.
- Modify `src-tauri/permissions/app-export-publishing.toml`: allow publish command.
- Modify `src-tauri/src/commands/static_publishing.rs`: add collection-to-static-publish helper.
- Modify `src-tauri/src/mcp/tools.rs`: add monitor status/show/publish/last-publish tools and tests.
- Modify `src-tauri/src/services/tokens.rs`: capability mapping for new MCP tools.
- Modify `src/lib/api.ts`: frontend command wrappers and types.
- Create `src/lib/clipboard-monitor.ts`: frontend helper functions for applying monitor status and events.
- Create `src/lib/clipboard-monitor.test.ts`: frontend state tests.
- Modify `src/lib/deeplink.ts`: listen for `navigate-collection`.
- Modify `src/lib/components/Sidebar.svelte`: add Clipboard Monitor controls.
- Modify `src/lib/tauri-mock.ts`: E2E/browser mock command responses.
- Modify `src/lib/tauri-command-contract.test.ts`: expected capability grouping for new commands.
- Modify `tests/e2e/smoke.py`: browser-only smoke check for monitor control rendering if the existing smoke suite has a stable sidebar checkpoint for feature controls.

## Task 1: Backend Filename, Settings, And Folder Helpers

**Files:**
- Create: `src-tauri/src/services/clipboard_monitor.rs`
- Modify: `src-tauri/src/services/mod.rs`
- Modify: `src-tauri/src/db_core/db.rs`

- [ ] **Step 1: Write failing Rust tests for filename generation**

Append this test module to the new `src-tauri/src/services/clipboard_monitor.rs` before implementation:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use chrono::{TimeZone, Utc};

    fn capture(
        original_filename: Option<&str>,
        source_url: Option<&str>,
        extension: &str,
    ) -> ClipboardCapture {
        ClipboardCapture {
            bytes: vec![1, 2, 3],
            extension: extension.to_string(),
            original_filename: original_filename.map(str::to_string),
            source_url: source_url.map(str::to_string),
            source_app: None,
            change_count: Some(42),
        }
    }

    #[test]
    fn filename_prefers_original_filename_and_local_24h_timestamp() {
        let now = Utc.with_ymd_and_hms(2026, 5, 30, 12, 35, 22).unwrap();
        let name = build_clipboard_capture_filename(
            &capture(Some("Living Room Ref@2x.JPG"), Some("https://www.pinterest.com/pin/123"), "png"),
            now,
            1,
        );
        assert_eq!(name, "2026-05-30_12-35-22_living-room-ref-2x_001.png");
    }

    #[test]
    fn filename_falls_back_to_source_host() {
        let now = Utc.with_ymd_and_hms(2026, 5, 30, 12, 36, 8).unwrap();
        let name = build_clipboard_capture_filename(
            &capture(None, Some("https://dribbble.com/shots/abc"), "png"),
            now,
            2,
        );
        assert_eq!(name, "2026-05-30_12-36-08_dribbble_002.png");
    }

    #[test]
    fn filename_falls_back_to_clipboard_and_sanitizes_extension() {
        let now = Utc.with_ymd_and_hms(2026, 5, 30, 12, 36, 41).unwrap();
        let name = build_clipboard_capture_filename(&capture(None, None, "../PNG"), now, 3);
        assert_eq!(name, "2026-05-30_12-36-41_clipboard_003.png");
    }
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run:

```bash
cd src-tauri && cargo test services::clipboard_monitor::tests::filename_ --lib
```

Expected: compile failure because `services::clipboard_monitor` and `build_clipboard_capture_filename` do not exist.

- [ ] **Step 3: Implement minimal helper types and filename logic**

Add to `src-tauri/src/services/mod.rs`:

```rust
pub mod clipboard_monitor;
```

Create `src-tauri/src/services/clipboard_monitor.rs` with:

```rust
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

pub const CAPTURE_DIR_SETTING: &str = "clipboard_monitor_capture_dir";
pub const LAST_COLLECTION_SETTING: &str = "clipboard_monitor_last_collection_id";
pub const DEFAULT_POLL_MS: u64 = 750;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum ClipboardAccessStatus {
    Supported,
    UnsupportedPlatform,
    PermissionRequired,
    PermissionDenied,
    Error(String),
}

#[derive(Debug, Clone)]
pub struct ClipboardCapture {
    pub bytes: Vec<u8>,
    pub extension: String,
    pub original_filename: Option<String>,
    pub source_url: Option<String>,
    pub source_app: Option<String>,
    pub change_count: Option<i64>,
}

#[derive(Debug, Default)]
pub struct ClipboardMonitorState {
    pub running: bool,
    pub collection_id: Option<String>,
    pub collection_name: Option<String>,
    pub capture_dir: Option<PathBuf>,
    pub captured_count: u32,
    pub last_change_count: Option<i64>,
    pub last_hash: Option<String>,
    pub last_error: Option<String>,
}

pub fn default_capture_dir(app_data_dir: &Path) -> PathBuf {
    app_data_dir.join("Clipboard Captures")
}

pub fn resolve_capture_dir(
    db: &crate::db_core::db::Database,
    app_data_dir: &Path,
    requested: Option<&str>,
) -> Result<PathBuf, String> {
    if let Some(path) = requested.map(str::trim).filter(|value| !value.is_empty()) {
        return validate_capture_dir(PathBuf::from(path));
    }
    if let Some(saved) = db
        .get_setting(CAPTURE_DIR_SETTING)
        .map_err(|e| e.to_string())?
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
    {
        return validate_capture_dir(PathBuf::from(saved));
    }
    Ok(default_capture_dir(app_data_dir))
}

fn validate_capture_dir(path: PathBuf) -> Result<PathBuf, String> {
    if path.components().any(|component| matches!(component, std::path::Component::ParentDir)) {
        return Err("Clipboard capture folder must not contain '..' components".to_string());
    }
    if path.is_absolute() {
        Ok(path)
    } else {
        Err("Clipboard capture folder must be an absolute path".to_string())
    }
}

pub fn build_clipboard_capture_filename(
    capture: &ClipboardCapture,
    now: DateTime<Utc>,
    sequence: u32,
) -> String {
    let timestamp = now.format("%Y-%m-%d_%H-%M-%S").to_string();
    let label = capture
        .original_filename
        .as_deref()
        .and_then(file_stem_label)
        .or_else(|| capture.source_url.as_deref().and_then(host_label))
        .unwrap_or_else(|| "clipboard".to_string());
    let ext = sanitize_extension(&capture.extension);
    format!("{timestamp}_{label}_{sequence:03}.{ext}")
}

fn file_stem_label(filename: &str) -> Option<String> {
    let stem = Path::new(filename).file_stem()?.to_string_lossy();
    slug_component(&stem)
}

fn host_label(url: &str) -> Option<String> {
    let after_scheme = url.split("://").nth(1).unwrap_or(url);
    let host = after_scheme.split('/').next()?.trim();
    let host = host.strip_prefix("www.").unwrap_or(host);
    let first = host.split('.').next().unwrap_or(host);
    slug_component(first)
}

fn sanitize_extension(extension: &str) -> String {
    let ext = extension
        .trim()
        .trim_start_matches('.')
        .chars()
        .filter(|ch| ch.is_ascii_alphanumeric())
        .collect::<String>()
        .to_ascii_lowercase();
    if ext.is_empty() { "png".to_string() } else { ext }
}

fn slug_component(value: &str) -> Option<String> {
    let mut out = String::new();
    let mut previous_dash = false;
    for ch in value.chars() {
        let lower = ch.to_ascii_lowercase();
        if lower.is_ascii_alphanumeric() {
            out.push(lower);
            previous_dash = false;
        } else if !previous_dash && !out.is_empty() {
            out.push('-');
            previous_dash = true;
        }
    }
    while out.ends_with('-') {
        out.pop();
    }
    if out.is_empty() { None } else { Some(out) }
}
```

- [ ] **Step 4: Run tests to verify they pass**

Run:

```bash
cd src-tauri && cargo test services::clipboard_monitor::tests::filename_ --lib
```

Expected: 3 tests pass.

- [ ] **Step 5: Write failing DB helper tests**

Add to the `#[cfg(test)] mod tests` in `src-tauri/src/db_core/db.rs`:

```rust
#[test]
fn test_collection_settings_json_round_trips() {
    let db = Database::open(std::path::Path::new(":memory:")).unwrap();
    let collection_id = db.create_collection("Clipboard 2026.05.30 14:35").unwrap();

    db.set_collection_settings_json(
        &collection_id,
        r#"{"source":"clipboard_monitor","capture_dir":"/tmp/cull"}"#,
    ).unwrap();

    let stored = db.get_collection_settings_json(&collection_id).unwrap();
    assert_eq!(
        stored.as_deref(),
        Some(r#"{"source":"clipboard_monitor","capture_dir":"/tmp/cull"}"#)
    );
}
```

- [ ] **Step 6: Run DB test to verify it fails**

Run:

```bash
cd src-tauri && cargo test db_core::db::tests::test_collection_settings_json_round_trips --lib
```

Expected: compile failure because helper methods do not exist.

- [ ] **Step 7: Implement DB helper methods**

Add near the collection methods in `src-tauri/src/db_core/db.rs`:

```rust
pub fn set_collection_settings_json(&self, collection_id: &str, settings_json: &str) -> Result<()> {
    let conn = self.conn.lock();
    conn.execute(
        "UPDATE projects SET settings_json = ?2 WHERE id = ?1",
        params![collection_id, settings_json],
    )?;
    Ok(())
}

pub fn get_collection_settings_json(&self, collection_id: &str) -> Result<Option<String>> {
    let conn = self.conn.lock();
    let mut stmt = conn.prepare("SELECT settings_json FROM projects WHERE id = ?1")?;
    let mut rows = stmt.query_map(params![collection_id], |row| row.get(0))?;
    match rows.next() {
        Some(Ok(value)) => Ok(value),
        Some(Err(err)) => Err(err),
        None => Ok(None),
    }
}
```

- [ ] **Step 8: Run helper tests**

Run:

```bash
cd src-tauri && cargo test services::clipboard_monitor::tests::filename_ db_core::db::tests::test_collection_settings_json_round_trips --lib
```

Expected: all selected tests pass.

- [ ] **Step 9: Commit Task 1**

Run:

```bash
git add src-tauri/src/services/mod.rs src-tauri/src/services/clipboard_monitor.rs src-tauri/src/db_core/db.rs
git commit -m "feat(clipboard): add monitor helper primitives"
```

## Task 2: Backend Capture Session Core With Fake Reader

**Files:**
- Modify: `src-tauri/src/services/clipboard_monitor.rs`
- Modify: `src-tauri/src/db_core/db.rs`

- [ ] **Step 1: Write failing capture-session tests**

Add to `src-tauri/src/services/clipboard_monitor.rs` tests:

```rust
use crate::db_core::db::Database;
use image::{ImageBuffer, Rgba};
use tempfile::tempdir;

fn png_bytes(color: [u8; 4]) -> Vec<u8> {
    let image: ImageBuffer<Rgba<u8>, Vec<u8>> = ImageBuffer::from_pixel(2, 2, Rgba(color));
    let mut bytes = Vec::new();
    image
        .write_to(&mut std::io::Cursor::new(&mut bytes), image::ImageFormat::Png)
        .unwrap();
    bytes
}

#[test]
fn start_session_creates_collection_and_metadata() {
    let db = Database::open(std::path::Path::new(":memory:")).unwrap();
    let tmp = tempdir().unwrap();
    let app_data = tmp.path().join("app-data");
    std::fs::create_dir_all(&app_data).unwrap();

    let session = create_monitor_session(&db, &app_data, None).unwrap();

    assert!(session.collection_name.starts_with("Clipboard "));
    let settings = db
        .get_collection_settings_json(&session.collection_id)
        .unwrap()
        .unwrap();
    assert!(settings.contains(r#""source":"clipboard_monitor""#));
    assert!(std::path::Path::new(&session.capture_dir).exists());
}

#[test]
fn capture_bytes_writes_file_imports_and_adds_to_collection() {
    let db = Database::open(std::path::Path::new(":memory:")).unwrap();
    let tmp = tempdir().unwrap();
    let app_data = tmp.path().join("app-data");
    std::fs::create_dir_all(&app_data).unwrap();
    let session = create_monitor_session(&db, &app_data, None).unwrap();
    let capture = ClipboardCapture {
        bytes: png_bytes([255, 0, 0, 255]),
        extension: "png".to_string(),
        original_filename: Some("Red Reference.png".to_string()),
        source_url: Some("https://www.pinterest.com/pin/red".to_string()),
        source_app: None,
        change_count: Some(1),
    };

    let result = capture_clipboard_image(&db, &app_data, &session, &capture, 1).unwrap();

    assert_eq!(result.imported, true);
    assert!(std::path::Path::new(&result.path).exists());
    assert!(result.path.ends_with("_red-reference_001.png"));
    let images = db.list_collection_images(&session.collection_id).unwrap();
    assert_eq!(images.len(), 1);
    assert_eq!(images[0].image.id, result.image_id.unwrap());
}

#[test]
fn duplicate_capture_hash_is_reported_as_existing_import() {
    let db = Database::open(std::path::Path::new(":memory:")).unwrap();
    let tmp = tempdir().unwrap();
    let app_data = tmp.path().join("app-data");
    std::fs::create_dir_all(&app_data).unwrap();
    let session = create_monitor_session(&db, &app_data, None).unwrap();
    let capture = ClipboardCapture {
        bytes: png_bytes([0, 255, 0, 255]),
        extension: "png".to_string(),
        original_filename: None,
        source_url: None,
        source_app: None,
        change_count: Some(1),
    };

    let first = capture_clipboard_image(&db, &app_data, &session, &capture, 1).unwrap();
    let second = capture_clipboard_image(&db, &app_data, &session, &capture, 2).unwrap();

    assert!(first.imported);
    assert!(!second.imported);
    assert_eq!(db.list_collection_images(&session.collection_id).unwrap().len(), 1);
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run:

```bash
cd src-tauri && cargo test services::clipboard_monitor::tests::start_session_creates_collection_and_metadata services::clipboard_monitor::tests::capture_bytes_writes_file_imports_and_adds_to_collection services::clipboard_monitor::tests::duplicate_capture_hash_is_reported_as_existing_import --lib
```

Expected: compile failure because session/capture functions do not exist.

- [ ] **Step 3: Implement session and capture core**

Add to `src-tauri/src/services/clipboard_monitor.rs`:

```rust
#[derive(Debug, Clone, Serialize)]
pub struct ClipboardMonitorSession {
    pub collection_id: String,
    pub collection_name: String,
    pub capture_dir: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct ClipboardCaptureResult {
    pub imported: bool,
    pub image_id: Option<String>,
    pub path: String,
    pub filename: String,
}

pub fn create_monitor_session(
    db: &crate::db_core::db::Database,
    app_data_dir: &Path,
    requested_capture_dir: Option<&str>,
) -> Result<ClipboardMonitorSession, String> {
    let capture_dir = resolve_capture_dir(db, app_data_dir, requested_capture_dir)?;
    std::fs::create_dir_all(&capture_dir)
        .map_err(|e| format!("Failed to create clipboard capture folder: {}", e))?;
    db.set_setting(CAPTURE_DIR_SETTING, &capture_dir.to_string_lossy())
        .map_err(|e| e.to_string())?;

    let now = Utc::now();
    let collection_name = format!("Clipboard {}", now.format("%Y.%m.%d %H:%M"));
    let collection_id = db.create_collection(&collection_name).map_err(|e| e.to_string())?;
    db.set_setting(LAST_COLLECTION_SETTING, &collection_id)
        .map_err(|e| e.to_string())?;
    let settings = serde_json::json!({
        "source": "clipboard_monitor",
        "capture_dir": capture_dir.to_string_lossy(),
        "started_at": now.to_rfc3339(),
    });
    db.set_collection_settings_json(&collection_id, &settings.to_string())
        .map_err(|e| e.to_string())?;

    Ok(ClipboardMonitorSession {
        collection_id,
        collection_name,
        capture_dir: capture_dir.to_string_lossy().to_string(),
    })
}

pub fn capture_clipboard_image(
    db: &crate::db_core::db::Database,
    app_data_dir: &Path,
    session: &ClipboardMonitorSession,
    capture: &ClipboardCapture,
    sequence: u32,
) -> Result<ClipboardCaptureResult, String> {
    let capture_dir = PathBuf::from(&session.capture_dir);
    std::fs::create_dir_all(&capture_dir)
        .map_err(|e| format!("Failed to create clipboard capture folder: {}", e))?;
    let filename = unique_capture_filename(&capture_dir, capture, sequence);
    let path = capture_dir.join(&filename);
    write_capture_file(&path, &capture.bytes)?;
    let imported_id = crate::db_core::import::import_file(db, &path, app_data_dir)?;
    if let Some(image_id) = imported_id.as_deref() {
        db.add_to_collection(&session.collection_id, &[image_id])
            .map_err(|e| e.to_string())?;
        let batch = db
            .create_import_batch("clipboard", 1, None)
            .map_err(|e| e.to_string())?;
        let _ = db.set_image_batch(image_id, &batch);
    }
    Ok(ClipboardCaptureResult {
        imported: imported_id.is_some(),
        image_id: imported_id,
        path: path.to_string_lossy().to_string(),
        filename,
    })
}

fn unique_capture_filename(capture_dir: &Path, capture: &ClipboardCapture, start_sequence: u32) -> String {
    let now = Utc::now();
    for sequence in start_sequence..start_sequence.saturating_add(1000) {
        let filename = build_clipboard_capture_filename(capture, now, sequence);
        if !capture_dir.join(&filename).exists() {
            return filename;
        }
    }
    build_clipboard_capture_filename(capture, now, start_sequence)
}

fn write_capture_file(path: &Path, bytes: &[u8]) -> Result<(), String> {
    let file_name = path
        .file_name()
        .ok_or_else(|| "Capture path has no file name".to_string())?
        .to_string_lossy();
    let tmp = path.with_file_name(format!(".{}.tmp", file_name));
    std::fs::write(&tmp, bytes).map_err(|e| format!("Failed to write clipboard capture: {}", e))?;
    std::fs::rename(&tmp, path).map_err(|e| format!("Failed to finalize clipboard capture: {}", e))?;
    Ok(())
}
```

- [ ] **Step 4: Run capture tests**

Run:

```bash
cd src-tauri && cargo test services::clipboard_monitor::tests --lib
```

Expected: all clipboard monitor service tests pass.

- [ ] **Step 5: Commit Task 2**

Run:

```bash
git add src-tauri/src/services/clipboard_monitor.rs src-tauri/src/db_core/db.rs
git commit -m "feat(clipboard): capture images into monitor collections"
```

## Task 3: Tauri Commands, App State, And Permissions

**Files:**
- Create: `src-tauri/src/commands/clipboard_monitor.rs`
- Modify: `src-tauri/src/commands/mod.rs`
- Modify: `src-tauri/src/lib.rs`
- Modify: `src-tauri/permissions/app-read.toml`
- Modify: `src-tauri/permissions/app-file-access.toml`
- Modify: `src-tauri/permissions/app-export-publishing.toml`
- Modify: `src/lib/tauri-command-contract.test.ts`

- [ ] **Step 1: Write failing command contract expectations**

In `src/lib/tauri-command-contract.test.ts`, extend `splits high-risk commands into dedicated app capability groups`:

```ts
expect(commandPermissions.get('get_clipboard_monitor_status')).toEqual(['app-read']);
expect(commandPermissions.get('start_clipboard_monitor')).toEqual(['app-file-access']);
expect(commandPermissions.get('stop_clipboard_monitor')).toEqual(['app-file-access']);
expect(commandPermissions.get('set_clipboard_monitor_capture_dir')).toEqual(['app-file-access']);
expect(commandPermissions.get('move_clipboard_capture_folder')).toEqual(['app-file-access']);
expect(commandPermissions.get('publish_clipboard_collection')).toEqual(['app-export-publishing']);
```

- [ ] **Step 2: Run contract test to verify it fails**

Run:

```bash
npm test -- src/lib/tauri-command-contract.test.ts
```

Expected: failure because commands are not registered or permitted.

- [ ] **Step 3: Add command module**

Create `src-tauri/src/commands/clipboard_monitor.rs`:

```rust
use crate::services::clipboard_monitor::{
    create_monitor_session, resolve_capture_dir, ClipboardAccessStatus, ClipboardMonitorSession,
    ClipboardMonitorState,
};
use crate::AppState;
use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager, State};

#[derive(Debug, Clone, Serialize)]
pub struct ClipboardMonitorStatus {
    pub running: bool,
    pub supported: bool,
    pub access_status: String,
    pub collection_id: Option<String>,
    pub collection_name: Option<String>,
    pub capture_dir: String,
    pub captured_count: u32,
    pub last_error: Option<String>,
}

fn status_from_state(state: &AppState, monitor: &ClipboardMonitorState) -> ClipboardMonitorStatus {
    let capture_dir = monitor
        .capture_dir
        .clone()
        .or_else(|| resolve_capture_dir(&state.db, &state.app_data_dir, None).ok())
        .unwrap_or_else(|| crate::services::clipboard_monitor::default_capture_dir(&state.app_data_dir));
    ClipboardMonitorStatus {
        running: monitor.running,
        supported: cfg!(target_os = "macos"),
        access_status: if cfg!(target_os = "macos") { "supported" } else { "unsupported_platform" }.to_string(),
        collection_id: monitor.collection_id.clone(),
        collection_name: monitor.collection_name.clone(),
        capture_dir: capture_dir.to_string_lossy().to_string(),
        captured_count: monitor.captured_count,
        last_error: monitor.last_error.clone(),
    }
}

#[tauri::command]
pub async fn get_clipboard_monitor_status(state: State<'_, AppState>) -> Result<ClipboardMonitorStatus, String> {
    let monitor = state.clipboard_monitor.lock();
    Ok(status_from_state(&state, &monitor))
}

#[tauri::command]
pub async fn start_clipboard_monitor(
    app: AppHandle,
    state: State<'_, AppState>,
    capture_dir: Option<String>,
) -> Result<ClipboardMonitorStatus, String> {
    if !cfg!(target_os = "macos") {
        let mut monitor = state.clipboard_monitor.lock();
        monitor.last_error = Some("Clipboard Monitor is not supported on this platform yet".to_string());
        return Ok(status_from_state(&state, &monitor));
    }

    {
        let monitor = state.clipboard_monitor.lock();
        if monitor.running {
            return Ok(status_from_state(&state, &monitor));
        }
    }

    let session = create_monitor_session(&state.db, &state.app_data_dir, capture_dir.as_deref())?;
    let capture_path = std::path::PathBuf::from(&session.capture_dir);
    // Capture directories are not added to asset protocol scope. The UI should
    // render app-owned thumbnails/generated previews, or an unavailable state.

    {
        let mut monitor = state.clipboard_monitor.lock();
        monitor.running = true;
        monitor.collection_id = Some(session.collection_id.clone());
        monitor.collection_name = Some(session.collection_name.clone());
        monitor.capture_dir = Some(capture_path);
        monitor.captured_count = 0;
        monitor.last_error = None;
    }

    let _ = app.emit("navigate-collection", serde_json::json!({ "collection_id": session.collection_id }));
    let monitor = state.clipboard_monitor.lock();
    Ok(status_from_state(&state, &monitor))
}

#[tauri::command]
pub async fn stop_clipboard_monitor(state: State<'_, AppState>) -> Result<ClipboardMonitorStatus, String> {
    let mut monitor = state.clipboard_monitor.lock();
    monitor.running = false;
    Ok(status_from_state(&state, &monitor))
}

#[tauri::command]
pub async fn set_clipboard_monitor_capture_dir(
    state: State<'_, AppState>,
    path: String,
) -> Result<ClipboardMonitorStatus, String> {
    let capture_dir = resolve_capture_dir(&state.db, &state.app_data_dir, Some(&path))?;
    state.db.set_setting(
        crate::services::clipboard_monitor::CAPTURE_DIR_SETTING,
        &capture_dir.to_string_lossy(),
    ).map_err(|e| e.to_string())?;
    let mut monitor = state.clipboard_monitor.lock();
    monitor.capture_dir = Some(capture_dir);
    Ok(status_from_state(&state, &monitor))
}

#[tauri::command]
pub async fn move_clipboard_capture_folder(
    state: State<'_, AppState>,
    new_path: String,
) -> Result<ClipboardMonitorStatus, String> {
    set_clipboard_monitor_capture_dir(state, new_path).await
}

#[tauri::command]
pub async fn publish_clipboard_collection(
    _app: AppHandle,
    _state: State<'_, AppState>,
    _collection_id: Option<String>,
) -> Result<serde_json::Value, String> {
    Err("Clipboard collection publishing is wired in the publishing task".to_string())
}
```

This starts with status/session wiring only; the real polling loop and publish body are added in later tasks.

- [ ] **Step 4: Wire module and app state**

In `src-tauri/src/commands/mod.rs`, add:

```rust
pub mod clipboard_monitor;
```

In `src-tauri/src/lib.rs`, add field:

```rust
pub clipboard_monitor: Mutex<services::clipboard_monitor::ClipboardMonitorState>,
```

Initialize in `app.manage(AppState { ... })`:

```rust
clipboard_monitor: Mutex::new(services::clipboard_monitor::ClipboardMonitorState::default()),
```

Add command handlers:

```rust
commands::clipboard_monitor::get_clipboard_monitor_status,
commands::clipboard_monitor::start_clipboard_monitor,
commands::clipboard_monitor::stop_clipboard_monitor,
commands::clipboard_monitor::set_clipboard_monitor_capture_dir,
commands::clipboard_monitor::move_clipboard_capture_folder,
commands::clipboard_monitor::publish_clipboard_collection,
```

- [ ] **Step 5: Add permissions**

In `src-tauri/permissions/app-read.toml`, add:

```toml
"get_clipboard_monitor_status",
```

In `src-tauri/permissions/app-file-access.toml`, add:

```toml
"move_clipboard_capture_folder",
"set_clipboard_monitor_capture_dir",
"start_clipboard_monitor",
"stop_clipboard_monitor",
```

In `src-tauri/permissions/app-export-publishing.toml`, add:

```toml
"publish_clipboard_collection",
```

- [ ] **Step 6: Run contract and Rust compile checks**

Run:

```bash
npm test -- src/lib/tauri-command-contract.test.ts
cd src-tauri && cargo test services::clipboard_monitor::tests --lib
```

Expected: frontend contract passes; Rust tests pass.

- [ ] **Step 7: Commit Task 3**

Run:

```bash
git add src-tauri/src/commands/clipboard_monitor.rs src-tauri/src/commands/mod.rs src-tauri/src/lib.rs src-tauri/permissions src/lib/tauri-command-contract.test.ts
git commit -m "feat(clipboard): expose monitor commands"
```

## Task 4: macOS Pasteboard Reader And Polling Loop

**Files:**
- Create: `src-tauri/src/services/clipboard_monitor_macos.rs`
- Modify: `src-tauri/src/services/mod.rs`
- Modify: `src-tauri/src/services/clipboard_monitor.rs`
- Modify: `src-tauri/src/commands/clipboard_monitor.rs`
- Modify: `src-tauri/Cargo.toml`

- [ ] **Step 1: Write failing trait tests with fake reader**

Add to `src-tauri/src/services/clipboard_monitor.rs`:

```rust
pub trait ClipboardImageReader: Send + 'static {
    fn status(&self) -> ClipboardAccessStatus;
    fn read_if_changed(&mut self) -> Result<Option<ClipboardCapture>, String>;
}
```

Add test:

```rust
struct FakeReader {
    captures: std::collections::VecDeque<ClipboardCapture>,
}

impl ClipboardImageReader for FakeReader {
    fn status(&self) -> ClipboardAccessStatus {
        ClipboardAccessStatus::Supported
    }

    fn read_if_changed(&mut self) -> Result<Option<ClipboardCapture>, String> {
        Ok(self.captures.pop_front())
    }
}

#[test]
fn process_reader_capture_skips_same_hash_twice() {
    let db = Database::open(std::path::Path::new(":memory:")).unwrap();
    let tmp = tempdir().unwrap();
    let app_data = tmp.path().join("app-data");
    std::fs::create_dir_all(&app_data).unwrap();
    let session = create_monitor_session(&db, &app_data, None).unwrap();
    let capture = ClipboardCapture {
        bytes: png_bytes([0, 0, 255, 255]),
        extension: "png".to_string(),
        original_filename: None,
        source_url: None,
        source_app: None,
        change_count: Some(2),
    };
    let mut state = ClipboardMonitorState::default();
    let mut reader = FakeReader { captures: vec![capture.clone(), capture].into() };

    let first = process_reader_once(&db, &app_data, &session, &mut state, &mut reader).unwrap();
    let second = process_reader_once(&db, &app_data, &session, &mut state, &mut reader).unwrap();

    assert!(first.is_some());
    assert!(second.is_none());
    assert_eq!(db.list_collection_images(&session.collection_id).unwrap().len(), 1);
}
```

- [ ] **Step 2: Run test to verify it fails**

Run:

```bash
cd src-tauri && cargo test services::clipboard_monitor::tests::process_reader_capture_skips_same_hash_twice --lib
```

Expected: compile failure because `process_reader_once` does not exist.

- [ ] **Step 3: Implement reader processing helper**

Add to `src-tauri/src/services/clipboard_monitor.rs`:

```rust
pub fn process_reader_once<R: ClipboardImageReader>(
    db: &crate::db_core::db::Database,
    app_data_dir: &Path,
    session: &ClipboardMonitorSession,
    state: &mut ClipboardMonitorState,
    reader: &mut R,
) -> Result<Option<ClipboardCaptureResult>, String> {
    let Some(capture) = reader.read_if_changed()? else {
        return Ok(None);
    };
    let hash = sha256_bytes(&capture.bytes);
    if state.last_hash.as_deref() == Some(hash.as_str()) {
        return Ok(None);
    }
    let sequence = state.captured_count.saturating_add(1);
    let result = capture_clipboard_image(db, app_data_dir, session, &capture, sequence)?;
    if result.imported {
        state.captured_count = state.captured_count.saturating_add(1);
        state.last_hash = Some(hash);
        state.last_change_count = capture.change_count;
    }
    Ok(Some(result))
}

fn sha256_bytes(bytes: &[u8]) -> String {
    use sha2::{Digest, Sha256};
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    format!("{:x}", hasher.finalize())
}
```

- [ ] **Step 4: Run fake reader test**

Run:

```bash
cd src-tauri && cargo test services::clipboard_monitor::tests::process_reader_capture_skips_same_hash_twice --lib
```

Expected: test passes.

- [ ] **Step 5: Add macOS adapter skeleton and unsupported fallback**

In `src-tauri/src/services/mod.rs`, add:

```rust
#[cfg(target_os = "macos")]
pub mod clipboard_monitor_macos;
```

Create `src-tauri/src/services/clipboard_monitor_macos.rs`:

```rust
use crate::services::clipboard_monitor::{
    ClipboardAccessStatus, ClipboardCapture, ClipboardImageReader,
};

pub struct MacPasteboardReader {
    last_change_count: i64,
}

impl MacPasteboardReader {
    pub fn new() -> Self {
        Self { last_change_count: -1 }
    }
}

impl ClipboardImageReader for MacPasteboardReader {
    fn status(&self) -> ClipboardAccessStatus {
        ClipboardAccessStatus::Supported
    }

    fn read_if_changed(&mut self) -> Result<Option<ClipboardCapture>, String> {
        read_macos_pasteboard_if_changed(&mut self.last_change_count)
    }
}

#[cfg(target_os = "macos")]
fn read_macos_pasteboard_if_changed(_last_change_count: &mut i64) -> Result<Option<ClipboardCapture>, String> {
    Ok(None)
}
```

- [ ] **Step 6: Replace skeleton with NSPasteboard read**

Add `NSPasteboard` and `NSPasteboardItem` to the existing macOS `objc2-app-kit` feature list in `src-tauri/Cargo.toml`:

```toml
objc2-app-kit = { version = "0.3", features = ["NSApplication", "NSImage", "NSPasteboard", "NSPasteboardItem", "NSResponder", "NSSharingService", "NSView", "NSWindow", "NSWorkspace"] }
```

Replace `src-tauri/src/services/clipboard_monitor_macos.rs` with:

```rust
use crate::services::clipboard_monitor::{
    ClipboardAccessStatus, ClipboardCapture, ClipboardImageReader,
};
use objc2_foundation::{NSData, NSString};
use objc2_app_kit::{
    NSPasteboard, NSPasteboardTypeFileURL, NSPasteboardTypeHTML, NSPasteboardTypePNG,
    NSPasteboardTypeString, NSPasteboardTypeTIFF, NSPasteboardTypeURL,
};

pub struct MacPasteboardReader {
    last_change_count: i64,
}

impl MacPasteboardReader {
    pub fn new() -> Self {
        Self { last_change_count: -1 }
    }
}

impl ClipboardImageReader for MacPasteboardReader {
    fn status(&self) -> ClipboardAccessStatus {
        ClipboardAccessStatus::Supported
    }

    fn read_if_changed(&mut self) -> Result<Option<ClipboardCapture>, String> {
        read_macos_pasteboard_if_changed(&mut self.last_change_count)
    }
}

fn read_macos_pasteboard_if_changed(last_change_count: &mut i64) -> Result<Option<ClipboardCapture>, String> {
    let pasteboard = NSPasteboard::generalPasteboard();
    let change_count = pasteboard.changeCount() as i64;
    if change_count == *last_change_count {
        return Ok(None);
    }
    *last_change_count = change_count;

    let source_url = read_string_for_type(&pasteboard, unsafe { NSPasteboardTypeURL })
        .or_else(|| read_string_for_type(&pasteboard, unsafe { NSPasteboardTypeString }))
        .or_else(|| extract_first_url(&read_string_for_type(&pasteboard, unsafe { NSPasteboardTypeHTML })?));

    if let Some(file_url) = read_string_for_type(&pasteboard, unsafe { NSPasteboardTypeFileURL }) {
        if let Some(path) = file_url.strip_prefix("file://") {
            let decoded = percent_decode_file_url(path);
            let path = std::path::PathBuf::from(decoded);
            if path.exists() {
                let bytes = std::fs::read(&path).map_err(|e| format!("Failed to read clipboard file URL: {}", e))?;
                let extension = path.extension().and_then(|ext| ext.to_str()).unwrap_or("png").to_string();
                let original_filename = path.file_name().map(|name| name.to_string_lossy().to_string());
                return Ok(Some(ClipboardCapture {
                    bytes,
                    extension,
                    original_filename,
                    source_url,
                    source_app: None,
                    change_count: Some(change_count),
                }));
            }
        }
    }

    if let Some(data) = pasteboard.dataForType(unsafe { NSPasteboardTypePNG }) {
        return Ok(Some(ClipboardCapture {
            bytes: nsdata_to_vec(&data),
            extension: "png".to_string(),
            original_filename: None,
            source_url,
            source_app: None,
            change_count: Some(change_count),
        }));
    }

    if let Some(data) = pasteboard.dataForType(unsafe { NSPasteboardTypeTIFF }) {
        return Ok(Some(ClipboardCapture {
            bytes: nsdata_to_vec(&data),
            extension: "tiff".to_string(),
            original_filename: None,
            source_url,
            source_app: None,
            change_count: Some(change_count),
        }));
    }

    Ok(None)
}

fn read_string_for_type(pasteboard: &NSPasteboard, ty: &NSString) -> Option<String> {
    pasteboard.stringForType(ty).map(|value| value.to_string())
}

fn nsdata_to_vec(data: &NSData) -> Vec<u8> {
    let len = data.length() as usize;
    let mut bytes = vec![0u8; len];
    if len > 0 {
        let ptr = std::ptr::NonNull::new(bytes.as_mut_ptr().cast()).expect("vec pointer is not null");
        unsafe { data.getBytes_length(ptr, len) };
    }
    bytes
}

fn extract_first_url(value: &str) -> Option<String> {
    value
        .split(|ch: char| ch.is_whitespace() || ch == '"' || ch == '\'')
        .find(|part| part.starts_with("http://") || part.starts_with("https://"))
        .map(|part| part.trim_end_matches(['<', '>', ')']).to_string())
}

fn percent_decode_file_url(value: &str) -> String {
    value.replace("%20", " ")
}
```

Keep all unsafe Objective-C calls inside `clipboard_monitor_macos.rs`. If `cargo test` reports a type mismatch on the generated `NSPasteboardType*` statics, fix the local helper signatures in this file only; do not move unsafe Objective-C calls into command code.

- [ ] **Step 7: Add polling loop to start command**

In `src-tauri/src/commands/clipboard_monitor.rs`, after session creation, spawn a guarded task on macOS:

```rust
#[cfg(target_os = "macos")]
{
    let app_clone = app.clone();
    let db = state.db.clone();
    let app_data_dir = state.app_data_dir.clone();
    let session = ClipboardMonitorSession {
        collection_id: session.collection_id.clone(),
        collection_name: session.collection_name.clone(),
        capture_dir: session.capture_dir.clone(),
    };
    crate::spawn_guarded(app.clone(), "clipboard-monitor", move || async move {
        let mut reader = crate::services::clipboard_monitor_macos::MacPasteboardReader::new();
        let mut interval = tokio::time::interval(std::time::Duration::from_millis(
            crate::services::clipboard_monitor::DEFAULT_POLL_MS,
        ));
        loop {
            interval.tick().await;
            let mut monitor = app_clone.state::<AppState>().clipboard_monitor.lock();
            if !monitor.running {
                break;
            }
            match crate::services::clipboard_monitor::process_reader_once(
                &db,
                &app_data_dir,
                &session,
                &mut monitor,
                &mut reader,
            ) {
                Ok(Some(result)) => {
                    let _ = app_clone.emit("clipboard-monitor:capture", &result);
                    let _ = app_clone.emit("images:changed", ());
                }
                Ok(None) => {}
                Err(error) => {
                    monitor.last_error = Some(error.clone());
                    let _ = app_clone.emit("clipboard-monitor:error", serde_json::json!({ "message": error }));
                }
            }
        }
    });
}
```

- [ ] **Step 8: Run backend tests**

Run:

```bash
cd src-tauri && cargo test services::clipboard_monitor::tests --lib
```

Expected: all service tests pass. On macOS, compile also proves the pasteboard adapter builds.

- [ ] **Step 9: Commit Task 4**

Run:

```bash
git add src-tauri/src/services src-tauri/src/commands/clipboard_monitor.rs src-tauri/Cargo.toml
git commit -m "feat(clipboard): monitor macOS pasteboard images"
```

## Task 5: Capture Folder Move With DB Path Updates

**Files:**
- Modify: `src-tauri/src/db_core/db.rs`
- Modify: `src-tauri/src/services/clipboard_monitor.rs`
- Modify: `src-tauri/src/commands/clipboard_monitor.rs`

- [ ] **Step 1: Write failing DB/service tests for moving captures**

Add test to `src-tauri/src/services/clipboard_monitor.rs`:

```rust
#[test]
fn move_capture_folder_copies_files_and_updates_paths() {
    let db = Database::open(std::path::Path::new(":memory:")).unwrap();
    let tmp = tempdir().unwrap();
    let app_data = tmp.path().join("app-data");
    let new_dir = tmp.path().join("moved-captures");
    std::fs::create_dir_all(&app_data).unwrap();
    let session = create_monitor_session(&db, &app_data, None).unwrap();
    let capture = ClipboardCapture {
        bytes: png_bytes([20, 30, 40, 255]),
        extension: "png".to_string(),
        original_filename: Some("Move Me.png".to_string()),
        source_url: None,
        source_app: None,
        change_count: Some(1),
    };
    let result = capture_clipboard_image(&db, &app_data, &session, &capture, 1).unwrap();

    move_capture_folder(&db, &session.capture_dir, &new_dir).unwrap();

    let moved = new_dir.join(std::path::Path::new(&result.path).file_name().unwrap());
    assert!(moved.exists());
    let image = db.get_images_by_ids(&[result.image_id.unwrap().as_str()]).unwrap().remove(0);
    assert_eq!(image.path, moved.to_string_lossy());
}
```

- [ ] **Step 2: Run test to verify it fails**

Run:

```bash
cd src-tauri && cargo test services::clipboard_monitor::tests::move_capture_folder_copies_files_and_updates_paths --lib
```

Expected: compile failure because `move_capture_folder` and DB path update helper do not exist.

- [ ] **Step 3: Add DB helper to list and update image file paths under a folder**

Add to `src-tauri/src/db_core/db.rs`:

```rust
pub fn list_image_files_under_path(&self, folder_path: &str) -> Result<Vec<(String, String)>> {
    let prefix = format!("{}/", folder_path.trim_end_matches('/'));
    let conn = self.conn.lock();
    let mut stmt = conn.prepare(
        "SELECT id, path FROM image_files WHERE path = ?1 OR path LIKE ?2",
    )?;
    let rows = stmt.query_map(params![folder_path, prefix + "%"], |row| {
        Ok((row.get(0)?, row.get(1)?))
    })?;
    rows.collect::<Result<Vec<_>>>()
}

pub fn update_image_file_path(&self, image_file_id: &str, new_path: &str) -> Result<()> {
    let conn = self.conn.lock();
    conn.execute(
        "UPDATE image_files SET path = ?2, last_seen_at = ?3, missing_at = NULL WHERE id = ?1",
        params![image_file_id, new_path, chrono::Utc::now().to_rfc3339()],
    )?;
    Ok(())
}
```

- [ ] **Step 4: Implement move service**

Add to `src-tauri/src/services/clipboard_monitor.rs`:

```rust
pub fn move_capture_folder(
    db: &crate::db_core::db::Database,
    old_dir: &str,
    new_dir: &Path,
) -> Result<(), String> {
    std::fs::create_dir_all(new_dir)
        .map_err(|e| format!("Failed to create destination capture folder: {}", e))?;
    let files = db.list_image_files_under_path(old_dir).map_err(|e| e.to_string())?;
    for (image_file_id, old_path) in files {
        let old = PathBuf::from(&old_path);
        if !old.exists() {
            continue;
        }
        let Some(file_name) = old.file_name() else {
            continue;
        };
        let new_path = new_dir.join(file_name);
        std::fs::copy(&old, &new_path)
            .map_err(|e| format!("Failed to copy clipboard capture: {}", e))?;
        let old_size = std::fs::metadata(&old).map_err(|e| e.to_string())?.len();
        let new_size = std::fs::metadata(&new_path).map_err(|e| e.to_string())?.len();
        if old_size != new_size {
            return Err(format!("Copied capture size mismatch for {}", old.display()));
        }
        db.update_image_file_path(&image_file_id, &new_path.to_string_lossy())
            .map_err(|e| e.to_string())?;
    }
    db.set_setting(CAPTURE_DIR_SETTING, &new_dir.to_string_lossy())
        .map_err(|e| e.to_string())?;
    Ok(())
}
```

- [ ] **Step 5: Wire command body**

Replace `move_clipboard_capture_folder` body in `src-tauri/src/commands/clipboard_monitor.rs`:

```rust
#[tauri::command]
pub async fn move_clipboard_capture_folder(
    app: AppHandle,
    state: State<'_, AppState>,
    new_path: String,
) -> Result<ClipboardMonitorStatus, String> {
    let new_dir = resolve_capture_dir(&state.db, &state.app_data_dir, Some(&new_path))?;
    let old_dir = {
        let monitor = state.clipboard_monitor.lock();
        monitor
            .capture_dir
            .as_ref()
            .map(|p| p.to_string_lossy().to_string())
            .or_else(|| state.db.get_setting(crate::services::clipboard_monitor::CAPTURE_DIR_SETTING).ok().flatten())
            .unwrap_or_else(|| crate::services::clipboard_monitor::default_capture_dir(&state.app_data_dir).to_string_lossy().to_string())
    };
    crate::services::clipboard_monitor::move_capture_folder(&state.db, &old_dir, &new_dir)?;
    // Moved capture directories remain outside asset protocol scope.
    let _ = app.emit("images:changed", ());
    let mut monitor = state.clipboard_monitor.lock();
    monitor.capture_dir = Some(new_dir);
    Ok(status_from_state(&state, &monitor))
}
```

- [ ] **Step 6: Run move tests**

Run:

```bash
cd src-tauri && cargo test services::clipboard_monitor::tests::move_capture_folder_copies_files_and_updates_paths --lib
```

Expected: test passes.

- [ ] **Step 7: Commit Task 5**

Run:

```bash
git add src-tauri/src/db_core/db.rs src-tauri/src/services/clipboard_monitor.rs src-tauri/src/commands/clipboard_monitor.rs
git commit -m "feat(clipboard): move capture folder references"
```

## Task 6: Collection Static Publishing Command

**Files:**
- Modify: `src-tauri/src/commands/static_publishing.rs`
- Modify: `src-tauri/src/commands/clipboard_monitor.rs`

- [ ] **Step 1: Write failing static publish collection test**

Add to `src-tauri/src/commands/static_publishing.rs` tests:

```rust
#[test]
fn export_static_publish_collection_uses_collection_images() {
    let (state, _tmp) = test_state();
    state.db.set_setting(MODULE_KEY, "true").unwrap();
    let image_id = insert_test_image(&state, "collection-source.png", 20, 20);
    let collection_id = state.db.create_collection("Clipboard References").unwrap();
    state.db.add_to_collection(&collection_id, &[&image_id]).unwrap();

    let result = export_static_publish_collection_inner(
        &state,
        collection_id.clone(),
        None,
        None,
    ).unwrap();

    assert_eq!(result.image_count, 1);
    assert!(std::path::PathBuf::from(&result.site_dir).join("data/canvas.json").exists());
}
```

- [ ] **Step 2: Run test to verify it fails**

Run:

```bash
cd src-tauri && cargo test commands::static_publishing::tests::export_static_publish_collection_uses_collection_images --lib
```

Expected: compile failure because `export_static_publish_collection_inner` does not exist.

- [ ] **Step 3: Implement collection publish helper**

Add to `src-tauri/src/commands/static_publishing.rs`:

```rust
pub fn export_static_publish_collection_inner(
    state: &AppState,
    collection_id: String,
    output_dir: Option<String>,
    share_url: Option<String>,
) -> Result<StaticPublishResult, String> {
    ensure_module_enabled(state)?;
    let collections = state.db.list_collections().map_err(|e| e.to_string())?;
    let collection_name = collections
        .iter()
        .find(|(id, _, _)| id == &collection_id)
        .map(|(_, name, _)| name.clone())
        .unwrap_or_else(|| "Clipboard Collection".to_string());
    let images = state.db.list_collection_images(&collection_id).map_err(|e| e.to_string())?;
    if images.is_empty() {
        return Err("Collection has no images to publish".to_string());
    }
    let items = images
        .into_iter()
        .map(|image| StaticPublishCanvasItem {
            image_id: image.image.id,
            x: None,
            y: None,
            width: None,
            height: None,
            hidden: None,
        })
        .collect();
    export_static_publish_package_inner(
        state,
        StaticPublishRequest {
            canvas_name: collection_name.clone(),
            items,
            layout_json: None,
            output_dir,
            share_url,
            site_title: Some(collection_name),
            site_description: Some("Cull clipboard reference collection".to_string()),
            indexable: false,
            links: Vec::new(),
            include_thumbnails: true,
            include_web: true,
            include_full: false,
        },
    )
}
```

- [ ] **Step 4: Implement clipboard publish command**

In `src-tauri/src/commands/clipboard_monitor.rs`, replace the temporary error body:

```rust
#[derive(Debug, Clone, Serialize)]
pub struct ClipboardPublishResult {
    pub collection_id: String,
    pub image_count: usize,
    pub site_dir: String,
    pub url: String,
    pub manifest_path: String,
    pub instructions_path: String,
}

#[tauri::command]
pub async fn publish_clipboard_collection(
    app: AppHandle,
    state: State<'_, AppState>,
    collection_id: Option<String>,
) -> Result<ClipboardPublishResult, String> {
    let collection_id = collection_id
        .or_else(|| state.clipboard_monitor.lock().collection_id.clone())
        .or_else(|| state.db.get_setting(crate::services::clipboard_monitor::LAST_COLLECTION_SETTING).ok().flatten())
        .ok_or_else(|| "No clipboard monitor collection is available".to_string())?;
    let export = crate::commands::static_publishing::export_static_publish_collection_inner(
        state.inner(),
        collection_id.clone(),
        None,
        None,
    )?;
    let server = crate::commands::static_publishing::serve_static_publish_package_inner(
        state.inner(),
        export.site_dir.clone(),
        Some("127.0.0.1".to_string()),
        None,
    ).await?;
    let result = ClipboardPublishResult {
        collection_id,
        image_count: export.image_count,
        site_dir: export.site_dir,
        url: server.url,
        manifest_path: export.manifest_path,
        instructions_path: export.instructions_path,
    };
    let _ = app.emit("clipboard-monitor:published", &result);
    state.db.set_setting("clipboard_monitor_last_publish", &serde_json::to_string(&result).unwrap_or_default())
        .map_err(|e| e.to_string())?;
    Ok(result)
}
```

- [ ] **Step 5: Run publish tests**

Run:

```bash
cd src-tauri && cargo test commands::static_publishing::tests::export_static_publish_collection_uses_collection_images --lib
```

Expected: test passes.

- [ ] **Step 6: Commit Task 6**

Run:

```bash
git add src-tauri/src/commands/static_publishing.rs src-tauri/src/commands/clipboard_monitor.rs
git commit -m "feat(clipboard): publish monitor collections"
```

## Task 7: Frontend API, State Helpers, And Navigation Listener

**Files:**
- Modify: `src/lib/api.ts`
- Create: `src/lib/clipboard-monitor.ts`
- Create: `src/lib/clipboard-monitor.test.ts`
- Modify: `src/lib/deeplink.ts`

- [ ] **Step 1: Write failing frontend tests**

Create `src/lib/clipboard-monitor.test.ts`:

```ts
import { beforeEach, describe, expect, it, vi } from 'vitest';
import { get } from 'svelte/store';
import { activeCollection, activeDetectedClass, activeFolder, activeSmartCollection, viewMode } from './stores';
import { applyClipboardMonitorCollection } from './clipboard-monitor';

vi.mock('./image-loading', () => ({
    loadImagesForCurrentScope: vi.fn().mockResolvedValue(undefined),
}));

describe('clipboard monitor frontend helpers', () => {
    beforeEach(() => {
        activeCollection.set(null);
        activeFolder.set('/old');
        activeSmartCollection.set({ id: 'smart', name: 'Smart', description: null, collection_type: 'smart', filter_json: '{}', nl_query: null, is_preset: false, sort_order: 0, created_at: 'now', image_count: 0 });
        activeDetectedClass.set('person');
        viewMode.set('loupe');
    });

    it('focuses monitor collection in grid and clears other scopes', async () => {
        await applyClipboardMonitorCollection('col_clip');

        expect(get(activeCollection)).toBe('col_clip');
        expect(get(activeFolder)).toBeNull();
        expect(get(activeSmartCollection)).toBeNull();
        expect(get(activeDetectedClass)).toBeNull();
        expect(get(viewMode)).toBe('grid');
    });
});
```

- [ ] **Step 2: Run test to verify it fails**

Run:

```bash
npm test -- src/lib/clipboard-monitor.test.ts
```

Expected: module not found failure for `./clipboard-monitor`.

- [ ] **Step 3: Add frontend API wrappers**

Add to `src/lib/api.ts`:

```ts
export interface ClipboardMonitorStatus {
    running: boolean;
    supported: boolean;
    access_status: string;
    collection_id: string | null;
    collection_name: string | null;
    capture_dir: string;
    captured_count: number;
    last_error: string | null;
}

export interface ClipboardPublishResult {
    collection_id: string;
    image_count: number;
    site_dir: string;
    url: string;
    manifest_path: string;
    instructions_path: string;
}

export async function getClipboardMonitorStatus(): Promise<ClipboardMonitorStatus> {
    return invoke('get_clipboard_monitor_status');
}

export async function startClipboardMonitor(captureDir?: string | null): Promise<ClipboardMonitorStatus> {
    return invoke('start_clipboard_monitor', { captureDir: captureDir ?? null });
}

export async function stopClipboardMonitor(): Promise<ClipboardMonitorStatus> {
    return invoke('stop_clipboard_monitor');
}

export async function setClipboardMonitorCaptureDir(path: string): Promise<ClipboardMonitorStatus> {
    return invoke('set_clipboard_monitor_capture_dir', { path });
}

export async function moveClipboardCaptureFolder(newPath: string): Promise<ClipboardMonitorStatus> {
    return invoke('move_clipboard_capture_folder', { newPath });
}

export async function publishClipboardCollection(collectionId?: string | null): Promise<ClipboardPublishResult> {
    return invoke('publish_clipboard_collection', { collectionId: collectionId ?? null });
}
```

- [ ] **Step 4: Add helper**

Create `src/lib/clipboard-monitor.ts`:

```ts
import {
    activeCollection,
    activeDetectedClass,
    activeFolder,
    activeSmartCollection,
    navigateTo,
} from './stores';
import { loadImagesForCurrentScope } from './image-loading';

export async function applyClipboardMonitorCollection(collectionId: string) {
    activeCollection.set(collectionId);
    activeFolder.set(null);
    activeSmartCollection.set(null);
    activeDetectedClass.set(null);
    navigateTo('grid');
    await loadImagesForCurrentScope({ force: true, invalidateCache: true });
}
```

- [ ] **Step 5: Add `navigate-collection` listener**

In `src/lib/deeplink.ts`, import helper:

```ts
import { applyClipboardMonitorCollection } from './clipboard-monitor';
```

In `initDeepLink`, after the `open-with-params` listener:

```ts
await listen<{ collection_id: string }>('navigate-collection', async (event) => {
    await applyClipboardMonitorCollection(event.payload.collection_id);
});
```

- [ ] **Step 6: Run frontend tests**

Run:

```bash
npm test -- src/lib/clipboard-monitor.test.ts src/lib/deeplink-integration.test.ts
```

Expected: tests pass.

- [ ] **Step 7: Commit Task 7**

Run:

```bash
git add src/lib/api.ts src/lib/clipboard-monitor.ts src/lib/clipboard-monitor.test.ts src/lib/deeplink.ts
git commit -m "feat(clipboard): focus monitor collections in grid"
```

## Task 8: Sidebar Controls And Browser Mock

**Files:**
- Modify: `src/lib/components/Sidebar.svelte`
- Modify: `src/lib/tauri-mock.ts`
- Create: `src/lib/clipboard-monitor-ui.test.ts`

- [ ] **Step 1: Write failing UI contract test**

Create `src/lib/clipboard-monitor-ui.test.ts`:

```ts
import { describe, expect, it } from 'vitest';
import { readFileSync } from 'node:fs';
import { join } from 'node:path';

const sidebar = readFileSync(join(process.cwd(), 'src/lib/components/Sidebar.svelte'), 'utf8');

describe('clipboard monitor sidebar UI contract', () => {
    it('renders operational clipboard monitor controls in the sidebar', () => {
        expect(sidebar).toContain('CLIPBOARD MONITOR');
        expect(sidebar).toContain('startClipboardMonitor');
        expect(sidebar).toContain('stopClipboardMonitor');
        expect(sidebar).toContain('publishClipboardCollection');
        expect(sidebar).toContain('navigator.clipboard.writeText');
    });
});
```

- [ ] **Step 2: Run test to verify it fails**

Run:

```bash
npm test -- src/lib/clipboard-monitor-ui.test.ts
```

Expected: sidebar does not contain monitor controls yet.

- [ ] **Step 3: Add mock command responses**

In `src/lib/tauri-mock.ts`, add handlers:

```ts
let clipboardMonitorStatus = {
  running: false,
  supported: true,
  access_status: 'supported',
  collection_id: null as string | null,
  collection_name: null as string | null,
  capture_dir: '/mock/clipboard-captures',
  captured_count: 0,
  last_error: null as string | null,
};
```

Inside the mock command map:

```ts
get_clipboard_monitor_status: () => clipboardMonitorStatus,
start_clipboard_monitor: () => {
  clipboardMonitorStatus = {
    ...clipboardMonitorStatus,
    running: true,
    collection_id: 'col_clipboard_mock',
    collection_name: 'Clipboard 2026.05.30 14:35',
  };
  return clipboardMonitorStatus;
},
stop_clipboard_monitor: () => {
  clipboardMonitorStatus = { ...clipboardMonitorStatus, running: false };
  return clipboardMonitorStatus;
},
set_clipboard_monitor_capture_dir: (_: any, args: { path: string }) => {
  clipboardMonitorStatus = { ...clipboardMonitorStatus, capture_dir: args.path };
  return clipboardMonitorStatus;
},
move_clipboard_capture_folder: (_: any, args: { newPath: string }) => {
  clipboardMonitorStatus = { ...clipboardMonitorStatus, capture_dir: args.newPath };
  return clipboardMonitorStatus;
},
publish_clipboard_collection: () => ({
  collection_id: clipboardMonitorStatus.collection_id ?? 'col_clipboard_mock',
  image_count: clipboardMonitorStatus.captured_count,
  site_dir: '/mock/static-publishing/clipboard/site',
  url: 'http://127.0.0.1:8000/',
  manifest_path: '/mock/static-publishing/clipboard/site/data/canvas.json',
  instructions_path: '/mock/static-publishing/clipboard/instructions/CLAUDE.md',
}),
```

- [ ] **Step 4: Add sidebar controls**

In `src/lib/components/Sidebar.svelte`, extend imports:

```ts
import {
    getClipboardMonitorStatus,
    startClipboardMonitor,
    stopClipboardMonitor,
    publishClipboardCollection,
    type ClipboardMonitorStatus,
    type ClipboardPublishResult,
} from '$lib/api';
import { applyClipboardMonitorCollection } from '$lib/clipboard-monitor';
```

Add state:

```ts
let clipboardStatus = $state<ClipboardMonitorStatus | null>(null);
let clipboardPublishing = $state(false);
let clipboardPublishResult = $state<ClipboardPublishResult | null>(null);
```

In `onMount`, load status:

```ts
try {
    clipboardStatus = await getClipboardMonitorStatus();
} catch (e) {
    console.error('Failed to load clipboard monitor status:', e);
}
```

Add handlers:

```ts
async function handleToggleClipboardMonitor() {
    try {
        clipboardStatus = clipboardStatus?.running
            ? await stopClipboardMonitor()
            : await startClipboardMonitor(null);
        if (clipboardStatus.collection_id) {
            await applyClipboardMonitorCollection(clipboardStatus.collection_id);
            collections.set(await listCollections());
        }
    } catch (e) {
        showToast('Clipboard Monitor failed', { detail: String(e), type: 'error', duration: 8000 });
    }
}

async function handlePublishClipboardCollection() {
    if (!clipboardStatus?.collection_id || clipboardPublishing) return;
    clipboardPublishing = true;
    try {
        clipboardPublishResult = await publishClipboardCollection(clipboardStatus.collection_id);
        try {
            await navigator.clipboard.writeText(clipboardPublishResult.url);
        } catch (e) {
            showToast('Published clipboard collection', { detail: `Copy failed: ${e}`, type: 'warning', duration: 8000 });
            return;
        }
        showToast('Published clipboard collection', { detail: clipboardPublishResult.url, type: 'success', duration: 10000 });
    } catch (e) {
        showToast('Publish failed', { detail: String(e), type: 'error', duration: 10000 });
    } finally {
        clipboardPublishing = false;
    }
}
```

Add markup above Collections:

```svelte
<div class="section clipboard-monitor">
    <div class="section-header">CLIPBOARD MONITOR</div>
    <button class="section-item" class:active={clipboardStatus?.running} onclick={handleToggleClipboardMonitor}>
        <span class="icon">{clipboardStatus?.running ? '■' : '▶'}</span>
        {clipboardStatus?.running ? 'Stop Monitor' : 'Monitor Clipboard'}
    </button>
    {#if clipboardStatus}
        <div class="section-meta">{clipboardStatus.access_status}</div>
        <div class="section-meta" title={clipboardStatus.capture_dir}>
            {clipboardStatus.capture_dir.split('/').pop() || clipboardStatus.capture_dir}
        </div>
        {#if clipboardStatus.collection_name}
            <div class="section-meta">{clipboardStatus.collection_name} · {clipboardStatus.captured_count}</div>
        {/if}
        <button
            class="section-item"
            onclick={handlePublishClipboardCollection}
            disabled={!clipboardStatus.collection_id || clipboardPublishing}
        >
            <span class="icon">↗</span>
            {clipboardPublishing ? 'Publishing...' : 'Publish Collection'}
        </button>
        {#if clipboardPublishResult}
            <div class="section-meta" title={clipboardPublishResult.url}>{clipboardPublishResult.url}</div>
        {/if}
    {/if}
</div>
```

Use existing `.section`, `.section-header`, `.section-item`, `.count`, `.active`; add `.section-meta` only if no equivalent exists, using CSS tokens.

- [ ] **Step 5: Run UI tests**

Run:

```bash
npm test -- src/lib/clipboard-monitor-ui.test.ts src/lib/tauri-command-contract.test.ts
```

Expected: tests pass.

- [ ] **Step 6: Commit Task 8**

Run:

```bash
git add src/lib/components/Sidebar.svelte src/lib/tauri-mock.ts src/lib/clipboard-monitor-ui.test.ts
git commit -m "feat(clipboard): add sidebar monitor controls"
```

## Task 9: MCP Tools

**Files:**
- Modify: `src-tauri/src/mcp/tools.rs`
- Modify: `src-tauri/src/services/tokens.rs`

- [ ] **Step 1: Write failing MCP capability tests**

In `src-tauri/src/mcp/tools.rs`, update tests:

```rust
#[test]
fn test_clipboard_monitor_tools_map_to_expected_capabilities() {
    assert_eq!(tokens::tool_capability("get_clipboard_monitor_status"), "library:read");
    assert_eq!(tokens::tool_capability("get_last_clipboard_publish"), "library:read");
    assert_eq!(tokens::tool_capability("show_clipboard_collection"), "display:navigate");
    assert_eq!(tokens::tool_capability("publish_clipboard_collection"), "export:read");
}

#[test]
fn test_clipboard_publish_tool_is_module_gated() {
    assert_eq!(
        super::required_module_for_tool("publish_clipboard_collection"),
        Some("module_static_publishing")
    );
}
```

- [ ] **Step 2: Run test to verify it fails**

Run:

```bash
cd src-tauri && cargo test mcp::tools::tests::test_clipboard_monitor_tools_map_to_expected_capabilities mcp::tools::tests::test_clipboard_publish_tool_is_module_gated --lib
```

Expected: capability assertions fail.

- [ ] **Step 3: Update capability mapping and module gate**

In `src-tauri/src/services/tokens.rs`, add:

```rust
"get_clipboard_monitor_status" | "get_last_clipboard_publish" => "library:read",
"publish_clipboard_collection" => "export:read",
"show_clipboard_collection" => "display:navigate",
```

In `src-tauri/src/mcp/tools.rs` `required_module_for_tool`, add:

```rust
| "publish_clipboard_collection"
```

- [ ] **Step 4: Add tool parameter structs**

Add near existing parameter structs in `src-tauri/src/mcp/tools.rs`:

```rust
#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
struct OptionalCollectionIdParams {
    collection_id: Option<String>,
}
```

- [ ] **Step 5: Add MCP tools**

Inside `impl CullMcp`, add:

```rust
#[tool(description = "Get Clipboard Monitor status and active collection ID")]
fn get_clipboard_monitor_status(&self, Parameters(_): Parameters<EmptyParams>) -> String {
    let state = self.app_handle.state::<AppState>();
    let monitor = state.clipboard_monitor.lock();
    serde_json::json!({
        "running": monitor.running,
        "collection_id": monitor.collection_id,
        "collection_name": monitor.collection_name,
        "captured_count": monitor.captured_count,
        "last_error": monitor.last_error,
    }).to_string()
}

#[tool(description = "Show the active Clipboard Monitor collection in the local app grid")]
fn show_clipboard_collection(&self, Parameters(_): Parameters<EmptyParams>) -> String {
    let state = self.app_handle.state::<AppState>();
    let collection_id = state
        .clipboard_monitor
        .lock()
        .collection_id
        .clone()
        .or_else(|| state.db.get_setting(crate::services::clipboard_monitor::LAST_COLLECTION_SETTING).ok().flatten());
    let Some(collection_id) = collection_id else {
        return "Error: No clipboard collection is available".to_string();
    };
    match crate::services::display::show_collection(&self.app_handle, &collection_id) {
        Ok(()) => serde_json::json!({"status":"ok","collection_id":collection_id}).to_string(),
        Err(e) => format!("Error: {}", e),
    }
}

#[tool(description = "Publish a Clipboard Monitor collection as a local static site and return the URL")]
async fn publish_clipboard_collection(
    &self,
    Parameters(params): Parameters<OptionalCollectionIdParams>,
) -> String {
    let state = self.app_handle.state::<AppState>();
    let collection_id = params
        .collection_id
        .or_else(|| state.clipboard_monitor.lock().collection_id.clone())
        .or_else(|| state.db.get_setting(crate::services::clipboard_monitor::LAST_COLLECTION_SETTING).ok().flatten());
    let Some(collection_id) = collection_id else {
        return "Error: No clipboard collection is available".to_string();
    };
    let export = match crate::commands::static_publishing::export_static_publish_collection_inner(
        state.inner(),
        collection_id.clone(),
        None,
        None,
    ) {
        Ok(result) => result,
        Err(e) => return format!("Error: {}", e),
    };
    let server = match crate::commands::static_publishing::serve_static_publish_package_inner(
        state.inner(),
        export.site_dir.clone(),
        Some("127.0.0.1".to_string()),
        None,
    ).await {
        Ok(result) => result,
        Err(e) => return format!("Error: {}", e),
    };
    let result = serde_json::json!({
        "collection_id": collection_id,
        "image_count": export.image_count,
        "site_dir": export.site_dir,
        "url": server.url,
        "manifest_path": export.manifest_path,
        "instructions_path": export.instructions_path,
    });
    let _ = state.db.set_setting("clipboard_monitor_last_publish", &result.to_string());
    result.to_string()
}

#[tool(description = "Return the last successful Clipboard Monitor publish result")]
fn get_last_clipboard_publish(&self, Parameters(_): Parameters<EmptyParams>) -> String {
    let state = self.app_handle.state::<AppState>();
    state
        .db
        .get_setting("clipboard_monitor_last_publish")
        .ok()
        .flatten()
        .unwrap_or_else(|| serde_json::json!({"status":"none"}).to_string())
}
```

- [ ] **Step 6: Run MCP tests**

Run:

```bash
cd src-tauri && cargo test mcp::tools::tests::test_clipboard_monitor_tools_map_to_expected_capabilities mcp::tools::tests::test_clipboard_publish_tool_is_module_gated --lib
```

Expected: tests pass.

- [ ] **Step 7: Commit Task 9**

Run:

```bash
git add src-tauri/src/mcp/tools.rs src-tauri/src/services/tokens.rs
git commit -m "feat(clipboard): expose monitor workflow through MCP"
```

## Task 10: Verification And Landing

**Files:**
- Potentially modify tests only if verification reveals real gaps.

- [ ] **Step 1: Run focused frontend tests**

Run:

```bash
npm test -- src/lib/clipboard-monitor.test.ts src/lib/clipboard-monitor-ui.test.ts src/lib/tauri-command-contract.test.ts src/lib/publishing-navigation-contract.test.ts
```

Expected: all selected frontend tests pass.

- [ ] **Step 2: Run focused Rust tests**

Run:

```bash
cd src-tauri && cargo test services::clipboard_monitor::tests commands::static_publishing::tests::export_static_publish_collection_uses_collection_images mcp::tools::tests::test_clipboard_monitor_tools_map_to_expected_capabilities mcp::tools::tests::test_clipboard_publish_tool_is_module_gated --lib
```

Expected: all selected Rust tests pass.

- [ ] **Step 3: Run broader quality gates**

Run:

```bash
npm run check
npm test
cd src-tauri && cargo test --lib
```

Expected: all commands pass. If `npm run check` reports pre-existing unrelated Svelte errors, capture the exact errors and run the narrower tests again after confirming the new files are not involved.

- [ ] **Step 4: Manual macOS verification**

Run the app and verify:

```bash
npm run tauri dev
```

Manual checks:

- Start monitor from sidebar.
- New collection appears and Grid focuses it.
- Copy an image from a browser.
- A readable PNG file appears in the capture folder.
- The image appears in the active collection.
- Publish collection shows a local URL.
- The URL is copied to the clipboard.
- MCP tools return status and last publish data.

- [ ] **Step 5: Commit verification fixes**

If verification required fixes:

```bash
git add src-tauri/src/services/clipboard_monitor.rs src-tauri/src/services/clipboard_monitor_macos.rs src-tauri/src/commands/clipboard_monitor.rs src-tauri/src/commands/static_publishing.rs src-tauri/src/mcp/tools.rs src-tauri/src/services/tokens.rs src-tauri/src/db_core/db.rs src-tauri/src/lib.rs src-tauri/src/commands/mod.rs src-tauri/permissions src/lib/api.ts src/lib/clipboard-monitor.ts src/lib/clipboard-monitor.test.ts src/lib/clipboard-monitor-ui.test.ts src/lib/deeplink.ts src/lib/components/Sidebar.svelte src/lib/tauri-mock.ts src/lib/tauri-command-contract.test.ts src-tauri/Cargo.toml
git commit -m "fix(clipboard): stabilize monitor workflow"
```

- [ ] **Step 6: Push final branch**

Run:

```bash
git pull --rebase
bd sync || (bd vc status && bd vc commit -m "Record clipboard monitor implementation handoff")
git push
git status --short --branch
```

Expected: branch is up to date with origin. If `bd sync` is unavailable or bead commands fail with the known `crystallizes` schema error, note the fallback and do not perform manual SQL repair.

## Spec Coverage Review

- Turn monitor on/off: Tasks 3, 4, 8.
- OS-level access and unsupported statuses: Tasks 3, 4, 8.
- New collection and Grid focus: Tasks 2, 3, 7, 8.
- Clipboard image import into collection: Tasks 2, 4.
- Readable date/time/source filenames: Task 1.
- Real files in configurable folder: Tasks 1, 2, 3, 8.
- Movable folder: Task 5.
- Static publishing with visible/copied URL: Tasks 6, 8.
- MCP availability: Task 9.
- Verification against Pinterest-style workflow: Task 10.
