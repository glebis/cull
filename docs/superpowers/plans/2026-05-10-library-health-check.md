# Library Health Check Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** On every library load, auto-purge DB entries for missing source files and regenerate missing thumbnails with progress feedback.

**Architecture:** A new `check_library_health` Tauri command scans all images in one pass — deleting orphaned DB records and collecting IDs needing thumbnail regeneration. A `regenerate_thumbnails_by_ids` command handles batch regen with progress events. `Thumbnail.svelte` gets a lazy per-image fallback via `regenerate_single_thumbnail`. The existing `JobProgressPanel` and `showToast` handle all UI feedback.

**Tech Stack:** Rust (Tauri commands), Svelte 5 (runes), TypeScript API layer

---

### Task 1: Add `check_library_health` Tauri command

**Files:**
- Modify: `src-tauri/src/commands/library.rs`
- Modify: `src-tauri/src/lib.rs:222` (invoke handler registration)

- [ ] **Step 1: Add the health check command to `library.rs`**

Add at the end of `src-tauri/src/commands/library.rs`, before any `#[cfg(test)]` block:

```rust
#[derive(Clone, serde::Serialize)]
pub struct LibraryHealthResult {
    pub purged: u32,
    pub missing_sources: u32,
    pub to_regenerate: Vec<String>,
}

#[tauri::command]
pub async fn check_library_health(
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<LibraryHealthResult, String> {
    let db = &state.db;
    let app_data_dir = &state.app_data_dir;

    let auto_purge = db.get_setting("auto_purge_missing")
        .unwrap_or(None)
        .unwrap_or_else(|| "true".to_string());
    let auto_purge = auto_purge == "true";

    let images = db.list_images(100000, 0).map_err(|e| e.to_string())?;
    let total = images.len() as u32;
    let mut purged = 0u32;
    let mut missing_sources = 0u32;
    let mut to_regenerate = Vec::new();

    for (i, img) in images.iter().enumerate() {
        let source_path = std::path::Path::new(&img.path);
        if !source_path.exists() {
            if auto_purge {
                let conn = db.conn.lock().unwrap();
                let _ = conn.execute("DELETE FROM images WHERE id = ?1", rusqlite::params![img.image.id]);
                // Clean up orphaned thumbnail files
                let thumb = crate::db_core::thumbnails::thumbnail_path(app_data_dir, &img.image.id);
                if thumb.exists() {
                    let _ = std::fs::remove_file(&thumb);
                }
                for &size in &crate::db_core::thumbnails::THUMBNAIL_SIZES {
                    let sized = crate::db_core::thumbnails::sized_thumbnail_path(app_data_dir, &img.image.id, size);
                    if sized.exists() {
                        let _ = std::fs::remove_file(&sized);
                    }
                }
                purged += 1;
            } else {
                missing_sources += 1;
            }
        } else {
            let thumb = crate::db_core::thumbnails::thumbnail_path(app_data_dir, &img.image.id);
            if !thumb.exists() {
                to_regenerate.push(img.image.id.clone());
            }
        }

        if i % 100 == 0 {
            let _ = app.emit("health-check-progress", serde_json::json!({
                "current": i + 1, "total": total
            }));
        }
    }

    Ok(LibraryHealthResult { purged, missing_sources, to_regenerate })
}
```

You'll need to add `use tauri::{AppHandle, Emitter, Manager, State};` at the top of the file if not already present. Check existing imports first — the file likely already has `State` but may need `AppHandle` and `Emitter`.

- [ ] **Step 2: Register the command in `lib.rs`**

In `src-tauri/src/lib.rs`, add `commands::library::check_library_health,` to the `invoke_handler` block, after the existing `commands::library::` entries (around line 237).

- [ ] **Step 3: Verify it compiles**

Run: `cd src-tauri && cargo check 2>&1 | tail -5`
Expected: no errors

- [ ] **Step 4: Commit**

```bash
git add src-tauri/src/commands/library.rs src-tauri/src/lib.rs
git commit -m "feat: add check_library_health Tauri command"
```

---

### Task 2: Add `regenerate_thumbnails_by_ids` Tauri command

**Files:**
- Modify: `src-tauri/src/commands/import.rs`
- Modify: `src-tauri/src/lib.rs` (invoke handler registration)

- [ ] **Step 1: Add the command to `import.rs`**

Add after the existing `regenerate_thumbnails` function (around line 182):

```rust
#[tauri::command]
pub async fn regenerate_thumbnails_by_ids(
    app: AppHandle,
    state: State<'_, AppState>,
    image_ids: Vec<String>,
) -> Result<u32, String> {
    let db = &state.db;
    let app_data_dir = &state.app_data_dir;
    let total = image_ids.len() as u32;
    let mut regenerated = 0u32;

    for (i, image_id) in image_ids.iter().enumerate() {
        let id_refs: Vec<&str> = vec![image_id.as_str()];
        if let Ok(found) = db.get_images_by_ids(&id_refs) {
            if let Some(img) = found.first() {
                let source_path = std::path::Path::new(&img.path);
                if source_path.exists() {
                    match crate::db_core::thumbnails::generate_thumbnail(source_path, app_data_dir, &img.image.id) {
                        Ok(_) => regenerated += 1,
                        Err(e) => eprintln!("Thumbnail failed for {}: {}", img.path, e),
                    }
                }
            }
        }
        let _ = app.emit("thumbnail-progress", ThumbnailProgress {
            current: (i + 1) as u32,
            total,
        });
    }

    Ok(regenerated)
}
```

Note: `ThumbnailProgress` struct is already defined in `import.rs` at line 150.

- [ ] **Step 2: Register the command in `lib.rs`**

In `src-tauri/src/lib.rs`, add `commands::import::regenerate_thumbnails_by_ids,` after the `commands::import::regenerate_thumbnails,` line.

- [ ] **Step 3: Verify it compiles**

Run: `cd src-tauri && cargo check 2>&1 | tail -5`
Expected: no errors

- [ ] **Step 4: Commit**

```bash
git add src-tauri/src/commands/import.rs src-tauri/src/lib.rs
git commit -m "feat: add regenerate_thumbnails_by_ids Tauri command"
```

---

### Task 3: Add `regenerate_single_thumbnail` Tauri command

**Files:**
- Modify: `src-tauri/src/commands/import.rs`
- Modify: `src-tauri/src/lib.rs` (invoke handler registration)

- [ ] **Step 1: Add the command to `import.rs`**

Add after `regenerate_thumbnails_by_ids`:

```rust
#[tauri::command]
pub async fn regenerate_single_thumbnail(
    state: State<'_, AppState>,
    image_id: String,
) -> Result<String, String> {
    let db = &state.db;
    let app_data_dir = &state.app_data_dir;
    let id_refs: Vec<&str> = vec![image_id.as_str()];
    let found = db.get_images_by_ids(&id_refs).map_err(|e| e.to_string())?;
    let img = found.first().ok_or_else(|| format!("Image '{}' not found", image_id))?;
    let source_path = std::path::Path::new(&img.path);
    if !source_path.exists() {
        return Err(format!("Source file missing: {}", img.path));
    }
    let thumb_path = crate::db_core::thumbnails::generate_thumbnail(source_path, app_data_dir, &image_id)?;
    Ok(thumb_path.to_string_lossy().to_string())
}
```

- [ ] **Step 2: Register the command in `lib.rs`**

In `src-tauri/src/lib.rs`, add `commands::import::regenerate_single_thumbnail,` after the `regenerate_thumbnails_by_ids` line.

- [ ] **Step 3: Verify it compiles**

Run: `cd src-tauri && cargo check 2>&1 | tail -5`
Expected: no errors

- [ ] **Step 4: Commit**

```bash
git add src-tauri/src/commands/import.rs src-tauri/src/lib.rs
git commit -m "feat: add regenerate_single_thumbnail Tauri command"
```

---

### Task 4: Add TypeScript API functions

**Files:**
- Modify: `src/lib/api.ts`

- [ ] **Step 1: Add the three new API functions**

Add after the existing `regenerateThumbnails` function (around line 108) in `src/lib/api.ts`:

```typescript
export interface LibraryHealthResult {
    purged: number;
    missing_sources: number;
    to_regenerate: string[];
}

export async function checkLibraryHealth(): Promise<LibraryHealthResult> {
    return invoke<LibraryHealthResult>('check_library_health');
}

export async function regenerateThumbnailsByIds(imageIds: string[]): Promise<number> {
    return invoke<number>('regenerate_thumbnails_by_ids', { imageIds });
}

export async function regenerateSingleThumbnail(imageId: string): Promise<string> {
    return invoke<string>('regenerate_single_thumbnail', { imageId });
}
```

- [ ] **Step 2: Verify TypeScript compiles**

Run: `npx svelte-check --threshold error 2>&1 | tail -10`
Expected: no errors

- [ ] **Step 3: Commit**

```bash
git add src/lib/api.ts
git commit -m "feat: add library health check API functions"
```

---

### Task 5: Wire health check into app startup

**Files:**
- Modify: `src/routes/+page.svelte`

- [ ] **Step 1: Import the new API functions**

In `src/routes/+page.svelte`, update the import from `$lib/api` (line 23) to include the new functions:

Add `checkLibraryHealth, regenerateThumbnailsByIds` to the existing import destructure.

- [ ] **Step 2: Add the health check call after `loadImages`**

In the `onMount` block (around line 126), modify the `init` function. Replace:

```typescript
        const init = async () => {
            const restored = restoreAppStateBeforeImages();
            await loadImages();
            applyRestoredViewState(restored);
            await initDeepLink();
        };
```

With:

```typescript
        const init = async () => {
            const restored = restoreAppStateBeforeImages();
            await loadImages();
            applyRestoredViewState(restored);
            await initDeepLink();

            try {
                const health = await checkLibraryHealth();
                if (health.purged > 0) {
                    showToast(`Cleaned up library`, {
                        detail: `Removed ${health.purged} image${health.purged === 1 ? '' : 's'} with missing source files`,
                        type: 'info',
                        duration: 7000,
                    });
                    await loadImages();
                }
                if (health.to_regenerate.length > 0) {
                    regenerateThumbnailsByIds(health.to_regenerate).then((count) => {
                        if (count > 0) {
                            loadImages();
                        }
                    });
                }
            } catch (e) {
                console.error('Library health check failed:', e);
            }
        };
```

The `regenerateThumbnailsByIds` call is intentionally not awaited — it runs in the background. The existing `JobProgressPanel` picks up `thumbnail-progress` events automatically. When complete, `loadImages()` refreshes the grid so regenerated thumbnails appear.

- [ ] **Step 3: Verify TypeScript compiles**

Run: `npx svelte-check --threshold error 2>&1 | tail -10`
Expected: no errors

- [ ] **Step 4: Commit**

```bash
git add src/routes/+page.svelte
git commit -m "feat: run library health check on startup"
```

---

### Task 6: Add `thumbnail-progress` listener to `JobProgressPanel`

**Files:**
- Modify: `src/lib/components/JobProgressPanel.svelte`

- [ ] **Step 1: Add listener for `thumbnail-progress`**

In `JobProgressPanel.svelte`, inside the `onMount` block, after the `u7` listener (line 46), add:

```typescript
            const u8 = await listen<any>('thumbnail-progress', (e) => {
                upsertJob(e.payload.job_id ?? `evt_thumbnails`, 'thumbnails', 'running', e.payload.current, e.payload.total, null);
            });
            unlisteners = [u1, u2, u3, u4, u5, u6, u7, u8];
```

Update the existing `unlisteners = [u1, u2, u3, u4, u5, u6, u7];` line to include `u8`.

- [ ] **Step 2: Add label for the new job kind**

In the `kindLabel` function (around line 93), add `thumbnails: 'Thumbnails',` to the labels record:

```typescript
    function kindLabel(kind: string): string {
        const labels: Record<string, string> = {
            import: 'Import',
            embeddings: 'Embeddings',
            detection: 'Detection',
            vision: 'Vision',
            rescan: 'Rescan',
            thumbnails: 'Thumbnails',
        };
        return labels[kind] ?? kind;
    }
```

- [ ] **Step 3: Commit**

```bash
git add src/lib/components/JobProgressPanel.svelte
git commit -m "feat: show thumbnail regeneration progress in JobProgressPanel"
```

---

### Task 7: Add lazy per-image fallback in `Thumbnail.svelte`

**Files:**
- Modify: `src/lib/components/Thumbnail.svelte`

- [ ] **Step 1: Add the regeneration import and logic**

In `Thumbnail.svelte`, add the import at the top of the `<script>` block:

```typescript
    import { regenerateSingleThumbnail } from '$lib/api';
```

Replace the existing `imgError` state and `handleImgError` function:

```typescript
    let imgError = $state(false);
    let regenerating = $state(false);
```

Replace the `handleImgError` function:

```typescript
    async function handleImgError() {
        if (regenerating) return;
        regenerating = true;
        try {
            const newPath = await regenerateSingleThumbnail(item.image.id);
            item.thumbnail_path = newPath;
        } catch {
            imgError = true;
        } finally {
            regenerating = false;
        }
    }
```

- [ ] **Step 2: Update the template for the regenerating state**

Replace the existing `{#if imgError}` block (lines 69-73):

```svelte
    {#if imgError}
        <div class="fallback-text">{filename}</div>
    {:else if regenerating}
        <div class="regenerating"></div>
    {:else}
        <img {src} alt={filename} loading="lazy" draggable="false" onerror={handleImgError} />
    {/if}
```

- [ ] **Step 3: Add the regenerating animation style**

Add to the `<style>` block:

```css
    .regenerating {
        width: 24px;
        height: 24px;
        border: 2px solid var(--border);
        border-top-color: var(--blue);
        border-radius: 50%;
        animation: spin 0.8s linear infinite;
    }
    @keyframes spin {
        to { transform: rotate(360deg); }
    }
```

- [ ] **Step 4: Verify TypeScript compiles**

Run: `npx svelte-check --threshold error 2>&1 | tail -10`
Expected: no errors

- [ ] **Step 5: Commit**

```bash
git add src/lib/components/Thumbnail.svelte
git commit -m "feat: lazy thumbnail regeneration on load failure"
```

---

### Task 8: Tauri mock for browser testing

**Files:**
- Modify: the Tauri mock layer (check `src/lib/` for the mock setup — likely in the file that defines `isTauri()` and mock handlers)

- [ ] **Step 1: Find and update the mock layer**

Run: `grep -rn "isTauri\|mockIPC\|mock.*invoke" src/lib/ --include="*.ts" | head -10`

Add mock handlers for the three new commands:

```typescript
// In the mock registration section:
'check_library_health': async () => ({ purged: 0, missing_sources: 0, to_regenerate: [] }),
'regenerate_thumbnails_by_ids': async () => 0,
'regenerate_single_thumbnail': async () => '',
```

The exact syntax depends on the mock framework used in this project. Match the existing pattern.

- [ ] **Step 2: Commit**

```bash
git add src/lib/<mock-file>.ts
git commit -m "feat: add Tauri mocks for library health check commands"
```

---

### Task 9: Build and manual test

- [ ] **Step 1: Full Rust build**

Run: `cd src-tauri && cargo build 2>&1 | tail -10`
Expected: successful build

- [ ] **Step 2: Full frontend check**

Run: `npx svelte-check --threshold error 2>&1 | tail -10`
Expected: no errors

- [ ] **Step 3: Run existing tests**

Run: `cd src-tauri && cargo test 2>&1 | tail -20`
Expected: all existing tests pass

- [ ] **Step 4: Manual testing checklist**

Start the dev server (`npm run tauri dev`) and verify:

1. App loads — check browser console for health check log
2. If images with missing source files exist, toast appears with count
3. If thumbnails are missing, progress bar appears in `JobProgressPanel`
4. Thumbnails pop in as they regenerate
5. Individual broken thumbnails show spinner then either load or show filename
6. Previously working thumbnails still display correctly (no regression)

- [ ] **Step 5: Final commit if any fixes needed**
