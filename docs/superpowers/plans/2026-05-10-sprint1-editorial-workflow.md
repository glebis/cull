# Sprint 1: Editorial Workflow — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add canvas mode (freeform image layout), crop & rotate editing, and prompt display in Loupe — turning the viewer into an editorial tool.

**Architecture:** Three independent features sharing the existing `ImageWithFile` data model. Canvas is a new Svelte component routed via `viewMode`. Crop/rotate adds a Rust backend command + UI overlay in Loupe. Prompt display extends the existing Loupe overlay bar and the Rust `Image` struct to surface `ai_prompt` from the DB.

**Tech Stack:** Svelte 5 (runes), Tauri 2, Rust, SQLite, CSS transforms

---

### Task 1: Prompt Display in Loupe — Backend

**Files:**
- Modify: `src-tauri/src/db_core/models.rs:4-10` (Image struct)
- Modify: `src-tauri/src/db_core/db.rs:234-271` (list_images query + all 7 ImageWithFile query functions)
- Modify: `src/lib/api.ts:3-12` (TypeScript Image interface)

This task adds `ai_prompt` to the data pipeline. The column already exists in the DB (added during migration) and is populated by source detection. It just isn't surfaced through the query → model → frontend path.

- [ ] **Step 1: Add `ai_prompt` to Rust `Image` struct**

In `src-tauri/src/db_core/models.rs`, add the field:

```rust
pub struct Image {
    pub id: String,
    pub sha256_hash: String,
    pub width: u32,
    pub height: u32,
    pub format: String,
    pub file_size: u64,
    pub created_at: String,
    pub imported_at: String,
    pub ai_prompt: Option<String>,  // <-- add this
}
```

- [ ] **Step 2: Update `list_images` query in `db.rs`**

In `src-tauri/src/db_core/db.rs:234`, add `i.ai_prompt` to the SELECT and read it from the row. The column index shifts — `source_label` was at index 12, `ai_prompt` will be 13:

```rust
pub fn list_images(&self, limit: u32, offset: u32) -> Result<Vec<ImageWithFile>> {
    let conn = self.conn.lock().unwrap();
    let mut stmt = conn.prepare(
        "SELECT i.id, i.sha256_hash, i.width, i.height, i.format, i.file_size,
                i.created_at, i.imported_at, f.path,
                s.star_rating, s.color_label, s.decision, i.source_label, i.ai_prompt
         FROM images i
         JOIN image_files f ON f.image_id = i.id AND f.missing_at IS NULL
         LEFT JOIN selections s ON s.image_id = i.id AND s.project_id = '__global__'
         GROUP BY i.id
         ORDER BY i.imported_at DESC
         LIMIT ?1 OFFSET ?2"
    )?;
    let rows = stmt.query_map(params![limit, offset], |row| {
        let star: Option<u8> = row.get(9)?;
        let color: Option<String> = row.get(10)?;
        let decision: Option<String> = row.get(11)?;
        let selection = decision.map(|d| Selection {
            image_id: row.get(0).unwrap(),
            project_id: None,
            star_rating: star,
            color_label: color,
            decision: d,
        });
        Ok(ImageWithFile {
            image: Image {
                id: row.get(0)?,
                sha256_hash: row.get(1)?,
                width: row.get(2)?,
                height: row.get(3)?,
                format: row.get(4)?,
                file_size: row.get(5)?,
                created_at: row.get(6)?,
                imported_at: row.get(7)?,
                ai_prompt: row.get(13)?,
            },
            path: row.get(8)?,
            thumbnail_path: None,
            selection,
            source_label: row.get(12)?,
        })
    })?;
    rows.collect::<Result<Vec<_>>>()
}
```

- [ ] **Step 3: Update all other ImageWithFile query functions**

Apply the same pattern (add `i.ai_prompt` to SELECT, add `ai_prompt: row.get(N)?` to Image construction) to these functions in `db.rs`:
- `list_images_by_folder` (~line 323)
- `list_images_filtered` (~line 369)
- `get_images_by_ids` (~line 675)
- `list_collection_images` (search for it)
- `list_smart_collection_images` (search for it)
- `get_iteration_siblings` / any other ImageWithFile-returning function

Each query adds `i.ai_prompt` as the last column in SELECT, and reads it with the appropriate index.

- [ ] **Step 4: Update TypeScript `Image` interface**

In `src/lib/api.ts:3-12`:

```typescript
export interface Image {
    id: string;
    sha256_hash: string;
    width: number;
    height: number;
    format: string;
    file_size: number;
    created_at: string;
    imported_at: string;
    ai_prompt: string | null;
}
```

- [ ] **Step 5: Build and verify**

Run: `cd src-tauri && cargo build 2>&1 | tail -20`
Expected: Successful compilation. Fix any missed query functions that construct `Image` without `ai_prompt`.

- [ ] **Step 6: Commit**

```bash
git add src-tauri/src/db_core/models.rs src-tauri/src/db_core/db.rs src/lib/api.ts
git commit -m "feat: surface ai_prompt through Image model pipeline"
```

---

### Task 2: Prompt Display in Loupe — Frontend

**Files:**
- Modify: `src/lib/components/Loupe.svelte:15-33` (add prompt derived state)
- Modify: `src/lib/components/Loupe.svelte:230-260` (overlay bar)
- Modify: `src/lib/components/Loupe.svelte:312+` (styles)

- [ ] **Step 1: Add prompt state to Loupe script**

In `src/lib/components/Loupe.svelte`, after line 33 (`sourceDisplay` derived), add:

```typescript
let prompt = $derived(image?.image.ai_prompt ?? null);
let promptExpanded = $state(false);
let promptTruncated = $derived(prompt && prompt.length > 80 ? prompt.slice(0, 80) + '…' : prompt);
```

- [ ] **Step 2: Add prompt indicator to overlay bar**

In `src/lib/components/Loupe.svelte`, inside the `.overlay-bar` div, after the zoom section (~line 258), add:

```svelte
{#if prompt}
    <span class="sep">|</span>
    <button class="prompt-toggle" onclick={() => promptExpanded = !promptExpanded}
            title={promptExpanded ? 'Collapse prompt' : 'Show full prompt'}>
        ✦ {promptExpanded ? 'Hide prompt' : 'Prompt'}
    </button>
{/if}
```

- [ ] **Step 3: Add expandable prompt panel**

After the `.overlay-bar` div closes (after line 260), add:

```svelte
{#if prompt && promptExpanded && !hideOverlays}
    <div class="prompt-panel">
        <div class="prompt-text">{prompt}</div>
    </div>
{/if}
```

- [ ] **Step 4: Add styles**

In the `<style>` section, add:

```css
.prompt-toggle {
    background: none;
    border: none;
    color: rgba(255,255,255,0.7);
    cursor: pointer;
    font-size: 0.75rem;
    padding: 0;
    font-family: inherit;
}
.prompt-toggle:hover {
    color: #fff;
}
.prompt-panel {
    position: absolute;
    bottom: 36px;
    left: 0;
    right: 0;
    background: rgba(0,0,0,0.85);
    padding: 12px 16px;
    font-size: 0.8rem;
    color: rgba(255,255,255,0.9);
    line-height: 1.5;
    max-height: 200px;
    overflow-y: auto;
    backdrop-filter: blur(8px);
    z-index: 10;
}
.prompt-text {
    white-space: pre-wrap;
    word-break: break-word;
    user-select: text;
}
```

- [ ] **Step 5: Test in browser**

Run: `npm run dev` (or `pnpm dev`)
1. Navigate to an AI-generated image that has a prompt in its PNG metadata
2. Enter Loupe mode (press 2 or double-click)
3. Verify: "✦ Prompt" button appears in the overlay bar
4. Click it — prompt panel expands below the overlay
5. Click again — panel collapses
6. Navigate to a non-AI image — no prompt indicator should appear

- [ ] **Step 6: Commit**

```bash
git add src/lib/components/Loupe.svelte
git commit -m "feat: show AI prompt in Loupe overlay with expand/collapse"
```

---

### Task 3: Canvas Mode — Component Shell

**Files:**
- Create: `src/lib/components/Canvas.svelte`
- Modify: `src/routes/+page.svelte:8-9,142-163` (import + route)

- [ ] **Step 1: Create Canvas component skeleton**

Create `src/lib/components/Canvas.svelte`:

```svelte
<script lang="ts">
    import { convertFileSrc } from '@tauri-apps/api/core';
    import { images, focusedIndex, selectedIds, statusHint, navigateBack } from '$lib/stores';
    import type { ImageWithFile } from '$lib/api';

    interface CanvasItem {
        id: string;
        image: ImageWithFile;
        x: number;
        y: number;
        width: number;
        height: number;
    }

    let canvasItems = $state<CanvasItem[]>([]);
    let canvasEl: HTMLDivElement | undefined = $state();
    let panX = $state(0);
    let panY = $state(0);
    let zoom = $state(1);
    let panning = $state(false);
    let panStartX = $state(0);
    let panStartY = $state(0);
    let panOriginX = $state(0);
    let panOriginY = $state(0);

    let dragItem = $state<string | null>(null);
    let dragOffsetX = $state(0);
    let dragOffsetY = $state(0);

    const ITEM_GAP = 20;
    const ITEM_HEIGHT = 200;

    $effect(() => {
        const imgs = $images;
        if (canvasItems.length === 0 && imgs.length > 0) {
            layoutGrid(imgs);
        }
    });

    function layoutGrid(imgs: ImageWithFile[]) {
        const cols = Math.ceil(Math.sqrt(imgs.length));
        canvasItems = imgs.map((img, i) => {
            const aspect = img.image.width / img.image.height;
            const w = ITEM_HEIGHT * aspect;
            const col = i % cols;
            const row = Math.floor(i / cols);
            return {
                id: img.image.id,
                image: img,
                x: col * (w + ITEM_GAP),
                y: row * (ITEM_HEIGHT + ITEM_GAP),
                width: w,
                height: ITEM_HEIGHT,
            };
        });
    }

    function handleCanvasMouseDown(e: MouseEvent) {
        if (e.button === 1 || (e.button === 0 && e.altKey)) {
            panning = true;
            panStartX = e.clientX;
            panStartY = e.clientY;
            panOriginX = panX;
            panOriginY = panY;
            e.preventDefault();
        }
    }

    function handleCanvasMouseMove(e: MouseEvent) {
        if (panning) {
            panX = panOriginX + (e.clientX - panStartX);
            panY = panOriginY + (e.clientY - panStartY);
        } else if (dragItem) {
            const item = canvasItems.find(it => it.id === dragItem);
            if (item) {
                item.x = (e.clientX - panX) / zoom - dragOffsetX;
                item.y = (e.clientY - panY) / zoom - dragOffsetY;
                canvasItems = canvasItems;
            }
        }
    }

    function handleCanvasMouseUp() {
        panning = false;
        dragItem = null;
    }

    function handleItemMouseDown(e: MouseEvent, item: CanvasItem) {
        if (e.button !== 0 || e.altKey) return;
        e.stopPropagation();
        dragItem = item.id;
        dragOffsetX = (e.clientX - panX) / zoom - item.x;
        dragOffsetY = (e.clientY - panY) / zoom - item.y;
    }

    function handleItemDblClick(item: CanvasItem) {
        const idx = $images.findIndex(img => img.image.id === item.id);
        if (idx >= 0) {
            focusedIndex.set(idx);
            navigateBack();
        }
    }

    function handleWheel(e: WheelEvent) {
        e.preventDefault();
        const factor = e.deltaY > 0 ? 0.9 : 1.1;
        const rect = canvasEl?.getBoundingClientRect();
        if (!rect) return;
        const mx = e.clientX - rect.left;
        const my = e.clientY - rect.top;
        const newZoom = Math.max(0.1, Math.min(5, zoom * factor));
        panX = mx - (mx - panX) * (newZoom / zoom);
        panY = my - (my - panY) * (newZoom / zoom);
        zoom = newZoom;
    }

    $effect(() => {
        const count = canvasItems.length;
        statusHint.set(`Canvas — ${count} image${count !== 1 ? 's' : ''} | Zoom: ${Math.round(zoom * 100)}%`);
        return () => statusHint.set(null);
    });
</script>

<div
    class="canvas-viewport"
    bind:this={canvasEl}
    onmousedown={handleCanvasMouseDown}
    onmousemove={handleCanvasMouseMove}
    onmouseup={handleCanvasMouseUp}
    onmouseleave={handleCanvasMouseUp}
    onwheel={handleWheel}
    role="application"
    aria-label="Image canvas"
>
    <div class="canvas-layer" style="transform: translate({panX}px, {panY}px) scale({zoom});">
        {#each canvasItems as item (item.id)}
            <div
                class="canvas-item"
                class:selected={$selectedIds.has(item.id)}
                style="left: {item.x}px; top: {item.y}px; width: {item.width}px; height: {item.height}px;"
                onmousedown={(e) => handleItemMouseDown(e, item)}
                ondblclick={() => handleItemDblClick(item)}
                role="img"
                aria-label={item.image.path.split('/').pop()}
            >
                <img
                    src={item.image.thumbnail_path ? convertFileSrc(item.image.thumbnail_path) : convertFileSrc(item.image.path)}
                    alt=""
                    draggable="false"
                />
            </div>
        {/each}
    </div>
</div>

<style>
    .canvas-viewport {
        grid-area: main;
        overflow: hidden;
        background: #111;
        cursor: grab;
        position: relative;
        user-select: none;
    }
    .canvas-viewport:active {
        cursor: grabbing;
    }
    .canvas-layer {
        position: absolute;
        top: 0;
        left: 0;
        transform-origin: 0 0;
    }
    .canvas-item {
        position: absolute;
        border: 2px solid transparent;
        border-radius: 2px;
        cursor: move;
        transition: border-color 0.1s;
    }
    .canvas-item.selected {
        border-color: #4a9eff;
    }
    .canvas-item:hover {
        border-color: rgba(255,255,255,0.4);
    }
    .canvas-item img {
        width: 100%;
        height: 100%;
        object-fit: cover;
        pointer-events: none;
        display: block;
    }
</style>
```

- [ ] **Step 2: Wire Canvas into the page router**

In `src/routes/+page.svelte`, add the import after line 8:

```typescript
import Canvas from '$lib/components/Canvas.svelte';
```

Then in the template, add a case before the `:else` fallback (before line 159):

```svelte
{:else if $viewMode === 'canvas'}
    <Canvas />
```

- [ ] **Step 3: Build and verify**

Run: `pnpm dev`
1. Import some images (if not already present)
2. Press `4` to switch to Canvas mode
3. Verify: images appear in a grid layout on a dark canvas
4. Drag an image — it repositions
5. Alt+drag or middle-click — pans the canvas
6. Scroll wheel — zooms in/out
7. Double-click an image — navigates back to previous view
8. Press `1` to go back to grid — works

- [ ] **Step 4: Commit**

```bash
git add src/lib/components/Canvas.svelte src/routes/+page.svelte
git commit -m "feat: add Canvas mode with drag-to-arrange and zoom/pan"
```

---

### Task 4: Canvas — Resize Handles

**Files:**
- Modify: `src/lib/components/Canvas.svelte`

- [ ] **Step 1: Add resize state and handlers**

In the `<script>` section of `Canvas.svelte`, add after the drag state variables:

```typescript
let resizeItem = $state<string | null>(null);
let resizeStartX = $state(0);
let resizeStartY = $state(0);
let resizeStartW = $state(0);
let resizeStartH = $state(0);

function handleResizeMouseDown(e: MouseEvent, item: CanvasItem) {
    e.stopPropagation();
    e.preventDefault();
    resizeItem = item.id;
    resizeStartX = e.clientX;
    resizeStartY = e.clientY;
    resizeStartW = item.width;
    resizeStartH = item.height;
}
```

- [ ] **Step 2: Update mousemove to handle resize**

In `handleCanvasMouseMove`, add the resize branch before the drag branch:

```typescript
function handleCanvasMouseMove(e: MouseEvent) {
    if (panning) {
        panX = panOriginX + (e.clientX - panStartX);
        panY = panOriginY + (e.clientY - panStartY);
    } else if (resizeItem) {
        const item = canvasItems.find(it => it.id === resizeItem);
        if (item) {
            const dx = (e.clientX - resizeStartX) / zoom;
            const aspect = item.image.image.width / item.image.image.height;
            item.width = Math.max(50, resizeStartW + dx);
            item.height = item.width / aspect;
            canvasItems = canvasItems;
        }
    } else if (dragItem) {
        const item = canvasItems.find(it => it.id === dragItem);
        if (item) {
            item.x = (e.clientX - panX) / zoom - dragOffsetX;
            item.y = (e.clientY - panY) / zoom - dragOffsetY;
            canvasItems = canvasItems;
        }
    }
}
```

- [ ] **Step 3: Clear resize on mouseup**

In `handleCanvasMouseUp`:

```typescript
function handleCanvasMouseUp() {
    panning = false;
    dragItem = null;
    resizeItem = null;
}
```

- [ ] **Step 4: Add resize handle to template**

Inside the `{#each canvasItems}` block, after the `<img>` element:

```svelte
<div
    class="resize-handle"
    onmousedown={(e) => handleResizeMouseDown(e, item)}
></div>
```

- [ ] **Step 5: Add resize handle styles**

```css
.resize-handle {
    position: absolute;
    bottom: -4px;
    right: -4px;
    width: 12px;
    height: 12px;
    background: #4a9eff;
    border-radius: 2px;
    cursor: nwse-resize;
    opacity: 0;
    transition: opacity 0.15s;
}
.canvas-item:hover .resize-handle {
    opacity: 1;
}
```

- [ ] **Step 6: Test resize**

1. Hover over a canvas item — blue handle appears at bottom-right corner
2. Drag the handle — image resizes proportionally (aspect ratio locked)
3. Release — image stays at new size

- [ ] **Step 7: Commit**

```bash
git add src/lib/components/Canvas.svelte
git commit -m "feat: add proportional resize handles to canvas items"
```

---

### Task 5: Crop — Backend Command

**Files:**
- Create: `src-tauri/src/commands/transform.rs`
- Modify: `src-tauri/src/commands/mod.rs`
- Modify: `src-tauri/src/lib.rs` (register command)

- [ ] **Step 1: Create transform command module**

Create `src-tauri/src/commands/transform.rs`:

```rust
use crate::AppState;
use image::GenericImageView;
use std::path::PathBuf;
use tauri::State;

#[tauri::command]
pub async fn crop_image(
    state: State<'_, AppState>,
    image_id: String,
    x: u32,
    y: u32,
    width: u32,
    height: u32,
    save_as_copy: bool,
) -> Result<String, String> {
    let images = state.db.get_images_by_ids(&[&image_id]).map_err(|e| e.to_string())?;
    let img_record = images.first().ok_or("Image not found")?;
    let path = PathBuf::from(&img_record.path);

    let img = image::open(&path).map_err(|e| format!("Failed to open image: {e}"))?;
    let (img_w, img_h) = img.dimensions();

    if x + width > img_w || y + height > img_h {
        return Err(format!("Crop region ({x},{y},{width},{height}) exceeds image dimensions ({img_w}x{img_h})"));
    }

    let cropped = img.crop_imm(x, y, width, height);

    let output_path = if save_as_copy {
        let stem = path.file_stem().unwrap().to_string_lossy();
        let ext = path.extension().unwrap_or_default().to_string_lossy();
        let parent = path.parent().unwrap();
        let new_name = format!("{stem}_crop.{ext}");
        parent.join(new_name)
    } else {
        path.clone()
    };

    cropped.save(&output_path).map_err(|e| format!("Failed to save: {e}"))?;

    if save_as_copy {
        let import_result = state.db.import_single_file(output_path.to_str().unwrap())
            .map_err(|e| e.to_string())?;
        Ok(import_result)
    } else {
        state.db.update_image_dimensions(&image_id, width, height)
            .map_err(|e| e.to_string())?;
        Ok(image_id)
    }
}

#[tauri::command]
pub async fn rotate_image(
    state: State<'_, AppState>,
    image_id: String,
    degrees: i32,
) -> Result<(), String> {
    let images = state.db.get_images_by_ids(&[&image_id]).map_err(|e| e.to_string())?;
    let img_record = images.first().ok_or("Image not found")?;
    let path = PathBuf::from(&img_record.path);

    let img = image::open(&path).map_err(|e| format!("Failed to open image: {e}"))?;

    let rotated = match degrees.rem_euclid(360) {
        90 => img.rotate90(),
        180 => img.rotate180(),
        270 => img.rotate270(),
        _ => return Err(format!("Only 90/180/270 degree rotations supported, got {degrees}")),
    };

    let (new_w, new_h) = rotated.dimensions();
    rotated.save(&path).map_err(|e| format!("Failed to save: {e}"))?;

    state.db.update_image_dimensions(&image_id, new_w, new_h)
        .map_err(|e| e.to_string())?;

    Ok(())
}
```

- [ ] **Step 2: Add `update_image_dimensions` to db.rs**

In `src-tauri/src/db_core/db.rs`, add:

```rust
pub fn update_image_dimensions(&self, image_id: &str, width: u32, height: u32) -> Result<()> {
    let conn = self.conn.lock().unwrap();
    let aspect = width as f64 / height as f64;
    let orientation = if width > height { "landscape" } else if height > width { "portrait" } else { "square" };
    let megapixels = (width as f64 * height as f64) / 1_000_000.0;
    conn.execute(
        "UPDATE images SET width = ?2, height = ?3, aspect_ratio = ?4, orientation = ?5, megapixels = ?6 WHERE id = ?1",
        rusqlite::params![image_id, width, height, aspect, orientation, megapixels],
    )?;
    Ok(())
}
```

- [ ] **Step 3: Register module and commands**

In `src-tauri/src/commands/mod.rs`, add:
```rust
pub mod transform;
```

In `src-tauri/src/lib.rs`, find the `.invoke_handler(tauri::generate_handler![...])` call and add:
```rust
commands::transform::crop_image,
commands::transform::rotate_image,
```

- [ ] **Step 4: Verify the `image` crate is in dependencies**

Run: `grep "^image" src-tauri/Cargo.toml`

If not present, add to `[dependencies]` in `src-tauri/Cargo.toml`:
```toml
image = "0.25"
```

- [ ] **Step 5: Build**

Run: `cd src-tauri && cargo build 2>&1 | tail -20`
Expected: Successful build. Note: `import_single_file` may not exist — if so, the `save_as_copy` path should use the existing `import_files` command flow instead. Adapt accordingly.

- [ ] **Step 6: Commit**

```bash
git add src-tauri/src/commands/transform.rs src-tauri/src/commands/mod.rs src-tauri/src/lib.rs src-tauri/src/db_core/db.rs
git commit -m "feat: add crop_image and rotate_image Tauri commands"
```

---

### Task 6: Crop — Frontend API + Keyboard Shortcuts

**Files:**
- Modify: `src/lib/api.ts` (add crop/rotate functions)
- Modify: `src/lib/keys.ts` (add rotate shortcut)

- [ ] **Step 1: Add API functions**

At the end of `src/lib/api.ts`:

```typescript
export async function cropImage(imageId: string, x: number, y: number, width: number, height: number, saveAsCopy: boolean): Promise<string> {
    return invoke<string>('crop_image', { imageId, x, y, width, height, saveAsCopy });
}

export async function rotateImage(imageId: string, degrees: number): Promise<void> {
    return invoke<void>('rotate_image', { imageId, degrees });
}
```

- [ ] **Step 2: Add rotate keyboard shortcut**

In `src/lib/keys.ts`, find an appropriate place in the Loupe keybindings section and add handlers for `[` (rotate left) and `]` (rotate right):

```typescript
if (key === '[' && $viewMode === 'loupe') {
    const img = get(focusedImage);
    if (img) {
        rotateImage(img.image.id, 270).then(() => {
            window.dispatchEvent(new CustomEvent('image-updated'));
        });
    }
    return;
}
if (key === ']' && $viewMode === 'loupe') {
    const img = get(focusedImage);
    if (img) {
        rotateImage(img.image.id, 90).then(() => {
            window.dispatchEvent(new CustomEvent('image-updated'));
        });
    }
    return;
}
```

Add `rotateImage` to the imports from `$lib/api`.

- [ ] **Step 3: Handle image-updated event in Loupe**

In `src/lib/components/Loupe.svelte`, in the `onMount` block, add a listener that forces the image to reload:

```typescript
function handleImageUpdated() {
    if (imgEl) {
        const currentSrc = imgEl.src;
        imgEl.src = '';
        imgEl.src = currentSrc + '?t=' + Date.now();
    }
}
window.addEventListener('image-updated', handleImageUpdated);
// Add to cleanup: window.removeEventListener('image-updated', handleImageUpdated);
```

- [ ] **Step 4: Test rotate**

1. Open Loupe mode on any image
2. Press `]` — image rotates 90° clockwise
3. Press `[` — image rotates 90° counter-clockwise
4. Image reloads showing the rotation applied

- [ ] **Step 5: Commit**

```bash
git add src/lib/api.ts src/lib/keys.ts src/lib/components/Loupe.svelte
git commit -m "feat: add rotate shortcuts ([/]) and crop/rotate API functions"
```

---

### Task 7: Crop — Interactive Crop UI in Loupe

**Files:**
- Modify: `src/lib/components/Loupe.svelte`

This is the most complex UI task. The crop overlay lets the user draw a selection rectangle on the image, preview the crop area, and apply or cancel.

- [ ] **Step 1: Add crop state**

In Loupe.svelte script, add:

```typescript
import { cropImage } from '$lib/api';
import { showToast } from '$lib/stores';

let cropMode = $state(false);
let cropStart = $state<{x: number, y: number} | null>(null);
let cropEnd = $state<{x: number, y: number} | null>(null);
let cropping = $state(false);

function enterCropMode() {
    cropMode = true;
    cropStart = null;
    cropEnd = null;
}

function cancelCrop() {
    cropMode = false;
    cropStart = null;
    cropEnd = null;
}

function getCropRect() {
    if (!cropStart || !cropEnd || !imgEl || !image) return null;
    const rect = imgEl.getBoundingClientRect();
    const scaleX = image.image.width / rect.width;
    const scaleY = image.image.height / rect.height;
    const x1 = Math.min(cropStart.x, cropEnd.x);
    const y1 = Math.min(cropStart.y, cropEnd.y);
    const x2 = Math.max(cropStart.x, cropEnd.x);
    const y2 = Math.max(cropStart.y, cropEnd.y);
    return {
        x: Math.round((x1 - rect.left) * scaleX),
        y: Math.round((y1 - rect.top) * scaleY),
        width: Math.round((x2 - x1) * scaleX),
        height: Math.round((y2 - y1) * scaleY),
    };
}

async function applyCrop() {
    const rect = getCropRect();
    if (!rect || !image || rect.width < 10 || rect.height < 10) return;
    cropping = true;
    try {
        await cropImage(image.image.id, rect.x, rect.y, rect.width, rect.height, false);
        showToast('Image cropped', { type: 'info', duration: 2000 });
        window.dispatchEvent(new CustomEvent('image-updated'));
    } catch (e) {
        showToast(`Crop failed: ${e}`, { type: 'error', duration: 5000 });
    }
    cropping = false;
    cancelCrop();
}

function handleCropMouseDown(e: MouseEvent) {
    if (!cropMode) return;
    cropStart = { x: e.clientX, y: e.clientY };
    cropEnd = { x: e.clientX, y: e.clientY };
}

function handleCropMouseMove(e: MouseEvent) {
    if (!cropMode || !cropStart) return;
    cropEnd = { x: e.clientX, y: e.clientY };
}

function handleCropMouseUp() {
    // Selection is complete, user can now Apply or Cancel
}
```

- [ ] **Step 2: Add crop overlay to template**

After the image element and before the overlay bar, add:

```svelte
{#if cropMode}
    <div
        class="crop-overlay"
        onmousedown={handleCropMouseDown}
        onmousemove={handleCropMouseMove}
        onmouseup={handleCropMouseUp}
    >
        {#if cropStart && cropEnd}
            {@const left = Math.min(cropStart.x, cropEnd.x)}
            {@const top = Math.min(cropStart.y, cropEnd.y)}
            {@const w = Math.abs(cropEnd.x - cropStart.x)}
            {@const h = Math.abs(cropEnd.y - cropStart.y)}
            <div class="crop-selection" style="left:{left}px;top:{top}px;width:{w}px;height:{h}px;"></div>
        {/if}
        <div class="crop-toolbar">
            <button onclick={applyCrop} disabled={cropping || !cropStart}>
                {cropping ? 'Cropping…' : '✓ Apply'}
            </button>
            <button onclick={cancelCrop}>✕ Cancel</button>
            <span class="crop-hint">Draw a rectangle to crop • Esc to cancel</span>
        </div>
    </div>
{/if}
```

- [ ] **Step 3: Add Escape key to cancel crop**

In the existing keydown handling or via a new `$effect`:

```typescript
$effect(() => {
    if (!cropMode) return;
    function handleEsc(e: KeyboardEvent) {
        if (e.key === 'Escape') cancelCrop();
    }
    window.addEventListener('keydown', handleEsc);
    return () => window.removeEventListener('keydown', handleEsc);
});
```

- [ ] **Step 4: Add `C` shortcut to enter crop mode**

In `src/lib/keys.ts`, add in the Loupe section:

```typescript
if (key === 'c' && $viewMode === 'loupe' && !e.metaKey && !e.ctrlKey) {
    window.dispatchEvent(new CustomEvent('enter-crop-mode'));
    return;
}
```

In Loupe.svelte `onMount`, add:

```typescript
window.addEventListener('enter-crop-mode', enterCropMode);
// cleanup: window.removeEventListener('enter-crop-mode', enterCropMode);
```

- [ ] **Step 5: Add crop styles**

```css
.crop-overlay {
    position: absolute;
    inset: 0;
    cursor: crosshair;
    z-index: 20;
}
.crop-selection {
    position: fixed;
    border: 2px dashed #4a9eff;
    background: rgba(74, 158, 255, 0.1);
    pointer-events: none;
}
.crop-toolbar {
    position: absolute;
    bottom: 48px;
    left: 50%;
    transform: translateX(-50%);
    background: rgba(0,0,0,0.85);
    padding: 8px 16px;
    border-radius: 8px;
    display: flex;
    gap: 12px;
    align-items: center;
    backdrop-filter: blur(8px);
}
.crop-toolbar button {
    background: none;
    border: 1px solid rgba(255,255,255,0.3);
    color: #fff;
    padding: 4px 12px;
    border-radius: 4px;
    cursor: pointer;
    font-size: 0.8rem;
}
.crop-toolbar button:hover:not(:disabled) {
    background: rgba(255,255,255,0.1);
}
.crop-toolbar button:disabled {
    opacity: 0.4;
}
.crop-hint {
    color: rgba(255,255,255,0.5);
    font-size: 0.75rem;
}
```

- [ ] **Step 6: Test crop workflow**

1. Open Loupe mode on any image
2. Press `C` — crop overlay appears with toolbar
3. Draw a rectangle — dashed blue selection appears
4. Click "✓ Apply" — image is cropped and reloaded
5. Press Esc — cancels crop mode
6. Verify dimensions update in the overlay bar

- [ ] **Step 7: Commit**

```bash
git add src/lib/components/Loupe.svelte src/lib/keys.ts
git commit -m "feat: interactive crop UI with draw-to-select and apply/cancel"
```

---

## Summary

| Task | Feature | Files | Commits |
|------|---------|-------|---------|
| 1 | Prompt — backend | models.rs, db.rs, api.ts | 1 |
| 2 | Prompt — frontend | Loupe.svelte | 1 |
| 3 | Canvas — shell | Canvas.svelte, +page.svelte | 1 |
| 4 | Canvas — resize | Canvas.svelte | 1 |
| 5 | Crop — backend | transform.rs, mod.rs, lib.rs, db.rs | 1 |
| 6 | Crop — API + rotate | api.ts, keys.ts, Loupe.svelte | 1 |
| 7 | Crop — interactive UI | Loupe.svelte, keys.ts | 1 |

**Dependencies:** Task 2 depends on Task 1. Tasks 3-4 are independent. Tasks 6-7 depend on Task 5. Tasks 1-2 and 3-4 and 5-7 can run in parallel tracks.
