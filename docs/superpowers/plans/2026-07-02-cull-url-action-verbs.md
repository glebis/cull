# `cull://` Action Verbs Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Extend the `cull://` URL scheme from navigation-only to a first wave of action verbs (import, export, rate, accept/reject/undecide, collection create/add) that route through the existing shared service functions.

**Architecture:** Replace the navigation-only `parse_deep_link → OpenParams` with `parse_deep_link → DeepLinkAction` (an enum whose `Navigate` variant preserves today's behavior). A single `dispatch_action(state, app, action)` routes each variant to the *existing* `services::{import,export,curation}` functions — no new business logic — enforcing the CLI = GUI = URL = MCP invariant. Mutation verbs pass every filesystem path through `path_policy::Deeplink`; disk-writing verbs (import/export) are tier-gated (GUI confirm sheet / headless token + Settings toggle).

**Tech Stack:** Rust, Tauri 2, rusqlite, `tauri-plugin-deep-link` (already integrated), `tauri-plugin-notification` (NEW), SvelteKit 5 frontend.

## Global Constraints

- Rust backend under `src-tauri/src/`; Tauri commands are `pub async fn`, take `State<'_, AppState>`, return `Result<T, String>`.
- `AppState { db: Database, app_data_dir: PathBuf, embedding_engine, detection_engine, safety_engine, secrets }` — the running app (tray or windowed) always has all fields.
- `ServiceContext<'a> { db, app_data_dir, embedding_engine, detection_engine, safety_engine, secrets, app_handle }` — build from `AppState` to call `services::curation::*`.
- Service signatures to reuse verbatim:
  - `services::import::import_folder(db: &Database, app_data_dir: &Path, params: ImportFolderParams) -> Result<ImportResult, String>`
  - `services::import::import_files(db: &Database, app_data_dir: &Path, params: ImportFilesParams) -> Result<ImportResult, String>`
  - `services::export::export_images(db: &Database, app_data_dir: &Path, params: ExportImagesParams) -> Result<ExportImagesResult, String>`
  - `services::curation::set_rating(ctx, image_id, rating: u8)`, `set_decision(ctx, image_id, decision: &str)`, `create_collection(ctx, name) -> String`, `add_to_collection(ctx, collection_id, image_ids: &[&str])`
- Path safety: every FS path → `crate::db_core::path_policy::validate_path(raw, PathMode::Deeplink)`. Do NOT weaken.
- All colors/CSS in frontend use `app.css` tokens (Tokyo Night). Never hardcode.
- License policy: run `npm run audit:licenses` after adding `tauri-plugin-notification`.
- Test harness for db-touching tests mirrors the `Fixture` pattern in `src-tauri/src/commands/mcp.rs` and `services/curation.rs` tests: `Database::open(":memory:")`, `MemoryStore`, `tempfile::tempdir()`.
- Commit after every task. Frontend-only commits may use `--no-verify`; Rust commits must not.

---

## File Structure

- `src-tauri/src/commands/deeplink/mod.rs` — `OpenParams`, existing navigation commands/emit/pending-queue (moved, behavior unchanged), re-exports.
- `src-tauri/src/commands/deeplink/parse.rs` — `DeepLinkAction`, `TargetSource`, `parse_deep_link`, `percent_decode`.
- `src-tauri/src/commands/deeplink/resolve.rs` — `resolve_targets`.
- `src-tauri/src/commands/deeplink/security.rs` — per-verb path validation, `ConfirmTier`, headless authorization.
- `src-tauri/src/commands/deeplink/dispatch.rs` — `dispatch_action`, `DispatchOutcome`.
- `src-tauri/src/lib.rs` — wire new dispatch into the three deep-link entry points; register `tauri-plugin-notification`.
- `src-tauri/src/services/settings.rs` (or existing settings module) — `allow_headless_url_actions` flag.
- `src/lib/deeplink.ts` + a component — frontend confirm sheet + result toast.
- `docs/cli-and-url-scheme.md`, `TEST_SCENARIOS.md` — docs sync.

The current `src-tauri/src/commands/deeplink.rs` is 628 lines and mixes parsing, validation, the pending-queue, Tauri commands, and tests; splitting it is in scope because we are adding substantially to it.

---

## Task 1: Split `deeplink.rs` into a module (pure refactor)

**Files:**
- Create: `src-tauri/src/commands/deeplink/mod.rs`
- Delete: `src-tauri/src/commands/deeplink.rs` (content moves into `mod.rs`)
- Modify: `src-tauri/src/commands/mod.rs` (no change needed — `pub mod deeplink;` still resolves to the directory)

**Interfaces:**
- Produces: unchanged public API — `OpenParams`, `parse_deep_link`, `validate_open_params`, `emit_open_params`, `open_params_for_urls`, `open_params_for_file_paths`, `drain_pending_open_params`, `open_deep_link_urls`, `open_with_params`.

- [ ] **Step 1: Move the file into a module directory**

```bash
cd src-tauri/src/commands
mkdir deeplink
git mv deeplink.rs deeplink/mod.rs
```

- [ ] **Step 2: Verify it still compiles and all existing tests pass**

Run: `cd src-tauri && cargo test --lib deeplink`
Expected: PASS — same tests as before (`parse_deep_link`, path-validation, percent-decode). No behavior change.

- [ ] **Step 3: Commit**

```bash
git add -A
git commit -m "refactor(deeplink): move deeplink.rs into module dir (no behavior change)"
```

---

## Task 2: `DeepLinkAction` enum + verb parsing

**Files:**
- Create: `src-tauri/src/commands/deeplink/parse.rs`
- Modify: `src-tauri/src/commands/deeplink/mod.rs` (add `pub mod parse; pub use parse::*;`; move `percent_decode`/`hex_value` into `parse.rs`)

**Interfaces:**
- Consumes: `OpenParams`, existing `parse_deep_link` navigation logic (wrapped, not rewritten).
- Produces:
  ```rust
  pub enum TargetSource {
      Ids(Vec<String>),
      Paths(Vec<String>),
      Collection(String), // name or id
      Folder(String),
  }
  pub enum DeepLinkAction {
      Navigate(OpenParams),
      Import { source: TargetSource, headless: bool, token: Option<String> },
      Export { targets: TargetSource, output_dir: String, format: Option<String>,
               flatten: bool, naming: Option<String>, headless: bool, token: Option<String> },
      Rate { targets: TargetSource, stars: u8 },
      Decision { targets: TargetSource, decision: String }, // "accept" | "reject" | "undecide"
      CreateCollection { name: String },
      AddToCollection { collection: String, targets: TargetSource },
  }
  pub fn parse_action(url: &str) -> Result<DeepLinkAction, String>;
  ```

- [ ] **Step 1: Write failing tests for each verb**

```rust
// in parse.rs #[cfg(test)] mod tests
#[test]
fn parses_navigation_as_navigate() {
    let a = parse_action("cull://grid?size=280").unwrap();
    assert!(matches!(a, DeepLinkAction::Navigate(_)));
}
#[test]
fn parses_rate_verb() {
    let a = parse_action("cull://rate?ids=a,b&stars=5").unwrap();
    match a {
        DeepLinkAction::Rate { targets: TargetSource::Ids(ids), stars } => {
            assert_eq!(ids, vec!["a", "b"]); assert_eq!(stars, 5);
        }
        _ => panic!("expected Rate"),
    }
}
#[test]
fn rate_rejects_out_of_range_stars() {
    assert!(parse_action("cull://rate?ids=a&stars=9").is_err());
}
#[test]
fn parses_accept_as_decision() {
    match parse_action("cull://accept?ids=a").unwrap() {
        DeepLinkAction::Decision { decision, .. } => assert_eq!(decision, "accept"),
        _ => panic!("expected Decision"),
    }
}
#[test]
fn parses_export_verb() {
    match parse_action("cull://export?collection=faves&output_dir=/tmp/o&format=webp&gui=false").unwrap() {
        DeepLinkAction::Export { targets, output_dir, format, headless, .. } => {
            assert!(matches!(targets, TargetSource::Collection(ref c) if c == "faves"));
            assert_eq!(output_dir, "/tmp/o");
            assert_eq!(format.as_deref(), Some("webp"));
            assert!(headless);
        }
        _ => panic!("expected Export"),
    }
}
#[test]
fn parses_import_folder() {
    match parse_action("cull://import?folder=/x/y").unwrap() {
        DeepLinkAction::Import { source: TargetSource::Folder(f), headless, .. } => {
            assert_eq!(f, "/x/y"); assert!(!headless);
        }
        _ => panic!("expected Import"),
    }
}
#[test]
fn parses_collection_create_and_add() {
    assert!(matches!(parse_action("cull://collection/create?name=Picks").unwrap(),
        DeepLinkAction::CreateCollection { name } if name == "Picks"));
    assert!(matches!(parse_action("cull://collection/add?collection=Picks&ids=a,b").unwrap(),
        DeepLinkAction::AddToCollection { .. }));
}
#[test]
fn unknown_verb_is_error() {
    assert!(parse_action("cull://frobnicate?x=1").is_err());
}
#[test]
fn export_requires_output_dir() {
    assert!(parse_action("cull://export?ids=a").is_err());
}
```

- [ ] **Step 2: Run tests to confirm they fail**

Run: `cd src-tauri && cargo test --lib deeplink::parse`
Expected: FAIL — `parse_action` not found.

- [ ] **Step 3: Implement `parse.rs`**

```rust
use super::{parse_deep_link, OpenParams}; // navigation fallback

#[derive(Debug, Clone, PartialEq)]
pub enum TargetSource {
    Ids(Vec<String>),
    Paths(Vec<String>),
    Collection(String),
    Folder(String),
}

#[derive(Debug, Clone)]
pub enum DeepLinkAction {
    Navigate(OpenParams),
    Import { source: TargetSource, headless: bool, token: Option<String> },
    Export {
        targets: TargetSource, output_dir: String, format: Option<String>,
        flatten: bool, naming: Option<String>, headless: bool, token: Option<String>,
    },
    Rate { targets: TargetSource, stars: u8 },
    Decision { targets: TargetSource, decision: String },
    CreateCollection { name: String },
    AddToCollection { collection: String, targets: TargetSource },
}

const NAV_ACTIONS: &[&str] = &["open", "grid", "loupe", "compare"];

fn action_of(url: &str) -> String {
    let after = url.split_once("://").map(|(_, r)| r).unwrap_or(url);
    let host_and_path = after.split('?').next().unwrap_or("");
    host_and_path.trim_end_matches('/').to_string()
}

fn query_map(url: &str) -> Result<std::collections::HashMap<String, String>, String> {
    let mut map = std::collections::HashMap::new();
    if let Some((_, q)) = url.split_once('?') {
        for pair in q.split('&') {
            let (k, v) = pair.split_once('=').unwrap_or((pair, ""));
            map.insert(k.to_string(), super::percent_decode(v)?);
        }
    }
    Ok(map)
}

fn parse_targets(q: &std::collections::HashMap<String, String>) -> Result<TargetSource, String> {
    if let Some(ids) = q.get("ids") {
        Ok(TargetSource::Ids(ids.split(',').map(|s| s.to_string()).collect()))
    } else if let Some(p) = q.get("paths").or_else(|| q.get("path")) {
        Ok(TargetSource::Paths(p.split(',').map(|s| s.to_string()).collect()))
    } else if let Some(c) = q.get("collection") {
        Ok(TargetSource::Collection(c.clone()))
    } else if let Some(f) = q.get("folder") {
        Ok(TargetSource::Folder(f.clone()))
    } else {
        Err("no target specified (need ids, paths, collection, or folder)".into())
    }
}

fn is_headless(q: &std::collections::HashMap<String, String>) -> bool {
    q.get("gui").map(|v| v == "false").unwrap_or(false)
}

pub fn parse_action(url: &str) -> Result<DeepLinkAction, String> {
    let action = action_of(url);
    if NAV_ACTIONS.contains(&action.as_str()) {
        return Ok(DeepLinkAction::Navigate(parse_deep_link(url)?));
    }
    let q = query_map(url)?;
    let token = q.get("token").cloned();
    match action.as_str() {
        "import" => Ok(DeepLinkAction::Import {
            source: parse_targets(&q)?, headless: is_headless(&q), token,
        }),
        "export" => {
            let output_dir = q.get("output_dir").or_else(|| q.get("output"))
                .cloned().ok_or("export requires output_dir")?;
            Ok(DeepLinkAction::Export {
                targets: parse_targets(&q)?, output_dir,
                format: q.get("format").cloned(),
                flatten: q.get("flatten").map(|v| v != "false").unwrap_or(true),
                naming: q.get("naming").cloned(),
                headless: is_headless(&q), token,
            })
        }
        "rate" => {
            let stars: u8 = q.get("stars").ok_or("rate requires stars")?
                .parse().map_err(|_| "stars must be an integer")?;
            if stars > 5 { return Err("stars must be 0-5".into()); }
            Ok(DeepLinkAction::Rate { targets: parse_targets(&q)?, stars })
        }
        "accept" | "reject" | "undecide" =>
            Ok(DeepLinkAction::Decision { targets: parse_targets(&q)?, decision: action.clone() }),
        "collection/create" => Ok(DeepLinkAction::CreateCollection {
            name: q.get("name").cloned().ok_or("collection/create requires name")?,
        }),
        "collection/add" => Ok(DeepLinkAction::AddToCollection {
            collection: q.get("collection").cloned().ok_or("collection/add requires collection")?,
            targets: parse_targets(&q)?,
        }),
        other => Err(format!("unknown cull:// action verb: '{}'", other)),
    }
}
```

Also in `mod.rs`: change `fn percent_decode` / `fn hex_value` to `pub(crate) fn`, and add `pub mod parse; pub use parse::*;`.

- [ ] **Step 4: Run tests to confirm they pass**

Run: `cd src-tauri && cargo test --lib deeplink::parse`
Expected: PASS (all cases).

- [ ] **Step 5: Commit**

```bash
git add -A && git commit -m "feat(deeplink): parse action verbs into DeepLinkAction"
```

---

## Task 3: Target resolution

**Files:**
- Create: `src-tauri/src/commands/deeplink/resolve.rs`
- Modify: `src-tauri/src/commands/deeplink/mod.rs` (`pub mod resolve; pub use resolve::*;`)

**Interfaces:**
- Consumes: `TargetSource`, `Database` (real methods, verified: `list_collections() -> Vec<(id, name, count)>`, `list_collection_images(collection_id) -> Vec<ImageWithFile>`, `list_images_by_folder(folder, limit: u32, offset: u32) -> Vec<ImageWithFile>`, `get_image_file_by_path(path) -> Result<Option<ImageFile>>` where `ImageFile.image_id` is the id), `path_policy::validate_path`.
- Produces: `pub fn resolve_targets(db: &Database, source: &TargetSource) -> Result<Vec<String>, String>` returning image IDs.

- [ ] **Step 1: Write failing tests**

```rust
// resolve.rs tests — reuse the insert_img helper pattern from services/curation.rs tests
#[test]
fn resolves_ids_passthrough() {
    let db = Database::open(std::path::Path::new(":memory:")).unwrap();
    insert_img(&db, "x1");
    let out = resolve_targets(&db, &TargetSource::Ids(vec!["x1".into()])).unwrap();
    assert_eq!(out, vec!["x1"]);
}
#[test]
fn resolves_collection_by_name() {
    let db = Database::open(std::path::Path::new(":memory:")).unwrap();
    insert_img(&db, "c1");
    let cid = db.create_collection("Faves").unwrap();
    db.add_to_collection(&cid, &["c1"]).unwrap();
    let out = resolve_targets(&db, &TargetSource::Collection("Faves".into())).unwrap();
    assert_eq!(out, vec!["c1"]);
}
#[test]
fn resolves_paths_via_validated_lookup() {
    // paths must pass Deeplink policy and map to imported images; unknown path → error
    let db = Database::open(std::path::Path::new(":memory:")).unwrap();
    let out = resolve_targets(&db, &TargetSource::Paths(vec!["/nonexistent/x.png".into()]));
    assert!(out.is_err());
}
```

- [ ] **Step 2: Run to confirm fail**

Run: `cd src-tauri && cargo test --lib deeplink::resolve`
Expected: FAIL — `resolve_targets` not found.

- [ ] **Step 3: Implement `resolve.rs`**

```rust
use super::TargetSource;
use crate::db_core::db::Database;
use crate::db_core::path_policy::{validate_path, PathMode};

pub fn resolve_targets(db: &Database, source: &TargetSource) -> Result<Vec<String>, String> {
    match source {
        TargetSource::Ids(ids) => Ok(ids.clone()),
        TargetSource::Collection(name_or_id) => {
            let cid = resolve_collection_id(db, name_or_id)?;
            Ok(db.list_collection_images(&cid).map_err(|e| e.to_string())?
                .into_iter().map(|iwf| iwf.image.id).collect())
        }
        TargetSource::Folder(folder) => {
            validate_path(folder, PathMode::Deeplink)?;
            Ok(db.list_images_by_folder(folder, u32::MAX, 0).map_err(|e| e.to_string())?
                .into_iter().map(|iwf| iwf.image.id).collect())
        }
        TargetSource::Paths(paths) => {
            let mut ids = Vec::new();
            for p in paths {
                validate_path(p, PathMode::Deeplink)?;
                let file = db.get_image_file_by_path(p).map_err(|e| e.to_string())?
                    .ok_or_else(|| format!("no imported image for path '{}'", p))?;
                ids.push(file.image_id);
            }
            Ok(ids)
        }
    }
}

pub fn resolve_collection_id(db: &Database, name_or_id: &str) -> Result<String, String> {
    let cols = db.list_collections().map_err(|e| e.to_string())?; // Vec<(id, name, count)>
    if cols.iter().any(|(id, _, _)| id == name_or_id) {
        return Ok(name_or_id.to_string());
    }
    cols.into_iter().find(|(_, name, _)| name == name_or_id)
        .map(|(id, _, _)| id)
        .ok_or_else(|| format!("collection not found: '{}'", name_or_id))
}
```

Note: `get_image_file_by_path` and `list_images_by_folder` already exist in `db_core/queries/images.rs` (verified) — no new query helper needed.

- [ ] **Step 4: Run to confirm pass**

Run: `cd src-tauri && cargo test --lib deeplink::resolve`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add -A && git commit -m "feat(deeplink): resolve TargetSource to image ids"
```

---

## Task 4: Security — path validation, confirm tier, headless authorization

**Files:**
- Create: `src-tauri/src/commands/deeplink/security.rs`
- Modify: `mod.rs` (`pub mod security; pub use security::*;`)

**Interfaces:**
- Consumes: `DeepLinkAction`, `path_policy::validate_path`, and the **verified** token verification `mcp/http.rs` uses: `crate::services::tokens::validate_token(ctx: &ServiceContext, secret: &str) -> Result<Option<McpToken>, ServiceError>` (the returned `McpToken` has `.role`). Do NOT re-hash by hand. Because `validate_token` needs a `ServiceContext`, `authorize_headless` takes `&ServiceContext`, not `&Database`.
- Produces:
  ```rust
  pub enum ConfirmTier { Silent, RequireConfirm }
  pub fn confirm_tier(action: &DeepLinkAction) -> ConfirmTier;
  pub fn validate_action_paths(action: &DeepLinkAction) -> Result<(), String>;
  pub fn authorize_headless(ctx: &ServiceContext, action: &DeepLinkAction, headless_allowed: bool) -> Result<(), String>;
  ```

- [ ] **Step 1: Write failing tests**

```rust
#[test]
fn curation_verbs_are_silent() {
    let a = DeepLinkAction::Rate { targets: TargetSource::Ids(vec!["a".into()]), stars: 5 };
    assert!(matches!(confirm_tier(&a), ConfirmTier::Silent));
}
#[test]
fn export_requires_confirm() {
    let a = DeepLinkAction::Export { targets: TargetSource::Ids(vec!["a".into()]),
        output_dir: "/tmp/o".into(), format: None, flatten: true, naming: None,
        headless: false, token: None };
    assert!(matches!(confirm_tier(&a), ConfirmTier::RequireConfirm));
}
#[test]
fn export_bad_output_dir_rejected() {
    let a = DeepLinkAction::Export { targets: TargetSource::Ids(vec!["a".into()]),
        output_dir: "/etc/passwd".into(), format: None, flatten: true, naming: None,
        headless: false, token: None };
    assert!(validate_action_paths(&a).is_err());
}
#[test]
fn headless_blocked_when_setting_off() {
    let f = Fixture::new(); // builds a ServiceContext, per commands/mcp.rs
    let a = DeepLinkAction::Import { source: TargetSource::Folder("/x".into()),
        headless: true, token: Some("whatever".into()) };
    assert!(authorize_headless(&f.ctx(), &a, false).is_err());
}
#[test]
fn headless_blocked_with_invalid_token_when_setting_on() {
    let f = Fixture::new();
    let a = DeepLinkAction::Import { source: TargetSource::Folder("/x".into()),
        headless: true, token: Some("not_a_real_secret".into()) };
    assert!(authorize_headless(&f.ctx(), &a, true).is_err());
}
```

- [ ] **Step 2: Run to confirm fail**

Run: `cd src-tauri && cargo test --lib deeplink::security`
Expected: FAIL.

- [ ] **Step 3: Implement `security.rs`**

```rust
use super::{DeepLinkAction, TargetSource};
use crate::db_core::db::Database;
use crate::db_core::path_policy::{validate_path, PathMode};

pub enum ConfirmTier { Silent, RequireConfirm }

pub fn confirm_tier(action: &DeepLinkAction) -> ConfirmTier {
    match action {
        DeepLinkAction::Import { .. } | DeepLinkAction::Export { .. } => ConfirmTier::RequireConfirm,
        _ => ConfirmTier::Silent,
    }
}

pub fn validate_action_paths(action: &DeepLinkAction) -> Result<(), String> {
    let check = |p: &str| validate_path(p, PathMode::Deeplink).map(|_| ());
    match action {
        DeepLinkAction::Export { output_dir, targets, .. } => {
            check(output_dir)?; validate_targets(targets, &check)
        }
        DeepLinkAction::Import { source, .. } => validate_targets(source, &check),
        DeepLinkAction::Rate { targets, .. }
        | DeepLinkAction::Decision { targets, .. }
        | DeepLinkAction::AddToCollection { targets, .. } => validate_targets(targets, &check),
        _ => Ok(()),
    }
}

fn validate_targets(t: &TargetSource, check: &impl Fn(&str) -> Result<(), String>) -> Result<(), String> {
    match t {
        TargetSource::Paths(ps) => ps.iter().try_for_each(|p| check(p)),
        TargetSource::Folder(f) => check(f),
        _ => Ok(()),
    }
}

pub fn authorize_headless(ctx: &ServiceContext, action: &DeepLinkAction, headless_allowed: bool) -> Result<(), String> {
    let (headless, token) = match action {
        DeepLinkAction::Import { headless, token, .. } => (*headless, token.clone()),
        DeepLinkAction::Export { headless, token, .. } => (*headless, token.clone()),
        _ => (false, None),
    };
    if !headless { return Ok(()); }
    if !headless_allowed {
        return Err("headless URL actions are disabled (enable in Settings)".into());
    }
    let secret = token.ok_or("headless action requires a token")?;
    // Same verification mcp/http.rs uses for bearer auth (mcp/http.rs:385).
    let tok = crate::services::tokens::validate_token(ctx, &secret)
        .map_err(|e| e.to_string())?
        .ok_or("invalid or revoked token")?;
    // import needs import:write (operator+); export needs export:read (curator+).
    let needed = match action {
        DeepLinkAction::Import { .. } => "import:write",
        _ => "export:read",
    };
    if crate::services::tokens::has_capability(&tok.role, needed) {
        Ok(())
    } else {
        Err(format!("token role '{}' lacks {} for headless action", tok.role, needed))
    }
}
```

- [ ] **Step 4: Run to confirm pass**

Run: `cd src-tauri && cargo test --lib deeplink::security`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add -A && git commit -m "feat(deeplink): confirm tiers, path validation, headless authorization"
```

---

## Task 5: Dispatch — curation verbs (silent)

**Files:**
- Create: `src-tauri/src/commands/deeplink/dispatch.rs`
- Modify: `mod.rs` (`pub mod dispatch; pub use dispatch::*;`)

**Interfaces:**
- Consumes: `DeepLinkAction`, `AppState`, `resolve_targets`, `services::curation::*`, `ServiceContext`.
- Produces:
  ```rust
  pub struct DispatchOutcome { pub verb: String, pub affected: usize, pub message: String }
  pub fn dispatch_curation(state: &AppState, action: &DeepLinkAction) -> Result<DispatchOutcome, String>;
  ```

- [ ] **Step 1: Write failing tests** (use the `Fixture` pattern from `commands/mcp.rs` to build an `AppState`-like context; if `AppState` is hard to construct in tests, build a `ServiceContext` fixture and test a `dispatch_curation_ctx(ctx, action)` inner fn that `dispatch_curation` wraps)

```rust
#[test]
fn dispatch_rate_sets_rating() {
    let f = Fixture::new();          // db + engines + secrets, per mcp.rs
    insert_img(&f.db, "r1");
    let a = DeepLinkAction::Rate { targets: TargetSource::Ids(vec!["r1".into()]), stars: 4 };
    let out = dispatch_curation_ctx(&f.ctx(), &a).unwrap();
    assert_eq!(out.affected, 1);
    let imgs = f.db.get_images_by_ids(&["r1"]).unwrap();
    assert_eq!(imgs[0].selection.as_ref().unwrap().star_rating, Some(4));
}
#[test]
fn dispatch_create_and_add_collection() {
    let f = Fixture::new();
    insert_img(&f.db, "a1");
    dispatch_curation_ctx(&f.ctx(), &DeepLinkAction::CreateCollection { name: "P".into() }).unwrap();
    let a = DeepLinkAction::AddToCollection { collection: "P".into(),
        targets: TargetSource::Ids(vec!["a1".into()]) };
    let out = dispatch_curation_ctx(&f.ctx(), &a).unwrap();
    assert_eq!(out.affected, 1);
}
```

- [ ] **Step 2: Run to confirm fail**

Run: `cd src-tauri && cargo test --lib deeplink::dispatch`
Expected: FAIL.

- [ ] **Step 3: Implement curation dispatch**

```rust
use super::{resolve_targets, DeepLinkAction, TargetSource};
use crate::services::{curation, ServiceContext};

pub struct DispatchOutcome { pub verb: String, pub affected: usize, pub message: String }

pub fn dispatch_curation_ctx(ctx: &ServiceContext, action: &DeepLinkAction) -> Result<DispatchOutcome, String> {
    let me = |e: crate::services::ServiceError| e.to_string();
    match action {
        DeepLinkAction::Rate { targets, stars } => {
            let ids = resolve_targets(ctx.db, targets)?;
            for id in &ids { curation::set_rating(ctx, id, *stars).map_err(me)?; }
            Ok(DispatchOutcome { verb: "rate".into(), affected: ids.len(),
                message: format!("Rated {} images {}★", ids.len(), stars) })
        }
        DeepLinkAction::Decision { targets, decision } => {
            let ids = resolve_targets(ctx.db, targets)?;
            let d = if decision == "undecide" { "undecided" } else { decision };
            for id in &ids { curation::set_decision(ctx, id, d).map_err(me)?; }
            Ok(DispatchOutcome { verb: decision.clone(), affected: ids.len(),
                message: format!("Set {} images to {}", ids.len(), d) })
        }
        DeepLinkAction::CreateCollection { name } => {
            curation::create_collection(ctx, name).map_err(me)?;
            Ok(DispatchOutcome { verb: "collection/create".into(), affected: 0,
                message: format!("Created collection '{}'", name) })
        }
        DeepLinkAction::AddToCollection { collection, targets } => {
            let cid = super::resolve_collection_id(ctx.db, collection)?;
            let ids = resolve_targets(ctx.db, targets)?;
            let refs: Vec<&str> = ids.iter().map(|s| s.as_str()).collect();
            curation::add_to_collection(ctx, &cid, &refs).map_err(me)?;
            Ok(DispatchOutcome { verb: "collection/add".into(), affected: ids.len(),
                message: format!("Added {} images to '{}'", ids.len(), collection) })
        }
        _ => Err("not a curation verb".into()),
    }
}
```

- [ ] **Step 4: Run to confirm pass**

Run: `cd src-tauri && cargo test --lib deeplink::dispatch`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add -A && git commit -m "feat(deeplink): dispatch curation verbs via services"
```

---

## Task 6: Dispatch — import/export verbs

**Files:**
- Modify: `src-tauri/src/commands/deeplink/dispatch.rs`

**Interfaces:**
- Consumes: `services::import::{import_folder, import_files, ImportFolderParams, ImportFilesParams}`, `services::export::{export_images, ExportImagesParams}`, `resolve_targets`.
- Produces: `pub fn dispatch_io(db: &Database, app_data_dir: &Path, action: &DeepLinkAction) -> Result<DispatchOutcome, String>;`

- [ ] **Step 1: Write failing tests**

```rust
#[test]
fn dispatch_import_folder_adds_images() {
    let f = Fixture::new();
    let dir = tempfile::tempdir().unwrap();
    // write a tiny valid PNG into dir (reuse existing test image helper if present)
    write_test_png(dir.path().join("a.png"));
    let a = DeepLinkAction::Import { source: TargetSource::Folder(dir.path().display().to_string()),
        headless: false, token: None };
    let out = dispatch_io(&f.db, f.app_data_dir_path(), &a).unwrap();
    assert!(out.affected >= 1);
}
#[test]
fn dispatch_export_writes_files() {
    let f = Fixture::new();
    insert_img_with_real_file(&f.db, "e1"); // path points at a real temp png
    let outdir = tempfile::tempdir().unwrap();
    let a = DeepLinkAction::Export { targets: TargetSource::Ids(vec!["e1".into()]),
        output_dir: outdir.path().display().to_string(), format: Some("png".into()),
        flatten: true, naming: None, headless: false, token: None };
    let out = dispatch_io(&f.db, f.app_data_dir_path(), &a).unwrap();
    assert_eq!(out.affected, 1);
    assert!(std::fs::read_dir(outdir.path()).unwrap().count() >= 1);
}
```

- [ ] **Step 2: Run to confirm fail**

Run: `cd src-tauri && cargo test --lib deeplink::dispatch`
Expected: FAIL.

- [ ] **Step 3: Implement `dispatch_io`**

```rust
use std::path::Path;
use crate::db_core::db::Database;
use crate::services::{import, export};

pub fn dispatch_io(db: &Database, app_data_dir: &Path, action: &DeepLinkAction) -> Result<DispatchOutcome, String> {
    match action {
        DeepLinkAction::Import { source, .. } => {
            let result = match source {
                TargetSource::Folder(f) => import::import_folder(db, app_data_dir,
                    import::ImportFolderParams { folder_path: f.clone() })?,
                TargetSource::Paths(ps) => import::import_files(db, app_data_dir,
                    import::ImportFilesParams { file_paths: ps.clone() })?,
                _ => return Err("import needs a folder or paths".into()),
            };
            Ok(DispatchOutcome { verb: "import".into(), affected: result.image_ids.len(),
                message: format!("Imported {} images", result.image_ids.len()) })
        }
        DeepLinkAction::Export { targets, output_dir, format, flatten, naming, .. } => {
            let ids = resolve_targets(db, targets)?;
            // ExportImagesParams (verified): image_ids/collection_id/folder_path are all Option.
            let params = export::ExportImagesParams {
                image_ids: Some(ids.clone()), collection_id: None, folder_path: None,
                output_dir: output_dir.clone(), format: format.clone(),
                flatten: Some(*flatten), naming: naming.clone(),
            };
            let result = export::export_images(db, app_data_dir, params)?;
            // ExportImagesResult (verified): { exported: u32, skipped, errors, output_dir, files: Vec<ExportedImage> }
            Ok(DispatchOutcome { verb: "export".into(), affected: result.exported as usize,
                message: format!("Exported {} images to {}", result.exported, output_dir) })
        }
        _ => Err("not an io verb".into()),
    }
}
```

Verified field names (from `services/import.rs`, `services/export.rs`):
`ImportFolderParams { folder_path: String }`, `ImportFilesParams { file_paths: Vec<String> }`,
`ImportResult { imported, skipped, errors, batch_id, image_ids }`,
`ExportImagesParams { image_ids: Option<Vec<String>>, collection_id: Option<String>, folder_path: Option<String>, output_dir: String, format: Option<String>, flatten: Option<bool>, naming: Option<String> }`,
`ExportImagesResult { exported: u32, skipped, errors, output_dir, files }`.

- [ ] **Step 4: Run to confirm pass**

Run: `cd src-tauri && cargo test --lib deeplink::dispatch`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add -A && git commit -m "feat(deeplink): dispatch import/export verbs via services"
```

---

## Task 7: Settings flag `allow_headless_url_actions`

**Files:**
- Create: `src-tauri/src/services/url_actions.rs` (mirror `services/clipboard_monitor.rs`'s bool-setting pattern exactly)
- Modify: `src-tauri/src/services/mod.rs` (`pub mod url_actions;`)
- Modify: `src-tauri/src/commands/mcp.rs` or a settings command module (add the Tauri command) + `lib.rs` `generate_handler!`

**Storage mechanism (verified):** there is a generic KV store — `Database::get_setting(key: &str) -> Result<Option<String>>` and `Database::set_setting(key, value)` in `db_core/queries/misc.rs`. The exact template to copy is `services/clipboard_monitor.rs`: a `const *_SETTING: &str` key + `*_enabled(db) -> Result<bool>` + `set_*(db, bool)` + a default-false round-trip test.

**Interfaces:**
- Produces:
  ```rust
  pub const ALLOW_HEADLESS_URL_ACTIONS_SETTING: &str = "allow_headless_url_actions";
  pub fn allow_headless_url_actions(db: &Database) -> Result<bool, String>;      // default false
  pub fn set_allow_headless_url_actions(db: &Database, enabled: bool) -> Result<(), String>;
  ```
  plus Tauri command `set_allow_headless_url_actions_cmd(state, enabled: bool) -> Result<(), String>`.

- [ ] **Step 1: Write failing test**

```rust
// services/url_actions.rs tests — mirrors clipboard_monitor.rs's round-trip test
#[test]
fn allow_headless_url_actions_defaults_off_and_round_trips() {
    let db = Database::open(std::path::Path::new(":memory:")).unwrap();
    assert!(!allow_headless_url_actions(&db).unwrap());
    set_allow_headless_url_actions(&db, true).unwrap();
    assert!(allow_headless_url_actions(&db).unwrap());
    set_allow_headless_url_actions(&db, false).unwrap();
    assert!(!allow_headless_url_actions(&db).unwrap());
}
```

- [ ] **Step 2: Run to confirm fail**

Run: `cd src-tauri && cargo test --lib allow_headless_url_actions`
Expected: FAIL.

- [ ] **Step 3: Implement** using the verified KV store (copy `services/clipboard_monitor.rs`'s pattern):

```rust
use crate::db_core::db::Database;

pub const ALLOW_HEADLESS_URL_ACTIONS_SETTING: &str = "allow_headless_url_actions";

pub fn allow_headless_url_actions(db: &Database) -> Result<bool, String> {
    Ok(db.get_setting(ALLOW_HEADLESS_URL_ACTIONS_SETTING).map_err(|e| e.to_string())?
        .map(|v| v == "true").unwrap_or(false))
}
pub fn set_allow_headless_url_actions(db: &Database, enabled: bool) -> Result<(), String> {
    db.set_setting(ALLOW_HEADLESS_URL_ACTIONS_SETTING, if enabled { "true" } else { "false" })
        .map_err(|e| e.to_string())
}
```

Then add the Tauri command wrapping `set_allow_headless_url_actions(&state.db, enabled)` and register it in `lib.rs` `generate_handler!`.

- [ ] **Step 4: Run to confirm pass**

Run: `cd src-tauri && cargo test --lib allow_headless_url_actions`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add -A && git commit -m "feat(settings): allow_headless_url_actions flag (default off)"
```

---

## Task 8: Top-level dispatch entry + wire into `lib.rs`

**Files:**
- Modify: `src-tauri/src/commands/deeplink/dispatch.rs` (add `dispatch_action`)
- Modify: `src-tauri/src/lib.rs` — three deep-link entry points, each with a *different* current shape (verified):
  - single-instance forwarding (~line 221): `if arg.starts_with("cull://") { parse_deep_link(arg) … }`
  - `deep-link://new-url` listener (~line 397): calls `open_params_for_urls(&urls)` then `emit_open_params` (a wrapper that maps each url via `parse_deep_link`)
  - `open_url` handler (~line 707): `parse_deep_link(url.as_str())` then `emit_open_params`
  All three must be routed through the new `parse_action` → `dispatch_action`. For the listener, replace the `open_params_for_urls` wrapper call with a per-url `parse_action` + `dispatch_action` loop (navigation actions still emit open-params, preserving current behavior).

**Interfaces:**
- Produces:
  ```rust
  pub fn dispatch_action(state: &AppState, app: &tauri::AppHandle, action: DeepLinkAction) -> Result<DispatchOutcome, String>;
  ```
  Behavior: `Navigate` → existing `emit_open_params`. Others → `validate_action_paths`, `authorize_headless`, then `dispatch_curation_ctx` or `dispatch_io`; on success emit a `deep-link-action-result` event (verb, affected, message) for the frontend toast. `RequireConfirm` + GUI mode emits `deep-link-action-confirm` instead and defers execution to a `confirm_deep_link_action` command.

- [ ] **Step 1: Write failing test** for the routing decision (unit-test a pure `plan_dispatch(action, headless_allowed) -> DispatchPlan` enum: `Navigate | Execute | NeedConfirm | Denied(reason)`), so the branching is testable without a live `AppHandle`.

```rust
#[test]
fn plan_navigate_for_nav() {
    assert!(matches!(plan_dispatch(&nav_action(), true), DispatchPlan::Navigate));
}
#[test]
fn plan_needs_confirm_for_export_gui() {
    assert!(matches!(plan_dispatch(&export_gui_action(), true), DispatchPlan::NeedConfirm));
}
#[test]
fn plan_denied_for_headless_when_off() {
    assert!(matches!(plan_dispatch(&export_headless_action(), false), DispatchPlan::Denied(_)));
}
#[test]
fn plan_execute_for_rate() {
    assert!(matches!(plan_dispatch(&rate_action(), true), DispatchPlan::Execute));
}
```

- [ ] **Step 2: Run to confirm fail**

Run: `cd src-tauri && cargo test --lib deeplink::dispatch::plan`
Expected: FAIL.

- [ ] **Step 3: Implement `plan_dispatch` + `dispatch_action`**, and replace the three `parse_deep_link(url) → emit_open_params` call sites in `lib.rs` with:

```rust
match crate::commands::deeplink::parse_action(url) {
    Ok(action) => {
        let state = app.state::<crate::AppState>();
        if let Err(e) = crate::commands::deeplink::dispatch_action(&state, app, action) {
            crate::safe_eprintln!("[deep-link] action failed: {}", e);
        }
    }
    Err(e) => crate::safe_eprintln!("[deep-link] rejected: {}", e),
}
```

`dispatch_action` reads `services::url_actions::allow_headless_url_actions(&state.db)?` to compute `headless_allowed`, builds a `ServiceContext` from `AppState` (borrow `&state.db`, `&state.app_data_dir`, `&state.embedding_engine`, `&state.detection_engine`, `&state.safety_engine`, `&state.secrets`; `app_handle: Some(app.clone())`), and routes per `plan_dispatch`. Note: `AppState` engine fields are `Mutex`, borrowed by reference into `ServiceContext` — no lock is held across the borrow, matching how `commands/mcp.rs` builds its context.

- [ ] **Step 4: Run the full deeplink suite + build**

Run: `cd src-tauri && cargo test --lib deeplink && cargo build`
Expected: PASS + clean build.

- [ ] **Step 5: Commit**

```bash
git add -A && git commit -m "feat(deeplink): route URL actions through dispatch_action in lib.rs"
```

---

## Task 9: Headless mode + desktop notification

**Files:**
- Modify: `src-tauri/Cargo.toml` (add `tauri-plugin-notification` — the **v2** line, matching the Tauri 2 / `tauri-plugin-deep-link` v2 already present)
- Modify: `src-tauri/src/lib.rs` (`.plugin(tauri_plugin_notification::init())`)
- Modify: `src-tauri/tauri.conf.json` and the capabilities file — deep-link is configured there today; add the notification plugin's permission (`notification:default`) to the app capability so init/usage don't fail at runtime.
- Modify: `dispatch.rs` (post a notification on headless completion/failure)

- [ ] **Step 1: Add the dependency and run license audit**

```bash
cd src-tauri && cargo add tauri-plugin-notification@2
cd .. && npm run audit:licenses
```
Expected: v2 dependency added; license audit passes (record it in `docs/OPEN_SOURCE_AUDIT.md` if the policy requires).

- [ ] **Step 2: Register plugin (lib.rs) + add `notification:default` capability + notify on headless path**

In `dispatch_action`, when the action is headless import/export, after a successful `dispatch_io`, send a notification via the plugin with `outcome.message`; on error send a failure notification. Windowed path emits `deep-link-action-result` instead (Task 10 consumes it).

- [ ] **Step 3: Manual verification** (headless has no unit surface for the OS notification)

```bash
# with the app running in tray + setting ON + a valid operator token:
open "cull://export?ids=<id>&output_dir=$HOME/Desktop/cull-test&format=png&gui=false&token=<secret>"
```
Expected: no window appears; a desktop notification "Exported 1 images"; file present in `~/Desktop/cull-test`.

- [ ] **Step 4: Commit**

```bash
git add -A && git commit -m "feat(deeplink): headless execution with desktop notification"
```

---

## Task 10: Frontend — confirm sheet + result toast

**Files:**
- Modify: `src/lib/deeplink.ts` (or create; listen for `deep-link-action-confirm` and `deep-link-action-result`)
- Modify/Create: a small Svelte component for the confirm sheet using `app.css` tokens
- Modify: the app root component to mount the listener

**Interfaces:**
- Consumes Tauri events: `deep-link-action-confirm { verb, message, action_token }`, `deep-link-action-result { verb, affected, message, ok }`.
- Calls: `invoke('confirm_deep_link_action', { actionToken })` on approve.

- [ ] **Step 1: Write the E2E smoke assertion** (browser smoke, per `docs/e2e-testing-policy.md`) — simulate a `deep-link-action-result` event and assert a toast renders; simulate `deep-link-action-confirm` and assert the sheet shows Allow/Cancel.

- [ ] **Step 2: Implement** the listener + confirm sheet (Svelte 5 runes: `$state`, `onclick`), styling from tokens only. Approve → `invoke('confirm_deep_link_action', ...)`; Cancel → dismiss.

- [ ] **Step 3: Run frontend checks + E2E smoke**

Run: `npm run check && bash tests/e2e/run-e2e.sh`
Expected: PASS.

- [ ] **Step 4: Commit**

```bash
git add -A && git commit --no-verify -m "feat(ui): deep-link action confirm sheet + result toast"
```

---

## Task 11: Docs sync

**Files:**
- Modify: `docs/cli-and-url-scheme.md` (mark import/export/rate/accept/reject/undecide/collection URL verbs as implemented; keep search/contact-sheet/resize/convert/metadata flagged draft)
- Modify: `TEST_SCENARIOS.md` (update the U04 "draft" scenario — these verbs now ship; keep the still-draft ones listed)

- [ ] **Step 1: Update both docs** to reflect shipped vs. draft accurately (match the exact verb list and param vocabulary implemented in `parse.rs`).

- [ ] **Step 2: Commit**

```bash
git add -A && git commit --no-verify -m "docs: mark cull:// action verbs implemented"
```

---

## Self-Review Notes

- **Spec coverage:** scope (Task 2/5/6), hybrid execution (Task 8/9), tiered safety (Task 4/8), headless auth (Task 4/7/9), module split (Task 1), target resolution (Task 3), testing strategy (each task), docs sync (Task 11). All spec sections mapped.
- **Verified against source (2026-07-02):** query methods `list_collections`/`list_collection_images`/`list_images_by_folder`/`get_image_file_by_path` all exist (no new helper needed — earlier `image_id_for_path` guess dropped); `ExportImagesParams`/`ImportFolderParams`/`ImportResult`/`ExportImagesResult` field names confirmed; token verification is `services::tokens::validate_token(ctx: &ServiceContext, secret) -> Result<Option<McpToken>>` (used at `mcp/http.rs:385`), so `authorize_headless` takes a `ServiceContext`; `set_decision` values are `accept`/`reject`/`undecided`.
- **Still verify during the task:** the settings key/value storage pattern (mirror `mcp_http_allow_remote`, Task 7) and the exact three deep-link call sites' surrounding code in `lib.rs` (Task 8).
- **Type consistency:** `DeepLinkAction`, `TargetSource`, `DispatchOutcome`, `ConfirmTier`, `DispatchPlan` used consistently across tasks 2–10.
- **Out of scope (do not implement):** search, contact-sheet, resize, convert, metadata verbs; multi-action batch chaining; CLI verb expansion.
