# Final Requirements: System Tray + MCP Server

> Produced by synthesizing the design spec, Codex architectural review, and implementation plan review. This is the authoritative requirements document for implementation.

## 1. Overview

1.1. **MUST** add system tray support so ImageView can continue running without a visible window.

1.2. **MUST** add an in-process MCP server that exposes ImageView library, curation, import, export, navigation, and AI/search operations to local MCP clients.

1.3. **MUST** share business logic between Tauri IPC commands and MCP tools through a Rust service layer.

1.4. **MUST NOT** delete, reset, migrate destructively, or recreate the user's existing `imageview.db`.

1.5. **MUST NOT** add a frontend or API mock fallback path. `src/lib/api.ts` must continue importing `invoke` directly from `@tauri-apps/api/core`.

1.6. **MUST** treat HTTP/remote MCP access as security-sensitive. Token auth, capability checks, audit logging, rate limits, and scope filtering are required before HTTP transport is considered shippable.

---

## 2. System Tray

2.1. **MUST** use Tauri 2 native tray support via `tauri::tray`.

2.2. **MUST** enable the Tauri tray feature:

```toml
tauri = { version = "2", features = ["protocol-asset", "tray-icon"] }
```

2.3. **MUST** provide a native tray menu with:

```text
ImageView
Show/Hide Window
image count, read-only
MCP connection count, read-only
MCP Settings...
Quit ImageView
```

2.4. **MUST** persist close behavior in `app_settings`:

```text
key: close_to_tray
default: true
```

2.5. **MUST** hide the window instead of quitting when `close_to_tray = true`.

2.6. **MUST** fully quit the app from tray quit, Cmd+Q, or explicit application quit, regardless of `close_to_tray`.

2.7. **MUST** support `imageview --tray`, initializing `AppState`, tray, and MCP server without showing the main window.

2.8. **MUST** avoid visible window flash during tray-only launch by creating the main window hidden and not calling `show()`.

2.9. **MUST** use macOS activation policy `Accessory` while tray-only and switch to `Regular` when a window is shown.

2.10. **SHOULD** update tray stats when the library count or MCP session count changes.

---

## 3. Service Layer

3.1. **MUST** add a `src-tauri/src/services/` module for business logic shared by Tauri commands and MCP tools.

3.2. **MUST** keep Tauri commands as thin wrappers: extract `State<'_, AppState>`, call services, map errors to strings.

3.3. **MUST** create service module stubs before declaring them in `services/mod.rs`, so the first `cargo check` succeeds.

3.4. **MUST** preserve existing app patterns (Database::open, Mutex<Connection>, ImageWithFile, LEFT JOIN selections).

3.5. **MUST** define a request context used by both commands and MCP tools:

```rust
pub struct ServiceContext<'a> {
    pub db: &'a Database,
    pub app_data_dir: &'a PathBuf,
    pub embedding_engine: &'a Mutex<EmbeddingEngine>,
    pub detection_engine: &'a Mutex<DetectionEngine>,
    pub safety_engine: &'a Mutex<DetectionEngine>,
    pub secrets: &'a dyn SecretStore,
    pub app_handle: Option<&'a AppHandle>,
    pub auth: AuthContext,
}
```

3.6. **MUST** support pagination for every list/search result (default 50, max 100).

3.7. **MUST** enforce `limit <= 100` at service boundaries.

3.8. **MUST NOT** hold the DB mutex while running model inference or filesystem scanning.

---

## 4. App State Access From MCP

4.1. **MUST NOT** perform a broad `State<'_, Arc<AppState>>` refactor.

4.2. **MUST** have MCP server structs hold a cloned `tauri::AppHandle`.

4.3. **MUST** resolve state inside each MCP tool call with `app_handle.state::<AppState>()`.

4.4. **MUST** build `ServiceContext` from that resolved state per request.

---

## 5. MCP SDK And Transport Requirements

5.1. **MUST** use `rmcp = "1.6"` with these feature flags:

```toml
rmcp = {
  version = "1.6",
  features = [
    "server",
    "macros",
    "schemars",
    "transport-async-rw",
    "transport-io",
    "transport-streamable-http-server"
  ]
}
```

5.2. **MUST NOT** use the non-existent feature `transport-sse-server`.

5.3. **MUST** use `transport-async-rw` for Unix socket MCP sessions.

5.4. **MUST** use rmcp Streamable HTTP for HTTP transport.

5.5. **MUST** start with an rmcp spike before implementing the full tool set. The spike must prove:
   1. `list_tools` works over Unix socket
   2. `call_tool` works over Unix socket
   3. One read-only tool (`get_library_stats`) calls the real service layer
   4. The server handles at least two simultaneous socket sessions
   5. The app shuts down sessions cleanly on quit

---

## 6. Unix Socket Transport

6.1. **MUST** create socket at `~/Library/Application Support/com.glebkalinin.imageview/mcp.sock`.

6.2. **MUST** set socket permissions to `0600` (owner-only).

6.3. **MUST** detect and remove stale sockets on startup.

6.4. **MUST** serve each `UnixStream` as an rmcp server session using `transport-async-rw`.

6.5. **MUST** allow multiple concurrent MCP sessions.

6.6. **MUST** treat Unix socket clients as local trusted (implicit admin auth context).

---

## 7. Stdio Bridge

7.1. **MUST** support `imageview --mcp-stdio` as a bridge between stdin/stdout and Unix socket.

7.2. **MUST** auto-launch `imageview --tray` when no socket is available.

7.3. **MUST** poll for socket readiness up to 10 seconds after auto-launch.

7.4. **MUST** exit cleanly on stdin EOF, socket disconnect, or app shutdown.

7.5. **MUST NOT** spawn a second app instance when single-instance plugin detects one running.

---

## 8. HTTP Transport

8.1. **MUST** implement as rmcp Streamable HTTP, not legacy SSE.

8.2. **MUST** be disabled by default. Enable via settings or `imageview --mcp-http [port]`.

8.3. **MUST** default to `127.0.0.1:9847`.

8.4. **MUST** require explicit setting/flag to bind `0.0.0.0`, with warning on first use.

8.5. **MUST** require `Authorization: Bearer <token_secret>` for every HTTP request.

8.6. **MUST** enforce 10 MB request size limit.

8.7. **MUST** implement restrictive CORS (configurable allowed origins).

8.8. **MUST** rate-limit to 100 requests/minute per token.

8.9. **MUST** lock out after 10 failed auth attempts for 15 minutes.

8.10. **MUST NOT** ship HTTP transport as a stub.

---

## 9. Token Auth

9.1. **MUST** add `mcp_tokens` table (id, name, secret_hash, role, scope_json, created_at, expires_at, last_used_at, revoked).

9.2. **MUST** hash secrets with SHA-256(keychain_pepper + secret).

9.3. **MUST** use constant-time comparison for verification.

9.4. **MUST** show full secret once at creation, never again.

9.5. **MUST** support token expiration, rotation, and revocation.

9.6. **MUST** restrict token management to `tokens:manage` capability.

---

## 10. Capabilities And Roles

10.1. **MUST** use capabilities: `library:read`, `library:search`, `curation:write`, `import:write`, `export:read`, `display:navigate`, `ai:run`, `tokens:manage`, `settings:manage`.

10.2. **MUST** use role presets:

| Role | Capabilities |
|---|---|
| `viewer` | `library:read`, `library:search` |
| `curator` | viewer + `curation:write`, `export:read` |
| `operator` | curator + `import:write`, `ai:run` |
| `admin` | all capabilities |

10.3. **MUST** map `list_collections` to `library:read` (not `curation:write`).

10.4. **MUST** map `display:navigate` as admin-only in default presets.

---

## 11. Scope Filtering

11.1. **MUST** treat scope as a first-class authorization constraint, not a UI filter.

11.2. **MUST** apply scope filtering inside service queries, not by post-filtering.

11.3. **MUST** preserve correct pagination, totals, counts, and search ranking under scope restrictions.

11.4. **MUST** apply to: list results, search results, get operations, folder/collection results, stats/counts, exports, embeddings, analysis jobs, similarity, object search, display targets, write targets.

11.5. **MUST** return access denied for operations targeting out-of-scope images.

11.6. **MUST** prevent restricted tokens from learning out-of-scope existence through counts, pagination gaps, errors, or job status.

---

## 12. Job Handling

12.1. **MUST** return `job_id` immediately for: import_folder, import_files, rescan_sources, generate_embeddings, analyze_images, assemble_pdf, large exports.

12.2. **MUST** define job statuses: `queued`, `running`, `succeeded`, `failed`, `cancelled`.

12.3. **MUST** expose `get_job`, `list_jobs`, `cancel_job` tools.

12.4. **MUST** enforce original job capability and scope on job status/cancellation.

12.5. **MUST** enforce 30-second timeout for synchronous operations; longer operations become jobs.

12.6. **MUST NOT** hold DB mutex during model inference or filesystem scanning.

---

## 13. MCP Tools (v1)

**Library:** list_images, get_image, list_folders, list_folder_images, list_collections, get_library_stats, get_detections, get_vision_metadata

**Curation:** set_rating, set_decision, create_collection, add_to_collection, delete_collection, create_smart_collection

**Import:** import_folder, import_files, rescan_sources

**Export:** export_images, list_export_presets, assemble_pdf

**Display:** show_image, navigate_to_folder, show_collection

**AI/Search:** search_images, find_similar, search_by_object, generate_embeddings, analyze_images

**Jobs:** get_job, list_jobs, cancel_job

---

## 14. Audit Log

14.1. **MUST** add `mcp_audit_log` table. Record ok/error/denied outcomes.

14.2. **MUST** redact secrets from params_json.

14.3. **MUST** prune entries older than 30 days on startup.

14.4. **MUST** expose audit entries in settings UI (restricted to `tokens:manage`/`settings:manage`).

---

## 15. Implementation Order

1. Service module stubs and compile-safe foundation
2. **rmcp Unix socket spike** with one working read-only tool
3. Service extraction (library, curation, import, export, AI, display) — parallel with tray
4. Tray support and `--tray`
5. Job registry/service
6. Token DB/service, capabilities, audit log, scope helpers
7. MCP tool core with auth and scope enforcement
8. Stdio bridge
9. Streamable HTTP transport with bearer auth
10. Frontend settings UI
11. Integration tests and manual verification

**MUST NOT** combine rmcp spike with broad AppState refactors.
**MUST NOT** implement HTTP before scope filtering, token auth, and audit exist.

---

## 16. Acceptance Criteria

- MCP `list_tools` and `call_tool` work over Unix socket
- Stdio client connects via `imageview --mcp-stdio`
- HTTP Streamable MCP works with bearer auth (disabled by default)
- Restricted tokens cannot see out-of-scope images
- `list_collections` works with `library:read`
- Long-running operations return `job_id`
- Close-to-tray keeps MCP running
- Quit stops tray, MCP, jobs, and HTTP cleanly
- No placeholder implementations shipped
