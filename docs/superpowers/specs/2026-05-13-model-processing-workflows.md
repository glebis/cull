# Model Processing Workflows

## Overview

Cull has several AI lanes today: local CLIP embeddings, Gemini embeddings, YOLO/NudeNet detection, Ollama vision metadata, live image generation, sidecar generation metadata, MCP background jobs, and export manifests. The implementation direction is to keep those typed outputs, but add a durable `model_run` provenance layer that every AI operation can attach to.

Hugging Face should not be bolted on as a parallel feature. It should enter the same pipeline either as a model source for local runtimes or as a remote transport for hosted inference.

## JTBD

When AI art workers curate large image libraries, they want to run repeatable model tasks over selected images and carry the results into search, review, generation, and export, so their production pipeline is auditable instead of a pile of one-off tool calls.

## Current System

- `src-tauri/src/services/jobs.rs` tracks background job state and terminal snapshots in `mcp_jobs`.
- `src-tauri/src/commands/embeddings.rs` runs local CLIP and Gemini embeddings.
- `src-tauri/src/commands/detection.rs` runs YOLO and NudeNet detections.
- `src-tauri/src/commands/vision.rs` calls Ollama VLMs.
- `src-tauri/src/services/generation.rs` runs OpenAI, OpenRouter, and Google generation with `generation_runs`.
- `src-tauri/src/export/manifest.rs` carries source and generated asset provenance into export workflows.
- `src-tauri/src/mcp/tools.rs` exposes long-running AI work through job-backed MCP tools.

## Common Pipeline

```mermaid
flowchart LR
    A["Select scope"] --> B["Resolve input assets"]
    B --> C["Choose task and model profile"]
    C --> D["Validate privacy, license, cost, and capability"]
    D --> E["Create job and model_run"]
    E --> F["Execute provider adapter"]
    F --> G["Normalize output"]
    G --> H["Persist typed projection"]
    H --> I["Update UI, MCP, search, and export surfaces"]
```

## Data Model

`model_runs` is the source of truth for AI provenance and run lifecycle. `mcp_jobs` remains execution status plumbing for the UI and MCP polling. `model_runs.job_id` is a nullable correlation string, not a foreign key.

### `model_profiles`

Profiles separate runtime, model source, and privacy class so Hugging Face can be represented precisely.

- `runtime`: `onnx`, `ollama`, `http_openai`, `http_gemini`, `http_hf_providers`, `http_hf_endpoint`
- `source`: `bundled`, `hf_hub`, `user_path`, `remote`
- `privacy_class`: `local`, `remote_byok`, `remote_hosted`

### `model_runs`

One row per model task over a scope. Important fields:

- `job_id`: optional job correlation id
- `parent_run_id`: pipeline and retry trees
- `task`, `provider`, `model_id`, `model_revision`
- `input_scope_json`, `params_json`, `output_summary_json`
- `cost_estimate_usd`, `cost_actual_usd`
- `status`, `error`, timestamps

### `model_run_items`

Slim per-item execution detail. Typed projection tables remain the query source of truth.

- `run_id`, `image_id`
- `input_asset_uri`, `input_hash`
- `status`, `attempt_count`
- `output_ref_kind`, `output_ref_id`
- optional `audit_payload_json`
- optional `cost_usd`

Existing typed tables should get nullable provenance links incrementally, starting with `embeddings.model_run_id`.

## Hugging Face Integration

Hugging Face maps to two separate concepts:

- Model source: HF Hub-distributed local assets, especially ONNX-safe models.
- Remote transport: HF Inference Providers or dedicated Inference Endpoints.

First useful tasks:

- image classification -> `image_metadata`
- object detection -> `detections`
- segmentation -> masks or future model artifacts
- image-to-image -> generated assets plus `generation_runs`
- text-to-image -> existing generation queue and `generation_runs`
- VQA/image-to-text -> `image_metadata`

Non-goals for the first implementation:

- arbitrary repository Python inside the Tauri app
- pretending every HF model is compatible
- bypassing license, gated-model, privacy, or cost checks

## Workflows

### Library Enrichment

User selects a folder, collection, smart collection, or search result. Cull creates model runs for selected profiles, stores typed projections, then updates search, smart collections, counts, and Loupe metadata.

### Curation Search

Search checks whether required projections exist for the current scope. Missing projections can be backfilled through model runs. Query results stay normal database queries, not provider-specific calls.

### Prompt Iteration

Generation creates `generation_runs` and files as it does today. The generation lane should also create `model_runs` so generated assets and downstream enrichment share one provenance vocabulary.

### Export Enrichment

Export is not a model task. Export declares required outputs, such as alt text, captions, object tags, safety scores, masks, focal hints, or generated variants. A dependency resolver chooses model task profiles to satisfy those requirements before render.

### Production Pipeline

Saved pipeline presets can run model profiles over a scope, then produce an export manifest, rendered files, and a run report. HF dedicated endpoints fit here once local and low-risk hosted tasks are proven.

### MCP / Agent Workflow

MCP tools return `job_id` today. Model-backed tools should also expose `model_run_id` once run creation happens before the async job starts. Agents can then poll jobs, inspect run provenance, read typed outputs, patch manifests, and render exports.

## Implementation Order

1. Add `model_profiles`, `model_runs`, `model_run_items`, and `embeddings.model_run_id`.
2. Add a model pipeline runner skeleton.
3. Wrap one safe lane first: local CLIP embeddings.
4. Add focused DB and runner tests.
5. Add model-run UI detail and a read API.
6. Wrap YOLO/NudeNet and Ollama after the CLIP path proves stable.
7. Add HF Hub discovery for ONNX-compatible local models.
8. Add HF Inference Providers, then dedicated Endpoints.
9. Add export dependency requirements and production pipeline presets.

## First Slice Status

Implemented slice:

- migrations for `model_profiles`, `model_runs`, `model_run_items`
- nullable `embeddings.model_run_id`
- DB helpers for model runs, items, and embedding provenance
- `services/model_pipeline.rs` with local CLIP embedding runner
- Tauri `generate_embeddings` routed through the runner
- MCP `generate_embeddings` routed through the runner
- tests for schema, embedding provenance, and failed item recording

