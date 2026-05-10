# Job System Requirements

1. MUST add `src-tauri/src/services/jobs.rs` with `JobRegistry`, `JobState`, `JobSnapshot`, `JobKind`, and `JobStatus`.

2. MUST store `JobRegistry` on `AppState` as `pub jobs: JobRegistry`; MUST NOT store it on `ImageViewMcp`, because MCP services are per request/session.

3. MUST initialize `jobs: JobRegistry::default()` in `app.manage(AppState { ... })`.

4. MUST use `Arc<Mutex<HashMap<Uuid, JobState>>>`; locks MUST NOT be held across `.await`.

5. MUST use `tokio_util::sync::CancellationToken` for cooperative cancellation.

6. MUST define params for rmcp 1.6 tools with `#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]`.

7. MUST define serializable outputs with `#[derive(Debug, Clone, serde::Serialize)]`.

8. MUST expose MCP tools: `get_job { job_id: String }`, `list_jobs {}`, `cancel_job { job_id: String }`.

9. MUST return MCP tool results as JSON strings matching current `src/mcp/tools.rs` convention.

10. MUST return unknown `job_id` as an MCP error, not an empty result.

11. MUST use string enum values: job kind `import|embeddings|detection|vision|rescan`; status `running|cancelling|completed|failed|cancelled`.

12. MUST include in `JobSnapshot`: `job_id`, `kind`, `status`, `current`, `total`, `message`, `error`, `created_at`, `updated_at`.

13. MUST use RFC3339 timestamps.

14. MUST set `total` before spawning when knowable; MAY use `0` only when total is genuinely unknown.

15. MUST insert a running job before spawning.

16. MUST return `{ "job_id": string }` immediately from converted long-running MCP tools.

17. MUST update progress after each item and on terminal state.

18. MUST check cancellation between items and before expensive CPU/IO/API work.

19. MUST set cancel request status to `cancelling` and signal the token.

20. MUST set terminal status exactly once: `completed`, `failed`, or `cancelled`.

21. MUST keep existing Tauri progress events.

22. MUST emit `job-status-changed` on terminal status.

23. MUST prune terminal jobs after 1 hour or when terminal count exceeds 100.

24. MUST prune on job creation and `list_jobs`; MUST never prune running/cancelling jobs.

25. MUST convert MCP `import_folder`, `rescan_sources`, `generate_embeddings`, `detect_objects`, and `analyze_images` to jobs.

26. MUST keep DB locks scoped to short reads/writes and release before thumbnail generation, inference, or Ollama calls.

27. SHOULD map MVP job tools to `settings:manage`.

28. MAY later map job access to each job kind's originating capability.
