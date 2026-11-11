# Safe Thumbnail Previews Through the Cull MCP — Design

Date: 2026-09-17
Issue: `imageview-9k1u.14` (epic `imageview-9k1u` — agent-first UX)
Status: approved, ready for implementation planning

## Purpose

MCP browse tools (`list_images`, `get_image`, `list_folder_images`) return
metadata only. An agent that wants to compare candidates visually must either
drive the live UI (`capture_current_view_snapshot`) or read original files,
which remote tokens cannot do. This adds a bounded, privacy-aware preview
surface so an agent can build an evidence-led shortlist from Cull's own
generated thumbnails.

Job: *When I browse a folder through MCP, I need a bounded visual representation
of the candidates so I can shortlist without a UI snapshot or access to
originals.*

## Scope

In scope:

- One new read-only MCP tool, `get_image_previews`, for explicit image IDs or a
  bounded folder page.
- Separate transport contracts for local stdio and authenticated HTTP.
- Reuse of generated thumbnails only.
- Scope/token authorization, path redaction, audit logging.
- Documented sizes, bounds, and per-image failure behavior in `docs/agents.md`.
- Tests for scope isolation, redaction, bounds, generated-thumbnail-only
  behavior, and JPEG/RAF pairs.

Out of scope:

- Headless CLI support (the CLI is a curated slice; previews are MCP-only).
- Any database migration, schema change, thumbnail generation change, or
  frontend change.
- Previewing originals, RAW files, or documents at full size.
- Live-view snapshots (already covered by `capture_current_view_snapshot`).

## Decisions

Resolved with the maintainer before writing this spec:

| Decision | Choice |
| --- | --- |
| Interface shape | New dedicated tool `get_image_previews` (IDs **or** bounded folder page) |
| Local vs authenticated payload | Local = file paths; authenticated = inline base64 blocks, no paths |
| Default thumbnail size | 256 px (64 / 128 / 256 / 800 selectable) |
| Per-call bounds | 20 images; 2 MB per image; 8 MB authenticated payload budget |

Rejected alternatives: an opt-in `include_thumbnails` flag on `get_image` /
`list_folder_images` (changes existing responses, two tool surfaces to keep in
sync); always-inline base64 for local callers (megabytes over the stdio pipe for
no benefit); a hybrid split (two contracts to document and test).

## Existing Context

- Thumbnails are already generated for every imported image by
  `src-tauri/src/db_core/thumbnails.rs`: `<id>.jpg` is the 800 px base, plus
  `<id>_{64,128,256}.jpg`. `THUMBNAIL_SIZES = [64, 128, 256, 800]`, longest
  edge, JPEG quality 90. No upscaling — small sources are copied at native size.
- `sized_thumbnail_path(app_data_dir, image_id, size)` returns the path for a
  given size; there is no read helper for bytes.
- RAF sources are decoded from an embedded JPEG (`src-tauri/src/raw/fuji.rs`),
  so a RAF row already has an ordinary generated JPEG thumbnail.
- Path redaction currently keys off `is_remote()` =
  `!can_expose_private_metadata(auth)`, which is false for `AuthContext::Local`
  **and** for admin-role tokens. An admin token over HTTP therefore sees
  filesystem paths today. The preview contract must be stricter.
- `rmcp 3.1` tool handlers may return `CallToolResult` directly, whose
  `content: Vec<ContentBlock>` accepts `ImageContent { data, mime_type }`.
- Capability and audit wiring: `mcp/auth.rs` holds `ALL_TOOLS` / `READ_TOOLS`;
  `services/tokens.rs::tool_capability` maps every tool name to a capability,
  and `auth.rs` has a test that fails when a tool falls through to the default
  `settings:manage`. `CullMcp::call_tool` logs every call via
  `redact_audit_params`, which already redacts any key containing `path`/`dir`/
  `folder` and any value that looks like a path.
- Existing MCP tests are helper-level: they exercise pure functions and
  in-memory databases, never a Tauri `AppState` (`:memory:` `Database` +
  `insert_image` / `insert_image_file`, as in `services/tokens.rs` tests).

## Tool Contract

`get_image_previews`

```jsonc
{
  "image_ids": ["img_a", "img_b"],   // mode 1: explicit IDs, 1..=20 (deduped)
  "folder_path": "/abs/folder",      // mode 2: bounded page from a folder
  "offset": 0,                       // mode 2 only, default 0
  "limit": 20,                       // mode 2 only, 1..=20, default 20
  "size": 256                        // optional: 64 | 128 | 256 | 800, default 256
}
```

Validation:

- Exactly one of `image_ids` / `folder_path`. Both or neither is an error.
- `image_ids` is deduplicated in request order and blank entries are skipped.
  An empty list (after dedupe), or more than 20 **distinct** entries, is an
  error — the tool never silently truncates an explicit request. The 20-item
  limit is applied after dedupe.
- `size` must be one of `64`, `128`, `256`, `800`.
- Folder pages are capped at 20 items regardless of `limit`; `limit` is clamped
  to `1..=20` (a `limit` of 0 becomes 1) and `offset` is treated as 0 when
  absent.
- `offset` and `limit` are ignored in IDs mode; they never error there.
- A folder page that matches no images returns an empty manifest
  (`count: 0`, `items: []`) rather than an error, matching how
  `list_folder_images` answers an empty or unseen folder.

`get_image_previews` is **read-only** and maps to the `library:read`
capability. It is not gated by a settings module and is not part of the
headless CLI slice.

## Response Contract

The handler returns a `CallToolResult`:

- `content[0]` — one `TextContent` block holding the JSON manifest.
- `content[1..]` — zero or more `ImageContent` blocks (`image/jpeg`), present
  for authenticated callers only, in manifest order.

Manifest:

```jsonc
{
  "size_requested": 256,
  "transport": "local_paths" | "inline_base64",
  "count": 2,
  "items": [
    { "image_id": "img_a", "status": "ok", "content_index": 0,
      "thumbnail_size": 256, "width": 256, "height": 256, "bytes": 18422,
      "thumbnail_path": "/…/thumbnails/img_a_256.jpg" },
    { "image_id": "img_b", "status": "missing" }
  ]
}
```

`count` is the number of items in the manifest. `content_index` is the index of
the item's block within `content[1..]`; it is present only for `ok` items and
only in `inline_base64` transport.

### Transport split

| | Local stdio (`AuthContext::Local`) | Authenticated (any role, incl. admin) |
| --- | --- | --- |
| `transport` | `local_paths` | `inline_base64` |
| `thumbnail_path` | absolute path to the generated file | never present |
| Image blocks | none | one per `ok` item |
| Original/RAW path | never present | never present |
| Source path (`image.path`) | never present | never present |

Rationale: a local stdio client can read files itself, so inline base64 would
only bloat the JSON-RPC pipe. An authenticated client — including an admin
token bound to a non-loopback host — may not read files and must never learn
local paths. Redaction for this tool keys off `AuthContext::Local` alone,
which is strictly stronger than `maybe_redact_path`.

Only generated thumbnail paths are ever exposed. Original and RAW source paths
are not part of the preview contract under any transport.

### Statuses

| Status | Meaning |
| --- | --- |
| `ok` | A generated thumbnail was found and returned (or its path reported). |
| `missing` | The image is authorized and present, but no generated thumbnail file exists. |
| `not_found` | The requested ID is not in the library. Local transport only. |
| `unavailable` | The ID is unknown **or** outside the token scope. Authenticated transport only. |
| `skipped_too_large` | The generated thumbnail file exceeds the 2 MB per-image cap (raw file bytes) (authenticated only). |
| `skipped_budget` | The 8 MB authenticated payload budget was already exhausted. |

`unavailable` deliberately does not distinguish unknown from out-of-scope, so a
scoped token cannot probe library existence by ID. `missing` is not a leak: the
caller already knows the image is authorized and present. A per-image failure
never fails the whole call; unrelated images still return normally.

`skipped_too_large` and `skipped_budget` still report `thumbnail_size` and
`bytes` when those are known, so the agent can see what it did not receive and
why; they carry no `content_index` and no image block.

## Thumbnail Resolution

Paths come from `db_core::thumbnails::sized_thumbnail_path` /
`thumbnail_path` only. Nothing is re-encoded, copied, or read from originals.

When the requested size file is absent, fall back to the next larger generated
size and report the size actually served in `thumbnail_size`:

| Requested | Fallback chain |
| --- | --- |
| 64 | 64 → 128 → 256 → 800 |
| 128 | 128 → 256 → 800 |
| 256 | 256 → 800 |
| 800 | 800 |

If no file in the chain exists, the item is `missing`. File size comes from
metadata; pixel dimensions come from `image::image_dimensions` (no full
decode). Local callers get `bytes` and `width`/`height` without any file read
of contents; authenticated callers get the same plus the bytes as base64.

A RAF row resolves to its generated JPEG thumbnail exactly like any other
image. A folder containing a JPEG/RAF pair yields one preview per library row,
both served as `image/jpeg`; the `.RAF` and `.JPG` source paths never appear in
the manifest or the blocks.

## Bounds

- Maximum 20 items per call (both modes). An explicit over-limit `image_ids`
  request is an error rather than a silent truncation.
- Per-image cap 2 MB of **raw file bytes** for **authenticated** responses. A
  larger generated file becomes `skipped_too_large`. Local transports carry no
  payload, so the cap does not apply to them.
- Authenticated total budget 8 MB of **base64-encoded** payload. Once an `ok`
  item would exceed the budget, that item and every remaining item that would
  otherwise be `ok` become `skipped_budget`. Items already resolved to
  `missing`, `not_found`, or `unavailable` keep those statuses. This is
  deterministic: no partial backfill of later, smaller images, so the manifest
  always mirrors the returned blocks.
- At the default 256 px, a full 20-image page is roughly 400 KB of base64, so
  the budget binds only for large (800 px) requests on heavy images.

## Authorization And Audit

- IDs mode: every ID passes `check_image_id_scope` before resolution. Out of
  scope → `unavailable` (authenticated).
- Folder mode: `tokens::folder_in_scope` first; a folder outside scope is an
  explicit error. Every item is then authorized individually inside resolution
  via `tokens::image_id_in_scope`, which loads folder and collection membership;
  this is a single source of truth for both request modes and lets a
  collection-scoped token authorize a row reached through a folder page. A
  folder page may therefore yield fewer `ok` items than `limit` when the scope
  is sparse.
- `get_image_previews` is added to `ALL_TOOLS` and `READ_TOOLS` in
  `mcp/auth.rs` and mapped to `library:read` in `tokens::tool_capability`, so
  capability checks and the completeness test stay correct.
- Audit rows are written by the existing `CullMcp::call_tool` wrapper.
  `redact_audit_params` already stores `folder_path` as `[redacted:path]`;
  `image_ids` are logged as-is.

## Documentation

`docs/agents.md` gains a "Thumbnail previews (evidence-led shortlisting)"
section covering: the parameter schema, the size table, the local vs
authenticated contract table, bounds, per-image statuses, failure behavior, the
generated-thumbnails-only guarantee, the recommended flow
(`list_folder_images` → `get_image_previews` → `set_rating` / `set_decision`),
and the explicit note that the tool is MCP-only and absent from the headless CLI
slice.

## Testing Strategy

The resolution and budget logic is extracted into helpers that take
`&Database` and `&Path` so it is testable without a Tauri `AppState`, matching
the existing `tools.rs` test style.

Helpers:

- `preview_size_chain(size) -> Vec<u32>` — pure fallback chain.
- `resolve_preview_items(db, app_data_dir, scope, local, refs, size)` — DB-backed
  per-item resolution against an in-memory database, returning status plus
  optional path / bytes / dimensions.
- `build_preview_response(plan, local, budget)` — pure: applies the per-image
  and total caps and produces the manifest value plus the ordered list of image
  block payloads.

Cases:

1. Status mapping — `ok`, `not_found`, `missing`, `unavailable`,
   `skipped_too_large`, `skipped_budget` are each produced by the expected
   input.
2. Scope isolation — a folder-scoped token gets `ok` for in-scope IDs and
   `unavailable` for out-of-scope IDs, using an in-memory database.
3. Path redaction — an authenticated manifest contains no app-data path, no
   `thumbnail_path` key, and no source path; the local manifest does contain an
   absolute `thumbnail_path`.
4. Payload bounds — over-20 `image_ids` errors; an over-2 MB file becomes
   `skipped_too_large`; exceeding the total budget marks that item and all
   following items `skipped_budget`; block count equals `ok` count and every
   `content_index` resolves to the right block.
5. Generated-thumbnail-only — an image whose source path ends in `.RAF` returns
   `ok` from its generated thumbnail, and neither `.RAF` nor the original path
   appears in the manifest.
6. JPEG/RAF pair — a folder holding `frame.JPG` and `frame.RAF` rows returns two
   `ok` items, both `image/jpeg`, with no source paths in the output.
7. Size fallback — with only the 800 px base file present, a 256 px request
   returns `ok` with `thumbnail_size: 800`; an invalid `size` errors.

## Risks

- **Preview scope creep into full originals.** Mitigated by the
  generated-thumbnail-only resolution path and test 5.
- **Payload blow-up on 800 px batches.** Mitigated by the 2 MB / 8 MB caps and
  deterministic `skipped_budget`.
- **A future admin-token HTTP deployment leaking paths.** Mitigated by keying
  preview redaction off `AuthContext::Local` rather than admin role.
- **Docs drift.** The `docs/agents.md` contract is prose; the release-contract
  test only pins a fixed tool-name list, so the new tool will not be silently
  unverified. Accept this and keep the section adjacent to the tool.
