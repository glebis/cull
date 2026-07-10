# Cull — Key Test Scenarios

Cull has one binary with four faces onto the same Rust core: **CLI** (headless
subcommands), **MCP** (server for agents), **URL scheme**, and the **GUI**. These
scenarios emphasize the three that automation and users actually exercise today —
CLI, MCP, and UI.

Scenario prefixes:
- **C##** — CLI / headless
- **M##** — MCP server & agents
- **U##** — `cull://` URL scheme (deep links)
- **S##** — GUI / interactive

Only shipped behavior is listed. The **navigation** subset of the URL scheme
(`open`/`grid`/`loupe`/`compare`) ships today; the **action-verb expansion**
(`import`, `export`, `search`, `contact-sheet`, `resize`, …) and the broader
verb-style CLI (`search`, `similar`, `contact-sheet`, `rate`, `detect`, `pipe`)
are draft — see `docs/cli-and-url-scheme.md` and open tasks `imageview-m2u`,
`imageview-b1k`.

---

## CLI (headless)

The shipped headless surface is MCP-aligned: subcommands use MCP tool names and
JSON param field names. Every headless command runs against an isolated DB when
given `--db` / `--app-data-dir`, which is how CLI scenarios should be tested
without touching the real library (`~/Library/Application Support/com.glebkalinin.cull/cull.db`).
See `docs/agent-cli-standards.md` for the harness convention.

```bash
CULL=./target/debug/cull
TMP=$(mktemp -d)
run() { "$CULL" --app-data-dir "$TMP/appdata" --db "$TMP/cull.db" --json "$@"; }
```

### C01 — Library inspection
1. `run get_library_stats` → JSON object with counts (images, folders, collections)
2. `run list_images --limit 20 --offset 0` → array of image records
3. `run list_images --limit 5 --offset 5` → next page, no overlap with offset 0
4. `run list_folders` and `run list_collections` → arrays (empty on a fresh DB)
5. Every response is a success envelope; exit code is `0`

### C02 — Headless import
1. `run import_folder --folder_path /path/to/images` → imports, reports count
2. Re-run the same import → no duplicate rows (idempotent on path)
3. `run import_files --file_paths /a.png,/b.png` → comma-split into two imports
4. `run get_library_stats` afterward reflects the new totals
5. `list_folders` shows the imported folder in the tree

### C03 — Export
1. `run list_export_presets` → available presets
2. `run export_images --image_ids id1,id2 --output_dir "$TMP/out" --format original`
3. `run export_images --collection_id <id> --output_dir "$TMP/out" --format webp`
4. `run export_images --folder_path /path --output_dir "$TMP/out" --format png --flatten false`
   → subfolder structure preserved when `--flatten false`
5. `--naming` template renames outputs; missing `--output_dir` errors cleanly

### C04 — Embeddings
1. `run get_embedding_model_download_info --model clip-vit-b32` → size/URL/license info
2. `run download_embedding_model --model clip-vit-b32` → model provisioned
3. `run generate_embeddings --model clip-vit-b32 --image_ids id1,id2` → embeddings written
4. Requesting embeddings for a model that isn't downloaded errors with guidance

### C05 — Quality analysis
1. `run analyze_image_quality --all` → analyzes the whole library
2. `run analyze_image_quality --image_ids id1,id2` → scoped analysis
3. `run get_image_quality --image_id id1` → per-image quality record
4. `run get_quality_count` → tally of analyzed vs. pending images

### C06 — Catalog (via `call_tool`)
1. `run call_tool list_catalog_presets` → presets (no dedicated subcommand; uses `call_tool`)
2. `run call_tool create_catalog_work --params_json '{...}'`
3. `run call_tool suggest_catalog_values --params_json '{...}'` → draft suggestions
4. `run call_tool approve_catalog_values` / `reject_catalog_values` → draft state changes
5. `run call_tool set_catalog_draft_values --params_json '{...}'` persists values

### C07 — `call_tool` contract & error handling
1. `run call_tool get_library_stats` equals the dedicated `get_library_stats` subcommand
2. `--params_file params.json` is accepted as an alternative to `--params_json`
3. Unknown tool name → error envelope listing supported tools, exit code `1`
4. Malformed `--params_json` → parse error, exit code `1`, nothing mutated
5. Without `--json`, output is human-readable; with `--json`, strictly parseable

---

## MCP (server & agents)

Cull exposes its core as MCP tools over local stdio or HTTP. Full tool set and
roles are in `docs/mcp-remote-access.md`.

### M01 — Local stdio (Claude Code)
1. Configure `command: cull`, `args: ["--mcp-stdio"]`
2. First tool call auto-launches Cull in tray mode if not running
3. Local stdio gets full admin access with no token (Unix socket, `0600` perms)
4. `tools/list` returns the `cull` tool catalog

### M02 — HTTP server enable
1. `cull --mcp-http` binds `127.0.0.1:9847`; `cull --mcp-http 8080` uses a custom port
2. Settings → Agent Access → MCP Connection toggles the same listener
3. Default bind is loopback only; no remote reachability without opt-in
4. `curl -H "Authorization: Bearer <token>" .../mcp -d '{"jsonrpc":"2.0","method":"tools/list","id":1}'`

### M03 — Tokens & roles
1. Settings → Agent Access → Access Tokens → Create Token → secret shown once, copyable
2. Create one token per role: viewer, curator, operator, admin
3. **viewer**: read/list/search succeed; `set_rating`, `import_folder` → permission denied
4. **curator**: ratings, decisions, collections, export succeed; import/AI engines denied
5. **operator**: import + AI engines (embeddings, detection, vision) succeed; token/settings denied
6. **admin**: everything, including `create_token` / `revoke_token` / settings

### M04 — Remote binding safety
1. Non-loopback bind requires `--mcp-http-allow-remote` (or `mcp_http_allow_remote=true`)
2. Without the flag, `--mcp-http-host 0.0.0.0` refuses to expose beyond loopback
3. Remote clients see filenames only — absolute paths are redacted
4. Every request without a valid bearer token → `401 Unauthorized`

### M05 — Scoped tokens
1. Create a token scoped to `{"collections":["col_abc"]}`
2. `list_images` / search through that token return only in-scope images
3. Out-of-scope image ids resolve to empty results, not errors
4. Union semantics: adding `folders`/`tags` widens (OR), not narrows

### M06 — Token lifecycle
1. `rotate_token` → new secret works, old secret → `401`
2. `revoke_token` → immediate `401` on the next call
3. `list_tokens` reflects created/rotated/revoked state

### M07 — Curation via MCP
1. `set_rating` / `set_decision` on image ids → persists, visible in GUI on refresh
2. `create_collection` + `add_to_collection` → collection appears in sidebar
3. `create_smart_collection` with a filter → returns matching images
4. `find_similar` / `search_by_object` / `detect_objects` return ranked results
5. `analyze_images` / `generate_embeddings` enqueue jobs; `get_job` / `list_jobs` track them; `cancel_job` stops one

### M08 — Agent-driven view control (admin)
1. `show_image` / `navigate_to_folder` / `show_collection` move the live GUI
2. `select_images_in_view` selects, `capture_current_view_snapshot` records what's shown
3. `get_last_view_snapshot` / `select_snapshot_labels` round-trip the selection
4. Requires admin (display control); denied for lower roles

### M09 — Clipboard publishing
1. `get_clipboard_monitor_status` reports monitor state (and empty state when idle)
2. `publish_clipboard_collection` publishes; `get_last_clipboard_publish` returns the result
3. `show_clipboard_collection` surfaces it in the GUI

### M10 — Static publishing via MCP
1. `export_static_publish_package` writes a portable `site/` folder
2. `serve_static_publish_package` starts a local preview server
3. `export_static_publish_canvas` exports a canvas layout

### M11 — Audit & approval boundary
1. Every MCP tool invocation is recorded in the audit log (`get_audit_log`)
2. `prune_audit_log` trims history (admin)
3. Destructive tools (`revoke_token`, `prune_audit_log`, `delete_collection`,
   broad `export_images`) are **not** self-confirming — confirmation must come from
   the MCP client / GUI / operator, per `docs/mcp-remote-access.md`

---

## URL Scheme (`cull://` deep links)

The shipped scheme is a **navigation/launch** surface: it opens the GUI and sets
view/focus. Action verbs that mutate the library (import/export/search/…) are
draft (tasks `imageview-m2u`, `imageview-b1k`). Parsing lives in
`src-tauri/src/commands/deeplink.rs`; all paths go through `path_policy::Deeplink`.

### U01 — Launch & navigation actions
1. `cull://open?path=/img.png` → app opens/focuses with that image
2. `cull://grid?size=280` → grid view at the given thumbnail size
3. `cull://loupe?image_id=<id>` → loupe on that image
4. `cull://compare?paths=/a.png,/b.png` → compare view
5. Recognized params: `path`, `paths`, `folder`, `view`, `zoom`, `size`, `fullscreen`, `focus`, `image_id`, `gap`

### U02 — Single-instance forwarding
1. With the app already running, invoking a `cull://` URL forwards to the live instance (no second window)
2. Cold start: the launching URL is drained and applied after setup

### U03 — Path safety
1. Paths outside `$HOME`, hidden components, or sensitive dirs → rejected
2. Malformed percent-encoding / invalid UTF-8 → rejected, not lossily decoded
3. A rejected deep link logs and is ignored; the app stays in a valid state

### U04 — Action verbs (draft — expected to fail today)
1. `cull://import?folder=…`, `cull://export?collection=…`, `cull://search?q=…`
   are **not** implemented — currently ignored/parsed as no-ops
2. Track under `imageview-m2u` (URL verb vocabulary) and `imageview-b1k` (batch pipeline)

---

## GUI — Navigation & Views

### S01 — View mode switching
1. Press `⌘1` through `⌘8` — each view loads correctly
2. Press `Tab` / `Shift+Tab` — cycles through views in order
3. Verify tab bar highlights the active view

### S02 — Grid navigation
1. Arrow keys / `h/j/k/l` move focus highlight through thumbnails
2. `Home` jumps to first image, `End` to last
3. `PageUp` / `PageDown` scroll by one viewport
4. `Enter` on focused image opens Loupe
5. Double-click on thumbnail opens Loupe

### S03 — Loupe navigation
1. `←/→` or `h/l` cycle through images
2. Mouse wheel zooms in/out; `+/-` keys zoom
3. Click-drag pans when zoomed in
4. `Home` resets zoom to 1×
5. `Escape` returns to Grid
6. Double-click returns to Grid
7. Bottom overlay shows filename, dimensions, format, zoom%
8. When generation metadata exists, prompt + provider/model/seed tags show

### S04 — Compare mode
1. With 2+ images selected, switch to Compare — shows side-by-side
2. Click left/right panel to set active side (blue border)
3. `←/→` switches active side; `↑/↓` swaps active image
4. `1` accepts left/rejects right; `2` accepts right/rejects left
5. `Escape` returns to Grid

### S05 — Canvas mode
1. Images appear on free-form canvas
2. Drag to reposition images
3. Space+drag pans the canvas
4. Mouse wheel zooms canvas
5. `r` rotates selected item
6. Layout persists after switching away and back

### S06 — Tinder mode
1. Images presented in pairs
2. `←` or `h` picks left (reject); `→` or `l` picks right (accept)
3. `↓` or `j` skips
4. `z` undoes last decision
5. Completion screen shows stats

### S07 — Lineage view
1. Groups of related images display correctly
2. `Enter`/`Space` on an image opens Loupe
3. Groups can be renamed and dissolved

### S08 — Embedding Explorer
1. Select a provider and generate embeddings
2. 2D scatter plot renders with thumbnails
3. Arrow keys navigate points
4. `p` toggles large preview panel
5. Click a point selects/focuses that image

---

## GUI — Ratings & Decisions

### S09 — Star ratings
1. In Grid: press `1`–`5` → star rating applied, visual dots shown on thumbnail
2. Press `0` → rating cleared
3. Chord: press `s` then `1`–`5` → same result
4. Rating persists after view switch
5. Undo (`⌘Z`) reverts the rating

### S10 — Accept / Reject / Undecided
1. Press `a` → green ✓ badge on thumbnail
2. Press `x` → red × badge
3. Press `u` → badge cleared
4. Works in Grid, Loupe, Compare, Canvas
5. Undo reverts the decision

---

## GUI — Selection & Collections

### S11 — Multi-selection
1. `Space` toggles selection on focused image
2. `Shift+click` selects a range
3. `⌘+Shift+A` deselects all
4. Selection count shown in status bar

### S12 — Collection creation
1. Select images → press `c` → dialog appears → name → collection created
2. `Shift+C` creates collection from unselected images
3. Sidebar shows new collection with correct count
4. Click collection in sidebar → grid scoped to that collection

### S13 — Collect mode
1. Press `b` in Grid → prompted for target collection
2. Navigate with arrows, press `Space` to add images
3. Press `b` again to exit collect mode
4. Images appear in target collection

### S14 — Smart collections
1. Open search (`/`), type a query, apply
2. Click "Save Collection" → name it
3. Smart collection appears in sidebar under SMART
4. Re-opening shows filtered results

### S15 — Collection management
1. Pin a collection (📎 icon) → new imports auto-added
2. Delete a collection → images remain in library
3. Right-click image → "Remove from Collection" (when in collection view)

---

## GUI — Search & Filtering

### S16 — Command bar search
1. Press `/` or `⌘F` → search bar appears (Grid view only)
2. Type natural language query (e.g. "landscape 4 stars")
3. Filter rules appear in RuleBuilder
4. Grid updates to show matching images
5. `Escape` closes/clears search

### S17 — Sidebar filters
1. Click size filter buttons (All, >64, >256, >512, >1024)
2. Grid updates to show only images matching size threshold
3. Toggle "Show missing files"

### S18 — Detection class filter
1. Click a detected class tag in sidebar (e.g. "person")
2. Grid filters to images containing that detection

---

## GUI — Command Palette

### S19 — Command palette
1. `⌘P` or **View > Command Palette...** opens palette with commands only
2. `⌘K` opens palette with all items (views, commands, folders, collections)
3. `⌘+Shift+P` opens with commands only
4. Type to fuzzy-filter; non-matching rows are hidden
5. `↑/↓` navigates; `Enter` executes the selected command
6. `Escape` closes
7. The last 5 commands launched through the palette appear first on an empty query

### S20 — Custom hotkeys
1. Open palette → right-click a command → "Set Hotkey"
2. Press a key combo → saved
3. Close palette → press the hotkey → command executes

---

## GUI — Import

### S21 — Folder import
1. Click "Import Folder" in sidebar → OS folder picker
2. Progress events stream (counter updates)
3. Import banner appears showing batch
4. Images appear in grid and sidebar folder tree

### S22 — Drag-and-drop import
1. Drag image files onto app window
2. Blue overlay appears ("Drop to import")
3. Drop → images imported, toast confirmation
4. Dropping a folder imports its contents (context-aware folder drop)

### S23 — Open with
1. Right-click an image in Finder → Open With → Cull
2. App opens/focuses with that image

---

## GUI — Image Operations

### S24 — Crop (Loupe)
1. Press `c` in Loupe → crop overlay appears
2. Drag handles to adjust crop area
3. `Enter` applies crop
4. `Escape` cancels crop

### S25 — Rotation (Loupe)
1. Press `[` → image rotates 90° counter-clockwise
2. Press `]` → image rotates 90° clockwise
3. Rotation persists

### S26 — Trash
1. Press `Backspace` → confirmation dialog
2. Confirm → image moved to trash, toast shown
3. `⌘+Backspace` → permanent delete (separate confirmation)
4. Undo reverts trash

### S27 — Context menu
1. Right-click image → full context menu appears
2. Rate submenu → set stars
3. Add to Collection submenu → pick/create collection
4. Copy submenu → path/filename/URL copied to clipboard
5. Reveal in Finder → Finder window opens at file location
6. Open With → submenu lists compatible apps
7. Rename → dialog → file renamed
8. Move to → folder picker or search
9. Find Similar → grid re-scoped to similar images
10. Keyboard navigation in menu (arrows, Enter, Escape)

---

## GUI — UI Chrome

### S28 — Sidebar toggle
1. `⌘B` or `\` toggles sidebar visibility
2. Sidebar content: sessions, folders, filters, AI models, collections
3. Folder tree expands/collapses correctly

### S29 — Zen mode
1. `>` (Shift+.) → tab bar, sidebar, status bar hidden
2. Only main view content visible
3. `Escape` exits zen mode
4. Works in all view modes

### S30 — Fullscreen
1. Press `f` → browser/app goes fullscreen
2. `Escape` exits fullscreen
3. Combines with zen mode for maximum immersion

### S31 — Undo / Redo
1. Make a rating change → `⌘Z` → reverted, toast shows "Undone: {label}"
2. `⌘+Shift+Z` → re-applied, toast shows "Redone: {label}"
3. Works across rating, decision, and collection changes

---

## GUI — NSFW & Detection

### S32 — Detection overlays
1. Press `d` → green bounding boxes appear on detected objects
2. Press `d` again → boxes hidden
3. Press `i` (Loupe/Compare) → detection inspector panel opens

### S33 — NSFW mode cycling
1. Press `b` (non-grid) → cycles blur → hide → show
2. In blur mode: NSFW images blurred with overlay text
3. Hold `Space` in Loupe → temporarily reveals blurred image
4. In hide mode: NSFW images not shown at all

---

## GUI — AI & Embeddings

### S34 — AI model configuration
1. Open Settings → AI
2. Provider Credentials is followed by Local Models, then Embedding Models
3. Select the YOLO variant and configure the Ollama vision model
4. Third-party weights (YOLO, NudeNet) require a user-supplied local ONNX path

### S35 — Batch detection
1. Run "Detect Objects in Library" from the command palette
2. Only images pending the selected YOLO model are processed
3. Progress appears in JobProgressPanel
4. After completion, detected-object filters appear under sidebar Filters

### S36 — Embedding generation
1. Open Embedding Explorer → select provider
2. Click generate → job runs with progress
3. After completion, scatter plot renders

### S37 — Find similar
1. Right-click image → Find Similar
2. Grid re-scopes to show visually similar images (cosine similarity)

---

## GUI — Settings & Infrastructure

### S38 — Settings dialog
1. `⌘,` or gear icon → settings modal opens
2. Tabs appear in order: General, Appearance, AI, Agent Access, Privacy, Plugins
3. AI owns credentials and model configuration; Agent Access owns skill installation, MCP, and tokens
4. The Cull skill instructions can be copied for npx, Claude, Codex, or another agent; Cull never executes installers
5. `Escape` closes

### S39 — Session management
1. SessionSwitcher dropdown → create new session
2. Switch between sessions → grid scope changes
3. Canvas list updates per session
4. Delete session (with/without files)

### S40 — Job progress
1. Start a background job (detect, embed, thumbnail regen)
2. Floating panel shows progress bar, percentage
3. Pause/Resume/Cancel buttons work
4. Multiple jobs tracked simultaneously

---

## GUI — Export

### S41 — Slide export
1. Select images → switch to Export view
2. Choose template (bleed/editorial/terminal)
3. Export renders slides with progress
4. Output PNGs / PDF saved to chosen location

### S42 — Static publishing
1. Settings → Modules → enable Static Publishing
2. Publish appears before Export in the top tab bar, command palette, and native View menu
3. Publish → choose scenario: local preview, client review link, agent handoff, or static host package
4. Configure site title, description, QR/share URL, related links, search-engine indexing, and image variants
5. Build Static Site → writes a portable `site/` folder with `index.html`, `robots.txt`, QR, manifest, images, and handoff notes
6. Optional: start local HTTP server to preview the generated site

---

## Edge Cases & Regressions

### S43 — Empty states
1. No images imported → appropriate empty state message in Grid
2. No matching filter results → "No results" indicator
3. No embeddings generated → Explorer shows setup prompt
4. Clipboard monitor idle → empty state shown

### S44 — Large library performance
1. 1000+ images → grid virtualizes correctly (no jank)
2. Scrolling is smooth
3. Thumbnail loading is lazy and progressive

### S45 — Persistence
1. Close and reopen app → last view mode restored
2. Focused image index restored
3. Active smart collection restored

### S46 — Missing files
1. Delete a file from disk outside app
2. "Show missing files" checkbox → missing files visible
3. Missing files indicated visually

### S47 — Cloud-evicted files
1. iCloud-evicted files in imported folder
2. Warning toast appears about cloud-evicted files

### S48 — CLI / GUI / MCP consistency
1. A rating set via `set_rating` (MCP) shows in the GUI after refresh
2. A folder imported via `import_folder` (CLI) appears in the GUI sidebar tree
3. Same operation via CLI, MCP, and GUI produces the same DB state
   (single canonical Rust implementation — CLI = GUI = URL = MCP)
