# `cull://` Action Verbs — Design

Date: 2026-07-02
Status: Approved (brainstorming), pending implementation plan
Tracks: `imageview-m2u` (URL verb vocabulary), partial `imageview-b1k` (batch pipeline)

## Problem

The `cull://` URL scheme ships a **navigation-only** surface today: `open`,
`grid`, `loupe`, `compare` with params `path/paths/folder/view/zoom/size/
fullscreen/focus/image_id/gap`, parsed in `src-tauri/src/commands/deeplink.rs`
into an `OpenParams` struct and emitted to the frontend. The draft in
`docs/cli-and-url-scheme.md` promises **action verbs** (`import`, `export`,
`search`, `contact-sheet`, `resize`, `rate`, …) that were never built.

This design scopes the first wave of action verbs — the ones whose core
operations already exist and route through shared service functions, preserving
the project invariant: **CLI = GUI = URL = MCP, one canonical Rust
implementation.**

## Scope

**In scope (verbs whose core ops exist today):**
- `import` — `services::import::import_folder` / `import_files`
- `export` — `services::export::export_images`
- `rate` — `services::curation::set_rating`
- `accept` / `reject` / `undecide` — `services::curation::set_decision`
- `collection` create/add — `services::curation::create_collection` / `add_to_collection`

**Out of scope (no core op exists — separate future projects):**
- `search` (text→image query — only text-*embedding generation* exists, no query verb)
- `contact-sheet` (only a frontend canvas render exists, no headless op)
- `resize`, `convert` (no image-processing op; format conversion exists only inside `export_images --format`)
- `metadata` (EXIF/IPTC read command — XMP code is export-sidecar write only)
- Multi-action batch chaining (`imageview-b1k` beyond a single action per URL)
- CLI verb expansion (separate surface; this plan is URL-only)

## Locked Decisions

1. **Verb scope:** curation wave (import, export, rate, decision, collection).
2. **Execution:** hybrid — GUI-reflected by default; `import`/`export` also run
   fully headless via `gui=false`.
3. **Safety:** tiered — reversible DB ops (rate/decision/collection) run silently
   (undoable); disk-writing ops (export/import) show a confirm sheet in GUI mode
   unless app-initiated; headless mutations require pre-authorization
   (Settings toggle + MCP token).
4. **Headless authorization:** reuse existing MCP token infra (operator+ role)
   plus a default-off Settings toggle "Allow headless URL actions".
5. **Headless completion feedback:** add `tauri-plugin-notification` for desktop
   notifications on headless completion.

## Architecture

### One parse, one dispatch, shared core

Replace the navigation-only parse result with an action enum:

```rust
pub enum DeepLinkAction {
    Navigate(OpenParams),                                  // existing behavior
    Import   { source: TargetSource, headless: bool },
    Export   { targets: TargetSource, output_dir: String,
               format: Option<String>, flatten: bool,
               naming: Option<String>, headless: bool },
    Rate     { targets: TargetSource, stars: u8 },
    Decision { targets: TargetSource, decision: Decision }, // accept/reject/undecide
    Collection(CollectionAction),                           // Create{name} | Add{ref, targets}
}
```

`parse_deep_link(url) -> Result<DeepLinkAction, String>` maps the URL host
(action verb) + query params to a variant. A single
`dispatch(action, ctx) -> Result<DispatchOutcome, String>` routes each variant to
the **existing service function** — no new business logic. This is what keeps the
CLI/GUI/URL/MCP invariant true.

### Module split

`commands/deeplink.rs` is 628 lines. Split into `commands/deeplink/`:
- `mod.rs` — `OpenParams`, existing navigation (behavior unchanged), Tauri command exports
- `parse.rs` — URL → `DeepLinkAction`, param vocabulary, percent-decode
- `dispatch.rs` — `DeepLinkAction` → service calls, GUI/headless routing
- `security.rs` — `path_policy::Deeplink` application + confirm tiering + headless auth

Existing tests move with their code; navigation parse/validation behavior is
preserved exactly (regression tests must still pass).

### Target resolution

Mutation verbs address images by any of:
- `ids=` — library image IDs (comma-separated)
- `path=` / `paths=` — filesystem paths, resolved to library IDs
- `collection=` — collection name or ID
- `folder=` — imported folder path

Modeled as `TargetSource`, resolved by one `resolve_targets(source, ctx) ->
Vec<image_id>` helper. Param names mirror MCP/CLI (`image_ids`, `collection_id`,
`folder_path`) where practical.

### Execution modes

- **GUI (default):** dispatch on the live app; emit a progress/result event
  (extends the existing `emit("open-with-params")` pattern with a new
  `deep-link-action-result` event), focus the window, and navigate to reflect the
  result (e.g. show the imported folder / confirm export). Launches app if not
  running; cold-start reuses the existing `PENDING_OPEN_PARAMS` queue mechanism.
- **Headless (`gui=false`):** run in tray context with no window; post a desktop
  notification on completion. Import/export only in this plan.

### Security & confirmation

- Every filesystem path passes `path_policy::validate_path(..., PathMode::Deeplink)`
  — unchanged strict rules (under `$HOME`, reject hidden/sensitive/traversal).
- **Tiered confirm:**
  - `rate`, `decision`, `collection` → execute immediately (undoable via the undo stack).
  - `export`, `import` → confirm sheet in GUI mode ("Cull wants to export N images
    to ~/X — Allow?"), skipped only when the link is app-initiated (internal origin).
- **Headless mutations** require: Settings toggle `allow_headless_url_actions`
  (default off) **and** a `token=` param matching an active MCP token with
  operator+ role. Reuses `services::tokens` verification; no new auth system.

### Error handling

Fire-and-forget to the caller (URL scheme has no return channel). Parse/policy
failures: log + ignore (existing behavior) + toast in GUI mode. Op failures:
toast (GUI) or desktop notification (headless). App always stays in a valid state.

## Data Flow

```
cull://export?collection=faves&output_dir=~/Desktop/out&format=webp&gui=false&token=T
  → single-instance / open-url handler (lib.rs)
  → parse_deep_link → DeepLinkAction::Export{ targets: Collection("faves"), .., headless: true }
  → security: validate output_dir (Deeplink policy); headless ⇒ require toggle + verify token(T) ≥ operator
  → resolve_targets(Collection("faves")) → [image_ids]
  → services::export::export_images(ctx, image_ids, output_dir, format, ..)
  → headless ⇒ desktop notification "Exported N images"
```

## Testing Strategy

- **Unit (parse):** each verb → correct `DeepLinkAction`; full param vocab;
  malformed percent-encoding rejected; path-policy rejection per verb.
- **Dispatch (temp-DB harness, mirrors CLI `--db`/`--app-data-dir`):** import adds
  rows; export writes files; rate/decision/collection mutate DB via shared services.
- **Security:** confirm-required verbs do not execute unconfirmed; headless
  mutation blocked without toggle+token; token below operator role rejected.
- **Regression:** all existing navigation deep-link tests still pass unchanged.
- **E2E smoke:** `cull://rate?ids=…&stars=5` reflects in the running GUI.

## Risks & Notes

- `tauri-plugin-notification` is a new dependency (the existing `notify` crate is
  the filesystem watcher, not desktop notifications). License must be checked with
  `npm run audit:licenses` per project policy.
- URL links are attacker-reachable; the tiered-confirm + headless-auth design is
  the mitigation. Do not weaken the confirm tier without revisiting this.
- Keep `docs/cli-and-url-scheme.md` in sync: mark these verbs implemented, leave
  the rest flagged draft.
