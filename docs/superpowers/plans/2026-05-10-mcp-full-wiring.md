# MCP Full Wiring Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Wire every Tauri capability into the MCP server — import, export, AI, token management, audit logging — and eliminate all dead code by connecting the services layer.

**Architecture:** The MCP server (`src-tauri/src/mcp/`) already has 20 tools for library/curation/search/display. The services layer (`src-tauri/src/services/`) provides a `ServiceContext` abstraction over `AppState`. We add new MCP tools that call through the services layer, wire audit logging into `call_tool`, and make display tools use `services::display`. Token management tools are added for admin users.

**Tech Stack:** Rust, Tauri, rmcp (MCP SDK), rusqlite, serde, tauri events

---

### Task 1: Wire audit logging into MCP call_tool

**Files:**
- Modify: `src-tauri/src/mcp/tools.rs` (call_tool handler, lines 586-601)

The `call_tool` method in `ServerHandler` impl is where every MCP tool invocation passes through. We add audit logging here using `services::tokens::log_audit`.

- [ ] **Step 1: Add ServiceContext construction and audit logging to call_tool**

In `src-tauri/src/mcp/tools.rs`, update the `call_tool` method to log every tool invocation:

```rust
fn call_tool(
    &self,
    request: CallToolRequestParams,
    context: RequestContext<RoleServer>,
) -> impl std::future::Future<Output = Result<CallToolResult, ErrorData>> + Send + '_ {
    async move {
        let tool_name: &str = &request.name;

        if let Err(msg) = require_capability(&self.auth, tool_name) {
            // Log denied access
            self.log_tool_call(tool_name, None, "denied");
            return Err(ErrorData::invalid_request(msg, None));
        }

        let params_json = serde_json::to_string(&request.params).ok();
        let call_context = ToolCallContext::new(self, request, context);
        let result = self.tool_router.call(call_context).await;

        let status = if result.is_ok() { "ok" } else { "error" };
        self.log_tool_call(tool_name, params_json.as_deref(), status);

        result
    }
}
```

Also add a helper method to `ImageViewMcp`:

```rust
fn log_tool_call(&self, tool_name: &str, params_json: Option<&str>, status: &str) {
    let state = self.app_handle.state::<AppState>();
    let ctx = crate::services::ServiceContext::from_app_state(&state, Some(self.app_handle.clone()));
    let token_id = match &self.auth {
        AuthContext::Local => None,
        AuthContext::Authenticated(t) => Some(t.id.as_str()),
    };
    let _ = crate::services::tokens::log_audit(&ctx, token_id, tool_name, params_json, status);
}
```

- [ ] **Step 2: Add necessary imports at top of tools.rs**

Add `use crate::services::ServiceContext;` if not present (it's not currently imported).

- [ ] **Step 3: Build and verify**

Run: `cargo check --manifest-path src-tauri/Cargo.toml 2>&1 | grep -E "error|warning"`
Expected: No new errors. The `log_audit`, `AuditEntry`, and `ServiceContext` dead_code warnings should be gone.

- [ ] **Step 4: Commit**

```bash
git add src-tauri/src/mcp/tools.rs
git commit -m "feat(mcp): wire audit logging into every tool call"
```

---

### Task 2: Wire MCP display tools through services::display

**Files:**
- Modify: `src-tauri/src/mcp/tools.rs` (show_image, navigate_to_folder, show_collection methods)

The three display MCP tools currently inline their event emissions. Refactor them to call `services::display::*` instead.

- [ ] **Step 1: Replace show_image MCP tool body**

Replace the `show_image` tool method (currently lines 536-551) with:

```rust
#[tool(description = "Open an image in the loupe (fullscreen detail) view on the local app")]
fn show_image(&self, Parameters(params): Parameters<ShowImageParams>) -> String {
    match self.check_image_id_scope(&params.image_id) {
        Ok(false) => return "Error: Access denied — image outside token scope".to_string(),
        Err(e) => return format!("Error: {}", e),
        _ => {}
    }
    match crate::services::display::show_image(&self.app_handle, &params.image_id) {
        Ok(()) => serde_json::json!({"status": "ok", "action": "opened in loupe"}).to_string(),
        Err(e) => format!("Error: {}", e),
    }
}
```

- [ ] **Step 2: Replace navigate_to_folder MCP tool body**

```rust
#[tool(description = "Navigate the local app to a folder in grid view")]
fn navigate_to_folder(&self, Parameters(params): Parameters<NavigateToFolderParams>) -> String {
    let scope = self.token_scope();
    if !tokens::folder_in_scope(&scope, &params.folder_path) {
        return "Error: Access denied — folder outside token scope".to_string();
    }
    match crate::services::display::navigate_to_folder(&self.app_handle, &params.folder_path) {
        Ok(()) => serde_json::json!({"status": "ok", "action": "navigated to folder"}).to_string(),
        Err(e) => format!("Error: {}", e),
    }
}
```

- [ ] **Step 3: Replace show_collection MCP tool body**

```rust
#[tool(description = "Display a collection in the local app grid view")]
fn show_collection(&self, Parameters(params): Parameters<CollectionIdParams>) -> String {
    match crate::services::display::show_collection(&self.app_handle, &params.collection_id) {
        Ok(()) => serde_json::json!({"status": "ok", "action": "showing collection"}).to_string(),
        Err(e) => format!("Error: {}", e),
    }
}
```

- [ ] **Step 4: Remove the `use tauri::Emitter;` lines inside the old display methods**

The `use tauri::Emitter;` was imported inline inside each method. Since we no longer emit events directly, these can be removed. (The import at the top of `services/display.rs` handles it.)

- [ ] **Step 5: Build and verify**

Run: `cargo check --manifest-path src-tauri/Cargo.toml 2>&1 | grep -E "error|warning"`
Expected: No errors. `services::display` dead_code warnings gone.

- [ ] **Step 6: Commit**

```bash
git add src-tauri/src/mcp/tools.rs
git commit -m "refactor(mcp): display tools call services::display instead of inlining"
```

---

### Task 3: Add import MCP tools

**Files:**
- Modify: `src-tauri/src/mcp/tools.rs` (add import_folder and rescan_sources tools)

The Tauri commands `import_folder` and `rescan_sources` exist but aren't exposed via MCP. The capability mapping in `services/tokens.rs` already maps `import_folder` → `import:write`.

- [ ] **Step 1: Add param structs**

Add to `tools.rs` after the existing param structs:

```rust
#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
pub struct ImportFolderParams {
    #[schemars(description = "Absolute path to folder to import")]
    pub folder_path: String,
}
```

- [ ] **Step 2: Add import_folder MCP tool**

Add inside the `#[tool_router] impl ImageViewMcp` block:

```rust
#[tool(description = "Import all images from a folder into the library. Returns count of imported/skipped/errors.")]
fn import_folder(&self, Parameters(params): Parameters<ImportFolderParams>) -> String {
    let state = self.app_handle.state::<AppState>();
    match crate::commands::import::do_import_folder(&state.db, &state.app_data_dir, &params.folder_path, &self.app_handle) {
        Ok(result) => serde_json::json!({
            "imported": result.imported,
            "skipped": result.skipped,
            "errors": result.errors.len(),
        }).to_string(),
        Err(e) => format!("Error: {}", e),
    }
}
```

Note: This requires `do_import_folder` to be a public function extracted from the Tauri command. Check if the import logic is already factored out; if the Tauri command directly contains the logic, extract it first.

- [ ] **Step 3: Check import command structure and extract if needed**

Read `src-tauri/src/commands/import.rs` to see if the import logic is in a reusable function or embedded in the Tauri command handler. If embedded, extract the core logic into a public function that both the Tauri command and MCP tool can call.

- [ ] **Step 4: Add rescan_sources MCP tool**

```rust
#[tool(description = "Rescan all imported source folders for new/changed/missing files")]
fn rescan_sources(&self, Parameters(_): Parameters<EmptyParams>) -> String {
    let state = self.app_handle.state::<AppState>();
    match state.db.rescan_sources(&state.app_data_dir) {
        Ok(stats) => serde_json::to_string(&stats).unwrap_or_else(|_| "{}".to_string()),
        Err(e) => format!("Error: {}", e),
    }
}
```

- [ ] **Step 5: Build and verify**

Run: `cargo check --manifest-path src-tauri/Cargo.toml 2>&1 | grep -E "error|warning"`

- [ ] **Step 6: Commit**

```bash
git add src-tauri/src/mcp/tools.rs src-tauri/src/commands/import.rs
git commit -m "feat(mcp): add import_folder and rescan_sources tools"
```

---

### Task 4: Add AI MCP tools (embeddings, detection, vision)

**Files:**
- Modify: `src-tauri/src/mcp/tools.rs`

Expose the AI capabilities: generate embeddings, run object detection, run vision analysis. The capability mapping already has `ai:run` for these.

- [ ] **Step 1: Add param structs**

```rust
#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
pub struct GenerateEmbeddingsParams {
    #[schemars(description = "List of image IDs to generate CLIP embeddings for. Pass empty array to process all unembedded images.")]
    pub image_ids: Vec<String>,
}

#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
pub struct DetectObjectsParams {
    #[schemars(description = "List of image IDs to run YOLO object detection on")]
    pub image_ids: Vec<String>,
    #[schemars(description = "YOLO variant: 'nano', 'small', or 'medium' (default: 'medium')")]
    pub variant: Option<String>,
}

#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
pub struct AnalyzeImagesParams {
    #[schemars(description = "List of image IDs to analyze with Ollama vision model")]
    pub image_ids: Vec<String>,
}
```

- [ ] **Step 2: Add generate_embeddings MCP tool**

```rust
#[tool(description = "Generate CLIP visual embeddings for images (required for find_similar). Returns count processed.")]
fn generate_embeddings(&self, Parameters(params): Parameters<GenerateEmbeddingsParams>) -> String {
    let state = self.app_handle.state::<AppState>();
    let mut engine = state.embedding_engine.lock().unwrap();
    match engine.generate_batch(&state.db, &params.image_ids) {
        Ok(count) => serde_json::json!({"processed": count}).to_string(),
        Err(e) => format!("Error: {}", e),
    }
}
```

Note: Check the actual `EmbeddingEngine` API — the method name and signature may differ. Read `src-tauri/src/db_core/embeddings.rs` to confirm.

- [ ] **Step 3: Add detect_objects MCP tool**

```rust
#[tool(description = "Run YOLO object detection on images. Returns count processed.")]
fn detect_objects(&self, Parameters(params): Parameters<DetectObjectsParams>) -> String {
    let state = self.app_handle.state::<AppState>();
    let variant = params.variant.as_deref().unwrap_or("medium");
    let mut engine = state.detection_engine.lock().unwrap();
    match engine.detect_batch(&state.db, &state.app_data_dir, &params.image_ids, variant) {
        Ok(count) => serde_json::json!({"processed": count, "variant": variant}).to_string(),
        Err(e) => format!("Error: {}", e),
    }
}
```

Note: Check actual `DetectionEngine` API in `src-tauri/src/db_core/detection.rs`.

- [ ] **Step 4: Add analyze_images MCP tool**

```rust
#[tool(description = "Analyze images with Ollama vision model for natural language descriptions. Returns count processed.")]
fn analyze_images(&self, Parameters(params): Parameters<AnalyzeImagesParams>) -> String {
    let state = self.app_handle.state::<AppState>();
    match crate::db_core::vision::analyze_batch(&state.db, &state.app_data_dir, &params.image_ids) {
        Ok(count) => serde_json::json!({"processed": count}).to_string(),
        Err(e) => format!("Error: {}", e),
    }
}
```

Note: Check actual vision analysis API in `src-tauri/src/db_core/vision.rs`.

- [ ] **Step 5: Build, fix API mismatches, verify**

Run: `cargo check --manifest-path src-tauri/Cargo.toml 2>&1 | grep -E "error|warning"`

The method names and signatures above are approximations. Fix any mismatches based on the actual engine APIs.

- [ ] **Step 6: Commit**

```bash
git add src-tauri/src/mcp/tools.rs
git commit -m "feat(mcp): add AI tools — embeddings, detection, vision analysis"
```

---

### Task 5: Add token management MCP tools

**Files:**
- Modify: `src-tauri/src/mcp/tools.rs`

Expose token CRUD operations as MCP tools. These require `tokens:manage` capability (admin only).

- [ ] **Step 1: Add param structs**

```rust
#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
pub struct CreateTokenParams {
    #[schemars(description = "Human-readable name for the token")]
    pub name: String,
    #[schemars(description = "Role: 'viewer', 'curator', 'operator', or 'admin'")]
    pub role: String,
    #[schemars(description = "Optional scope restriction (folders and/or collections)")]
    pub scope: Option<crate::db_core::models::TokenScope>,
}

#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
pub struct TokenIdParams {
    #[schemars(description = "Token ID (e.g. tok_abc123)")]
    pub token_id: String,
}
```

- [ ] **Step 2: Add create_token MCP tool**

```rust
#[tool(description = "Create a new MCP access token. Returns the token ID and secret (secret is shown only once).")]
fn create_token(&self, Parameters(params): Parameters<CreateTokenParams>) -> String {
    let state = self.app_handle.state::<AppState>();
    let ctx = ServiceContext::from_app_state(&state, Some(self.app_handle.clone()));
    match tokens::create_token(&ctx, &params.name, &params.role, params.scope) {
        Ok((token, secret)) => serde_json::json!({
            "token_id": token.id,
            "name": token.name,
            "role": token.role,
            "secret": secret,
            "warning": "Store the secret securely — it cannot be retrieved again"
        }).to_string(),
        Err(e) => format!("Error: {}", e),
    }
}
```

- [ ] **Step 3: Add list_tokens MCP tool**

```rust
#[tool(description = "List all active (non-revoked) MCP tokens")]
fn list_tokens(&self, Parameters(_): Parameters<EmptyParams>) -> String {
    let state = self.app_handle.state::<AppState>();
    let ctx = ServiceContext::from_app_state(&state, Some(self.app_handle.clone()));
    match tokens::list_tokens(&ctx) {
        Ok(tokens_list) => {
            let result: Vec<serde_json::Value> = tokens_list.iter().map(|t| {
                serde_json::json!({
                    "id": t.id,
                    "name": t.name,
                    "role": t.role,
                    "created_at": t.created_at,
                    "last_used_at": t.last_used_at,
                })
            }).collect();
            serde_json::to_string(&result).unwrap_or_else(|_| "[]".to_string())
        }
        Err(e) => format!("Error: {}", e),
    }
}
```

- [ ] **Step 4: Add revoke_token and rotate_token MCP tools**

```rust
#[tool(description = "Revoke an MCP token permanently")]
fn revoke_token(&self, Parameters(params): Parameters<TokenIdParams>) -> String {
    let state = self.app_handle.state::<AppState>();
    let ctx = ServiceContext::from_app_state(&state, Some(self.app_handle.clone()));
    match tokens::revoke_token(&ctx, &params.token_id) {
        Ok(()) => serde_json::json!({"status": "ok", "revoked": params.token_id}).to_string(),
        Err(e) => format!("Error: {}", e),
    }
}

#[tool(description = "Rotate a token's secret. Returns the new secret (old secret becomes invalid).")]
fn rotate_token(&self, Parameters(params): Parameters<TokenIdParams>) -> String {
    let state = self.app_handle.state::<AppState>();
    let ctx = ServiceContext::from_app_state(&state, Some(self.app_handle.clone()));
    match tokens::rotate_token(&ctx, &params.token_id) {
        Ok(new_secret) => serde_json::json!({
            "token_id": params.token_id,
            "new_secret": new_secret,
            "warning": "Store the new secret securely — it cannot be retrieved again"
        }).to_string(),
        Err(e) => format!("Error: {}", e),
    }
}
```

- [ ] **Step 5: Build and verify**

Run: `cargo check --manifest-path src-tauri/Cargo.toml 2>&1 | grep -E "error|warning"`

- [ ] **Step 6: Commit**

```bash
git add src-tauri/src/mcp/tools.rs
git commit -m "feat(mcp): add token management tools — create, list, revoke, rotate"
```

---

### Task 6: Add audit management MCP tools

**Files:**
- Modify: `src-tauri/src/mcp/tools.rs`

- [ ] **Step 1: Add param structs**

```rust
#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
pub struct AuditLogParams {
    #[schemars(description = "Max number of entries to return (default 50)")]
    pub limit: Option<u32>,
}

#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
pub struct PruneAuditParams {
    #[schemars(description = "Delete entries older than this many days (default 30)")]
    pub retention_days: Option<u32>,
}
```

- [ ] **Step 2: Add get_audit_log MCP tool**

```rust
#[tool(description = "Get recent MCP audit log entries showing tool usage history")]
fn get_audit_log(&self, Parameters(params): Parameters<AuditLogParams>) -> String {
    let state = self.app_handle.state::<AppState>();
    let ctx = ServiceContext::from_app_state(&state, Some(self.app_handle.clone()));
    let limit = params.limit.unwrap_or(50).min(500);
    match tokens::get_recent_audit(&ctx, limit) {
        Ok(entries) => {
            let result: Vec<serde_json::Value> = entries.iter().map(|e| {
                serde_json::json!({
                    "id": e.id,
                    "token_id": e.token_id,
                    "tool_name": e.tool_name,
                    "result_status": e.result_status,
                    "timestamp": e.timestamp,
                })
            }).collect();
            serde_json::to_string(&result).unwrap_or_else(|_| "[]".to_string())
        }
        Err(e) => format!("Error: {}", e),
    }
}
```

- [ ] **Step 3: Add prune_audit_log MCP tool**

```rust
#[tool(description = "Delete old audit log entries. Returns count of deleted entries.")]
fn prune_audit_log(&self, Parameters(params): Parameters<PruneAuditParams>) -> String {
    let state = self.app_handle.state::<AppState>();
    let ctx = ServiceContext::from_app_state(&state, Some(self.app_handle.clone()));
    let days = params.retention_days.unwrap_or(30);
    match tokens::prune_audit_log(&ctx, days) {
        Ok(deleted) => serde_json::json!({"deleted": deleted, "retention_days": days}).to_string(),
        Err(e) => format!("Error: {}", e),
    }
}
```

- [ ] **Step 4: Build and verify**

Run: `cargo check --manifest-path src-tauri/Cargo.toml 2>&1 | grep -E "error|warning"`
Expected: `get_recent_audit`, `prune_audit_log`, `AuditEntry` dead_code warnings gone.

- [ ] **Step 5: Commit**

```bash
git add src-tauri/src/mcp/tools.rs
git commit -m "feat(mcp): add audit log tools — view and prune"
```

---

### Task 7: Add export MCP tools

**Files:**
- Modify: `src-tauri/src/mcp/tools.rs`

Expose export presets listing via MCP. The full export pipeline (manifest → validate → apply → assemble PDF) is complex; start with listing presets and creating export manifests.

- [ ] **Step 1: Add list_export_presets MCP tool**

```rust
#[tool(description = "List available export presets (platforms, sizes, formats)")]
fn list_export_presets(&self, Parameters(_): Parameters<EmptyParams>) -> String {
    let presets = crate::services::export::list_presets();
    serde_json::to_string(&presets).unwrap_or_else(|_| "[]".to_string())
}
```

- [ ] **Step 2: Build and verify**

Run: `cargo check --manifest-path src-tauri/Cargo.toml 2>&1 | grep -E "error|warning"`
Expected: `services::export::list_presets` and `export::presets::list_presets` dead_code warnings gone.

- [ ] **Step 3: Commit**

```bash
git add src-tauri/src/mcp/tools.rs
git commit -m "feat(mcp): add list_export_presets tool"
```

---

### Task 8: Clean up dead code and remove #[allow(dead_code)]

**Files:**
- Modify: `src-tauri/src/services/mod.rs` — remove `#[allow(dead_code)]` from wired modules
- Modify: `src-tauri/src/mcp/mod.rs` — remove `#[allow(dead_code)]`
- Modify: `src-tauri/src/db_core/models.rs` — remove `#[allow(dead_code)]` from AuditEntry
- Modify: `src-tauri/src/db_core/secrets.rs` — remove `#[allow(dead_code)]` from MemoryStore
- Modify: `src-tauri/src/export/presets.rs` — remove `#[allow(dead_code)]` from list_presets
- Delete: `src-tauri/src/services/import.rs` — empty file, remove module declaration

- [ ] **Step 1: Remove #[allow(dead_code)] from services/mod.rs**

Remove all `#[allow(dead_code)]` annotations from module declarations and structs. Keep them only on genuinely unused items (like `PagedResult` if it's still unused, or `ServiceContext` fields that MCP tools don't read through the struct).

- [ ] **Step 2: Remove #[allow(dead_code)] from mcp/mod.rs**

Remove all `#[allow(dead_code)]` annotations — all MCP modules are now used.

- [ ] **Step 3: Remove #[allow(dead_code)] from db_core/models.rs and secrets.rs**

Remove from `AuditEntry` (now used by audit tools) and `MemoryStore` (used in tests).

- [ ] **Step 4: Remove #[allow(dead_code)] from export/presets.rs**

Remove from `list_presets` (now called by MCP export tool).

- [ ] **Step 5: Delete empty services/import.rs and remove from mod.rs**

```bash
trash src-tauri/src/services/import.rs
```

Remove `pub mod import;` line from `src-tauri/src/services/mod.rs`.

- [ ] **Step 6: Build and verify zero warnings**

Run: `cargo check --manifest-path src-tauri/Cargo.toml 2>&1 | grep -E "error|warning"`
Expected: Clean build with no warnings.

- [ ] **Step 7: If any remaining warnings, add targeted #[allow(dead_code)] only on genuinely unused scaffolding**

Some items like `PagedResult`, `services::library::get_image` may still be unused. Add targeted suppression only for those.

- [ ] **Step 8: Commit**

```bash
git add -A src-tauri/src/
git commit -m "chore: remove dead code annotations, delete empty import.rs"
```

---

### Task 9: Update tool_capability mapping

**Files:**
- Modify: `src-tauri/src/services/tokens.rs` (tool_capability function)

Ensure the new MCP tool names are mapped to the correct capabilities.

- [ ] **Step 1: Add new tool names to tool_capability**

Verify that `tool_capability` in `services/tokens.rs` (lines 42-64) covers all new tools:

```rust
pub fn tool_capability(tool_name: &str) -> &'static str {
    match tool_name {
        // Library (existing)
        "list_images" | "get_image" | "list_folders" | "list_folder_images"
        | "list_collections" | "list_collection_images" | "get_library_stats"
        | "get_detections" | "get_vision_metadata" => "library:read",

        "search_images" | "find_similar" | "search_by_object" => "library:search",

        "set_rating" | "set_decision" | "create_collection" | "add_to_collection"
        | "delete_collection" | "create_smart_collection" => "curation:write",

        // New tools
        "import_folder" | "rescan_sources" => "import:write",

        "list_export_presets" => "export:read",

        "show_image" | "navigate_to_folder" | "show_collection" => "display:navigate",

        "generate_embeddings" | "detect_objects" | "analyze_images" => "ai:run",

        "create_token" | "list_tokens" | "revoke_token" | "rotate_token" => "tokens:manage",

        "get_audit_log" | "prune_audit_log" => "tokens:manage",

        _ => "settings:manage",
    }
}
```

- [ ] **Step 2: Build and run existing tests**

Run: `cargo test --manifest-path src-tauri/Cargo.toml -- tokens 2>&1 | tail -20`
Expected: All token tests pass. Update `test_tool_capability_mapping` if new assertions are needed.

- [ ] **Step 3: Commit**

```bash
git add src-tauri/src/services/tokens.rs
git commit -m "feat(mcp): update tool capability mapping for new tools"
```
