# Sessions Architecture — Design Spec

**Date:** 2026-05-10
**Status:** Approved for implementation
**Approach:** A — Sessions as `collection_type = 'session'` in existing `projects` table

---

## 1. Overview

Sessions are file-system-based workspaces that own files, have canvases, and can be created from imports or materialized from smart collections. They reuse the existing `projects` table with `collection_type = 'session'`, making the promotion chain — smart collection → session → canvas — a natural type progression.

### Scope

**In scope:**
- Session data model (DB + file system)
- Session lifecycle (create, import, convert, delete)
- Top-level session switcher UI
- Session-scoped collections and smart collections
- Session-scoped canvases (manual + query types)
- File operations (copy/move with hash validation)
- Schema normalization (missing indexes, canvases table)

**Out of scope (separate specs):**
- Vision analysis pipeline (cheap cloud AI for image metadata)
- Canvas query language (SQL-like interface for populating canvases)
- AI canvas layout algorithms (magazine, gallery, museum presets)
- Entity extraction for images

---

## 2. Data Model

### 2.1 `projects` table additions

New columns, only populated when `collection_type = 'session'`:

| Column | Type | Purpose |
|--------|------|---------|
| `folder_path` | TEXT | Absolute path to session folder on disk |
| `owning_session_id` | TEXT FK → projects(id) | Scopes collections/smart collections to a session (on the child row, not the session) |
| `settings_json` | TEXT | Per-session preferences (copy vs move, default layout) |

**Constraints:**
- Session-only columns must be NULL for non-sessions: `CHECK (collection_type = 'session' OR (folder_path IS NULL AND settings_json IS NULL))`
- No nested sessions: `CHECK (owning_session_id IS NULL OR collection_type != 'session')`
- `owning_session_id` lives on collection/smart-collection rows to scope them to a session, not on the session row itself

### 2.2 `canvases` table (new)

```sql
CREATE TABLE canvases (
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
CREATE INDEX idx_canvases_session ON canvases(session_id);
```

- **Manual canvas:** free-form drag-and-arrange. `layout_json` stores per-image positions, zoom, pan state.
- **Query canvas:** live grid populated by `filter_json` rules. `grid_config_json` stores row/column counts. Auto-reflows when images match/unmatch the query.

### 2.3 `collection_type` values

| Type | Has folder | Has canvases | Has items | Has filter | Scoped to session |
|------|-----------|-------------|-----------|-----------|------------------|
| `'manual'` | no | no | yes (ordered) | no | optionally via `owning_session_id` |
| `'smart'` | no | no | dynamic | yes | optionally via `owning_session_id` |
| `'session'` | yes | yes | yes | no | no (top-level only) |

### 2.4 Missing indexes (add now)

```sql
CREATE INDEX idx_collection_items_image ON collection_items(image_id);
CREATE INDEX idx_selections_project ON selections(project_id);
CREATE INDEX idx_embeddings_image ON embeddings(image_id);
CREATE INDEX idx_images_import_batch ON images(import_batch_id);
-- Note: lineage_group_id index — verify column exists on images table before adding
-- If lineage is tracked via lineage_groups/lineage_members join, skip this index
```

### 2.5 Future normalization (not blocking)

- Extract `source_*` and `ai_*` columns from `images` into versioned `image_source_detections` table
- Structured table for vision analysis results (separate spec)
- Keep `image_metadata` EAV for EXIF/XMP key-value pairs

---

## 3. Session Folder Structure

```
~/ImageView/Sessions/
  2026-05-10-portrait-shoot/
    Imports/          ← originals land here
    Selects/          ← accepted images (physical copies)
    Exports/          ← rendered outputs
```

- Session folder created on disk when session is created in DB
- Subfolder structure is fixed and predictable
- Folder name derived from date + user-provided name, sanitized for filesystem

---

## 4. Session Lifecycle

### 4.1 Create session

1. User clicks "New Session" in session switcher or via command bar
2. Prompted for name (default: today's date)
3. DB: insert into `projects` with `collection_type = 'session'`, `folder_path`
4. Disk: create session folder + Imports/, Selects/, Exports/ subfolders
5. UI: session switcher updates, new session becomes active

### 4.2 Import into session

1. User selects files/folder to import while a session is active
2. Files are **copied or moved** into `session/Imports/` (configurable in settings, overridable per-import)
3. **Hash validation:** sha256 computed on source, compared after copy/move to verify integrity
4. DB: images created, linked to session via `collection_items` (implementation note: verify whether existing import code uses `collection_items` or `image_projects` — session imports must use the same join table for consistency)
5. Transient import batch created for "just imported" banner, auto-cleaned after 7 days on app launch
6. Post-import detection runs (YOLO, NudeNet) as today

**Partial failure handling:** If import fails mid-batch, successfully imported images are kept. Errors reported to user via toast with details. No rollback of successful imports.

### 4.3 Smart collection → Session

1. User right-clicks smart collection → "Materialize as Session"
2. New session created (4.1)
3. Smart collection's current results are imported as files into session (4.2)
4. Original smart collection kept independently — it continues to evaluate dynamically
5. No ongoing link between the smart collection and the materialized session

### 4.4 Session → Collection

1. User right-clicks session → "Convert to Collection"
2. DB: `collection_type` changed from `'session'` to `'manual'`, `folder_path` cleared
3. Image references in `collection_items` preserved
4. Session folder remains on disk (user can delete manually) but is no longer tracked
5. Canvases for the session are **explicitly deleted** via `DELETE FROM canvases WHERE session_id = ?` (CASCADE only fires on row deletion, not type changes)

### 4.5 Delete session

1. User deletes session from session switcher
2. DB: session row deleted, CASCADE removes `collection_items` and `canvases`
3. Disk: **user prompted** — delete session folder and files, or keep on disk?
4. If files kept on disk, they become orphaned (not tracked in DB)

### 4.6 Session folder deleted externally (Finder)

- On session open: validate `folder_path` exists
- If missing: mark session with warning state, show banner "Session folder missing — files may be unavailable"
- Individual images: existing `missing_at` column on `image_files` handles per-file absence

---

## 5. UI Design

### 5.1 Session switcher

- **Position:** compact dropdown with search at the top of the sidebar, above all other sections
- **Default state:** "All Images" (no session selected) — shows the flat library view as today
- **Session selected:** sidebar shows that session's folders, collections, smart collections, and canvases
- **Actions:** New Session, session list with search, active session indicator

### 5.2 Sidebar (session-scoped)

When a session is selected, the sidebar sections are:

1. **Canvases** — list of canvases in this session (manual + query), with "New Canvas" button
2. **Folders** — source paths within the session folder
3. **Collections** — manual collections scoped to this session (`owning_session_id`)
4. **Smart Collections** — filter-based collections scoped to this session

### 5.3 Global view (no session)

When "All Images" is selected in the session switcher:
- Sidebar shows all folders, all collections, all smart collections (unscoped)
- Canvas view not available (canvases are session-scoped)
- This is the current behavior, unchanged

---

## 6. Canvas Architecture

### 6.1 Manual canvas

- Free-form drag-and-arrange (existing behavior, now session-scoped)
- `layout_json` stores: image positions (x, y), sizes, zoom level, pan offset
- Multiple manual canvases per session
- Persisted to `canvases` table on change

### 6.2 Query canvas

- Populated by filter rules (same `filter_json` format as smart collections)
- Live: images flow in/out as data changes
- Layout: auto-grid with configurable rows/columns via `grid_config_json`
- User can resize the grid dynamically but not position individual images
- If a filter references deleted data (tag, collection): show empty canvas with warning toast

### 6.3 Canvas types (future, out of scope)

AI-driven layout presets will be added in a separate spec:
- Magazine layout (hero + supporting grid)
- Gallery wall (salon-style, varied sizes)
- Museum exposition (linear walk-through)
- Contact sheet (dense uniform grid)
- Comparison (side-by-side pairs)

---

## 7. File Operations

### 7.1 Copy vs Move

- **Global default:** configurable in app settings (Settings → Sessions → "Import mode")
- **Per-import override:** import dialog offers copy/move toggle
- **Copy:** original stays in place, duplicate in session folder
- **Move:** original relocated to session folder (original location forgotten)

### 7.2 Hash validation

- Computed on both copy and move operations
- Source sha256 compared against `images.sha256_hash` after file operation
- Mismatch: operation fails for that file, error reported, file not added to session

### 7.3 No original path tracking

- Once imported into a session, only the session path is tracked in `image_files`
- Original source path is not stored — this is a deliberate simplification
- Users who need to trace provenance should use copy mode (original stays in place)

---

## 8. Migration

### 8.1 Schema migration

- `ALTER TABLE projects ADD COLUMN folder_path TEXT`
- `ALTER TABLE projects ADD COLUMN owning_session_id TEXT REFERENCES projects(id)`
- `ALTER TABLE projects ADD COLUMN settings_json TEXT`
- `CREATE TABLE canvases (...)` with constraints
- Add missing indexes
- CHECK constraints: enforce in application layer (Rust validation before insert/update), not via SQLite ALTER — table recreation with all FK dependents (collection_items, selections, image_projects, canvases) is too risky

### 8.2 Existing data

- 553 existing images remain in the global library (no session)
- Existing collections and smart collections unaffected (`owning_session_id` defaults to NULL = global)
- No automatic migration to sessions — users create sessions and import into them
- Existing canvas state in localStorage (persistence.ts): migrate to `canvases` table as a global manual canvas with `session_id` pointing to a special "Unsorted" project (not a session — no `folder_path`). This avoids the contradiction of a session without a folder. Alternatively, skip auto-migration and let users manually recreate canvas layouts in sessions

### 8.3 Import batch cleanup

- On app launch: delete `import_batches` rows older than 7 days
- Clear `import_batch_id` on affected images
- This formalizes the current "transient batch" behavior

---

## 9. MCP Integration

Sessions are file-system folders, making them naturally accessible to Claude Code and MCP tools:

- **Discovery:** MCP tools can list sessions via existing `list_collections` tool (filtered by type)
- **File access:** session folders are plain directories, readable by any tool
- **New MCP tools (future):** `create_session`, `import_into_session`, `list_session_canvases`
- **Scope filtering:** existing `TokenScope.collections` can reference session IDs

---

## 10. Settings

New settings section: **Sessions**

| Setting | Type | Default | Description |
|---------|------|---------|-------------|
| Sessions root folder | path | `~/ImageView/Sessions/` | Where session folders are created |
| Default import mode | copy \| move | copy | Whether imports copy or move files |
| Auto-create session on import | boolean | false | Whether importing without an active session prompts for session creation |
| Batch cleanup interval | days | 7 | How long transient import batches are kept |
