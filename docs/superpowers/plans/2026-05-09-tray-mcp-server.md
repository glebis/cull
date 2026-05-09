# System Tray + MCP Server Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.
>
> **For Codex workers:** Opus 4.7 will be reviewing your code. Write clean, idiomatic Rust. Follow existing codebase patterns (rusqlite, Mutex<Connection>, Tauri 2 command conventions).

**Goal:** Add system tray support and an MCP server to ImageView so AI agents can read, curate, import, export, navigate, and search the image library over stdio, Unix socket, or HTTP/SSE with token-based auth.

**Architecture:** Extract a service layer from existing Tauri commands so both IPC and MCP call the same business logic. MCP server runs in-process sharing AppState. Three transports: Unix socket (always-on), stdio bridge (for Claude Code), HTTP/SSE (optional, for remote access). Capability-based auth with role presets (viewer/curator/operator/admin) and optional scope filters.

**Tech Stack:** Rust, Tauri 2, rmcp (Rust MCP SDK), tokio, hyper, rusqlite, serde_json

---

## File Structure

```
src-tauri/src/
  services/                    # NEW — pure business logic layer
    mod.rs                     # ServiceContext struct, error types, Pagination/PagedResult
    library.rs                 # list/get/filter/search images, folders, stats
    curation.rs                # ratings, decisions, collections, smart collections
    import.rs                  # folder/file import, rescan (no thumbnail regen — separate)
    export.rs                  # manifests, presets, PDF assembly
    ai.rs                      # embeddings, similarity, detection, vision
    display.rs                 # emit navigation events to frontend
    tokens.rs                  # token CRUD, hashing, capability checks, audit log
  mcp/                         # NEW — MCP server
    mod.rs                     # McpServer struct, startup, shutdown
    tools.rs                   # tool definitions and dispatch (rmcp #[tool] macros)
    auth.rs                    # token validation, capability enforcement, scope filtering
    http.rs                    # HTTP/SSE transport (hyper)
    socket.rs                  # Unix socket listener
  tray.rs                      # NEW — system tray setup, menu, event handling
  cli.rs                       # NEW — CLI arg parsing (--tray, --mcp-stdio, --mcp-http)
  lib.rs                       # MODIFY — integrate tray, MCP, CLI args
  main.rs                      # MODIFY — pass CLI args to run()
  commands/                    # EXISTING — refactor to call services
    library.rs                 # MODIFY
    selection.rs               # MODIFY
    collections.rs             # MODIFY
    import.rs                  # MODIFY
    export.rs                  # MODIFY
    detection.rs               # MODIFY (partially — keep download commands as-is)
    embeddings.rs              # MODIFY (partially — keep download commands as-is)
    smart_collections.rs       # MODIFY
    vision.rs                  # MODIFY
    window.rs                  # MODIFY (display service)
  db_core/
    db.rs                      # MODIFY — add mcp_tokens and mcp_audit_log tables
    models.rs                  # MODIFY — add McpToken, AuditEntry models
```

---

## Task 1: Service Layer Foundation — Types and ServiceContext

**Files:**
- Create: `src-tauri/src/services/mod.rs`
- Modify: `src-tauri/src/lib.rs:1` (add `mod services;`)

This task creates the shared types and the `ServiceContext` struct that all service modules and MCP tools use.

- [ ] **Step 1: Create `services/mod.rs` with core types**

```rust
// src-tauri/src/services/mod.rs
pub mod library;
pub mod curation;
pub mod import;
pub mod export;
pub mod ai;
pub mod display;
pub mod tokens;

use crate::db_core::db::Database;
use crate::db_core::embeddings::EmbeddingEngine;
use crate::db_core::detection::DetectionEngine;
use crate::db_core::secrets::SecretStore;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::sync::Mutex;

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Pagination {
    pub offset: u32,
    pub limit: u32,
}

impl Default for Pagination {
    fn default() -> Self {
        Self { offset: 0, limit: 50 }
    }
}

impl Pagination {
    pub fn clamped(offset: u32, limit: u32) -> Self {
        Self {
            offset,
            limit: limit.min(100).max(1),
        }
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct PagedResult<T> {
    pub items: Vec<T>,
    pub total: u32,
    pub offset: u32,
    pub has_more: bool,
}

#[derive(Debug, thiserror::Error)]
pub enum ServiceError {
    #[error("Database error: {0}")]
    Database(#[from] rusqlite::Error),
    #[error("Not found: {0}")]
    NotFound(String),
    #[error("Invalid input: {0}")]
    InvalidInput(String),
    #[error("Engine error: {0}")]
    Engine(String),
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
}

impl From<ServiceError> for String {
    fn from(e: ServiceError) -> String {
        e.to_string()
    }
}

pub struct ServiceContext<'a> {
    pub db: &'a Database,
    pub app_data_dir: &'a PathBuf,
    pub embedding_engine: &'a Mutex<EmbeddingEngine>,
    pub detection_engine: &'a Mutex<DetectionEngine>,
    pub safety_engine: &'a Mutex<DetectionEngine>,
    pub secrets: &'a dyn SecretStore,
    pub app_handle: Option<tauri::AppHandle>,
}

impl<'a> ServiceContext<'a> {
    pub fn from_app_state(
        state: &'a crate::AppState,
        app_handle: Option<tauri::AppHandle>,
    ) -> Self {
        Self {
            db: &state.db,
            app_data_dir: &state.app_data_dir,
            embedding_engine: &state.embedding_engine,
            detection_engine: &state.detection_engine,
            safety_engine: &state.safety_engine,
            secrets: state.secrets.as_ref(),
            app_handle,
        }
    }
}
```

- [ ] **Step 2: Add `mod services` to lib.rs**

In `src-tauri/src/lib.rs`, add `mod services;` after the existing module declarations:

```rust
mod commands;
mod db_core;
mod export;
mod menu;
mod services;  // ADD THIS LINE
```

- [ ] **Step 3: Add `thiserror` dependency**

In `src-tauri/Cargo.toml`, add:

```toml
thiserror = "1"
```

- [ ] **Step 4: Verify it compiles**

Run: `cd src-tauri && cargo check 2>&1 | tail -5`

Expected: compilation succeeds (the sub-modules are declared but can be empty files for now).

- [ ] **Step 5: Create stub files for all service modules**

Create empty files so the `mod` declarations in `services/mod.rs` resolve:

```bash
touch src-tauri/src/services/library.rs
touch src-tauri/src/services/curation.rs
touch src-tauri/src/services/import.rs
touch src-tauri/src/services/export.rs
touch src-tauri/src/services/ai.rs
touch src-tauri/src/services/display.rs
touch src-tauri/src/services/tokens.rs
```

- [ ] **Step 6: Verify it compiles again**

Run: `cd src-tauri && cargo check 2>&1 | tail -5`

Expected: PASS — all modules resolve, no errors.

- [ ] **Step 7: Commit**

```bash
git add src-tauri/src/services/ src-tauri/Cargo.toml src-tauri/src/lib.rs
git commit -m "feat: add service layer foundation — ServiceContext, Pagination, error types"
```

---

## Task 2: Library Service — Extract from commands/library.rs

**Files:**
- Modify: `src-tauri/src/services/library.rs`
- Modify: `src-tauri/src/commands/library.rs`

Extract the core logic (thumbnail path enrichment, listing, filtering) into the service layer. Commands become thin wrappers.

**Codex-eligible:** This is mechanical extraction. Opus 4.7 will review.

- [ ] **Step 1: Write tests for library service**

```rust
// src-tauri/src/services/library.rs
use crate::db_core::db::Database;
use crate::db_core::models::ImageWithFile;
use crate::db_core::thumbnails;
use crate::services::{Pagination, PagedResult, ServiceContext, ServiceError};
use std::path::PathBuf;

pub fn enrich_thumbnails(images: &mut [ImageWithFile], app_data_dir: &PathBuf) {
    for img in images.iter_mut() {
        let thumb = thumbnails::thumbnail_path(app_data_dir, &img.image.id);
        if thumb.exists() {
            img.thumbnail_path = Some(thumb.to_string_lossy().to_string());
        }
    }
}

pub fn list_images(
    ctx: &ServiceContext,
    page: Pagination,
) -> Result<Vec<ImageWithFile>, ServiceError> {
    let page = Pagination::clamped(page.offset, page.limit);
    let mut images = ctx.db.list_images(page.limit, page.offset)?;
    enrich_thumbnails(&mut images, ctx.app_data_dir);
    Ok(images)
}

pub fn list_images_by_folder(
    ctx: &ServiceContext,
    folder: &str,
    page: Pagination,
) -> Result<Vec<ImageWithFile>, ServiceError> {
    let page = Pagination::clamped(page.offset, page.limit);
    let mut images = ctx.db.list_images_by_folder(folder, page.limit, page.offset)?;
    enrich_thumbnails(&mut images, ctx.app_data_dir);
    Ok(images)
}

pub fn list_images_filtered(
    ctx: &ServiceContext,
    min_width: Option<u32>,
    min_height: Option<u32>,
    page: Pagination,
) -> Result<Vec<ImageWithFile>, ServiceError> {
    let page = Pagination::clamped(page.offset, page.limit);
    let mut images = ctx.db.list_images_filtered(min_width, min_height, page.limit, page.offset)?;
    enrich_thumbnails(&mut images, ctx.app_data_dir);
    Ok(images)
}

pub fn get_images_by_ids(
    ctx: &ServiceContext,
    image_ids: &[&str],
) -> Result<Vec<ImageWithFile>, ServiceError> {
    let mut images = ctx.db.get_images_by_ids(image_ids)?;
    enrich_thumbnails(&mut images, ctx.app_data_dir);
    Ok(images)
}

pub fn get_image(
    ctx: &ServiceContext,
    image_id: &str,
) -> Result<ImageWithFile, ServiceError> {
    let id_refs = vec![image_id];
    let mut images = ctx.db.get_images_by_ids(&id_refs)?;
    enrich_thumbnails(&mut images, ctx.app_data_dir);
    images.into_iter().next()
        .ok_or_else(|| ServiceError::NotFound(format!("Image '{}'", image_id)))
}

pub fn list_folders(ctx: &ServiceContext) -> Result<Vec<(String, u32)>, ServiceError> {
    Ok(ctx.db.list_folders()?)
}

pub fn get_image_count(ctx: &ServiceContext) -> Result<u32, ServiceError> {
    Ok(ctx.db.image_count()?)
}

pub fn get_iteration_siblings(
    ctx: &ServiceContext,
    parent_id: &str,
) -> Result<Vec<ImageWithFile>, ServiceError> {
    let mut images = ctx.db.get_iteration_siblings(parent_id)?;
    enrich_thumbnails(&mut images, ctx.app_data_dir);
    Ok(images)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::services::Pagination;

    #[test]
    fn test_pagination_clamped() {
        let p = Pagination::clamped(0, 200);
        assert_eq!(p.limit, 100);
        assert_eq!(p.offset, 0);

        let p = Pagination::clamped(10, 0);
        assert_eq!(p.limit, 1);
    }
}
```

- [ ] **Step 2: Run tests**

Run: `cd src-tauri && cargo test services::library::tests --lib 2>&1 | tail -10`

Expected: PASS

- [ ] **Step 3: Refactor commands/library.rs to use service**

```rust
// src-tauri/src/commands/library.rs
use tauri::State;
use crate::AppState;
use crate::db_core::models::ImageWithFile;
use crate::services::{Pagination, ServiceContext};
use crate::services::library as svc;

#[tauri::command]
pub async fn list_folders(state: State<'_, AppState>) -> Result<Vec<(String, u32)>, String> {
    let ctx = ServiceContext::from_app_state(&state, None);
    svc::list_folders(&ctx).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn list_images_by_folder(
    state: State<'_, AppState>,
    folder: String,
    limit: u32,
    offset: u32,
) -> Result<Vec<ImageWithFile>, String> {
    let ctx = ServiceContext::from_app_state(&state, None);
    svc::list_images_by_folder(&ctx, &folder, Pagination::clamped(offset, limit))
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn list_images(
    state: State<'_, AppState>,
    limit: u32,
    offset: u32,
) -> Result<Vec<ImageWithFile>, String> {
    let ctx = ServiceContext::from_app_state(&state, None);
    svc::list_images(&ctx, Pagination::clamped(offset, limit))
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn delete_folder(
    state: State<'_, AppState>,
    folder: String,
) -> Result<u32, String> {
    state.db.delete_images_by_folder(&folder).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn list_images_filtered(
    state: State<'_, AppState>,
    min_width: Option<u32>,
    min_height: Option<u32>,
    limit: u32,
    offset: u32,
) -> Result<Vec<ImageWithFile>, String> {
    let ctx = ServiceContext::from_app_state(&state, None);
    svc::list_images_filtered(&ctx, min_width, min_height, Pagination::clamped(offset, limit))
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn get_image_count(state: State<'_, AppState>) -> Result<u32, String> {
    let ctx = ServiceContext::from_app_state(&state, None);
    svc::get_image_count(&ctx).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn get_images_by_ids(
    state: State<'_, AppState>,
    image_ids: Vec<String>,
) -> Result<Vec<ImageWithFile>, String> {
    let ctx = ServiceContext::from_app_state(&state, None);
    let id_refs: Vec<&str> = image_ids.iter().map(|s| s.as_str()).collect();
    svc::get_images_by_ids(&ctx, &id_refs).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn get_iteration_siblings(
    state: State<'_, AppState>,
    parent_id: String,
) -> Result<Vec<ImageWithFile>, String> {
    let ctx = ServiceContext::from_app_state(&state, None);
    svc::get_iteration_siblings(&ctx, &parent_id).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn trash_images(
    state: State<'_, AppState>,
    image_ids: Vec<String>,
) -> Result<u32, String> {
    let mut trashed = 0u32;
    for image_id in &image_ids {
        let id_refs: Vec<&str> = vec![image_id.as_str()];
        let found = state.db.get_images_by_ids(&id_refs).map_err(|e| e.to_string())?;
        if let Some(img) = found.first() {
            #[cfg(target_os = "macos")]
            {
                let status = std::process::Command::new("osascript")
                    .args(["-e", &format!(
                        "tell application \"Finder\" to delete POSIX file \"{}\"",
                        img.path.replace('"', "\\\"")
                    )])
                    .output();
                if let Ok(output) = status {
                    if output.status.success() {
                        trashed += 1;
                    }
                }
            }
        }
    }
    Ok(trashed)
}

#[tauri::command]
pub async fn delete_images_permanently(
    state: State<'_, AppState>,
    image_ids: Vec<String>,
) -> Result<u32, String> {
    let mut deleted = 0u32;
    for image_id in &image_ids {
        let id_refs: Vec<&str> = vec![image_id.as_str()];
        let found = state.db.get_images_by_ids(&id_refs).map_err(|e| e.to_string())?;
        if let Some(img) = found.first() {
            let path = std::path::Path::new(&img.path);
            if path.exists() && std::fs::remove_file(path).is_ok() {
                deleted += 1;
            }
        }
    }
    Ok(deleted)
}

#[tauri::command]
pub async fn get_app_setting(
    state: State<'_, AppState>,
    key: String,
) -> Result<Option<String>, String> {
    state.db.get_setting(&key).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn set_app_setting(
    state: State<'_, AppState>,
    key: String,
    value: String,
) -> Result<(), String> {
    state.db.set_setting(&key, &value).map_err(|e| e.to_string())
}
```

- [ ] **Step 4: Verify it compiles and existing tests pass**

Run: `cd src-tauri && cargo test 2>&1 | tail -10`

Expected: PASS — all existing tests still work, commands are now thin wrappers.

- [ ] **Step 5: Commit**

```bash
git add src-tauri/src/services/library.rs src-tauri/src/commands/library.rs
git commit -m "refactor: extract library service from commands"
```

---

## Task 3: Curation Service — Extract from selection + collections + smart_collections

**Files:**
- Modify: `src-tauri/src/services/curation.rs`
- Modify: `src-tauri/src/commands/selection.rs`
- Modify: `src-tauri/src/commands/collections.rs`
- Modify: `src-tauri/src/commands/smart_collections.rs`

**Codex-eligible:** Mechanical extraction. Opus 4.7 will review.

- [ ] **Step 1: Write curation service**

```rust
// src-tauri/src/services/curation.rs
use crate::db_core::models::ImageWithFile;
use crate::db_core::smart_collections::SmartCollection;
use crate::services::{ServiceContext, ServiceError};
use crate::services::library::enrich_thumbnails;

pub fn set_rating(ctx: &ServiceContext, image_id: &str, rating: u8) -> Result<(), ServiceError> {
    Ok(ctx.db.set_rating(image_id, rating)?)
}

pub fn set_decision(ctx: &ServiceContext, image_id: &str, decision: &str) -> Result<(), ServiceError> {
    Ok(ctx.db.set_decision(image_id, decision)?)
}

pub fn create_collection(ctx: &ServiceContext, name: &str) -> Result<String, ServiceError> {
    Ok(ctx.db.create_collection(name)?)
}

pub fn list_collections(ctx: &ServiceContext) -> Result<Vec<(String, String, u32)>, ServiceError> {
    Ok(ctx.db.list_collections()?)
}

pub fn add_to_collection(ctx: &ServiceContext, collection_id: &str, image_ids: &[&str]) -> Result<(), ServiceError> {
    Ok(ctx.db.add_to_collection(collection_id, image_ids)?)
}

pub fn list_collection_images(ctx: &ServiceContext, collection_id: &str) -> Result<Vec<ImageWithFile>, ServiceError> {
    let mut images = ctx.db.list_collection_images(collection_id)?;
    enrich_thumbnails(&mut images, ctx.app_data_dir);
    Ok(images)
}

pub fn delete_collection(ctx: &ServiceContext, collection_id: &str) -> Result<(), ServiceError> {
    Ok(ctx.db.delete_collection(collection_id)?)
}

pub fn create_smart_collection(
    ctx: &ServiceContext,
    name: &str,
    filter_json: &str,
    nl_query: Option<&str>,
) -> Result<String, ServiceError> {
    Ok(ctx.db.create_smart_collection(name, filter_json, nl_query, false)?)
}

pub fn list_smart_collections(ctx: &ServiceContext) -> Result<Vec<SmartCollection>, ServiceError> {
    Ok(ctx.db.list_smart_collections()?)
}

pub fn evaluate_smart_collection(ctx: &ServiceContext, filter_json: &str) -> Result<Vec<ImageWithFile>, ServiceError> {
    Ok(ctx.db.evaluate_smart_collection(filter_json)?)
}

pub fn delete_smart_collection(ctx: &ServiceContext, id: &str) -> Result<(), ServiceError> {
    Ok(ctx.db.delete_smart_collection(id)?)
}

pub fn update_smart_collection(
    ctx: &ServiceContext,
    id: &str,
    name: &str,
    filter_json: &str,
    nl_query: Option<&str>,
) -> Result<(), ServiceError> {
    Ok(ctx.db.update_smart_collection(id, name, filter_json, nl_query)?)
}
```

- [ ] **Step 2: Refactor commands/selection.rs**

```rust
// src-tauri/src/commands/selection.rs
use tauri::State;
use crate::AppState;
use crate::services::ServiceContext;
use crate::services::curation as svc;

#[tauri::command]
pub async fn set_rating(state: State<'_, AppState>, image_id: String, rating: u8) -> Result<(), String> {
    let ctx = ServiceContext::from_app_state(&state, None);
    svc::set_rating(&ctx, &image_id, rating).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn set_decision(state: State<'_, AppState>, image_id: String, decision: String) -> Result<(), String> {
    let ctx = ServiceContext::from_app_state(&state, None);
    svc::set_decision(&ctx, &image_id, &decision).map_err(|e| e.to_string())
}
```

- [ ] **Step 3: Refactor commands/collections.rs**

```rust
// src-tauri/src/commands/collections.rs
use tauri::State;
use crate::AppState;
use crate::db_core::models::ImageWithFile;
use crate::services::ServiceContext;
use crate::services::curation as svc;

#[tauri::command]
pub async fn create_collection(state: State<'_, AppState>, name: String) -> Result<String, String> {
    let ctx = ServiceContext::from_app_state(&state, None);
    svc::create_collection(&ctx, &name).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn list_collections(state: State<'_, AppState>) -> Result<Vec<(String, String, u32)>, String> {
    let ctx = ServiceContext::from_app_state(&state, None);
    svc::list_collections(&ctx).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn add_to_collection(state: State<'_, AppState>, collection_id: String, image_ids: Vec<String>) -> Result<(), String> {
    let ctx = ServiceContext::from_app_state(&state, None);
    let refs: Vec<&str> = image_ids.iter().map(|s| s.as_str()).collect();
    svc::add_to_collection(&ctx, &collection_id, &refs).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn list_collection_images(state: State<'_, AppState>, collection_id: String) -> Result<Vec<ImageWithFile>, String> {
    let ctx = ServiceContext::from_app_state(&state, None);
    svc::list_collection_images(&ctx, &collection_id).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn delete_collection(state: State<'_, AppState>, collection_id: String) -> Result<(), String> {
    let ctx = ServiceContext::from_app_state(&state, None);
    svc::delete_collection(&ctx, &collection_id).map_err(|e| e.to_string())
}
```

- [ ] **Step 4: Refactor commands/smart_collections.rs**

```rust
// src-tauri/src/commands/smart_collections.rs
use tauri::State;
use crate::AppState;
use crate::db_core::smart_collections::SmartCollection;
use crate::db_core::models::ImageWithFile;
use crate::db_core::nl_parser::parse_query;
use crate::services::ServiceContext;
use crate::services::curation as svc;

#[tauri::command]
pub async fn create_smart_collection(
    state: State<'_, AppState>,
    name: String,
    filter_json: String,
    nl_query: Option<String>,
) -> Result<String, String> {
    let ctx = ServiceContext::from_app_state(&state, None);
    svc::create_smart_collection(&ctx, &name, &filter_json, nl_query.as_deref())
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn list_smart_collections(state: State<'_, AppState>) -> Result<Vec<SmartCollection>, String> {
    let ctx = ServiceContext::from_app_state(&state, None);
    svc::list_smart_collections(&ctx).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn evaluate_smart_collection(state: State<'_, AppState>, filter_json: String) -> Result<Vec<ImageWithFile>, String> {
    let ctx = ServiceContext::from_app_state(&state, None);
    svc::evaluate_smart_collection(&ctx, &filter_json).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn delete_smart_collection(state: State<'_, AppState>, id: String) -> Result<(), String> {
    let ctx = ServiceContext::from_app_state(&state, None);
    svc::delete_smart_collection(&ctx, &id).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn update_smart_collection(
    state: State<'_, AppState>,
    id: String,
    name: String,
    filter_json: String,
    nl_query: Option<String>,
) -> Result<(), String> {
    let ctx = ServiceContext::from_app_state(&state, None);
    svc::update_smart_collection(&ctx, &id, &name, &filter_json, nl_query.as_deref())
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn parse_nl_query(query: String) -> Result<String, String> {
    let filter = parse_query(&query);
    serde_json::to_string(&filter).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn backfill_image_metadata(state: State<'_, AppState>) -> Result<u32, String> {
    state.db.backfill_image_metadata().map_err(|e| e.to_string())
}
```

- [ ] **Step 5: Verify compilation and tests**

Run: `cd src-tauri && cargo test 2>&1 | tail -10`

Expected: PASS

- [ ] **Step 6: Commit**

```bash
git add src-tauri/src/services/curation.rs src-tauri/src/commands/selection.rs \
  src-tauri/src/commands/collections.rs src-tauri/src/commands/smart_collections.rs
git commit -m "refactor: extract curation service from selection/collection/smart_collection commands"
```

---

## Task 4: AI Service — Extract from embeddings + detection + vision

**Files:**
- Modify: `src-tauri/src/services/ai.rs`
- Modify: `src-tauri/src/commands/embeddings.rs` (partially — keep download commands)
- Modify: `src-tauri/src/commands/detection.rs` (partially — keep download commands)
- Modify: `src-tauri/src/commands/vision.rs`

**Codex-eligible:** Mechanical extraction. Opus 4.7 will review. Note: download commands stay in commands/ since they need AppHandle for progress events and are tightly coupled to Tauri.

- [ ] **Step 1: Write AI service**

```rust
// src-tauri/src/services/ai.rs
use crate::db_core::detection::Detection;
use crate::services::{ServiceContext, ServiceError};

pub fn find_similar_images(
    ctx: &ServiceContext,
    image_id: &str,
    top_k: usize,
    model: Option<&str>,
) -> Result<Vec<(String, f32)>, ServiceError> {
    let model_name = model.unwrap_or("clip-vit-b32");
    let all = ctx.db.get_all_embeddings(model_name)?;
    let query = all.iter().find(|(id, _)| id == image_id)
        .ok_or_else(|| ServiceError::NotFound("Image has no embedding".into()))?;
    Ok(ctx.db.find_similar(&query.1, model_name, top_k)?)
}

pub fn get_all_embeddings(
    ctx: &ServiceContext,
    model: Option<&str>,
) -> Result<Vec<(String, Vec<f32>)>, ServiceError> {
    let model_name = model.unwrap_or("clip-vit-b32");
    Ok(ctx.db.get_all_embeddings(model_name)?)
}

pub fn get_embedding_count(
    ctx: &ServiceContext,
    model: Option<&str>,
) -> Result<u32, ServiceError> {
    let model_name = model.unwrap_or("clip-vit-b32");
    Ok(ctx.db.embedding_count(model_name)?)
}

pub fn is_clip_available(ctx: &ServiceContext) -> Result<bool, ServiceError> {
    let engine = ctx.embedding_engine.lock().unwrap();
    Ok(engine.is_model_available())
}

pub fn get_detections(
    ctx: &ServiceContext,
    image_id: &str,
    model: Option<&str>,
) -> Result<Vec<Detection>, ServiceError> {
    Ok(ctx.db.get_detections(image_id, model)?)
}

pub fn search_by_detected_class(
    ctx: &ServiceContext,
    class_name: &str,
    limit: u32,
) -> Result<Vec<(String, f32)>, ServiceError> {
    Ok(ctx.db.search_by_class(class_name, limit)?)
}

pub fn get_detection_count(ctx: &ServiceContext, model: &str) -> Result<u32, ServiceError> {
    Ok(ctx.db.detection_count(model)?)
}

pub fn get_vision_metadata(
    ctx: &ServiceContext,
    image_id: &str,
) -> Result<Vec<(String, String, String)>, ServiceError> {
    Ok(ctx.db.get_vision_metadata(image_id)?)
}

pub fn get_vision_count(ctx: &ServiceContext, source: Option<&str>) -> Result<u32, ServiceError> {
    let src = source.unwrap_or("minicpm-v");
    Ok(ctx.db.count_vision_processed(src)?)
}
```

- [ ] **Step 2: Refactor commands that use these (embeddings.rs partial, detection.rs partial, vision.rs partial)**

Only refactor the simple query commands. Leave download/generate commands as-is since they need AppHandle for progress events.

In `commands/embeddings.rs`, refactor `get_all_embeddings`, `find_similar_images`, `is_model_available`, `get_embedding_count` to call `services::ai`. Leave `generate_embeddings`, `download_clip_model`, `set_api_key`, `get_api_key`, `validate_api_key`, `generate_gemini_embeddings` as-is.

In `commands/detection.rs`, refactor `get_detections`, `search_by_detected_class`, `get_detection_count` to call `services::ai`. Leave download and detect commands as-is.

In `commands/vision.rs`, refactor `get_vision_metadata`, `get_vision_count` to call `services::ai`. Leave `analyze_images`, `check_ollama`, `set_ollama_config`, `get_ollama_config` as-is.

- [ ] **Step 3: Verify compilation and tests**

Run: `cd src-tauri && cargo test 2>&1 | tail -10`

Expected: PASS

- [ ] **Step 4: Commit**

```bash
git add src-tauri/src/services/ai.rs src-tauri/src/commands/embeddings.rs \
  src-tauri/src/commands/detection.rs src-tauri/src/commands/vision.rs
git commit -m "refactor: extract AI service from embeddings/detection/vision commands"
```

---

## Task 5: Export and Display Services

**Files:**
- Modify: `src-tauri/src/services/export.rs`
- Modify: `src-tauri/src/services/display.rs`

**Codex-eligible:** Mechanical extraction. Opus 4.7 will review.

- [ ] **Step 1: Write export service**

```rust
// src-tauri/src/services/export.rs
use crate::export::presets;
use crate::commands::export::PresetInfo;
use crate::services::ServiceError;

pub fn list_presets() -> Vec<PresetInfo> {
    presets::PRESETS
        .iter()
        .map(|p| PresetInfo {
            id: p.id.to_string(),
            platform: p.platform.to_string(),
            format: p.format.to_string(),
            width: p.width,
            height: p.height,
            mime: p.mime.to_string(),
        })
        .collect()
}
```

- [ ] **Step 2: Write display service**

```rust
// src-tauri/src/services/display.rs
use crate::services::ServiceError;
use tauri::Emitter;

pub fn show_image(app_handle: &tauri::AppHandle, image_id: &str) -> Result<(), ServiceError> {
    let params = serde_json::json!({
        "path": null,
        "paths": null,
        "folder": null,
        "view": "loupe",
        "focus": image_id,
    });
    app_handle.emit("open-with-params", params)
        .map_err(|e| ServiceError::Engine(e.to_string()))
}

pub fn navigate_to_folder(app_handle: &tauri::AppHandle, folder_path: &str) -> Result<(), ServiceError> {
    let params = serde_json::json!({
        "folder": folder_path,
        "view": "grid",
    });
    app_handle.emit("open-with-params", params)
        .map_err(|e| ServiceError::Engine(e.to_string()))
}

pub fn show_collection(app_handle: &tauri::AppHandle, collection_id: &str) -> Result<(), ServiceError> {
    app_handle.emit("navigate-collection", serde_json::json!({
        "collection_id": collection_id,
    }))
    .map_err(|e| ServiceError::Engine(e.to_string()))
}
```

- [ ] **Step 3: Verify compilation**

Run: `cd src-tauri && cargo check 2>&1 | tail -5`

Expected: PASS

- [ ] **Step 4: Commit**

```bash
git add src-tauri/src/services/export.rs src-tauri/src/services/display.rs
git commit -m "refactor: add export and display services"
```

---

## Task 6: System Tray

**Files:**
- Create: `src-tauri/src/tray.rs`
- Create: `src-tauri/src/cli.rs`
- Modify: `src-tauri/src/lib.rs`
- Modify: `src-tauri/src/main.rs`
- Modify: `src-tauri/Cargo.toml`

- [ ] **Step 1: Add tray-icon feature to Cargo.toml**

In `src-tauri/Cargo.toml`, change the tauri dependency:

```toml
tauri = { version = "2", features = ["protocol-asset", "tray-icon"] }
```

Also add `clap` for CLI parsing:

```toml
clap = { version = "4", features = ["derive"] }
```

- [ ] **Step 2: Create CLI args parser**

```rust
// src-tauri/src/cli.rs
use clap::Parser;

#[derive(Parser, Debug, Clone)]
#[command(name = "imageview")]
pub struct CliArgs {
    /// Start in tray-only mode (no window)
    #[arg(long)]
    pub tray: bool,

    /// Run as MCP stdio bridge
    #[arg(long)]
    pub mcp_stdio: bool,

    /// Enable MCP HTTP/SSE server
    #[arg(long)]
    pub mcp_http: Option<Option<u16>>,

    /// HTTP listen host (default: 127.0.0.1)
    #[arg(long, default_value = "127.0.0.1")]
    pub mcp_http_host: String,
}
```

- [ ] **Step 3: Create tray module**

```rust
// src-tauri/src/tray.rs
use tauri::{
    tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent},
    menu::{Menu, MenuItem, PredefinedMenuItem},
    Manager, Emitter, AppHandle,
};

pub fn setup_tray(app: &AppHandle) -> tauri::Result<()> {
    let show_hide = MenuItem::with_id(app, "show_hide", "Show Window", true, None::<&str>)?;
    let stats = MenuItem::with_id(app, "stats", "Loading...", false, None::<&str>)?;
    let mcp_status = MenuItem::with_id(app, "mcp_status", "MCP: starting...", false, None::<&str>)?;
    let sep1 = PredefinedMenuItem::separator(app)?;
    let sep2 = PredefinedMenuItem::separator(app)?;
    let mcp_settings = MenuItem::with_id(app, "mcp_settings", "MCP Settings...", true, None::<&str>)?;
    let sep3 = PredefinedMenuItem::separator(app)?;
    let quit = MenuItem::with_id(app, "quit_app", "Quit ImageView", true, None::<&str>)?;

    let menu = Menu::with_items(app, &[
        &show_hide, &sep1, &stats, &mcp_status, &sep2, &mcp_settings, &sep3, &quit,
    ])?;

    let _tray = TrayIconBuilder::new()
        .icon(app.default_window_icon().unwrap().clone())
        .menu(&menu)
        .on_menu_event(move |app, event| {
            match event.id().0.as_str() {
                "show_hide" => toggle_window(app),
                "mcp_settings" => {
                    let _ = app.emit("menu-action", "settings");
                    if let Some(w) = app.get_webview_window("main") {
                        let _ = w.show();
                        let _ = w.set_focus();
                    }
                }
                "quit_app" => {
                    app.exit(0);
                }
                _ => {}
            }
        })
        .on_tray_icon_event(|tray, event| {
            if let TrayIconEvent::Click {
                button: MouseButton::Left,
                button_state: MouseButtonState::Up,
                ..
            } = event
            {
                toggle_window(tray.app_handle());
            }
        })
        .build(app)?;

    Ok(())
}

fn toggle_window(app: &AppHandle) {
    if let Some(window) = app.get_webview_window("main") {
        if window.is_visible().unwrap_or(false) {
            let _ = window.hide();
        } else {
            let _ = window.show();
            let _ = window.set_focus();
        }
    }
}

pub fn update_stats(app: &AppHandle, image_count: u32, mcp_connections: u32) {
    if let Some(tray) = app.tray_by_id("main") {
        // Tray menu items can be updated via app state events
        // For now, stats update happens through tray rebuild or menu item text update
    }
    // Stats updates will be connected when MCP server is integrated
}
```

- [ ] **Step 4: Integrate tray and CLI into lib.rs**

Modify `src-tauri/src/lib.rs`:

Add `mod tray;` and `mod cli;` declarations.

In the `run()` function:
1. Parse CLI args at the start.
2. If `--mcp-stdio`, run stdio bridge (Task 10) and return.
3. In `.setup()`, call `tray::setup_tray()`.
4. If `--tray` flag, skip showing the main window.
5. Handle close-to-tray in the `WindowEvent::CloseRequested` event.

```rust
// Add to the top of lib.rs
mod cli;
mod tray;

// In run():
pub fn run() {
    let args = <cli::CliArgs as clap::Parser>::parse();

    // MCP stdio bridge mode — no Tauri app, just bridge stdin/stdout to socket
    if args.mcp_stdio {
        // Will be implemented in Task 10
        eprintln!("MCP stdio bridge not yet implemented");
        std::process::exit(1);
    }

    let start_hidden = args.tray;

    tauri::Builder::default()
        // ... existing plugins ...
        .setup(move |app| {
            // ... existing setup code ...

            // Set up system tray
            tray::setup_tray(app.handle())?;

            // Hide window if --tray mode
            if start_hidden {
                if let Some(w) = app.get_webview_window("main") {
                    let _ = w.hide();
                }
            }

            Ok(())
        })
        // ... existing invoke_handler ...
        .build(tauri::generate_context!())
        .expect("error while building tauri application")
        .run(|app, event| {
            // Handle close-to-tray
            if let tauri::RunEvent::WindowEvent {
                event: tauri::WindowEvent::CloseRequested { api, .. },
                label,
                ..
            } = &event {
                if label == "main" {
                    let close_to_tray = app.state::<AppState>().db
                        .get_setting("close_to_tray")
                        .ok()
                        .flatten()
                        .map(|v| v == "true")
                        .unwrap_or(true); // default: close to tray

                    if close_to_tray {
                        api.prevent_close();
                        if let Some(w) = app.get_webview_window("main") {
                            let _ = w.hide();
                        }
                    }
                }
            }

            // ... existing event handling (Opened, DragDrop) ...
        });
}
```

- [ ] **Step 5: Update main.rs to pass args through**

No change needed — `clap::Parser::parse()` reads from `std::env::args()` directly in `lib.rs`.

- [ ] **Step 6: Verify compilation**

Run: `cd src-tauri && cargo check 2>&1 | tail -10`

Expected: PASS

- [ ] **Step 7: Test manually — normal launch and --tray launch**

Run normal: `cd src-tauri && cargo run`
Expected: app launches with tray icon visible, window shows normally.

Run tray-only: `cd src-tauri && cargo run -- --tray`
Expected: tray icon appears, no window visible. Click tray → "Show Window" reveals the window.

- [ ] **Step 8: Commit**

```bash
git add src-tauri/src/tray.rs src-tauri/src/cli.rs src-tauri/src/lib.rs \
  src-tauri/src/main.rs src-tauri/Cargo.toml
git commit -m "feat: add system tray with close-to-tray and --tray flag"
```

---

## Task 7: Token Auth — Database Schema + Service

**Files:**
- Modify: `src-tauri/src/db_core/db.rs` (add migration)
- Modify: `src-tauri/src/db_core/models.rs` (add token/audit types)
- Modify: `src-tauri/src/services/tokens.rs`
- Modify: `src-tauri/Cargo.toml` (add rand, subtle)

- [ ] **Step 1: Add dependencies**

In `src-tauri/Cargo.toml`:

```toml
rand = "0.8"
subtle = "2"
```

- [ ] **Step 2: Add token and audit models**

In `src-tauri/src/db_core/models.rs`, append:

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpToken {
    pub id: String,
    pub name: String,
    pub role: String,
    pub scope_json: Option<String>,
    pub created_at: String,
    pub expires_at: Option<String>,
    pub last_used_at: Option<String>,
    pub revoked: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Capability {
    pub name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuditEntry {
    pub id: i64,
    pub token_id: Option<String>,
    pub tool_name: String,
    pub params_json: Option<String>,
    pub result_status: String,
    pub timestamp: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TokenScope {
    pub collections: Option<Vec<String>>,
    pub folders: Option<Vec<String>>,
    pub tags: Option<Vec<String>>,
}
```

- [ ] **Step 3: Add database migration for mcp_tokens and mcp_audit_log**

In `src-tauri/src/db_core/db.rs`, add a new migration method called from `run_migrations`:

```rust
fn migrate_mcp_tables(&self) -> Result<()> {
    let conn = self.conn.lock().unwrap();
    conn.execute_batch("
        CREATE TABLE IF NOT EXISTS mcp_tokens (
            id TEXT PRIMARY KEY,
            name TEXT NOT NULL,
            secret_hash TEXT NOT NULL,
            role TEXT NOT NULL,
            scope_json TEXT,
            created_at TEXT NOT NULL,
            expires_at TEXT,
            last_used_at TEXT,
            revoked INTEGER DEFAULT 0
        );
        CREATE TABLE IF NOT EXISTS mcp_audit_log (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            token_id TEXT,
            tool_name TEXT NOT NULL,
            params_json TEXT,
            result_status TEXT NOT NULL,
            timestamp TEXT NOT NULL
        );
    ")?;
    Ok(())
}
```

Add `self.migrate_mcp_tables()?;` to the end of `run_migrations()`.

- [ ] **Step 4: Write token service**

```rust
// src-tauri/src/services/tokens.rs
use crate::db_core::models::{McpToken, AuditEntry, TokenScope};
use crate::services::{ServiceContext, ServiceError};
use rand::Rng;
use sha2::{Sha256, Digest};
use subtle::ConstantTimeEq;

const TOKEN_PREFIX: &str = "tok_";
const SECRET_LENGTH: usize = 32;

pub const ROLE_VIEWER: &str = "viewer";
pub const ROLE_CURATOR: &str = "curator";
pub const ROLE_OPERATOR: &str = "operator";
pub const ROLE_ADMIN: &str = "admin";

pub fn capabilities_for_role(role: &str) -> Vec<&'static str> {
    match role {
        ROLE_VIEWER => vec!["library:read", "library:search"],
        ROLE_CURATOR => vec!["library:read", "library:search", "curation:write", "export:read"],
        ROLE_OPERATOR => vec![
            "library:read", "library:search", "curation:write", "export:read",
            "import:write", "ai:run",
        ],
        ROLE_ADMIN => vec![
            "library:read", "library:search", "curation:write", "export:read",
            "import:write", "ai:run", "display:navigate", "tokens:manage", "settings:manage",
        ],
        _ => vec![],
    }
}

pub fn has_capability(role: &str, capability: &str) -> bool {
    capabilities_for_role(role).contains(&capability)
}

fn generate_token_id() -> String {
    let mut rng = rand::thread_rng();
    let chars: String = (0..12)
        .map(|_| {
            let idx = rng.gen_range(0..36);
            if idx < 10 { (b'0' + idx) as char } else { (b'a' + idx - 10) as char }
        })
        .collect();
    format!("{}{}", TOKEN_PREFIX, chars)
}

fn generate_secret() -> String {
    let mut rng = rand::thread_rng();
    let bytes: Vec<u8> = (0..SECRET_LENGTH).map(|_| rng.gen()).collect();
    hex::encode(bytes)
}

fn hash_secret(pepper: &str, secret: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(pepper.as_bytes());
    hasher.update(secret.as_bytes());
    hex::encode(hasher.finalize())
}

fn verify_secret(pepper: &str, secret: &str, stored_hash: &str) -> bool {
    let computed = hash_secret(pepper, secret);
    let a = computed.as_bytes();
    let b = stored_hash.as_bytes();
    if a.len() != b.len() { return false; }
    a.ct_eq(b).into()
}

fn get_pepper(ctx: &ServiceContext) -> Result<String, ServiceError> {
    let pepper = ctx.secrets.get("mcp_pepper")
        .map_err(|e| ServiceError::Engine(e))?;
    match pepper {
        Some(p) => Ok(p),
        None => {
            let new_pepper = generate_secret();
            ctx.secrets.set("mcp_pepper", &new_pepper)
                .map_err(|e| ServiceError::Engine(e))?;
            Ok(new_pepper)
        }
    }
}

/// Creates a new token. Returns (token_metadata, plaintext_secret).
/// The secret is shown once and never stored in plaintext.
pub fn create_token(
    ctx: &ServiceContext,
    name: &str,
    role: &str,
    scope: Option<TokenScope>,
) -> Result<(McpToken, String), ServiceError> {
    if !matches!(role, ROLE_VIEWER | ROLE_CURATOR | ROLE_OPERATOR | ROLE_ADMIN) {
        return Err(ServiceError::InvalidInput(format!("Invalid role: {}", role)));
    }

    let id = generate_token_id();
    let secret = generate_secret();
    let pepper = get_pepper(ctx)?;
    let secret_hash = hash_secret(&pepper, &secret);
    let scope_json = scope.as_ref()
        .map(|s| serde_json::to_string(s).unwrap());
    let now = chrono::Utc::now().to_rfc3339();

    let conn = ctx.db.conn.lock().unwrap();
    conn.execute(
        "INSERT INTO mcp_tokens (id, name, secret_hash, role, scope_json, created_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
        rusqlite::params![id, name, secret_hash, role, scope_json, now],
    )?;

    let token = McpToken {
        id,
        name: name.to_string(),
        role: role.to_string(),
        scope_json,
        created_at: now,
        expires_at: None,
        last_used_at: None,
        revoked: false,
    };

    Ok((token, secret))
}

/// Validates a secret and returns the associated token if valid.
pub fn validate_token(ctx: &ServiceContext, secret: &str) -> Result<Option<McpToken>, ServiceError> {
    let pepper = get_pepper(ctx)?;
    let conn = ctx.db.conn.lock().unwrap();

    let mut stmt = conn.prepare(
        "SELECT id, name, secret_hash, role, scope_json, created_at, expires_at, last_used_at, revoked FROM mcp_tokens WHERE revoked = 0"
    )?;

    let tokens: Vec<(String, String, String, String, Option<String>, String, Option<String>, Option<String>, bool)> = stmt.query_map([], |row| {
        Ok((
            row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?,
            row.get(4)?, row.get(5)?, row.get(6)?, row.get(7)?,
            row.get::<_, i32>(8)? != 0,
        ))
    })?.collect::<rusqlite::Result<Vec<_>>>()?;

    drop(stmt);

    for (id, name, stored_hash, role, scope_json, created_at, expires_at, last_used_at, revoked) in tokens {
        if verify_secret(&pepper, secret, &stored_hash) {
            // Check expiration
            if let Some(ref exp) = expires_at {
                if let Ok(exp_time) = chrono::DateTime::parse_from_rfc3339(exp) {
                    if exp_time < chrono::Utc::now() {
                        return Ok(None);
                    }
                }
            }

            // Update last_used_at
            let now = chrono::Utc::now().to_rfc3339();
            let _ = conn.execute(
                "UPDATE mcp_tokens SET last_used_at = ?1 WHERE id = ?2",
                rusqlite::params![now, id],
            );

            return Ok(Some(McpToken {
                id, name, role, scope_json, created_at, expires_at,
                last_used_at: Some(now), revoked,
            }));
        }
    }

    Ok(None)
}

pub fn list_tokens(ctx: &ServiceContext) -> Result<Vec<McpToken>, ServiceError> {
    let conn = ctx.db.conn.lock().unwrap();
    let mut stmt = conn.prepare(
        "SELECT id, name, role, scope_json, created_at, expires_at, last_used_at, revoked FROM mcp_tokens WHERE revoked = 0"
    )?;
    let tokens = stmt.query_map([], |row| {
        Ok(McpToken {
            id: row.get(0)?,
            name: row.get(1)?,
            role: row.get(2)?,
            scope_json: row.get(3)?,
            created_at: row.get(4)?,
            expires_at: row.get(5)?,
            last_used_at: row.get(6)?,
            revoked: row.get::<_, i32>(7)? != 0,
        })
    })?.collect::<rusqlite::Result<Vec<_>>>()?;
    Ok(tokens)
}

pub fn revoke_token(ctx: &ServiceContext, token_id: &str) -> Result<(), ServiceError> {
    let conn = ctx.db.conn.lock().unwrap();
    let updated = conn.execute(
        "UPDATE mcp_tokens SET revoked = 1 WHERE id = ?1",
        rusqlite::params![token_id],
    )?;
    if updated == 0 {
        return Err(ServiceError::NotFound(format!("Token '{}'", token_id)));
    }
    Ok(())
}

pub fn rotate_token(ctx: &ServiceContext, token_id: &str) -> Result<String, ServiceError> {
    let pepper = get_pepper(ctx)?;
    let new_secret = generate_secret();
    let new_hash = hash_secret(&pepper, &new_secret);

    let conn = ctx.db.conn.lock().unwrap();
    let updated = conn.execute(
        "UPDATE mcp_tokens SET secret_hash = ?1 WHERE id = ?2 AND revoked = 0",
        rusqlite::params![new_hash, token_id],
    )?;
    if updated == 0 {
        return Err(ServiceError::NotFound(format!("Token '{}'", token_id)));
    }
    Ok(new_secret)
}

pub fn log_audit(
    ctx: &ServiceContext,
    token_id: Option<&str>,
    tool_name: &str,
    params_json: Option<&str>,
    result_status: &str,
) -> Result<(), ServiceError> {
    let now = chrono::Utc::now().to_rfc3339();
    let conn = ctx.db.conn.lock().unwrap();
    conn.execute(
        "INSERT INTO mcp_audit_log (token_id, tool_name, params_json, result_status, timestamp) VALUES (?1, ?2, ?3, ?4, ?5)",
        rusqlite::params![token_id, tool_name, params_json, result_status, now],
    )?;
    Ok(())
}

pub fn prune_audit_log(ctx: &ServiceContext, retention_days: u32) -> Result<u32, ServiceError> {
    let cutoff = chrono::Utc::now() - chrono::Duration::days(retention_days as i64);
    let conn = ctx.db.conn.lock().unwrap();
    let deleted = conn.execute(
        "DELETE FROM mcp_audit_log WHERE timestamp < ?1",
        rusqlite::params![cutoff.to_rfc3339()],
    )?;
    Ok(deleted as u32)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_capabilities_for_roles() {
        assert!(has_capability(ROLE_ADMIN, "tokens:manage"));
        assert!(!has_capability(ROLE_VIEWER, "curation:write"));
        assert!(has_capability(ROLE_CURATOR, "library:read"));
        assert!(has_capability(ROLE_OPERATOR, "import:write"));
        assert!(!has_capability(ROLE_CURATOR, "import:write"));
    }

    #[test]
    fn test_hash_verify() {
        let pepper = "test_pepper";
        let secret = "test_secret_123";
        let hash = hash_secret(pepper, secret);
        assert!(verify_secret(pepper, secret, &hash));
        assert!(!verify_secret(pepper, "wrong_secret", &hash));
        assert!(!verify_secret("wrong_pepper", secret, &hash));
    }

    #[test]
    fn test_generate_token_id() {
        let id = generate_token_id();
        assert!(id.starts_with("tok_"));
        assert_eq!(id.len(), 16); // "tok_" + 12 chars
    }
}
```

- [ ] **Step 5: Add hex dependency**

In `src-tauri/Cargo.toml`:

```toml
hex = "0.4"
sha2 = "0.10"  # already present
```

- [ ] **Step 6: Verify tests pass**

Run: `cd src-tauri && cargo test services::tokens::tests --lib 2>&1 | tail -10`

Expected: PASS

- [ ] **Step 7: Commit**

```bash
git add src-tauri/src/services/tokens.rs src-tauri/src/db_core/db.rs \
  src-tauri/src/db_core/models.rs src-tauri/Cargo.toml
git commit -m "feat: add token auth service with capability model, hashing, and audit log"
```

---

## Task 8: MCP Server Core — rmcp Integration + Unix Socket

**Files:**
- Create: `src-tauri/src/mcp/mod.rs`
- Create: `src-tauri/src/mcp/server.rs`
- Create: `src-tauri/src/mcp/tools.rs`
- Create: `src-tauri/src/mcp/socket.rs`
- Modify: `src-tauri/src/lib.rs` (add `mod mcp`, start socket server in setup)
- Modify: `src-tauri/Cargo.toml` (add rmcp)

This is the most complex task. It wires up rmcp with the service layer and starts a Unix socket listener.

- [ ] **Step 1: Add rmcp dependency**

In `src-tauri/Cargo.toml`:

```toml
rmcp = { version = "1", features = ["server", "transport-io", "transport-sse-server"] }
```

- [ ] **Step 2: Create MCP module structure**

```rust
// src-tauri/src/mcp/mod.rs
pub mod server;
pub mod tools;
pub mod socket;
pub mod auth;
pub mod http;
```

- [ ] **Step 3: Create MCP tools using rmcp macros**

This is the core MCP tool definitions. Each tool calls the service layer.

```rust
// src-tauri/src/mcp/tools.rs
use rmcp::tool;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::sync::Arc;
use crate::AppState;
use crate::services::{Pagination, ServiceContext};
use crate::services::{library, curation, ai, display};

pub struct ImageViewTools {
    pub state: Arc<AppState>,
    pub app_handle: Option<tauri::AppHandle>,
}

impl ImageViewTools {
    fn ctx(&self) -> ServiceContext<'_> {
        ServiceContext::from_app_state(&self.state, self.app_handle.clone())
    }
}

// Define tools using rmcp's #[tool] macro pattern.
// The exact API depends on rmcp version — adapt to rmcp 1.x conventions.
// Each tool function takes JSON params, calls service, returns JSON result.

impl ImageViewTools {
    pub fn list_images(&self, params: Value) -> Result<Value, String> {
        let offset = params.get("offset").and_then(|v| v.as_u64()).unwrap_or(0) as u32;
        let limit = params.get("limit").and_then(|v| v.as_u64()).unwrap_or(50) as u32;
        let ctx = self.ctx();
        let images = library::list_images(&ctx, Pagination::clamped(offset, limit))
            .map_err(|e| e.to_string())?;
        serde_json::to_value(images).map_err(|e| e.to_string())
    }

    pub fn get_image(&self, params: Value) -> Result<Value, String> {
        let image_id = params.get("image_id")
            .and_then(|v| v.as_str())
            .ok_or("missing image_id")?;
        let ctx = self.ctx();
        let image = library::get_image(&ctx, image_id).map_err(|e| e.to_string())?;
        serde_json::to_value(image).map_err(|e| e.to_string())
    }

    pub fn list_folders(&self, _params: Value) -> Result<Value, String> {
        let ctx = self.ctx();
        let folders = library::list_folders(&ctx).map_err(|e| e.to_string())?;
        serde_json::to_value(folders).map_err(|e| e.to_string())
    }

    pub fn get_library_stats(&self, _params: Value) -> Result<Value, String> {
        let ctx = self.ctx();
        let count = library::get_image_count(&ctx).map_err(|e| e.to_string())?;
        let folders = library::list_folders(&ctx).map_err(|e| e.to_string())?;
        let collections = curation::list_collections(&ctx).map_err(|e| e.to_string())?;
        Ok(serde_json::json!({
            "image_count": count,
            "folder_count": folders.len(),
            "collection_count": collections.len(),
        }))
    }

    pub fn set_rating(&self, params: Value) -> Result<Value, String> {
        let image_id = params.get("image_id").and_then(|v| v.as_str()).ok_or("missing image_id")?;
        let rating = params.get("rating").and_then(|v| v.as_u64()).ok_or("missing rating")? as u8;
        let ctx = self.ctx();
        curation::set_rating(&ctx, image_id, rating).map_err(|e| e.to_string())?;
        Ok(serde_json::json!({"status": "ok"}))
    }

    pub fn set_decision(&self, params: Value) -> Result<Value, String> {
        let image_id = params.get("image_id").and_then(|v| v.as_str()).ok_or("missing image_id")?;
        let decision = params.get("decision").and_then(|v| v.as_str()).ok_or("missing decision")?;
        let ctx = self.ctx();
        curation::set_decision(&ctx, image_id, decision).map_err(|e| e.to_string())?;
        Ok(serde_json::json!({"status": "ok"}))
    }

    pub fn create_collection(&self, params: Value) -> Result<Value, String> {
        let name = params.get("name").and_then(|v| v.as_str()).ok_or("missing name")?;
        let ctx = self.ctx();
        let id = curation::create_collection(&ctx, name).map_err(|e| e.to_string())?;
        Ok(serde_json::json!({"collection_id": id}))
    }

    pub fn list_collections(&self, _params: Value) -> Result<Value, String> {
        let ctx = self.ctx();
        let cols = curation::list_collections(&ctx).map_err(|e| e.to_string())?;
        let result: Vec<Value> = cols.iter().map(|(id, name, count)| {
            serde_json::json!({"id": id, "name": name, "image_count": count})
        }).collect();
        Ok(Value::Array(result))
    }

    pub fn search_images(&self, params: Value) -> Result<Value, String> {
        let query = params.get("query").and_then(|v| v.as_str()).ok_or("missing query")?;
        let limit = params.get("limit").and_then(|v| v.as_u64()).unwrap_or(10) as usize;
        let ctx = self.ctx();

        // Use CLIP text embedding to search
        let engine = ctx.embedding_engine.lock().unwrap();
        if engine.session.is_none() {
            return Err("CLIP model not loaded. Run generate_embeddings first.".into());
        }
        let text_embedding = engine.generate_text_embedding(query)
            .map_err(|e| format!("Text embedding error: {}", e))?;
        drop(engine);

        let results = ctx.db.find_similar(&text_embedding, "clip-vit-b32", limit)
            .map_err(|e| e.to_string())?;
        serde_json::to_value(results).map_err(|e| e.to_string())
    }

    pub fn find_similar(&self, params: Value) -> Result<Value, String> {
        let image_id = params.get("image_id").and_then(|v| v.as_str()).ok_or("missing image_id")?;
        let limit = params.get("limit").and_then(|v| v.as_u64()).unwrap_or(10) as usize;
        let ctx = self.ctx();
        let results = ai::find_similar_images(&ctx, image_id, limit, None)
            .map_err(|e| e.to_string())?;
        serde_json::to_value(results).map_err(|e| e.to_string())
    }

    pub fn show_image(&self, params: Value) -> Result<Value, String> {
        let image_id = params.get("image_id").and_then(|v| v.as_str()).ok_or("missing image_id")?;
        let handle = self.app_handle.as_ref().ok_or("No app handle — display not available")?;
        display::show_image(handle, image_id).map_err(|e| e.to_string())?;
        Ok(serde_json::json!({"status": "ok"}))
    }

    pub fn navigate_to_folder(&self, params: Value) -> Result<Value, String> {
        let folder = params.get("folder_path").and_then(|v| v.as_str()).ok_or("missing folder_path")?;
        let handle = self.app_handle.as_ref().ok_or("No app handle — display not available")?;
        display::navigate_to_folder(handle, folder).map_err(|e| e.to_string())?;
        Ok(serde_json::json!({"status": "ok"}))
    }
}
```

Note: The exact rmcp API for defining tools will need adaptation based on rmcp 1.x documentation. The tool definitions above show the logical structure — the wiring to rmcp's `#[tool]` macros or `ServerHandler` trait should be done during implementation after checking `rmcp` docs.

- [ ] **Step 4: Create Unix socket server**

```rust
// src-tauri/src/mcp/socket.rs
use std::path::PathBuf;
use std::sync::Arc;
use tokio::net::UnixListener;
use crate::AppState;

pub fn socket_path(app_data_dir: &PathBuf) -> PathBuf {
    app_data_dir.join("mcp.sock")
}

pub async fn start_socket_server(
    state: Arc<AppState>,
    app_handle: tauri::AppHandle,
    sock_path: PathBuf,
) -> Result<(), Box<dyn std::error::Error>> {
    // Remove stale socket
    if sock_path.exists() {
        let _ = std::fs::remove_file(&sock_path);
    }

    let listener = UnixListener::bind(&sock_path)?;

    // Set permissions to owner-only
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&sock_path, std::fs::Permissions::from_mode(0o600))?;
    }

    eprintln!("MCP socket server listening on {:?}", sock_path);

    loop {
        let (stream, _) = listener.accept().await?;
        let state = state.clone();
        let handle = app_handle.clone();

        tokio::spawn(async move {
            // Each connection gets an rmcp server session
            // Wire up stream (AsyncRead + AsyncWrite) to rmcp transport
            // The exact rmcp API for this will be adapted during implementation
            eprintln!("MCP client connected");

            // TODO: implement rmcp session over the Unix stream
            // This will use rmcp's transport-io feature
        });
    }
}
```

- [ ] **Step 5: Create server orchestrator**

```rust
// src-tauri/src/mcp/server.rs
use std::sync::Arc;
use crate::AppState;

pub fn start_mcp_server(
    state: Arc<AppState>,
    app_handle: tauri::AppHandle,
) {
    let app_data_dir = state.app_data_dir.clone();

    tokio::spawn(async move {
        let sock_path = super::socket::socket_path(&app_data_dir);
        if let Err(e) = super::socket::start_socket_server(
            state, app_handle, sock_path,
        ).await {
            eprintln!("MCP socket server error: {}", e);
        }
    });
}
```

- [ ] **Step 6: Create stub auth and http modules**

```rust
// src-tauri/src/mcp/auth.rs
// Will be implemented in Task 9

// src-tauri/src/mcp/http.rs
// Will be implemented in Task 11
```

- [ ] **Step 7: Integrate MCP server start into lib.rs setup**

In `lib.rs`, inside `.setup()`, after AppState is managed:

```rust
// Start MCP socket server
{
    let state: Arc<AppState> = Arc::new(/* need to share state */);
    mcp::server::start_mcp_server(state, app.handle().clone());
}
```

Note: AppState is currently owned by Tauri's state management. To share it with MCP, we need to wrap it in `Arc`. This requires changing `app.manage(AppState { ... })` to use `Arc<AppState>` and managing the Arc instead. This refactor affects how `State<'_, AppState>` works in commands — Tauri can manage `Arc<AppState>` and commands access it via `State<'_, Arc<AppState>>`.

- [ ] **Step 8: Refactor AppState to Arc-wrapped for sharing**

Change `lib.rs`:
- Wrap AppState creation in `Arc::new(...)`
- `app.manage(Arc::clone(&state_arc))`
- Commands that use `State<'_, AppState>` change to `State<'_, Arc<AppState>>`

This is a mechanical refactor across all command files. ServiceContext::from_app_state needs to accept `&Arc<AppState>` via `Deref`.

- [ ] **Step 9: Verify compilation**

Run: `cd src-tauri && cargo check 2>&1 | tail -15`

Expected: PASS (with warnings about unused MCP code)

- [ ] **Step 10: Commit**

```bash
git add src-tauri/src/mcp/ src-tauri/src/lib.rs src-tauri/Cargo.toml \
  src-tauri/src/commands/
git commit -m "feat: add MCP server core with Unix socket transport and tool definitions"
```

---

## Task 9: MCP Auth Middleware

**Files:**
- Modify: `src-tauri/src/mcp/auth.rs`

Wire token validation into MCP request handling. Unix socket requests bypass auth; HTTP requests require Bearer tokens.

- [ ] **Step 1: Implement auth middleware**

```rust
// src-tauri/src/mcp/auth.rs
use crate::db_core::models::{McpToken, TokenScope};
use crate::services::{ServiceContext, tokens};

#[derive(Debug, Clone)]
pub enum AuthContext {
    Local,
    Authenticated(McpToken),
}

impl AuthContext {
    pub fn has_capability(&self, capability: &str) -> bool {
        match self {
            AuthContext::Local => true,
            AuthContext::Authenticated(token) => tokens::has_capability(&token.role, capability),
        }
    }

    pub fn token(&self) -> Option<&McpToken> {
        match self {
            AuthContext::Local => None,
            AuthContext::Authenticated(t) => Some(t),
        }
    }

    pub fn token_id(&self) -> Option<&str> {
        self.token().map(|t| t.id.as_str())
    }
}

pub fn require_capability(auth: &AuthContext, capability: &str) -> Result<(), String> {
    if auth.has_capability(capability) {
        Ok(())
    } else {
        Err(format!("Permission denied: requires '{}'", capability))
    }
}

pub fn tool_capability(tool_name: &str) -> &'static str {
    match tool_name {
        "list_images" | "get_image" | "list_folders" | "list_folder_images"
        | "get_library_stats" | "get_detections" | "get_vision_metadata" => "library:read",

        "search_images" | "find_similar" | "search_by_object" => "library:search",

        "set_rating" | "set_decision" | "create_collection" | "add_to_collection"
        | "delete_collection" | "create_smart_collection" | "list_collections" => "curation:write",

        "import_folder" | "import_files" | "rescan_sources" => "import:write",

        "export_images" | "list_export_presets" | "assemble_pdf" => "export:read",

        "show_image" | "navigate_to_folder" | "show_collection" => "display:navigate",

        "generate_embeddings" | "analyze_images" => "ai:run",

        "create_token" | "list_tokens" | "revoke_token" | "rotate_token" => "tokens:manage",

        _ => "admin",
    }
}
```

- [ ] **Step 2: Add tests**

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tool_capabilities() {
        assert_eq!(tool_capability("list_images"), "library:read");
        assert_eq!(tool_capability("set_rating"), "curation:write");
        assert_eq!(tool_capability("show_image"), "display:navigate");
        assert_eq!(tool_capability("create_token"), "tokens:manage");
    }

    #[test]
    fn test_local_has_all_capabilities() {
        let auth = AuthContext::Local;
        assert!(auth.has_capability("tokens:manage"));
        assert!(auth.has_capability("anything"));
    }
}
```

- [ ] **Step 3: Run tests**

Run: `cd src-tauri && cargo test mcp::auth::tests --lib 2>&1 | tail -10`

Expected: PASS

- [ ] **Step 4: Commit**

```bash
git add src-tauri/src/mcp/auth.rs
git commit -m "feat: add MCP auth middleware with capability checks"
```

---

## Task 10: Stdio Bridge

**Files:**
- Modify: `src-tauri/src/lib.rs` (implement the --mcp-stdio branch)

The stdio bridge connects stdin/stdout to the Unix socket. It auto-launches the app in tray mode if not running.

- [ ] **Step 1: Implement stdio bridge in lib.rs**

In the `run()` function, the `if args.mcp_stdio` block:

```rust
if args.mcp_stdio {
    let rt = tokio::runtime::Runtime::new().expect("Failed to create tokio runtime");
    rt.block_on(async {
        let app_data_dir = dirs::data_dir()
            .expect("No data dir")
            .join("com.glebkalinin.imageview");

        let sock_path = mcp::socket::socket_path(&app_data_dir);

        // Try connecting to existing socket
        let stream = match tokio::net::UnixStream::connect(&sock_path).await {
            Ok(s) => s,
            Err(_) => {
                // Launch app in tray mode
                let exe = std::env::current_exe().expect("Can't find own executable");
                std::process::Command::new(&exe)
                    .arg("--tray")
                    .spawn()
                    .expect("Failed to launch app in tray mode");

                // Poll for socket readiness
                let mut attempts = 0;
                loop {
                    tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;
                    match tokio::net::UnixStream::connect(&sock_path).await {
                        Ok(s) => break s,
                        Err(_) if attempts < 20 => { attempts += 1; }
                        Err(e) => {
                            eprintln!("Failed to connect to MCP socket after 10s: {}", e);
                            std::process::exit(1);
                        }
                    }
                }
            }
        };

        // Bridge stdin/stdout <-> socket
        let (sock_read, sock_write) = tokio::io::split(stream);
        let stdin = tokio::io::stdin();
        let stdout = tokio::io::stdout();

        let to_socket = tokio::io::copy(&mut tokio::io::BufReader::new(stdin), &mut tokio::io::BufWriter::new(sock_write));
        let from_socket = tokio::io::copy(&mut tokio::io::BufReader::new(sock_read), &mut tokio::io::BufWriter::new(stdout));

        tokio::select! {
            r = to_socket => { if let Err(e) = r { eprintln!("stdin->socket error: {}", e); } }
            r = from_socket => { if let Err(e) = r { eprintln!("socket->stdout error: {}", e); } }
        }
    });
    return;
}
```

- [ ] **Step 2: Add dirs dependency**

In `src-tauri/Cargo.toml`:

```toml
dirs = "5"
```

- [ ] **Step 3: Verify compilation**

Run: `cd src-tauri && cargo check 2>&1 | tail -5`

Expected: PASS

- [ ] **Step 4: Commit**

```bash
git add src-tauri/src/lib.rs src-tauri/Cargo.toml
git commit -m "feat: add MCP stdio bridge with auto-launch"
```

---

## Task 11: HTTP/SSE Transport

**Files:**
- Modify: `src-tauri/src/mcp/http.rs`
- Modify: `src-tauri/Cargo.toml` (add hyper + http-body-util)

- [ ] **Step 1: Add hyper dependencies**

```toml
hyper = { version = "1", features = ["server", "http1"] }
hyper-util = { version = "0.1", features = ["tokio"] }
http-body-util = "0.1"
```

- [ ] **Step 2: Implement HTTP/SSE server**

```rust
// src-tauri/src/mcp/http.rs
use std::sync::Arc;
use std::net::SocketAddr;
use crate::AppState;
use crate::services::{ServiceContext, tokens};

pub async fn start_http_server(
    state: Arc<AppState>,
    app_handle: tauri::AppHandle,
    host: String,
    port: u16,
) -> Result<(), Box<dyn std::error::Error>> {
    let addr: SocketAddr = format!("{}:{}", host, port).parse()?;
    eprintln!("MCP HTTP/SSE server listening on {}", addr);

    // Implementation uses hyper to handle:
    // POST /mcp/message — JSON-RPC requests with Bearer auth
    // GET /mcp/sse — SSE stream for server-sent events
    // GET /health — health check endpoint

    // Rate limiting: track failed auth attempts per IP
    // CORS: restrictive by default

    // The actual implementation will adapt based on rmcp's SSE server transport feature
    // rmcp 1.x includes transport-sse-server which handles most of this

    Ok(())
}
```

- [ ] **Step 3: Integrate HTTP server start in lib.rs**

In the setup block, after MCP socket server start, conditionally start HTTP:

```rust
if let Some(port) = args.mcp_http {
    let port = port.unwrap_or(9847);
    let host = args.mcp_http_host.clone();
    let state_clone = Arc::clone(&state_arc);
    let handle_clone = app.handle().clone();
    tokio::spawn(async move {
        if let Err(e) = mcp::http::start_http_server(state_clone, handle_clone, host, port).await {
            eprintln!("MCP HTTP server error: {}", e);
        }
    });
}
```

Also check the `mcp_http_enabled` setting to auto-start if previously enabled.

- [ ] **Step 4: Verify compilation**

Run: `cd src-tauri && cargo check 2>&1 | tail -10`

Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add src-tauri/src/mcp/http.rs src-tauri/src/lib.rs src-tauri/Cargo.toml
git commit -m "feat: add MCP HTTP/SSE transport with token auth"
```

---

## Task 12: Frontend — MCP Settings UI

**Files:**
- Create: `src/lib/components/McpSettings.svelte`
- Modify: `src/lib/api.ts` (add MCP-related API calls)
- Modify: settings page to include MCP section

**Codex-eligible:** Svelte 5 component. Opus 4.7 will review. Must follow existing CSS design system (Tokyo Night dark theme, CSS variables from AGENTS.md).

- [ ] **Step 1: Add Tauri commands for token management**

In `src-tauri/src/commands/`, add a new `mcp.rs` module:

```rust
// src-tauri/src/commands/mcp.rs
use std::sync::Arc;
use tauri::State;
use crate::AppState;
use crate::db_core::models::{McpToken, TokenScope};
use crate::services::{ServiceContext, tokens};

#[tauri::command]
pub async fn create_mcp_token(
    state: State<'_, Arc<AppState>>,
    name: String,
    role: String,
    scope: Option<TokenScope>,
) -> Result<(McpToken, String), String> {
    let ctx = ServiceContext::from_app_state(&state, None);
    tokens::create_token(&ctx, &name, &role, scope).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn list_mcp_tokens(state: State<'_, Arc<AppState>>) -> Result<Vec<McpToken>, String> {
    let ctx = ServiceContext::from_app_state(&state, None);
    tokens::list_tokens(&ctx).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn revoke_mcp_token(state: State<'_, Arc<AppState>>, token_id: String) -> Result<(), String> {
    let ctx = ServiceContext::from_app_state(&state, None);
    tokens::revoke_token(&ctx, &token_id).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn rotate_mcp_token(state: State<'_, Arc<AppState>>, token_id: String) -> Result<String, String> {
    let ctx = ServiceContext::from_app_state(&state, None);
    tokens::rotate_token(&ctx, &token_id).map_err(|e| e.to_string())
}
```

Register in `commands/mod.rs` and `lib.rs` invoke_handler.

- [ ] **Step 2: Add API functions in api.ts**

```typescript
// In src/lib/api.ts — add these functions
export async function createMcpToken(name: string, role: string, scope?: TokenScope): Promise<[McpToken, string]> {
  return invoke('create_mcp_token', { name, role, scope });
}

export async function listMcpTokens(): Promise<McpToken[]> {
  return invoke('list_mcp_tokens');
}

export async function revokeMcpToken(tokenId: string): Promise<void> {
  return invoke('revoke_mcp_token', { tokenId });
}

export async function rotateMcpToken(tokenId: string): Promise<string> {
  return invoke('rotate_mcp_token', { tokenId });
}
```

- [ ] **Step 3: Create McpSettings.svelte component**

Build a settings panel with:
- Close-to-tray toggle
- HTTP/SSE server toggle + port
- Token list (name, role, last used, actions)
- Create token dialog
- Uses existing CSS design system variables

The component should use Svelte 5 runes (`$state()`, `$effect()`) and follow the existing component patterns.

- [ ] **Step 4: Integrate into settings page**

Add the MCP section to the existing settings UI.

- [ ] **Step 5: Test in dev server**

Run: `npm run dev` and navigate to settings.
Expected: MCP section visible, can create/revoke tokens, toggle settings.

- [ ] **Step 6: Commit**

```bash
git add src/lib/components/McpSettings.svelte src/lib/api.ts \
  src-tauri/src/commands/mcp.rs src-tauri/src/commands/mod.rs
git commit -m "feat: add MCP settings UI with token management"
```

---

## Task 13: Integration Testing

**Files:**
- Create: `src-tauri/tests/mcp_integration.rs`

- [ ] **Step 1: Write integration test for MCP over Unix socket**

```rust
// src-tauri/tests/mcp_integration.rs
// This test:
// 1. Opens a test database
// 2. Creates AppState
// 3. Starts Unix socket MCP server
// 4. Connects as a client
// 5. Sends JSON-RPC requests
// 6. Asserts responses

#[tokio::test]
async fn test_mcp_list_images() {
    // Setup test DB, AppState, start socket server on temp path
    // Connect, send initialize request
    // Send tools/call for list_images
    // Assert response contains expected structure
}

#[tokio::test]
async fn test_mcp_auth_required_for_http() {
    // Start HTTP server on random port
    // Send request without Bearer token
    // Assert 401 response
}
```

- [ ] **Step 2: Run integration tests**

Run: `cd src-tauri && cargo test --test mcp_integration 2>&1 | tail -15`

Expected: PASS

- [ ] **Step 3: Commit**

```bash
git add src-tauri/tests/mcp_integration.rs
git commit -m "test: add MCP integration tests for socket and auth"
```

---

## Task 14: Tunnel Documentation

**Files:**
- Create: `docs/mcp-remote-access.md`

- [ ] **Step 1: Write tunnel setup documentation**

```markdown
# Remote MCP Access

## Prerequisites
Enable HTTP/SSE in ImageView settings (or launch with `--mcp-http`).

## Tailscale (Recommended)
1. Install Tailscale on both machines
2. Access via `http://<tailscale-ip>:9847`
3. Create a token with appropriate role/scope

## Cloudflare Tunnel
cloudflared tunnel --url http://localhost:9847

## ngrok
ngrok http 9847
```

- [ ] **Step 2: Write Claude Code MCP config example**

Include the config snippet for `.claude/settings.json`:

```json
{
  "mcpServers": {
    "imageview": {
      "command": "/Applications/ImageView.app/Contents/MacOS/imageview",
      "args": ["--mcp-stdio"]
    }
  }
}
```

- [ ] **Step 3: Commit**

```bash
git add docs/mcp-remote-access.md
git commit -m "docs: add MCP remote access and Claude Code setup guide"
```

---

## Task 15: Final Integration + Smoke Test

- [ ] **Step 1: Full build check**

Run: `cd src-tauri && cargo build 2>&1 | tail -10`

Expected: PASS — full binary compiles.

- [ ] **Step 2: Run all tests**

Run: `cd src-tauri && cargo test 2>&1 | tail -20`

Expected: All tests pass.

- [ ] **Step 3: Manual smoke test**

1. Launch app normally — tray icon visible, window shows
2. Close window — app stays in tray (default behavior)
3. Click tray → Show Window — window reappears
4. Launch `imageview --mcp-stdio` — connects to running app's socket
5. Create a token via settings UI
6. Test HTTP endpoint with curl:
   ```bash
   curl -H "Authorization: Bearer <token>" http://localhost:9847/mcp/message \
     -d '{"jsonrpc":"2.0","method":"tools/call","params":{"name":"get_library_stats","arguments":{}},"id":1}'
   ```

- [ ] **Step 4: Commit any fixes**

```bash
git add -A
git commit -m "fix: integration fixes from smoke testing"
```
