# Prompt Pipeline Slice 1: Capture Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Import AI generation metadata from sidecar JSON files and display prompts in Loupe, backed by a canonical `generation_runs` table.

**Architecture:** New `generation_runs` table is the canonical source of truth for prompt metadata. Sidecar JSON parsing plugs into the existing import pipeline after source detection. The Loupe overlay bar already shows prompt text — we enhance it with richer generation metadata. Two MCP tools expose generation data externally.

**Tech Stack:** Rust (Tauri backend, SQLite), Svelte 5 (frontend), serde_json (JSON parsing)

---

### Task 1: Add `generation_runs` table migration

**Files:**
- Modify: `src-tauri/src/db_core/db.rs` — add `migrate_generation_runs()` method and call it from `run_migrations()`
- Modify: `src-tauri/src/db_core/models.rs` — add `GenerationRun` struct

- [ ] **Step 1: Add `GenerationRun` model**

In `src-tauri/src/db_core/models.rs`, add:

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GenerationRun {
    pub id: String,
    pub prompt: Option<String>,
    pub negative_prompt: Option<String>,
    pub provider: Option<String>,
    pub model: Option<String>,
    pub settings_json: String,
    pub seed: Option<String>,
    pub parent_run_id: Option<String>,
    pub source_type: String,
    pub source_path: Option<String>,
    pub raw_metadata_json: Option<String>,
    pub created_at: Option<String>,
    pub imported_at: String,
}
```

- [ ] **Step 2: Add migration in db.rs**

Add a new method `migrate_generation_runs` in `db.rs` following the pattern of `migrate_lineage_tables` (line 127). Create the table and add `generation_run_id` FK to images:

```rust
fn migrate_generation_runs(&self) -> Result<()> {
    let conn = self.conn.lock().unwrap();
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS generation_runs (
            id TEXT PRIMARY KEY,
            prompt TEXT,
            negative_prompt TEXT,
            provider TEXT,
            model TEXT,
            settings_json TEXT NOT NULL DEFAULT '{}',
            seed TEXT,
            parent_run_id TEXT REFERENCES generation_runs(id),
            source_type TEXT NOT NULL,
            source_path TEXT,
            raw_metadata_json TEXT,
            created_at TEXT,
            imported_at TEXT NOT NULL
        );"
    )?;
    let sql = "ALTER TABLE images ADD COLUMN generation_run_id TEXT REFERENCES generation_runs(id)";
    let _ = conn.execute(sql, []);
    Ok(())
}
```

Call it from `run_migrations()` after `migrate_mcp_tables()`.

- [ ] **Step 3: Add DB helper methods**

Add `insert_generation_run` and `get_generation_run_for_image` methods to `Database`:

```rust
pub fn insert_generation_run(&self, run: &GenerationRun) -> Result<()> {
    let conn = self.conn.lock().unwrap();
    conn.execute(
        "INSERT OR IGNORE INTO generation_runs (id, prompt, negative_prompt, provider, model, settings_json, seed, parent_run_id, source_type, source_path, raw_metadata_json, created_at, imported_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13)",
        rusqlite::params![run.id, run.prompt, run.negative_prompt, run.provider, run.model, run.settings_json, run.seed, run.parent_run_id, run.source_type, run.source_path, run.raw_metadata_json, run.created_at, run.imported_at],
    )?;
    Ok(())
}

pub fn link_image_to_run(&self, image_id: &str, run_id: &str) -> Result<()> {
    let conn = self.conn.lock().unwrap();
    conn.execute(
        "UPDATE images SET generation_run_id = ?1 WHERE id = ?2",
        rusqlite::params![run_id, image_id],
    )?;
    Ok(())
}

pub fn get_generation_run_for_image(&self, image_id: &str) -> Result<Option<GenerationRun>> {
    let conn = self.conn.lock().unwrap();
    let mut stmt = conn.prepare(
        "SELECT g.* FROM generation_runs g
         JOIN images i ON i.generation_run_id = g.id
         WHERE i.id = ?1"
    )?;
    let run = stmt.query_row(rusqlite::params![image_id], |row| {
        Ok(GenerationRun {
            id: row.get(0)?,
            prompt: row.get(1)?,
            negative_prompt: row.get(2)?,
            provider: row.get(3)?,
            model: row.get(4)?,
            settings_json: row.get(5)?,
            seed: row.get(6)?,
            parent_run_id: row.get(7)?,
            source_type: row.get(8)?,
            source_path: row.get(9)?,
            raw_metadata_json: row.get(10)?,
            created_at: row.get(11)?,
            imported_at: row.get(12)?,
        })
    }).optional()?;
    Ok(run)
}
```

- [ ] **Step 4: Verify migration runs**

Run: `cargo build --manifest-path src-tauri/Cargo.toml`
Expected: compiles without errors.

- [ ] **Step 5: Commit**

```bash
git add src-tauri/src/db_core/db.rs src-tauri/src/db_core/models.rs
git commit -m "feat: add generation_runs table and DB methods"
```

---

### Task 2: Sidecar JSON parser

**Files:**
- Create: `src-tauri/src/db_core/sidecar.rs`
- Modify: `src-tauri/src/db_core/mod.rs` — add `pub mod sidecar;`

- [ ] **Step 1: Create sidecar parser module**

Create `src-tauri/src/db_core/sidecar.rs`. The parser handles two known schemas (OpenAI gpt-image-2 and Gemini) and falls back to generic extraction:

```rust
use serde_json::Value;
use std::path::Path;

use super::models::GenerationRun;

pub struct SidecarResult {
    pub prompt: Option<String>,
    pub negative_prompt: Option<String>,
    pub provider: Option<String>,
    pub model: Option<String>,
    pub settings_json: String,
    pub seed: Option<String>,
    pub created_at: Option<String>,
    pub raw_json: String,
}

pub fn find_sidecar(image_path: &Path) -> Option<std::path::PathBuf> {
    // Check for {name}.json (e.g., photo.png -> photo.json)
    let stem = image_path.file_stem()?.to_str()?;
    let parent = image_path.parent()?;
    let sidecar = parent.join(format!("{}.json", stem));
    if sidecar.exists() {
        return Some(sidecar);
    }
    // Check for {name}.{ext}.json (e.g., photo.png.json)
    let full_name = image_path.file_name()?.to_str()?;
    let sidecar2 = parent.join(format!("{}.json", full_name));
    if sidecar2.exists() {
        return Some(sidecar2);
    }
    None
}

pub fn parse_sidecar(sidecar_path: &Path) -> Result<SidecarResult, String> {
    let content = std::fs::read_to_string(sidecar_path)
        .map_err(|e| format!("Failed to read sidecar: {}", e))?;
    let json: Value = serde_json::from_str(&content)
        .map_err(|e| format!("Invalid sidecar JSON: {}", e))?;

    let obj = json.as_object().ok_or("Sidecar is not a JSON object")?;

    let prompt = obj.get("prompt").and_then(|v| v.as_str()).map(String::from);
    let provider = obj.get("provider").and_then(|v| v.as_str()).map(String::from);
    let model = obj.get("model").and_then(|v| v.as_str()).map(String::from);
    let seed = obj.get("seed").and_then(|v| {
        v.as_i64().map(|n| n.to_string()).or_else(|| v.as_str().map(String::from))
    });
    let created_at = obj.get("timestamp").and_then(|v| v.as_str()).map(String::from);

    // Build settings from known fields
    let mut settings = serde_json::Map::new();
    for key in &["quality", "thinking", "n", "platform", "preset", "estimated_cost", "duration_s", "edit_source"] {
        if let Some(val) = obj.get(*key) {
            settings.insert(key.to_string(), val.clone());
        }
    }
    let settings_json = serde_json::to_string(&settings).unwrap_or_else(|_| "{}".to_string());

    Ok(SidecarResult {
        prompt,
        negative_prompt: None,
        provider,
        model,
        settings_json,
        seed,
        created_at,
        raw_json: content,
    })
}
```

- [ ] **Step 2: Register module**

In `src-tauri/src/db_core/mod.rs`, add:

```rust
pub mod sidecar;
```

- [ ] **Step 3: Verify it compiles**

Run: `cargo build --manifest-path src-tauri/Cargo.toml`

- [ ] **Step 4: Commit**

```bash
git add src-tauri/src/db_core/sidecar.rs src-tauri/src/db_core/mod.rs
git commit -m "feat: add sidecar JSON parser for generation metadata"
```

---

### Task 3: Plug sidecar import into the import pipeline

**Files:**
- Modify: `src-tauri/src/db_core/import.rs` — add sidecar detection after source detection (line 71)

- [ ] **Step 1: Add sidecar import logic**

In `import.rs`, add the sidecar import after the source detection block (after line 71, before line 73). Add the import at the top:

```rust
use super::sidecar;
```

After line 71 (`let detection = detect_source(filename, &png_chunks, file_path);`), add:

```rust
    // Check for sidecar JSON and create generation_run
    if let Some(sidecar_path) = sidecar::find_sidecar(file_path) {
        if let Ok(sc) = sidecar::parse_sidecar(&sidecar_path) {
            let run_id = Uuid::new_v4().to_string();
            let run = super::models::GenerationRun {
                id: run_id.clone(),
                prompt: sc.prompt.clone(),
                negative_prompt: sc.negative_prompt,
                provider: sc.provider,
                model: sc.model,
                settings_json: sc.settings_json,
                seed: sc.seed,
                parent_run_id: None,
                source_type: "sidecar".to_string(),
                source_path: Some(sidecar_path.to_string_lossy().to_string()),
                raw_metadata_json: Some(sc.raw_json),
                created_at: sc.created_at,
                imported_at: Utc::now().to_rfc3339(),
            };
            let _ = db.insert_generation_run(&run);
            let _ = db.link_image_to_run(&image_id, &run_id);
        }
    }
```

This runs after source detection so both systems coexist. Sidecar data is richer (full prompt, model, settings) while source detection handles images without sidecars.

- [ ] **Step 2: Verify it compiles and the existing import tests pass**

Run: `cargo build --manifest-path src-tauri/Cargo.toml`

- [ ] **Step 3: Commit**

```bash
git add src-tauri/src/db_core/import.rs
git commit -m "feat: import sidecar JSON metadata during folder scan"
```

---

### Task 4: Add Tauri command to fetch generation run

**Files:**
- Modify: `src-tauri/src/commands/` — find or create the relevant command file
- Modify: frontend `src/lib/api.ts` — add `getGenerationRun()` function

- [ ] **Step 1: Find the command registration pattern**

Check how existing commands are structured:

```bash
grep -rn "get_vision_metadata\|#\[tauri::command\]" src-tauri/src/commands/ | head -20
ls src-tauri/src/commands/
```

- [ ] **Step 2: Add Tauri command**

Add a `get_generation_run` command following existing patterns (likely in `src-tauri/src/commands/metadata.rs` or similar). The command takes an `image_id` and returns the `GenerationRun` as JSON:

```rust
#[tauri::command]
pub fn get_generation_run(state: tauri::State<'_, AppState>, image_id: String) -> Result<Option<GenerationRun>, String> {
    state.db.get_generation_run_for_image(&image_id).map_err(|e| e.to_string())
}
```

Register it in `src-tauri/src/lib.rs` alongside other commands.

- [ ] **Step 3: Add frontend API function**

In `src/lib/api.ts`, add:

```typescript
export interface GenerationRun {
    id: string;
    prompt: string | null;
    negative_prompt: string | null;
    provider: string | null;
    model: string | null;
    settings_json: string;
    seed: string | null;
    parent_run_id: string | null;
    source_type: string;
    source_path: string | null;
    raw_metadata_json: string | null;
    created_at: string | null;
    imported_at: string;
}

export async function getGenerationRun(imageId: string): Promise<GenerationRun | null> {
    return invoke<GenerationRun | null>('get_generation_run', { imageId });
}
```

- [ ] **Step 4: Verify it compiles**

Run: `cargo build --manifest-path src-tauri/Cargo.toml`

- [ ] **Step 5: Commit**

```bash
git add src-tauri/src/commands/ src-tauri/src/lib.rs src/lib/api.ts
git commit -m "feat: add get_generation_run Tauri command and API"
```

---

### Task 5: Enhance Loupe prompt display with generation metadata

**Files:**
- Modify: `src/lib/components/Loupe.svelte` — enhance the existing prompt panel

The Loupe already has a prompt toggle (line 394-405) that shows `image.ai_prompt`. We enhance it to show richer data from `generation_runs` when available.

- [ ] **Step 1: Fetch generation run data**

In `Loupe.svelte`, import the new API function and fetch run data when the image changes. Add near the existing `getVisionMetadata` call (line 99):

```typescript
import { getDetections, getVisionMetadata, cropImage, getImagesByIds, getGenerationRun } from '$lib/api';
import type { Detection, GenerationRun } from '$lib/api';

let generationRun = $state<GenerationRun | null>(null);
```

In the `$effect` that loads detections (around line 90), add:

```typescript
getGenerationRun(id).then(r => { generationRun = r; }).catch(() => { generationRun = null; });
```

- [ ] **Step 2: Enhance prompt display**

Update the prompt derived value to prefer generation_run data:

```typescript
let prompt = $derived(generationRun?.prompt ?? image?.image.ai_prompt ?? null);
let genModel = $derived(generationRun?.model ?? null);
let genProvider = $derived(generationRun?.provider ?? null);
let genSeed = $derived(generationRun?.seed ?? null);
```

Replace the existing `prompt-panel` div (lines 402-405) with a richer version:

```svelte
{#if prompt && promptExpanded}
    <div class="prompt-panel">
        <div class="prompt-text">{prompt}</div>
        {#if genModel || genProvider || genSeed}
            <div class="prompt-meta">
                {#if genProvider}<span class="meta-tag">{genProvider}</span>{/if}
                {#if genModel}<span class="meta-tag">{genModel}</span>{/if}
                {#if genSeed}<span class="meta-tag">seed:{genSeed}</span>{/if}
            </div>
        {/if}
    </div>
{/if}
```

- [ ] **Step 3: Add CSS for prompt metadata tags**

Add styles in the `<style>` block:

```css
.prompt-meta {
    display: flex;
    gap: 6px;
    margin-top: 6px;
    flex-wrap: wrap;
}
.meta-tag {
    background: var(--bg-elevated, #2a2a3e);
    color: var(--text-secondary, #888);
    padding: 1px 6px;
    border-radius: 3px;
    font-size: 10px;
    font-family: var(--font-mono);
}
```

- [ ] **Step 4: Test in browser**

Start the Tauri dev build and import the `site/public/images/` folder. Click on an image with a sidecar JSON. Verify:
- Prompt text appears when clicking "✦ Prompt"
- Provider, model, and seed tags show below the prompt text
- Images without sidecars still show the basic prompt (from source detection)

- [ ] **Step 5: Commit**

```bash
git add src/lib/components/Loupe.svelte src/lib/api.ts
git commit -m "feat: display generation metadata in Loupe prompt panel"
```

---

### Task 6: Add MCP tools for generation metadata

**Files:**
- Modify: `src-tauri/src/mcp/tools.rs` — add `get_generation_run` and `set_generation_metadata` tools

- [ ] **Step 1: Add parameter structs**

Near the other params structs in `tools.rs`:

```rust
#[derive(Debug, Deserialize, schemars::JsonSchema)]
struct SetGenerationMetadataParams {
    image_id: String,
    prompt: String,
    model: Option<String>,
    provider: Option<String>,
    seed: Option<String>,
    settings_json: Option<String>,
}
```

- [ ] **Step 2: Add `get_generation_run` tool**

Following the pattern of `get_vision_metadata` (line 639), add in the `#[tool]` impl block:

```rust
#[tool(description = "Get AI generation metadata (prompt, model, seed, provider) for an image")]
fn get_generation_run(&self, Parameters(params): Parameters<ImageIdParams>) -> String {
    match self.check_image_id_scope(&params.image_id) {
        Ok(false) => return "Error: Access denied — image outside token scope".to_string(),
        Err(e) => return format!("Error: {}", e),
        _ => {}
    }
    let state = self.app_handle.state::<AppState>();
    match state.db.get_generation_run_for_image(&params.image_id) {
        Ok(Some(run)) => serde_json::to_string(&run).unwrap_or_else(|_| "null".to_string()),
        Ok(None) => "null".to_string(),
        Err(e) => format!("Error: {}", e),
    }
}
```

- [ ] **Step 3: Add `set_generation_metadata` tool**

```rust
#[tool(description = "Manually attach AI generation metadata to an image (creates a generation run record)")]
fn set_generation_metadata(&self, Parameters(params): Parameters<SetGenerationMetadataParams>) -> String {
    match self.check_image_id_scope(&params.image_id) {
        Ok(false) => return "Error: Access denied — image outside token scope".to_string(),
        Err(e) => return format!("Error: {}", e),
        _ => {}
    }
    let state = self.app_handle.state::<AppState>();
    let run = crate::db_core::models::GenerationRun {
        id: uuid::Uuid::new_v4().to_string(),
        prompt: Some(params.prompt),
        negative_prompt: None,
        provider: params.provider,
        model: params.model,
        settings_json: params.settings_json.unwrap_or_else(|| "{}".to_string()),
        seed: params.seed,
        parent_run_id: None,
        source_type: "manual".to_string(),
        source_path: None,
        raw_metadata_json: None,
        created_at: Some(chrono::Utc::now().to_rfc3339()),
        imported_at: chrono::Utc::now().to_rfc3339(),
    };
    let run_id = run.id.clone();
    if let Err(e) = state.db.insert_generation_run(&run) {
        return format!("Error creating run: {}", e);
    }
    if let Err(e) = state.db.link_image_to_run(&params.image_id, &run_id) {
        return format!("Error linking image: {}", e);
    }
    format!("Created generation run {} for image {}", run_id, params.image_id)
}
```

- [ ] **Step 4: Verify compilation**

Run: `cargo build --manifest-path src-tauri/Cargo.toml`

- [ ] **Step 5: Commit**

```bash
git add src-tauri/src/mcp/tools.rs
git commit -m "feat(mcp): add get_generation_run and set_generation_metadata tools"
```

---

### Task 7: Update AGENTS.md with generation_runs documentation

**Files:**
- Modify: `AGENTS.md`

- [ ] **Step 1: Add generation_runs section**

Add documentation about the new table, sidecar format support, and MCP tools to AGENTS.md under the appropriate section.

- [ ] **Step 2: Commit**

```bash
git add AGENTS.md
git commit -m "docs: add generation_runs and sidecar import to AGENTS.md"
```

---

## Self-Review

**Spec coverage check:**
- ✅ `generation_runs` table — Task 1
- ✅ Sidecar JSON parser (OpenAI + Gemini schemas) — Task 2
- ✅ Import pipeline integration — Task 3
- ✅ Loupe prompt display (enhanced with model/provider/seed) — Task 5
- ✅ MCP `get_generation_run` tool — Task 6
- ✅ MCP `set_generation_metadata` tool — Task 6
- ✅ Tauri command for frontend — Task 4
- ✅ Idempotency: `INSERT OR IGNORE` in Task 1, one run per image in Task 3
- ✅ Raw JSON preserved in `raw_metadata_json` — Task 2/3
- ✅ Conflict precedence: sidecar import runs after source detection, both coexist

**Placeholder scan:** No TBDs, TODOs, or vague steps found.

**Type consistency:** `GenerationRun` struct matches across models.rs, api.ts, and all tool/command usages. `SidecarResult` maps cleanly to `GenerationRun` in Task 3.
