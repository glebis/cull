# Library Health Check: Auto-Purge & Thumbnail Regeneration

## Problem

When source image files are moved or deleted, the library shows broken thumbnails (filename text fallback). No mechanism exists to clean up orphaned DB entries or regenerate missing thumbnails.

## Solution

A "library health check" that runs on every library load:

1. **Purge orphaned entries** — delete DB records for images whose source files no longer exist (default: auto-purge enabled)
2. **Batch regenerate missing thumbnails** — for images that have source files but no thumbnails, regenerate in background with progress bar
3. **Lazy per-image fallback** — if a thumbnail fails to load at render time, attempt single-image regeneration before showing filename fallback

## Architecture

### New Tauri Commands

#### `check_library_health`

Single-pass scan of all images:
- For each image, check if source file exists on disk
- **Source missing + auto_purge enabled**: delete the image record (CASCADE handles image_files, selections, embeddings, etc.) and delete orphaned thumbnail files
- **Source missing + auto_purge disabled**: count but don't delete
- **Source exists + thumbnail missing**: collect image ID for regeneration

Returns:
```rust
#[derive(serde::Serialize)]
struct LibraryHealthResult {
    purged: u32,
    missing_sources: u32,     // only populated when auto_purge is off
    to_regenerate: Vec<String>, // image IDs needing thumbnails
}
```

Uses existing `app_settings` table for `auto_purge_missing` setting (default: `"true"`).

#### `regenerate_thumbnails_by_ids`

- Takes `Vec<String>` of image IDs
- Looks up source path for each, calls existing `generate_thumbnail()`
- Emits `thumbnail-progress` events (reuses existing `ThumbnailProgress` struct)
- Returns count of successfully regenerated thumbnails

#### `regenerate_single_thumbnail`

- Takes one `image_id`
- Looks up source path from DB, generates thumbnail
- Returns the new thumbnail path as `String` on success
- Returns error if source file is missing

### Frontend Changes

#### Startup flow (`+page.svelte`)

After images load:
1. Call `check_library_health()`
2. If `purged > 0`: show toast "Removed N orphaned images from library", refresh image list
3. If `to_regenerate.length > 0`: call `regenerate_thumbnails_by_ids(ids)`, show progress bar
4. Listen for `thumbnail-progress` events to update progress bar
5. On completion, refresh affected thumbnails reactively

#### Per-image fallback (`Thumbnail.svelte`)

Current flow:
```
onerror → imgError = true → show filename
```

New flow:
```
onerror → call regenerate_single_thumbnail(image_id)
        → success: update src to new thumbnail path
        → failure: imgError = true → show filename
```

Show a subtle loading indicator (opacity pulse) during regeneration attempt.

### Settings

- `auto_purge_missing`: stored in `app_settings` table, defaults to `true`
- Accessible via existing `getAppSetting`/`setAppSetting` API
- When `false`, health check reports count but doesn't delete; frontend could show a confirmation prompt (future enhancement)

## Data Flow

```
App startup
    → load images
    → check_library_health()
    ├── purge orphans (if auto_purge=true)
    │   └── toast: "Removed N images"
    │   └── refresh image list
    └── collect missing thumbs
        └── regenerate_thumbnails_by_ids(ids)
            └── progress bar
            └── thumbnails pop in as ready

Image renders
    → <img> onerror fires
    → regenerate_single_thumbnail(id)
    ├── success → update src
    └── failure → show filename fallback
```

## Edge Cases

- **Source deleted**: both batch and lazy paths gracefully fall to filename text
- **Concurrent regeneration**: batch + lazy hitting same image is safe — second write overwrites identical file
- **Empty library**: health check returns zeros, no UI shown
- **Large libraries**: batch is async with progress bar, no blocking
- **Thumbnail dir missing**: `thumbnail_dir()` already calls `create_dir_all`

## Existing Infrastructure Reused

- `generate_thumbnail()` in `thumbnails.rs` — no changes needed
- `ThumbnailProgress` struct and `thumbnail-progress` event pattern
- `app_settings` table for `auto_purge_missing` preference
- `DELETE FROM images WHERE id = ?1` with CASCADE cleanup
- `enrich_thumbnails()` for post-regeneration path population
