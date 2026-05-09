# System Tray + MCP Server Design

## Overview

Add system tray support and an in-process MCP server to ImageView, enabling AI agents to read, curate, import, export, navigate, and search the image library. The MCP server is always active when the app runs, supports local (stdio, Unix socket) and remote (HTTP/SSE with tunneling) transports, and uses capability-based token auth with role presets.

## Goals

- App runs in the background via system tray without a visible window
- AI agents (Claude Code, Cursor, etc.) interact with the full image library over MCP
- Remote access via tunnels (Cloudflare/Tailscale) with rotatable token auth
- Clean service layer shared by Tauri commands and MCP tools — no logic duplication

## Non-Goals

- Built-in tunnel binary/daemon (document external setup instead)
- MCP Resources (v1 is tools-only)
- Multi-user server (single-user app with delegated access tokens)

---

## 1. System Tray

### Plugin

`tauri-plugin-tray` (Tauri 2 built-in tray support via `tauri::tray`).

### Tray Menu

```
ImageView
─────────────────
Show/Hide Window     (toggles main window visibility)
─────────────────
📷 1,234 images      (quick stats, read-only)
🔌 MCP: 2 connected  (active MCP session count)
─────────────────
MCP Settings...      (opens MCP management UI)
─────────────────
Quit ImageView       (full quit, stops MCP server)
```

### Close Behavior

User preference stored in `app_settings` table:

- **Key:** `close_to_tray`
- **Default:** `true`
- **Behavior when true:** closing the window hides it; app stays in tray. MCP server keeps running.
- **Behavior when false:** closing the window quits the app entirely (current behavior).
- **Quit from menu/Cmd+Q:** always quits regardless of setting.

### Tray-Only Launch

`imageview --tray` starts the app with no visible window. Used by the stdio bridge when auto-launching. The app initializes AppState, tray, and MCP server but skips showing the main window.

### Platform Notes (macOS)

- Set activation policy to `Accessory` when running tray-only (no dock icon).
- When the window is shown, switch to `Regular` activation policy (dock icon appears).
- Prevent window flash on tray-only startup: create the window hidden, don't call `show()`.
- Single-instance plugin (already present) prevents duplicate launches.

---

## 2. Service Layer

### Motivation

Existing Tauri commands mix IPC concerns (State extraction, string errors, Tauri types) with business logic. MCP tools need the same logic without Tauri coupling. Extract a service layer that both consumers call.

### Structure

```
src-tauri/src/
  services/           # NEW — pure business logic
    mod.rs
    library.rs        # list, get, filter, search images
    curation.rs       # ratings, decisions, collections, smart collections
    import.rs         # folder/file import, thumbnail regen, rescan
    export.rs         # manifests, patches, PDF assembly, presets
    ai.rs             # embeddings, similarity, detection, vision
    display.rs        # window/view navigation (emits events to frontend)
    tokens.rs         # token CRUD, validation, capability checks
  commands/           # EXISTING — thin Tauri wrappers over services
  mcp/                # NEW — MCP server
  db_core/            # EXISTING — database, models, engines
```

### Service Traits

Services are methods on a `Services` struct that borrows from `AppState`:

```rust
pub struct Services<'a> {
    db: &'a Database,
    embedding_engine: &'a Mutex<EmbeddingEngine>,
    detection_engine: &'a Mutex<DetectionEngine>,
    app_handle: Option<&'a AppHandle>,  // for display/navigation
}

impl<'a> Services<'a> {
    // Library
    pub fn list_images(&self, filter: ImageFilter, page: Pagination) -> Result<PagedResult<ImageWithFile>>;
    pub fn get_image(&self, id: &str) -> Result<ImageWithFile>;
    pub fn get_library_stats(&self) -> Result<LibraryStats>;

    // Curation
    pub fn set_rating(&self, image_id: &str, rating: u8) -> Result<()>;
    pub fn set_decision(&self, image_id: &str, decision: &str) -> Result<()>;
    pub fn create_collection(&self, name: &str) -> Result<String>;
    // ... etc
}
```

### Migration Path

1. Extract service methods from existing command functions.
2. Refactor commands to call services (keeps all existing IPC working).
3. MCP tools call the same services.
4. Commands become ~3-line wrappers: extract state, call service, map error.

### Pagination

All list/search operations support pagination:

```rust
pub struct Pagination {
    pub offset: u32,
    pub limit: u32,  // max 100, default 50
}

pub struct PagedResult<T> {
    pub items: Vec<T>,
    pub total: u32,
    pub offset: u32,
    pub has_more: bool,
}
```

### Concurrency Rules

- Never hold the DB mutex while running model inference.
- Long operations (import, embedding generation, detection) return a job handle and report progress via events.
- MCP tools enforce a 30-second timeout for synchronous operations.
- GPU/model operations are serialized through the existing `Mutex<Engine>` pattern.

---

## 3. MCP Server

### Rust Crate

Use `rmcp` (Rust MCP SDK) or implement the protocol directly over `serde_json` + `tokio`. Evaluate both during implementation; prefer `rmcp` if it supports stdio + SSE transports without heavy dependencies.

### Tools (v1)

Organized by the 6 categories. Each tool maps to one or more service methods.

#### Read Library

| Tool | Description | Params |
|------|-------------|--------|
| `list_images` | List images with filtering/pagination | `filter`, `page` |
| `get_image` | Get single image with all metadata | `image_id` |
| `list_folders` | List imported folders with counts | — |
| `list_folder_images` | Images in a specific folder | `folder_path`, `page` |
| `get_library_stats` | Image count, folder count, collection count | — |
| `get_detections` | Object detections for an image | `image_id` |
| `get_vision_metadata` | Vision descriptions for an image | `image_id` |

#### Write / Curate

| Tool | Description | Params |
|------|-------------|--------|
| `set_rating` | Rate an image 0-5 | `image_id`, `rating` |
| `set_decision` | Select/reject an image | `image_id`, `decision` |
| `create_collection` | Create a manual collection | `name` |
| `add_to_collection` | Add images to collection | `collection_id`, `image_ids` |
| `delete_collection` | Delete a collection | `collection_id` |
| `create_smart_collection` | Create from NL query | `name`, `query` |

#### Import

| Tool | Description | Params |
|------|-------------|--------|
| `import_folder` | Import a folder of images | `path`, `recursive` |
| `import_files` | Import specific files | `paths` |
| `rescan_sources` | Re-scan all imported folders | — |

#### Export

| Tool | Description | Params |
|------|-------------|--------|
| `export_images` | Export images with format/size options | `image_ids`, `format`, `options` |
| `list_export_presets` | Available export presets | — |
| `assemble_pdf` | Create PDF from images | `image_ids`, `layout` |

#### Navigate / Display

| Tool | Description | Params |
|------|-------------|--------|
| `show_image` | Open image in loupe view | `image_id` |
| `navigate_to_folder` | Switch to folder in grid view | `folder_path` |
| `show_collection` | Display collection in grid | `collection_id` |

#### AI / Search

| Tool | Description | Params |
|------|-------------|--------|
| `search_images` | Semantic search (text query → CLIP) | `query`, `limit` |
| `find_similar` | Find visually similar images | `image_id`, `limit` |
| `search_by_object` | Search by detected object class | `class_name`, `page` |
| `generate_embeddings` | Trigger CLIP embedding generation | `image_ids` |
| `analyze_images` | Run vision analysis (Ollama) | `image_ids` |

### Tool Schema Conventions

- All tools return structured JSON (not strings).
- Image results include `id`, `path`, `thumbnail_path`, `rating`, `decision`, `metadata`.
- Error responses use MCP error codes with descriptive messages.
- Tools that modify state return the updated object.
- Long-running tools (import, embeddings, analysis) return a `job_id` for status polling.

---

## 4. Transports

### Unix Socket (always-on)

- Created on app startup at `~/Library/Application Support/com.glebkalinin.imageview/mcp.sock`.
- Permissions: `0600` (owner-only).
- Handles multiple simultaneous MCP sessions.
- Stale socket detection: on startup, check if socket file exists and try connecting. If connection fails, remove stale file and create new socket.
- MCP JSON-RPC messages over the socket, newline-delimited.

### Stdio Bridge

- `imageview --mcp-stdio` binary flag.
- On launch:
  1. Check if Unix socket exists and is connectable.
  2. If not, launch `imageview --tray` and poll for socket readiness (up to 10s).
  3. Connect to socket and bridge stdin/stdout ↔ socket.
  4. On stdin EOF or socket disconnect, exit cleanly.
- Claude Code config:
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

### HTTP/SSE

- Disabled by default. Enabled via app settings or `imageview --mcp-http [port]`.
- Default port: `9847`.
- Binds to `127.0.0.1` by default.
- Optional `--mcp-http-host 0.0.0.0` for LAN/remote access (with warning on first use).
- SSE endpoint: `GET /mcp/sse` (server-sent events for server→client messages).
- Request endpoint: `POST /mcp/message` (client→server JSON-RPC).
- CORS: restrictive by default, configurable allowed origins.
- Request size limit: 10MB.
- Rate limiting: 100 requests/minute per token, 10 failed auth attempts → 15-minute lockout.

### Tunnel Integration (documentation-only)

Not built into the app. Provide documented recipes for:

- **Tailscale:** share the device, access via Tailscale IP + port. Recommended for personal use.
- **Cloudflare Tunnel:** `cloudflared tunnel --url http://localhost:9847`. Free, handles TLS.
- **ngrok:** `ngrok http 9847`. Quick testing.

The HTTP/SSE transport + token auth is sufficient — tunnels just provide the network path.

---

## 5. Authentication

### Capability Model

Internal capabilities (granular permissions):

```
library:read        — list/get images, folders, stats
library:search      — text search, similarity, object search
curation:write      — ratings, decisions, collections
import:write        — import folders/files, rescan
export:read         — export images, PDFs
display:navigate    — control what's visible on screen
ai:run              — trigger embedding/detection/vision jobs
tokens:manage       — create/rotate/revoke tokens
settings:manage     — app settings
```

### Role Presets

| Role | Capabilities | Use Case |
|------|-------------|----------|
| `viewer` | `library:read`, `library:search` | Telegram bot browsing gallery |
| `curator` | viewer + `curation:write`, `export:read` | Friend curating shared project |
| `operator` | curator + `import:write`, `ai:run` | Automation pipeline |
| `admin` | all capabilities | Your own Claude Code session |

### Scope Filters

Each token can optionally restrict access to specific content:

```json
{
  "collections": ["col_abc", "col_def"],
  "folders": ["/art/midjourney"],
  "tags": ["public", "portfolio"]
}
```

- **Union semantics:** image is accessible if it matches ANY filter (collection OR folder OR tag).
- **Null scope:** access all content (only meaningful for admin/operator).
- **Scope enforcement applies to:** list results, search results, get operations, export, stats/counts, embeddings.
- **Write operations:** check both capability AND target is within scope.

### Token Structure

```sql
CREATE TABLE mcp_tokens (
    id TEXT PRIMARY KEY,           -- 'tok_' + 12 random chars
    name TEXT NOT NULL,            -- human label
    secret_hash TEXT NOT NULL,     -- SHA-256(pepper + secret)
    role TEXT NOT NULL,            -- viewer/curator/operator/admin
    scope_json TEXT,               -- nullable JSON scope filter
    created_at TEXT NOT NULL,
    expires_at TEXT,               -- nullable, ISO 8601
    last_used_at TEXT,
    revoked INTEGER DEFAULT 0
);
```

- **Pepper:** stored in OS keychain (existing `KeychainStore`), not in SQLite.
- **Token display:** full secret shown once at creation, never retrievable again.
- **Rotation:** generates new secret, updates hash, old secret invalidated immediately.
- **Constant-time comparison** for hash verification.

### Transport Auth

- **Unix socket:** no auth required (filesystem permissions are sufficient — socket is `0600`).
- **Stdio:** no auth required (inherits Unix socket trust).
- **HTTP/SSE:** `Authorization: Bearer <token_secret>` header required on every request.

### Audit Log

```sql
CREATE TABLE mcp_audit_log (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    token_id TEXT,
    tool_name TEXT NOT NULL,
    params_json TEXT,
    result_status TEXT NOT NULL,    -- 'ok' | 'error' | 'denied'
    timestamp TEXT NOT NULL
);
```

Retained for 30 days, auto-pruned on app startup.

---

## 6. Frontend Changes

### Settings UI

Add "MCP Server" section to settings:

- **Close to tray:** toggle (existing setting, new UI)
- **HTTP/SSE server:** toggle + port field
- **Listen address:** dropdown (localhost / LAN / custom)
- **Active tokens:** list with name, role, scope summary, last used, actions (rotate/revoke)
- **Create token:** button → dialog with name, role picker, optional scope filters
- **Audit log:** recent activity viewer (last 50 entries)

### Tray Menu

Native tray icon and menu (see Section 1).

---

## 7. Implementation Phases

While shipping as one feature, implementation is ordered by dependency:

### Phase A: Service Layer Extraction

Extract core logic from 10 key command modules into `services/`. Refactor commands to call services. All existing tests must pass.

**Files:** `src-tauri/src/services/{mod,library,curation,import,export,ai,display}.rs`
**Can be parallelized:** each service module is independent.

### Phase B: System Tray

Add tray icon, menu, close-to-tray behavior, `--tray` flag.

**Files:** `src-tauri/src/tray.rs`, modifications to `lib.rs` and `tauri.conf.json`
**Dependency:** none (can run parallel with Phase A).

### Phase C: MCP Server Core

MCP protocol handler, tool registry, Unix socket transport.

**Files:** `src-tauri/src/mcp/{mod,server,tools,protocol,socket}.rs`
**Dependency:** Phase A (services must exist).

### Phase D: Stdio Bridge

CLI flag parsing, socket connection, stdin/stdout bridging, app auto-launch.

**Files:** modifications to `src-tauri/src/main.rs` or separate binary entry point.
**Dependency:** Phase C (socket must exist).

### Phase E: Token Auth + HTTP/SSE

Token CRUD, capability enforcement, HTTP server, SSE transport, audit log.

**Files:** `src-tauri/src/mcp/{auth,http,audit}.rs`, `src-tauri/src/services/tokens.rs`
**Dependency:** Phase C.

### Phase F: Frontend

Settings UI for MCP management, tray menu integration.

**Files:** `src/lib/components/McpSettings.svelte`, settings page additions.
**Dependency:** Phase E (needs token API).

### Parallelization Opportunities

- Phase A and B are fully independent.
- Phase D and E depend on C but are independent of each other.
- Phase F can start UI scaffolding in parallel, wiring up once APIs exist.
- Service modules within Phase A can be extracted in parallel (different files).

---

## 8. Dependencies

### New Cargo Dependencies

```toml
# Tray (built into tauri 2, needs feature flag)
tauri = { version = "2", features = ["protocol-asset", "tray-icon"] }

# MCP protocol / transport
tokio = { version = "1", features = ["time", "net", "io-util"] }  # extend existing
hyper = { version = "1", features = ["server", "http1"] }          # HTTP/SSE server

# Auth
rand = "0.8"             # token generation
subtle = "2"             # constant-time comparison
```

### Evaluate

- `rmcp` — Rust MCP SDK. If mature enough, use instead of hand-rolling JSON-RPC.

---

## 9. Testing Strategy

- **Service layer:** unit tests per service module (mock Database where needed).
- **MCP tools:** integration tests — spawn MCP server, send JSON-RPC over socket, assert responses.
- **Auth:** unit tests for capability checks, scope filtering, token validation.
- **Tray:** manual testing (platform-specific behavior).
- **Stdio bridge:** integration test — pipe JSON-RPC through `imageview --mcp-stdio`, assert output.
- **HTTP/SSE:** integration tests with `reqwest` client.

---

## 10. Security Considerations

- Unix socket: `0600` permissions, per-user path.
- HTTP default: `127.0.0.1` only. Warning dialog before binding `0.0.0.0`.
- Token secrets: SHA-256 with keychain-stored pepper.
- Rate limiting on HTTP auth failures.
- Audit log for all MCP tool invocations.
- No filesystem traversal outside imported folder roots.
- Import paths validated against a configurable allowlist.
- Navigate/display tools restricted to admin role (prevents remote UI hijacking).
- Token management restricted to admin role.
