# Prompt Re-Submit Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Let users re-submit a generation prompt from the Loupe viewer to OpenAI's Image API, generate N variations, and compare them — turning ImageView from a viewer into a creative tool.

**Architecture:** New Rust service calls OpenAI Image API (`gpt-image-2`) via `reqwest`. Generation runs as an async job (existing `JobRegistry`). Each output image is saved to disk, imported into the DB, linked via `generation_runs` with `parent_run_id` for lineage, and auto-grouped. Frontend adds a re-submit dialog launched from Loupe's prompt panel, with settings and budget estimate. Results appear in a strip that can open Compare view.

**Tech Stack:** Rust (`reqwest`, `serde`, `base64`, `image`), Tauri events, Svelte 5 runes, existing `JobRegistry` + `JobProgressPanel`, `generation_runs` table, `lineage_groups` table.

---

## File Structure

### New Files

| File | Responsibility |
|------|---------------|
| `src-tauri/src/services/generation.rs` | OpenAI API client, image download, DB import, lineage grouping |
| `src-tauri/src/commands/generation.rs` | Tauri IPC commands: `resubmit_prompt`, `estimate_generation_cost` |
| `src/lib/components/PromptResubmitDialog.svelte` | Modal dialog with prompt editor, model/size/quality settings, budget estimate, submit button |
| `src/lib/components/GenerationResultsStrip.svelte` | Horizontal strip showing generated images with "Open in Compare" action |

### Modified Files

| File | Changes |
|------|---------|
| `src-tauri/src/commands/mod.rs` | Add `pub mod generation;` |
| `src-tauri/src/services/mod.rs` | Add `pub mod generation;` |
| `src-tauri/src/lib.rs:222` | Register new commands in `invoke_handler` |
| `src-tauri/src/db_core/db.rs:7-9` | Wrap `Database.conn` in `Arc` to make it `Clone` for async tasks |
| `src-tauri/src/commands/embeddings.rs:160` | Add OpenAI API key validation |
| `src/lib/api.ts` | Add `resubmitPrompt`, `estimateGenerationCost` functions and types |
| `src/lib/components/Loupe.svelte:418` | Add "Re-generate" button next to prompt toggle |
| `src/lib/components/JobProgressPanel.svelte:22` | Add `generation-progress` event listener |

---

## Task 0: Make Database Cloneable

**Files:**
- Modify: `src-tauri/src/db_core/db.rs:7-14`

The `Database` struct wraps `Mutex<Connection>` which is not `Clone`. The generation command needs to move a `Database` handle into a `tokio::spawn` task. Wrapping the connection in `Arc` makes `Database` cheaply cloneable without changing any call sites.

- [ ] **Step 1: Update the Database struct**

In `src-tauri/src/db_core/db.rs`, change:

```rust
pub struct Database {
    pub(crate) conn: Mutex<Connection>,
}

impl Database {
    pub fn open(db_path: &Path) -> Result<Self> {
        let conn = Connection::open(db_path)?;
        let db = Database { conn: Mutex::new(conn) };
```

To:

```rust
#[derive(Clone)]
pub struct Database {
    pub(crate) conn: Arc<Mutex<Connection>>,
}

impl Database {
    pub fn open(db_path: &Path) -> Result<Self> {
        let conn = Connection::open(db_path)?;
        let db = Database { conn: Arc::new(Mutex::new(conn)) };
```

- [ ] **Step 2: Add the Arc import**

At the top of `db.rs`, ensure `use std::sync::Arc;` is present (it may already be imported — check before adding a duplicate).

- [ ] **Step 3: Build and verify no regressions**

Run: `cd src-tauri && cargo check 2>&1 | tail -10`
Expected: compiles without errors. All existing call sites use `self.conn.lock().unwrap()` which works identically with `Arc<Mutex<…>>`.

- [ ] **Step 4: Run existing tests**

Run: `cd src-tauri && cargo test 2>&1 | tail -20`
Expected: all tests pass

- [ ] **Step 5: Commit**

```bash
git add src-tauri/src/db_core/db.rs
git commit -m "refactor: wrap Database conn in Arc for Clone support"
```

---

## Task 1: OpenAI API Key Validation

**Files:**
- Modify: `src-tauri/src/commands/embeddings.rs:160-170`

- [ ] **Step 1: Write the failing test**

In `src-tauri/src/commands/embeddings.rs`, the test module doesn't exist yet for this function. We'll test via integration. First, read the current `validate_api_key` function — it returns `Ok(false)` for any non-Google provider. We need it to validate OpenAI keys too.

- [ ] **Step 2: Add OpenAI validation branch**

In `src-tauri/src/commands/embeddings.rs`, replace the `validate_api_key` function:

```rust
#[tauri::command]
pub async fn validate_api_key(provider: String, key: String) -> Result<bool, String> {
    let client = reqwest::Client::new();
    match provider.as_str() {
        "google" => {
            let url = format!(
                "https://generativelanguage.googleapis.com/v1beta/models?key={}", key
            );
            let resp = client.get(&url).send().await.map_err(|e| format!("{}", e))?;
            Ok(resp.status().is_success())
        }
        "openai" => {
            let resp = client
                .get("https://api.openai.com/v1/models")
                .header("Authorization", format!("Bearer {}", key))
                .send()
                .await
                .map_err(|e| format!("{}", e))?;
            Ok(resp.status().is_success())
        }
        _ => Ok(false),
    }
}
```

- [ ] **Step 3: Build and verify**

Run: `cd src-tauri && cargo check 2>&1 | tail -5`
Expected: compiles without errors

- [ ] **Step 4: Commit**

```bash
git add src-tauri/src/commands/embeddings.rs
git commit -m "feat: add OpenAI API key validation"
```

---

## Task 2: Generation Service (Rust Backend)

**Files:**
- Create: `src-tauri/src/services/generation.rs`
- Modify: `src-tauri/src/services/mod.rs`

- [ ] **Step 1: Add module to mod.rs**

In `src-tauri/src/services/mod.rs`, add:

```rust
pub mod generation;
```

- [ ] **Step 2: Write the generation service with types and API call**

Create `src-tauri/src/services/generation.rs`:

```rust
use serde::{Deserialize, Serialize};
use uuid::Uuid;
use chrono::Utc;
use std::path::Path;
use sha2::{Sha256, Digest};
use tauri::Emitter;

use crate::db_core::db::Database;
use crate::db_core::models::GenerationRun;
use crate::services::jobs::JobRegistry;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GenerationRequest {
    pub source_image_id: Option<String>,
    pub prompt: String,
    pub n: u8,
    pub model: String,
    pub size: String,
    pub quality: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GenerationResult {
    pub job_id: String,
    pub image_ids: Vec<String>,
    pub generation_run_ids: Vec<String>,
    pub lineage_group_id: Option<String>,
    pub errors: Vec<String>,
}

#[derive(Debug, Deserialize)]
struct OpenAiImageResponse {
    data: Vec<OpenAiImageData>,
}

#[derive(Debug, Deserialize)]
struct OpenAiImageData {
    b64_json: Option<String>,
}

const PRICING: &[(&str, &str, f64)] = &[
    ("gpt-image-2", "1024x1024", 0.040),
    ("gpt-image-2", "1024x1536", 0.060),
    ("gpt-image-2", "1536x1024", 0.060),
    ("gpt-image-2", "auto", 0.040),
];

pub fn estimate_cost(model: &str, size: &str, quality: &str, n: u8) -> f64 {
    let base = PRICING.iter()
        .find(|(m, s, _)| *m == model && *s == size)
        .map(|(_, _, p)| *p)
        .unwrap_or(0.040);
    let multiplier = if quality == "high" { 2.0 } else { 1.0 };
    base * multiplier * n as f64
}

pub async fn generate_images(
    request: &GenerationRequest,
    api_key: &str,
    app_data_dir: &Path,
    db: &Database,
    jobs: &JobRegistry,
    job_id: &str,
    cancel: &tokio_util::sync::CancellationToken,
    app_handle: &tauri::AppHandle,
) -> Result<GenerationResult, String> {
    let _ = app_handle.emit("job-status-changed", serde_json::json!({
        "job_id": &job_id,
        "kind": "generation",
        "status": "running",
        "current": 0,
        "total": request.n,
    }));

    let generated_dir = app_data_dir.join("generated");
    std::fs::create_dir_all(&generated_dir).map_err(|e| format!("Dir create error: {}", e))?;

    let client = reqwest::Client::new();
    let resp = client
        .post("https://api.openai.com/v1/images/generations")
        .header("Authorization", format!("Bearer {}", api_key))
        .json(&serde_json::json!({
            "model": &request.model,
            "prompt": &request.prompt,
            "n": request.n,
            "size": &request.size,
            "quality": &request.quality,
        }))
        .send()
        .await
        .map_err(|e| {
            jobs.fail(&job_id, &e.to_string());
            let _ = app_handle.emit("job-status-changed", serde_json::json!({
                "job_id": &job_id, "kind": "generation", "status": "failed",
            }));
            format!("API request failed: {}", e)
        })?;

    if !resp.status().is_success() {
        let status = resp.status();
        let body = resp.text().await.unwrap_or_default();
        let msg = format!("OpenAI API error {}: {}", status, body);
        jobs.fail(&job_id, &msg);
        let _ = app_handle.emit("job-status-changed", serde_json::json!({
            "job_id": &job_id, "kind": "generation", "status": "failed",
        }));
        return Err(msg);
    }

    let resp_body = resp.text().await
        .map_err(|e| { jobs.fail(job_id, &e.to_string()); format!("Read error: {}", e) })?;
    let api_resp: OpenAiImageResponse = serde_json::from_str(&resp_body)
        .map_err(|e| { jobs.fail(job_id, &e.to_string()); format!("Parse error: {}", e) })?;

    let parent_run_id = if let Some(ref src_id) = request.source_image_id {
        db.get_generation_run_for_image(src_id)
            .ok()
            .flatten()
            .map(|r| r.id)
    } else {
        None
    };

    let mut image_ids = Vec::new();
    let mut run_ids = Vec::new();
    let mut errors = Vec::new();

    for (i, item) in api_resp.data.iter().enumerate() {
        if cancel.is_cancelled() {
            jobs.mark_cancelled(&job_id);
            let _ = app_handle.emit("job-status-changed", serde_json::json!({
                "job_id": &job_id, "kind": "generation", "status": "cancelled",
            }));
            break;
        }

        match save_generated_image(item, i, &request, &generated_dir, db, parent_run_id.as_deref(), &resp_body) {
            Ok((image_id, run_id)) => {
                image_ids.push(image_id);
                run_ids.push(run_id);
            }
            Err(e) => errors.push(format!("Image {}: {}", i, e)),
        }

        jobs.update_progress(&job_id, (i + 1) as u32, Some(&format!("Saved image {}/{}", i + 1, request.n)));
        let _ = app_handle.emit("generation-progress", serde_json::json!({
            "job_id": &job_id,
            "current": i + 1,
            "total": request.n,
        }));
    }

    let lineage_group_id = if image_ids.len() > 1 || request.source_image_id.is_some() {
        create_generation_lineage(db, &image_ids, request.source_image_id.as_deref(), &request.prompt).ok()
    } else {
        None
    };

    if errors.is_empty() {
        jobs.complete(&job_id);
    } else if image_ids.is_empty() {
        jobs.fail(&job_id, &errors.join("; "));
    } else {
        jobs.complete(&job_id);
    }

    let _ = app_handle.emit("job-status-changed", serde_json::json!({
        "job_id": &job_id,
        "kind": "generation",
        "status": if errors.is_empty() { "completed" } else if image_ids.is_empty() { "failed" } else { "completed" },
        "current": image_ids.len(),
        "total": request.n,
    }));

    let _ = app_handle.emit("generation-complete", serde_json::json!({
        "job_id": &job_id,
        "image_ids": &image_ids,
        "lineage_group_id": &lineage_group_id,
    }));

    Ok(GenerationResult {
        job_id,
        image_ids,
        generation_run_ids: run_ids,
        lineage_group_id,
        errors,
    })
}

fn save_generated_image(
    item: &OpenAiImageData,
    index: usize,
    request: &GenerationRequest,
    generated_dir: &Path,
    db: &Database,
    parent_run_id: Option<&str>,
    raw_api_response: &str,
) -> Result<(String, String), String> {
    let b64 = item.b64_json.as_deref()
        .ok_or("No b64_json in response")?;

    let bytes = base64::Engine::decode(
        &base64::engine::general_purpose::STANDARD, b64
    ).map_err(|e| format!("Base64 decode error: {}", e))?;

    let image_id = Uuid::new_v4().to_string();
    let filename = format!("{}_{}.png", &image_id[..8], index);
    let file_path = generated_dir.join(&filename);

    std::fs::write(&file_path, &bytes).map_err(|e| format!("Write error: {}", e))?;

    let mut hasher = Sha256::new();
    hasher.update(&bytes);
    let hash = format!("{:x}", hasher.finalize());

    let img = image::open(&file_path).map_err(|e| format!("Image decode error: {}", e))?;
    let (width, height) = (img.width(), img.height());

    let now = Utc::now().to_rfc3339();
    let image_record = crate::db_core::models::Image {
        id: image_id.clone(),
        sha256_hash: hash,
        width,
        height,
        format: "png".to_string(),
        file_size: bytes.len() as u64,
        created_at: now.clone(),
        imported_at: now.clone(),
        ai_prompt: Some(request.prompt.clone()),
    };
    db.insert_image(&image_record).map_err(|e| e.to_string())?;

    let file_record = crate::db_core::models::ImageFile {
        id: Uuid::new_v4().to_string(),
        image_id: image_id.clone(),
        path: file_path.to_string_lossy().to_string(),
        last_seen_at: now.clone(),
        missing_at: None,
    };
    db.insert_image_file(&file_record).map_err(|e| e.to_string())?;

    let aspect = width as f64 / height.max(1) as f64;
    let orientation = if (aspect - 1.0).abs() < 0.05 { "square" }
        else if aspect > 1.0 { "landscape" } else { "portrait" };
    let megapixels = (width as f64 * height as f64) / 1_000_000.0;
    db.update_source_detection(
        &image_id, Some("openai"), 100.0,
        "{\"source\":\"openai_api_generation\"}", Some(true),
        Some(&request.prompt), aspect, orientation, megapixels,
    ).map_err(|e| e.to_string())?;

    let run_id = Uuid::new_v4().to_string();
    let settings = serde_json::json!({
        "n": request.n,
        "size": &request.size,
        "quality": &request.quality,
        "variation_index": index,
        "estimated_cost": estimate_cost(&request.model, &request.size, &request.quality, 1),
    });
    let run = GenerationRun {
        id: run_id.clone(),
        prompt: Some(request.prompt.clone()),
        negative_prompt: None,
        provider: Some("openai".to_string()),
        model: Some(request.model.clone()),
        settings_json: settings.to_string(),
        seed: None,
        parent_run_id: parent_run_id.map(|s| s.to_string()),
        source_type: "openai_api".to_string(),
        source_path: Some(file_path.to_string_lossy().to_string()),
        raw_metadata_json: Some(raw_api_response.to_string()),
        created_at: Some(now.clone()),
        imported_at: now,
    };
    db.insert_generation_run(&run).map_err(|e| e.to_string())?;
    db.link_image_to_run(&image_id, &run_id).map_err(|e| e.to_string())?;

    let _ = crate::db_core::thumbnails::generate_thumbnail(
        &file_path,
        generated_dir.parent().unwrap_or(generated_dir),
        &image_id,
    );

    Ok((image_id, run_id))
}

fn create_generation_lineage(
    db: &Database,
    new_image_ids: &[String],
    source_image_id: Option<&str>,
    prompt: &str,
) -> Result<String, String> {
    let truncated = if prompt.len() > 40 { &prompt[..40] } else { prompt };
    let name = format!("Gen: {}", truncated);
    let group_id = db.create_lineage_group(&name, "generation", 100.0)
        .map_err(|e| e.to_string())?;

    let mut order = 0;
    if let Some(src_id) = source_image_id {
        db.assign_to_lineage_group(src_id, &group_id, order)
            .map_err(|e| e.to_string())?;
        order += 1;
    }

    for id in new_image_ids {
        db.assign_to_lineage_group(id, &group_id, order)
            .map_err(|e| e.to_string())?;
        order += 1;
    }

    Ok(group_id)
}
```

- [ ] **Step 3: Build and verify**

Run: `cd src-tauri && cargo check 2>&1 | tail -10`
Expected: compiles (may have warnings about unused, that's fine — commands aren't wired yet)

- [ ] **Step 4: Commit**

```bash
git add src-tauri/src/services/generation.rs src-tauri/src/services/mod.rs
git commit -m "feat: add generation service with OpenAI Image API client"
```

---

## Task 3: Tauri Commands

**Files:**
- Create: `src-tauri/src/commands/generation.rs`
- Modify: `src-tauri/src/commands/mod.rs`
- Modify: `src-tauri/src/lib.rs`

- [ ] **Step 1: Add module to commands/mod.rs**

In `src-tauri/src/commands/mod.rs`, add:

```rust
pub mod generation;
```

- [ ] **Step 2: Create the commands file**

Create `src-tauri/src/commands/generation.rs`:

```rust
use tauri::State;
use serde::{Deserialize, Serialize};
use crate::AppState;
use crate::services::generation;

#[derive(Debug, Deserialize)]
pub struct ResubmitRequest {
    pub source_image_id: Option<String>,
    pub prompt: String,
    pub n: u8,
    pub model: String,
    pub size: String,
    pub quality: String,
}

#[derive(Debug, Serialize)]
pub struct ResubmitResponse {
    pub job_id: String,
}

#[derive(Debug, Serialize)]
pub struct CostEstimate {
    pub estimated_cost: f64,
    pub model: String,
    pub size: String,
    pub quality: String,
    pub n: u8,
}

#[tauri::command]
pub async fn resubmit_prompt(
    app: tauri::AppHandle,
    state: State<'_, AppState>,
    request: ResubmitRequest,
) -> Result<ResubmitResponse, String> {
    let api_key = state.secrets.get("api_key_openai")?
        .ok_or("OpenAI API key not set. Go to Settings to add it.")?;

    if request.n < 1 || request.n > 4 {
        return Err("n must be between 1 and 4".to_string());
    }

    let gen_request = generation::GenerationRequest {
        source_image_id: request.source_image_id,
        prompt: request.prompt,
        n: request.n,
        model: request.model,
        size: request.size,
        quality: request.quality,
    };

    // Clone what we need for the spawned task
    // Database is Clone after Task 0 (Arc<Mutex<Connection>>)
    let db = state.db.clone();
    let jobs = state.jobs.clone();
    let app_data_dir = state.app_data_dir.clone();
    let app_clone = app.clone();

    // Create job here so we can return job_id immediately
    let (job_id, cancel) = state.jobs.create_job("generation", gen_request.n as u32);
    let job_id_for_task = job_id.clone();

    tokio::spawn(async move {
        let _ = generation::generate_images(
            &gen_request,
            &api_key,
            &app_data_dir,
            &db,
            &jobs,
            &job_id_for_task,
            &cancel,
            &app_clone,
        ).await;
    });

    Ok(ResubmitResponse { job_id })
}

#[tauri::command]
pub async fn estimate_generation_cost(
    model: String,
    size: String,
    quality: String,
    n: u8,
) -> Result<CostEstimate, String> {
    Ok(CostEstimate {
        estimated_cost: generation::estimate_cost(&model, &size, &quality, n),
        model,
        size,
        quality,
        n,
    })
}
```

- [ ] **Step 3: Register commands in lib.rs**

In `src-tauri/src/lib.rs`, add to the `invoke_handler` list (after the `commands::undo::list_undo_history` line):

```rust
            commands::generation::resubmit_prompt,
            commands::generation::estimate_generation_cost,
```

- [ ] **Step 4: Build and verify**

Run: `cd src-tauri && cargo check 2>&1 | tail -10`
Expected: compiles without errors

- [ ] **Step 5: Commit**

```bash
git add src-tauri/src/commands/generation.rs src-tauri/src/commands/mod.rs src-tauri/src/lib.rs src-tauri/src/services/generation.rs
git commit -m "feat: add resubmit_prompt and estimate_generation_cost commands"
```

---

## Task 4: Frontend API Layer

**Files:**
- Modify: `src/lib/api.ts`

- [ ] **Step 1: Add types and functions**

At the end of `src/lib/api.ts`, add:

```typescript
export interface ResubmitPromptRequest {
    source_image_id: string | null;
    prompt: string;
    n: number;
    model: string;
    size: string;
    quality: string;
}

export interface ResubmitPromptResponse {
    job_id: string;
}

export interface CostEstimate {
    estimated_cost: number;
    model: string;
    size: string;
    quality: string;
    n: number;
}

export async function resubmitPrompt(request: ResubmitPromptRequest): Promise<ResubmitPromptResponse> {
    return invoke<ResubmitPromptResponse>('resubmit_prompt', { request });
}

export async function estimateGenerationCost(model: string, size: string, quality: string, n: number): Promise<CostEstimate> {
    return invoke<CostEstimate>('estimate_generation_cost', { model, size, quality, n });
}
```

- [ ] **Step 2: Verify TypeScript compiles**

Run: `cd /Users/glebkalinin/ai_projects/claude-code-lab/20260502-obsidian/imageview && npx tsc --noEmit 2>&1 | tail -5`
Expected: no errors (or only pre-existing ones)

- [ ] **Step 3: Commit**

```bash
git add src/lib/api.ts
git commit -m "feat: add resubmitPrompt and estimateGenerationCost to API layer"
```

---

## Task 5: Generation Progress in JobProgressPanel

**Files:**
- Modify: `src/lib/components/JobProgressPanel.svelte:22-44`

- [ ] **Step 1: Add generation-progress listener**

In `JobProgressPanel.svelte`, inside the `onMount` block, after the `rescan-progress` listener (line ~41), add:

```typescript
            const u7 = await listen<any>('generation-progress', (e) => {
                upsertJob(e.payload.job_id ?? `evt_generation`, 'generation', 'running', e.payload.current, e.payload.total, `Generating image ${e.payload.current}/${e.payload.total}`);
            });
```

Update the `unlisteners` array on line ~44 to include `u7`:

```typescript
            unlisteners = [u1, u2, u3, u4, u5, u6, u7];
```

- [ ] **Step 2: Verify it builds**

Run: `cd /Users/glebkalinin/ai_projects/claude-code-lab/20260502-obsidian/imageview && npx vite build 2>&1 | tail -5`
Expected: build succeeds

- [ ] **Step 3: Commit**

```bash
git add src/lib/components/JobProgressPanel.svelte
git commit -m "feat: show generation progress in job panel"
```

---

## Task 6: PromptResubmitDialog Component

**Files:**
- Create: `src/lib/components/PromptResubmitDialog.svelte`

This is the core UX component. It's a modal dialog launched from Loupe's prompt panel.

- [ ] **Step 1: Create the dialog component**

Create `src/lib/components/PromptResubmitDialog.svelte`:

```svelte
<script lang="ts">
    import { resubmitPrompt, estimateGenerationCost, type CostEstimate } from '$lib/api';

    interface Props {
        visible: boolean;
        initialPrompt: string;
        sourceImageId: string | null;
        onclose: () => void;
        ongenerated: (imageIds: string[], jobId: string) => void;
    }

    let { visible, initialPrompt, sourceImageId, onclose, ongenerated }: Props = $props();

    let prompt = $state('');
    let model = $state('gpt-image-2');
    let size = $state('1024x1024');
    let quality = $state('auto');
    let n = $state(4);
    let submitting = $state(false);
    let error = $state<string | null>(null);
    let costEstimate = $state<CostEstimate | null>(null);

    const SIZES = ['1024x1024', '1024x1536', '1536x1024', 'auto'];
    const QUALITIES = ['auto', 'low', 'high'];

    $effect(() => {
        if (visible) {
            prompt = initialPrompt;
            error = null;
            submitting = false;
            updateCost();
        }
    });

    async function updateCost() {
        try {
            costEstimate = await estimateGenerationCost(model, size, quality, n);
        } catch {
            costEstimate = null;
        }
    }

    async function submit() {
        if (!prompt.trim() || submitting) return;
        submitting = true;
        error = null;
        try {
            const resp = await resubmitPrompt({
                source_image_id: sourceImageId,
                prompt: prompt.trim(),
                n,
                model,
                size,
                quality,
            });
            ongenerated([], resp.job_id);
            onclose();
        } catch (e: any) {
            error = e?.toString() ?? 'Generation failed';
            submitting = false;
        }
    }

    function handleKeydown(e: KeyboardEvent) {
        if (e.key === 'Escape') onclose();
        if (e.key === 'Enter' && e.metaKey) submit();
    }
</script>

{#if visible}
<!-- svelte-ignore a11y_no_static_element_interactions -->
<div class="dialog-overlay" onclick={onclose} onkeydown={handleKeydown}>
    <!-- svelte-ignore a11y_no_static_element_interactions -->
    <div class="dialog" onclick={(e) => e.stopPropagation()} onkeydown={handleKeydown}>
        <div class="dialog-header">
            <h3>Re-generate</h3>
            <button class="close-btn" onclick={onclose}>&times;</button>
        </div>

        <div class="dialog-body">
            <label class="field">
                <span class="field-label">Prompt</span>
                <textarea
                    bind:value={prompt}
                    rows={4}
                    placeholder="Describe the image..."
                ></textarea>
            </label>

            <div class="settings-row">
                <label class="field compact">
                    <span class="field-label">Size</span>
                    <select bind:value={size} onchange={updateCost}>
                        {#each SIZES as s}
                            <option value={s}>{s}</option>
                        {/each}
                    </select>
                </label>

                <label class="field compact">
                    <span class="field-label">Quality</span>
                    <select bind:value={quality} onchange={updateCost}>
                        {#each QUALITIES as q}
                            <option value={q}>{q}</option>
                        {/each}
                    </select>
                </label>

                <label class="field compact">
                    <span class="field-label">Variations</span>
                    <select bind:value={n} onchange={updateCost}>
                        {#each [1, 2, 3, 4] as v}
                            <option value={v}>{v}</option>
                        {/each}
                    </select>
                </label>
            </div>

            {#if costEstimate}
                <div class="cost-estimate">
                    Estimated cost: ~${costEstimate.estimated_cost.toFixed(3)}
                </div>
            {/if}

            {#if error}
                <div class="error-msg">{error}</div>
            {/if}
        </div>

        <div class="dialog-footer">
            <button class="btn secondary" onclick={onclose}>Cancel</button>
            <button class="btn primary" onclick={submit} disabled={submitting || !prompt.trim()}>
                {submitting ? 'Generating...' : `Generate ${n} variation${n > 1 ? 's' : ''}`}
            </button>
        </div>
    </div>
</div>
{/if}

<style>
    .dialog-overlay {
        position: fixed;
        inset: 0;
        background: color-mix(in srgb, var(--bg) 80%, transparent);
        display: flex;
        align-items: center;
        justify-content: center;
        z-index: 1000;
    }
    .dialog {
        background: var(--surface);
        border: 1px solid var(--border);
        border-radius: calc(var(--radius) * 2);
        width: 480px;
        max-width: 90vw;
        max-height: 80vh;
        overflow-y: auto;
    }
    .dialog-header {
        display: flex;
        justify-content: space-between;
        align-items: center;
        padding: calc(var(--spacing) * 2);
        border-bottom: 1px solid var(--border);
    }
    .dialog-header h3 {
        margin: 0;
        font-size: 14px;
        color: var(--text);
    }
    .close-btn {
        background: none;
        border: none;
        color: var(--text-secondary);
        font-size: 18px;
        cursor: pointer;
        padding: 0 4px;
    }
    .close-btn:hover { color: var(--text); }
    .dialog-body {
        padding: calc(var(--spacing) * 2);
        display: flex;
        flex-direction: column;
        gap: calc(var(--spacing) * 1.5);
    }
    .field {
        display: flex;
        flex-direction: column;
        gap: 4px;
    }
    .field-label {
        font-size: 11px;
        color: var(--text-secondary);
        text-transform: uppercase;
        letter-spacing: 0.04em;
    }
    textarea, select {
        background: var(--bg);
        border: 1px solid var(--border);
        border-radius: var(--radius);
        color: var(--text);
        font-family: var(--font);
        font-size: 13px;
        padding: var(--spacing);
        resize: vertical;
    }
    textarea:focus, select:focus {
        outline: none;
        border-color: var(--blue);
    }
    .settings-row {
        display: flex;
        gap: var(--spacing);
    }
    .compact { flex: 1; }
    .compact select { width: 100%; }
    .cost-estimate {
        font-size: 12px;
        color: var(--text-secondary);
        padding: var(--spacing);
        background: var(--bg);
        border-radius: var(--radius);
        text-align: center;
    }
    .error-msg {
        font-size: 12px;
        color: var(--red);
        padding: var(--spacing);
        background: color-mix(in srgb, var(--red) 10%, transparent);
        border-radius: var(--radius);
    }
    .dialog-footer {
        display: flex;
        justify-content: flex-end;
        gap: var(--spacing);
        padding: calc(var(--spacing) * 2);
        border-top: 1px solid var(--border);
    }
    .btn {
        padding: var(--spacing) calc(var(--spacing) * 2);
        border-radius: var(--radius);
        font-size: 13px;
        font-family: var(--font);
        cursor: pointer;
        border: 1px solid var(--border);
    }
    .btn.secondary {
        background: var(--bg);
        color: var(--text-secondary);
    }
    .btn.primary {
        background: var(--blue);
        color: var(--bg);
        border-color: var(--blue);
    }
    .btn.primary:disabled {
        opacity: 0.5;
        cursor: not-allowed;
    }
    .btn:hover:not(:disabled) {
        filter: brightness(1.1);
    }
</style>
```

- [ ] **Step 2: Verify it builds**

Run: `npx vite build 2>&1 | tail -5`
Expected: build succeeds

- [ ] **Step 3: Commit**

```bash
git add src/lib/components/PromptResubmitDialog.svelte
git commit -m "feat: add PromptResubmitDialog component"
```

---

## Task 7: GenerationResultsStrip Component

**Files:**
- Create: `src/lib/components/GenerationResultsStrip.svelte`

A horizontal strip showing generated images after completion. Listens for `generation-complete` events.

- [ ] **Step 1: Create the component**

Create `src/lib/components/GenerationResultsStrip.svelte`:

```svelte
<script lang="ts">
    import { onMount, onDestroy } from 'svelte';
    import { listen } from '@tauri-apps/api/event';
    import { convertFileSrc } from '@tauri-apps/api/core';
    import { getImagesByIds, type ImageWithFile } from '$lib/api';

    interface Props {
        oncompare: (imageIds: string[]) => void;
        onselect: (imageId: string) => void;
    }

    let { oncompare, onselect }: Props = $props();

    let images = $state<ImageWithFile[]>([]);
    let visible = $state(false);
    let jobId = $state<string | null>(null);
    let unlistener: (() => void) | null = null;

    onMount(async () => {
        try {
            unlistener = await listen<any>('generation-complete', async (e) => {
                const ids: string[] = e.payload.image_ids ?? [];
                jobId = e.payload.job_id ?? null;
                if (ids.length > 0) {
                    images = await getImagesByIds(ids);
                    visible = true;
                }
            });
        } catch {
            // Not in Tauri
        }
    });

    onDestroy(() => {
        unlistener?.();
    });

    function dismiss() {
        visible = false;
        images = [];
    }

    function openCompare() {
        oncompare(images.map(i => i.image.id));
        dismiss();
    }

    function thumbnailUrl(img: ImageWithFile): string {
        return convertFileSrc(img.thumbnail_path ?? img.path);
    }
</script>

{#if visible && images.length > 0}
    <div class="results-strip">
        <div class="strip-header">
            <span class="strip-title">Generated {images.length} image{images.length > 1 ? 's' : ''}</span>
            <div class="strip-actions">
                {#if images.length > 1}
                    <button class="strip-btn" onclick={openCompare}>Compare</button>
                {/if}
                <button class="strip-btn dismiss" onclick={dismiss}>&times;</button>
            </div>
        </div>
        <div class="strip-images">
            {#each images as img}
                <button class="strip-thumb" onclick={() => onselect(img.image.id)}>
                    <img src={thumbnailUrl(img)} alt="" />
                </button>
            {/each}
        </div>
    </div>
{/if}

<style>
    .results-strip {
        position: fixed;
        bottom: 48px;
        left: 50%;
        transform: translateX(-50%);
        background: var(--surface);
        border: 1px solid var(--border);
        border-radius: calc(var(--radius) * 2);
        padding: var(--spacing);
        z-index: 900;
        min-width: 200px;
        max-width: 90vw;
    }
    .strip-header {
        display: flex;
        justify-content: space-between;
        align-items: center;
        margin-bottom: var(--spacing);
    }
    .strip-title {
        font-size: 11px;
        color: var(--text-secondary);
        text-transform: uppercase;
        letter-spacing: 0.04em;
    }
    .strip-actions {
        display: flex;
        gap: 4px;
    }
    .strip-btn {
        background: var(--bg);
        border: 1px solid var(--border);
        border-radius: var(--radius);
        color: var(--blue);
        font-size: 11px;
        font-family: var(--font);
        padding: 2px 8px;
        cursor: pointer;
    }
    .strip-btn.dismiss {
        color: var(--text-secondary);
        border: none;
        background: none;
        font-size: 14px;
    }
    .strip-images {
        display: flex;
        gap: var(--spacing);
        overflow-x: auto;
    }
    .strip-thumb {
        flex-shrink: 0;
        width: 80px;
        height: 80px;
        border-radius: var(--radius);
        overflow: hidden;
        border: 1px solid var(--border);
        padding: 0;
        background: var(--bg);
        cursor: pointer;
    }
    .strip-thumb:hover {
        border-color: var(--blue);
    }
    .strip-thumb img {
        width: 100%;
        height: 100%;
        object-fit: contain;
    }
</style>
```

- [ ] **Step 2: Verify build**

Run: `npx vite build 2>&1 | tail -5`
Expected: build succeeds

- [ ] **Step 3: Commit**

```bash
git add src/lib/components/GenerationResultsStrip.svelte
git commit -m "feat: add GenerationResultsStrip component"
```

---

## Task 8: Wire Into Loupe

**Files:**
- Modify: `src/lib/components/Loupe.svelte`

This is the integration point — add a "Re-generate" button to the prompt panel, and mount the dialog and results strip.

- [ ] **Step 1: Add imports and state**

Near the top of `Loupe.svelte`'s `<script>` block, add imports:

```typescript
    import PromptResubmitDialog from './PromptResubmitDialog.svelte';
    import GenerationResultsStrip from './GenerationResultsStrip.svelte';
```

Add state variables (near the other prompt-related state around line 41):

```typescript
    let resubmitVisible = $state(false);
```

- [ ] **Step 2: Add the Re-generate button to the prompt panel**

In the prompt panel section (around line 424-444), after the "Copy" button in `prompt-header`, add a "Re-generate" button:

Find this block:

```svelte
                <button class="prompt-copy" onclick={copyPrompt} title="Copy prompt">
```

After the closing `</button>` of that copy button, add:

```svelte
                <button class="prompt-action" onclick={() => resubmitVisible = true} title="Re-generate with this prompt">
                    Re-generate
                </button>
```

- [ ] **Step 3: Mount the dialog and results strip**

At the very end of the component template (just before the closing tag), add:

```svelte
    <PromptResubmitDialog
        visible={resubmitVisible}
        initialPrompt={prompt ?? ''}
        sourceImageId={image?.image.id ?? null}
        onclose={() => resubmitVisible = false}
        ongenerated={(ids, jobId) => { /* results strip handles via event */ }}
    />

    <GenerationResultsStrip
        oncompare={(ids) => { /* TODO: wire to compare view in parent */ }}
        onselect={(id) => { /* TODO: navigate to image */ }}
    />
```

- [ ] **Step 4: Add CSS for the Re-generate button**

In Loupe's `<style>` block, add:

```css
    .prompt-action {
        background: var(--blue);
        color: var(--bg);
        border: none;
        border-radius: var(--radius);
        font-family: var(--font);
        font-size: 11px;
        padding: 2px 8px;
        cursor: pointer;
        margin-left: auto;
    }
    .prompt-action:hover {
        filter: brightness(1.15);
    }
```

- [ ] **Step 5: Build and verify**

Run: `npx vite build 2>&1 | tail -5`
Expected: build succeeds

- [ ] **Step 6: Commit**

```bash
git add src/lib/components/Loupe.svelte
git commit -m "feat: wire Re-generate button and dialog into Loupe"
```

---

## Task 9: Manual Integration Test

**Files:** None (manual testing)

- [ ] **Step 1: Set an OpenAI API key**

In the running app, go to any settings/API key area. Or via MCP/CLI if available. The key gets stored with provider `"openai"` in the keychain.

If there's no settings UI for OpenAI keys yet, use the browser console in dev mode:

```javascript
__TAURI__.core.invoke('set_api_key', { provider: 'openai', key: 'sk-...' })
```

- [ ] **Step 2: Test the flow**

1. Open an image in Loupe that has a prompt (from sidecar JSON)
2. Click the "Prompt" toggle to expand the prompt panel
3. Click "Re-generate"
4. The dialog should open with the prompt pre-filled
5. Adjust settings (n=2 for testing to save cost), click "Generate 2 variations"
6. Job progress should appear in the JobProgressPanel
7. After completion, the GenerationResultsStrip should appear at the bottom
8. Click a thumbnail to navigate to it; click "Compare" to open compare view

- [ ] **Step 3: Verify DB state**

Check that `generation_runs` has new rows with `parent_run_id` set, and that a lineage group was created:

```sql
-- In SQLite CLI or via MCP
SELECT id, prompt, provider, model, parent_run_id FROM generation_runs ORDER BY imported_at DESC LIMIT 5;
SELECT * FROM lineage_groups ORDER BY created_at DESC LIMIT 3;
```

- [ ] **Step 4: Commit any fixes**

```bash
git add -A
git commit -m "fix: integration test fixes for prompt resubmit"
```

---

## Open Items (not in this plan)

These are deferred to follow-up work:

- **Settings UI for OpenAI API key** — currently relies on `set_api_key` command; needs a proper Settings panel
- **N-up Compare view** — current Compare is 2-up only (`selectedIds` pair). Sprint 2 spec says "compare N variations" — needs a grid/gallery compare mode that accepts 2-4+ images. The "Compare" button in GenerationResultsStrip should set `selectedIds` and switch to this new mode.
- **Image navigation from results strip** — clicking a result thumbnail should set `focusedIndex` to navigate Loupe to that image; requires the strip to know the parent image list or emit an event
- **Cost tracking dashboard** — aggregate spend over time from `settings_json.estimated_cost`
- **Prompt editing tree** — Sprint 2 says "compare N prompt variations" which implies editing the prompt before re-submitting; the dialog supports prompt editing but tree-style exploration (alter style, color, elements) is deferred
- **Configurable pricing** — hardcoded pricing table will drift; should be updateable via app settings or fetched from a config file
