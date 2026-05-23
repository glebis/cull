# Cull — Key Test Scenarios

## Navigation & Views

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

## Ratings & Decisions

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

## Selection & Collections

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

## Search & Filtering

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

## Command Palette

### S19 — Command palette
1. `⌘K` opens palette with all items (views, commands, collections)
2. `⌘+Shift+P` opens with commands only
3. Type to filter; `↑/↓` to navigate; `Enter` to execute
4. `Escape` closes
5. Recently used items appear first

### S20 — Custom hotkeys
1. Open palette → right-click a command → "Set Hotkey"
2. Press a key combo → saved
3. Close palette → press the hotkey → command executes

---

## Import

### S21 — Folder import
1. Click "Import Folder" in sidebar → OS folder picker
2. Progress events stream (counter updates)
3. Import banner appears showing batch
4. Images appear in grid and sidebar folder tree

### S22 — Drag-and-drop import
1. Drag image files onto app window
2. Blue overlay appears ("Drop to import")
3. Drop → images imported, toast confirmation

### S23 — Open with
1. Right-click an image in Finder → Open With → Cull
2. App opens/focuses with that image

---

## Image Operations

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

## UI Chrome

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

## NSFW & Detection

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

## AI & Embeddings

### S34 — Model download
1. In sidebar AI Models section, click download for YOLO/NudeNet
2. Progress bar shows download
3. After download, "Detect" / "Analyze" buttons become active

### S35 — Batch detection
1. Click "Detect" → job starts, progress in JobProgressPanel
2. Pause/Resume/Cancel job
3. After completion, detection tags appear in sidebar

### S36 — Embedding generation
1. Open Embedding Explorer → select provider
2. Click generate → job runs with progress
3. After completion, scatter plot renders

### S37 — Find similar
1. Right-click image → Find Similar
2. Grid re-scopes to show visually similar images (cosine similarity)

---

## Settings & Infrastructure

### S38 — Settings dialog
1. `⌘,` or gear icon → settings modal opens
2. General tab: MCP tokens, HTTP server, API keys
3. Appearance tab: icon variants
4. Privacy tab: data flow status, audit log
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

## Export

### S41 — Slide export
1. Select images → switch to Export view
2. Choose template (bleed/editorial/terminal)
3. Export renders slides with progress
4. Output PNGs / PDF saved to chosen location

### S42 — Static publishing
1. Settings → Static Publishing tab
2. Configure output options (thumbnails, web, full)
3. Export → generates portable web gallery
4. Optional: start local HTTP server to preview

---

## Edge Cases & Regressions

### S43 — Empty states
1. No images imported → appropriate empty state message in Grid
2. No matching filter results → "No results" indicator
3. No embeddings generated → Explorer shows setup prompt

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
