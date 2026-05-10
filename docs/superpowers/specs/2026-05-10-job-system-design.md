# Job System Design

## Overview

Add a background job system to ImageView so long-running MCP operations return immediately with a `job_id` while work runs asynchronously. Clients poll for status. The frontend receives progress events for UI updates.

## Motivation

RAW image support is coming — processing CR2/NEF/ARW files takes 1-15 seconds per file. Current MCP tools block until completion, which times out for large imports. CLIP embedding, YOLO detection, and Ollama vision analysis also need async handling.

## Architecture

In-memory job registry. No persistence, no external dependencies. Jobs are tokio tasks with cooperative cancellation.

### Core Types

```rust
pub struct JobRegistry {
    jobs: Arc<Mutex<HashMap<Uuid, JobState>>>,
}

pub struct JobState {
    pub snapshot: JobSnapshot,
    pub cancel: CancellationToken,
}

#[derive(Clone, Serialize)]
pub struct JobSnapshot {
    pub job_id: String,
    pub kind: String,          // "import", "embeddings", "detection", "vision", "rescan"
    pub status: String,        // "running", "cancelling", "completed", "failed", "cancelled"
    pub current: u32,
    pub total: u32,
    pub message: Option<String>,
    pub error: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}
```

### Job Lifecycle

1. MCP tool validates auth/scope
2. Creates `JobState` with `status: "running"`, inserts in registry
3. Spawns `tokio::spawn` with the work
4. Returns `{ "job_id": "..." }` immediately
5. Spawned task: loops over items, checks `CancellationToken` between each, updates progress in registry, emits Tauri event
6. On completion: sets `status: "completed"` (or `"failed"` with error message)
7. On cancel request: sets `status: "cancelling"`, task checks token and sets `"cancelled"` when it stops

### MCP Tools

- `get_job { job_id }` → JobSnapshot
- `list_jobs` → Vec<JobSnapshot> (running + recent completed)
- `cancel_job { job_id }` → { status: "cancelling" }

### Capability Mapping

- `get_job` / `list_jobs` → same capability as the job's kind (viewer can see import jobs only if they have `import:write`)
- `cancel_job` → same capability as the job's kind
- Simplified: all job tools map to `settings:manage` (admin-only) for MVP

### Flows to Convert

| Flow | Kind | Current Location | Lock Discipline |
|---|---|---|---|
| Import folder | `import` | `commands/import.rs` | DB lock per-file, release before thumbnail gen |
| Generate embeddings | `embeddings` | `commands/embeddings.rs` | DB lock for store only, release before inference |
| Detect objects | `detection` | `commands/detection.rs` | DB lock for store only, release before inference |
| Analyze images | `vision` | `commands/vision.rs` | DB lock for store only, release before API call |
| Rescan sources | `rescan` | `commands/import.rs` | DB lock per-file |

### Concurrency Rules

- Engine mutexes (CLIP, YOLO) naturally serialize — only one job can use each engine at a time
- DB lock: acquire only for short reads/writes, release before CPU/IO work
- Ollama: external API, can run concurrently with other jobs
- Import: can run concurrently with detection/embedding jobs

### Memory Management

- Completed/failed/cancelled jobs pruned after 1 hour or when count exceeds 100
- Pruning runs on `list_jobs` calls and on new job creation
- Running jobs never pruned

### Frontend Integration

Existing Tauri events (`import-progress`, `embedding-progress`, etc.) continue to fire from within jobs. The frontend progress UI works unchanged. New: `job-status-changed` event when a job completes/fails/cancels.

### Files

```
src-tauri/src/services/jobs.rs    — JobRegistry, JobSnapshot, job lifecycle
src-tauri/src/mcp/tools.rs       — get_job, list_jobs, cancel_job tools + convert existing tools to spawn jobs
```

## Non-Goals

- Job persistence across app restarts
- Job retry/resume
- Worker pool or concurrency limits beyond engine mutexes
- Priority queues
