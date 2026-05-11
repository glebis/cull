# Import Sessions & Image Lineage

Connects CLI/file-open import batches, auto-collections, and image lineage tracking so users can efficiently cull AI-generated image variants.

## Problem

When opening multiple images via CLI (`open -a imageview file1.png file2.png`), they get imported into the global database with no distinction. Users iterating on AI image generation (GPT Image, DALL-E, Midjourney) need to:
- See only what they just imported, not the entire library
- Accumulate iterations across multiple import sessions into one workspace
- Understand which images are variants of each other (lineage)
- Compare and cull variants efficiently

## Data Model

### New table: `lineage_groups`

Flat sibling groups — "these images are variants of each other."

```sql
CREATE TABLE lineage_groups (
    id TEXT PRIMARY KEY,
    name TEXT,                    -- auto: "icon-v5 series", user can rename
    created_at TEXT NOT NULL,
    detection_method TEXT,        -- 'auto' | 'manual' | 'import_batch'
    detection_score REAL          -- confidence 0-100
);
```

### New table: `import_batches`

Tracks images imported together in a single action.

```sql
CREATE TABLE import_batches (
    id TEXT PRIMARY KEY,
    created_at TEXT NOT NULL,
    source TEXT,                  -- 'cli' | 'drag_drop' | 'folder' | 'deep_link'
    image_count INTEGER,
    collection_id TEXT            -- if auto-added to active collection
);
```

### Altered table: `images`

```sql
ALTER TABLE images ADD COLUMN lineage_group_id TEXT REFERENCES lineage_groups(id);
ALTER TABLE images ADD COLUMN lineage_order INTEGER DEFAULT 0;
ALTER TABLE images ADD COLUMN import_batch_id TEXT REFERENCES import_batches(id);
```

### Existing table: `iterations` (unchanged)

Kept for explicit directed parent→child relationships with prompt/model metadata. Lineage groups are auto-detected flat siblings; iterations are user-curated directed edges.

### Concept relationships

- **Lineage group**: "these are variants" — flat, auto-detected or manual
- **Collection**: "these belong to a project" — user-curated, ordered, can contain multiple lineage groups
- **Iteration**: "this was derived from that" — directed edge, user-set, stores prompt/model

An image belongs to one lineage group (or none) and can be in multiple collections.

## Detection Pipeline

Runs at import time. Two phases:

### Phase 1: Intra-batch grouping (always runs)

Compare images within the current import batch against each other.

Scoring signals:

| Signal | Score | Notes |
|--------|-------|-------|
| Same prompt in PNG metadata | +50 | SD/ComfyUI tEXt chunks, fuzzy match |
| Filename stem match | +25 | Strip version suffixes, compare stems |
| Same import batch | +10 | Imported together via CLI/drag-drop |
| Temporal proximity (<60s) | +10 | File creation timestamps within 1 min |
| CLIP cosine similarity >0.85 | +15 | Visual embedding distance |
| Same dimensions | +5 | Width × height match |

Thresholds:
- Score ≥50: auto-link into lineage group
- Score 25–49: suggest grouping (toast with Yes/No)
- Score <25: ignore

### Phase 2: Cross-library matching (forward-only by default)

Check new images against recent imports (last 7 days, same folder) using the same scoring. Merges into existing lineage groups when matches are found.

Group merging: if image C links group A and group B, merge both into one group.

### Retroactive scan

Manual action: "Scan library for lineage groups" in settings/sidebar. Runs Phase 2 across all images. Shows progress bar, can be cancelled. Writes auto-detected groups, user reviews suggestions before confirming.

### Filename stem extraction

Normalize filenames by stripping trailing version markers:

- Sequential: `image(1).png` → `image`
- Letter suffixes: `icon-v5a.png` → `icon-v5`
- Number suffixes: `favicon-v2.png` → `favicon`
- DALL-E timestamps: `DALL·E 2026-05-09 14.32.01.png` → group by same date, within 10 min
- ComfyUI batch: `ComfyUI_00042_.png` → sequential batch numbers
- Midjourney: `_V1`, `_V2` suffixes, grid variants
- Manual versioning: strip `-final`, ` (copy)`, `-v2`, `_v3`

## Import Flow

```
Files arrive (CLI / drag-drop / folder)
  → Create import_batch record
  → Import each file to DB (existing flow)
  → Run lineage detection (Phase 1 + 2)
  → If active collection pinned:
      → Append to it silently
      → Toast: "4 images added to 'Icon Exploration'" — Undo · Move to… · Remove
  → If no active collection:
      → Filter grid to show only this batch
      → Banner: "4 images imported | Save as collection | Show all"
  → If auto-create-collection setting ON:
      → Create collection named from folder or filename stem
      → Switch to it
```

### Import batch filter

When no active collection, the grid auto-filters to show only the current batch. This is a transient UI state (stored in a Svelte store, not persisted). The banner provides:
- **Save as collection**: creates a manual collection from the batch, pins it as active
- **Show all**: clears the filter, returns to full library view

### Active collection

A collection marked as "active" / "pinned" in the UI. New imports append to it automatically.

- Pin via: right-click collection → "Pin as active", or pin icon in sidebar
- Unpin via: click unpin in sidebar indicator or toast
- Indicator: sidebar shows pinned collection name with 📌 icon
- Persists across app restarts (stored in app preferences)

Both session-based (pin, work, unpin) and project-based (keep pinned for days/weeks) workflows use the same mechanism.

### Toast actions

All toast actions are one-click, immediately reversible:
- **Undo**: removes images from collection, restores previous state
- **Move to…**: opens collection picker dropdown
- **Remove**: removes from collection (images stay in library)

Toast auto-dismisses after 8 seconds.

## Lineage Tab (⌘5)

Context-aware: shows lineage groups within the currently active context (collection, folder, or all images). Two switchable layouts:

### Timeline layout (default)

- Horizontal strips per lineage group, stacked vertically, newest first
- Each strip: group header (name, count, date, source model) → thumbnails left→right by creation order
- Inline star ratings below each thumbnail
- Pick/reject badges on thumbnails
- Ungrouped images shown at bottom in a separate "Ungrouped" section

### Comparison layout

- Pill/tab selector for lineage groups at top of view
- Selected group fills the view as a large comparison grid (2×2, 3×3, etc. depending on count)
- Large thumbnails for side-by-side evaluation
- Pick/reject/star inline on each thumbnail
- Left/right arrows or pills to switch between groups

### Layout toggle

Button in the Lineage tab header area to switch between Timeline and Comparison. Preference persisted.

### Interactions (both layouts)

- Click thumbnail → open in Loupe, ← → cycles siblings within lineage group
- Star rating / Pick / Reject work inline
- Right-click group → Rename, Merge with…, Split, Dissolve
- Drag image onto group header to add manually
- Right-click image → "Remove from lineage group"
- Double-click group name to rename inline

## Settings

Three toggles in app preferences:

1. **Auto-create collection on import** — OFF by default. Every import batch creates a named collection. Name derivation: use the parent folder name if all files share one (e.g., "icon-exploration"), otherwise use the common filename stem (e.g., "icon-v5"), otherwise use "Import YYYY-MM-DD HH:MM".
2. **Auto-detect lineage groups** — ON by default. Runs the detection pipeline on import.
3. **Auto-switch to collection after import** — OFF by default. Immediately activates the new/target collection after import.

## Sidebar Changes

- **Active collection indicator**: when a collection is pinned, show it prominently at top of sidebar with 📌 and "Unpin" action
- **Collection context menu**: add "Pin as active" option
- **Scan action**: "Scan for lineage groups" button in sidebar or settings, triggers retroactive scan

## Files to Create/Modify

### Rust (backend)
- `src-tauri/src/db_core/db.rs` — migration: new tables, alter images
- `src-tauri/src/db_core/lineage.rs` — new: lineage group CRUD, detection pipeline, filename stem extraction, scoring
- `src-tauri/src/db_core/import.rs` — modify: create import_batch, run lineage detection after import
- `src-tauri/src/commands/lineage.rs` — new: Tauri commands for lineage operations
- `src-tauri/src/commands/import.rs` — modify: return batch_id, support active collection append

### Svelte (frontend)
- `src/lib/components/LineageView.svelte` — new: the ⌘5 tab with both layouts
- `src/lib/components/ImportBanner.svelte` — new: the transient filter banner
- `src/lib/components/Sidebar.svelte` — modify: active collection indicator, pin/unpin
- `src/lib/components/Toast.svelte` — modify: support action buttons (Undo, Move to, Remove)
- `src/lib/stores.ts` — modify: add activeCollection, importBatchFilter, lineageLayout stores
- `src/lib/deeplink.ts` — modify: create batch on import, handle active collection
- `src/lib/api.ts` — add: lineage group API calls
