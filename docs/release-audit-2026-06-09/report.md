# Cull Release Audit — Stage 1 Report

**Date:** 2026-06-09
**Inputs:** `inventory.json` (92 items), `findings.json` (42 findings), 3-advocate identity panel + judge, 92-item triage, Track C plugin spec.
**Companion artifact:** `decision-sheet.md` (machine-parseable taste calls awaiting user sign-off).

---

## 0. Must-resolve before flip (the hard-blocking list)

Exactly one finding hard-blocks the release definition (public repo + installable
DMG + soft launch); everything else is pre-launch or post-launch:

1. **HYG-001** — GitHub Actions billing failure: CI and the signed Release
   pipeline have never produced an artifact. Until resolved, no signed DMG can
   exist. (Runtime-verified via `gh`; note this proof depends on live GitHub
   state, not the repo snapshot.)

Pre-launch gates promoted from the completeness notes (must be closed or
explicitly waived in the decision sheet before the repo flips public):

- **Content-sensitivity pass over `docs/cull-audit-2026-06-03.md`** — excluded
  from this audit's inputs by the fresh-eyes rule, so its publishability is
  unassessed (see HYG-004).
- **PERF-07 partial measurement** — **CLOSED 2026-06-10** (bd `imageview-dkz.31`),
  both formerly-unmeasured thresholds now measured:
  - *Thumbnail load p95 < 200ms:* **PASS at 10k** for the synchronous view
    pipeline. New vitest scenario (`npm run test:perf`, view-utils.test.ts
    "10k-library per-thumbnail load-path") drives the real Grid/Thumbnail
    pipeline (scroll direction → overscan → visible window → per-tile
    base+variant path resolution → a11y label → prefetch warming) across a
    synthesized 10k-image library, all 10,000 items rendered. Measured over 3
    runs: per-thumbnail p50 0.0006–0.0014ms / p95 0.0011–0.0041ms; per
    scroll-batch (whole visible window) p50 0.30–0.67ms / p95 0.54–1.95ms —
    worst observed p95 is ~100x under budget even at whole-batch granularity.
    Caveat: the harness cannot observe the webview's asset fetch + JPEG decode
    of the pre-generated thumbnail files; that cost is per-file and independent
    of library size, and real-app browsing (below) showed no thumbnail stalls.
    The assertion stays in the suite as a regression guard.
  - *Resident memory < 1.5GB after browsing the full library:* **PASS,
    measured at 2,687 images** (real library size found via
    `get_library_stats`: 2,687 images / 180 folders / 19 collections — NOT
    10k; no 10k measurement is claimed). MCP-driven browse of the running
    production app (~35 grid views via navigate_to_folder/show_collection,
    including the 1,335-image folder, plus collection views): baseline GUI RSS
    27.6MB (phys_footprint 108MB) → peak 1.17GB RSS mid-browse → 0.80GB
    resident immediately after browsing (GUI 628MB + WebContent webview
    176MB) → 0.67GB at +60s → 0.17GB at +4min. Worst resident reading 1.17GB
    < 1.5GB. Honest caveats for 10k: (a) extrapolation — resident peak scales
    with browse rate/views, not library size alone; macOS compression kept
    residency falling while footprint plateaued, so a linear 10k-views bound
    is not meaningful, but the threshold is only *measured* at 2,687; (b)
    `phys_footprint` (dirty incl. compressed) grew 108MB → 1.8GB peak and
    settled at 1.66GB (≈1.5GB dirty MALLOC_LARGE+SMALL in the Rust process)
    without returning to baseline — resident memory passes, but the
    footprint-retention pattern is worth a post-launch leak look (file with
    CQ-4).
- **Identity-panel verification asymmetry** — the judge spot-verified only
  Advocate B's citations; Advocates A and C's claims were not independently
  re-verified against the repo. **WAIVED 2026-06-10:** the user overrode the
  identity to "agent-native image tool" as a taste call at the decision gate
  (decision-sheet row `identity`); the chosen identity no longer rests on the
  advocate evidence, so re-verification would not change any decision.

## 1. Verified Findings by Severity

Severity tiers: **release-blocker** (release definition unreachable until fixed) → **pre-launch** (fix before flipping the repo public / shipping the DMG) → **post-launch** (file and fix after the soft launch). Each finding lists its evidence and a refutation note recording whether the finding was challenged and survived.

### 1.1 Release Blockers

#### HYG-001 — GitHub Actions billing failure: CI and signed Release pipeline have never produced an artifact
- **Lens:** release-hygiene · **Effort:** S · **Runtime-verified:** yes
- **Evidence:**
  - `gh run list --workflow=release.yml`: v0.2.1 run 26952066452 and v0.2.0 run 26893830563 both 'completed failure' in ~5s
  - `gh run view 26952066452`: 'The job was not started because recent account payments have failed or your spending limit needs to be increased' (both macOS matrix jobs)
  - `gh run list --workflow=ci.yml`: latest main push run 27236486149 (2026-06-09) failed the same way — ubuntu and macos jobs never started
  - `gh release list`: empty — git tags v0.2.0 and v0.2.1 exist locally but no GitHub release or DMG exists
  - `.github/workflows/release.yml:46-91` has correct Developer ID + notarization env wiring (APPLE_CERTIFICATE, APPLE_ID, APPLE_TEAM_ID, TAURI_SIGNING_PRIVATE_KEY) with non-empty secret assertions, so config is ready once runners start
- **Proposed fix:** Fix GitHub billing/spending limit, then re-run the v0.2.1 Release workflow (workflow_dispatch or re-tag), confirm green CI on main, and verify the produced DMG with `codesign --verify --deep --strict`, `spctl --assess`, and `xcrun stapler validate`. Publish the GitHub release with notes from CHANGELOG.md (current to 0.2.1). The release definition (installable signed macOS app + public repo with green CI) is unreachable until this is fixed.
- **Refutation notes:** Refutation attempted and recorded as null — no counter-evidence found. The workflow definition itself was checked and is correct; the failure is purely account billing, confirmed live via `gh` (runtime-verified). The finding stands.

### 1.2 Pre-Launch

#### CQ-1 — NaN-unsafe sort in detection NMS panics and permanently poisons the ONNX session mutex
- **Lens:** code-quality/rust · **Effort:** S · **Runtime-verified:** yes
- **Evidence:**
  - src-tauri/src/db_core/detection.rs:295 — `indices.sort_by(|&a, &b| confidences[b].partial_cmp(&confidences[a]).unwrap());` panics if any model confidence is NaN
  - src-tauri/src/db_core/detection.rs:160 — `let mut session = session_mutex.lock().unwrap();` on a std::sync::Mutex (detection.rs:6); nms() at :267 runs while this lock is held, so a panic poisons it and every subsequent detect() panics on the unwrap
  - src-tauri/src/commands/detection.rs:70 and :125 — detect() called from `detect_objects`/`detect_nsfw` Tauri commands; a panic there strands the invoke promise
  - Every other float sort in the codebase guards this: perceptual_hash.rs:81, db.rs:1878, color.rs:210 all use `unwrap_or(Ordering::Equal)` — detection.rs:295 is the lone outlier
  - AGENTS.md explicitly allows user-supplied local ONNX weights, which makes NaN outputs a realistic input
  - Runtime repro: standalone rustc program sorting `[NaN, 1.0, 0.5]` with the identical comparator panicked (`called Option::unwrap() on a None value`, exit 101)
- **Proposed fix:** Replace with `confidences[b].total_cmp(&confidences[a])` (or `partial_cmp(...).unwrap_or(Ordering::Equal)` to match sibling sites), and optionally filter non-finite confidences before NMS.
- **Refutation notes:** Challenged on "is NaN realistic?" — refuted the refutation: user-supplied ONNX weights are an explicitly supported input per AGENTS.md, and the panic was reproduced at runtime with the identical comparator. Stands.

#### CQ-2 — Trash/permanent-delete swallow per-file failures end-to-end; UI can claim success while files remain on disk
- **Lens:** code-quality/ipc-error-handling · **Effort:** M · **Runtime-verified:** no (code-traced end-to-end)
- **Evidence:**
  - src-tauri/src/commands/library.rs:135-137 — `trash_images` logs trash::delete failures with eprintln only and returns a success count; no error detail reaches the caller
  - src-tauri/src/commands/library.rs:119 and :125 — `let _ = state.db.mark_file_missing(...)` and `let _ = state.action_manager.record_action(...)` discard DB/undo-log errors after a successful trash
  - src-tauri/src/commands/library.rs:159-161 — `delete_images_permanently` uses `std::fs::remove_file(path).is_ok()` with no logging at all; failures are completely invisible
  - src/lib/menu.ts:366 — `await trashImages(ids);` ignores the returned count, then `reloadAfterImageRemoval(ids)` removes ALL requested ids from the UI even if the backend trashed only some
  - src/lib/components/ContextMenu.svelte:312 — multi-image trash also discards the count and has no try/catch, so an invoke rejection becomes an unhandled promise rejection
  - src/routes/+page.svelte:93-100 and :136-142 — `executeTrash`/`handlePermanentDelete` show no feedback at all when count is 0 and have no try/catch around the invoke
- **Proposed fix:** Return per-id results ({succeeded, failed: [(id, error)]}); surface partial failures via showToast (toast infra at src/lib/stores.ts:342 already supports detail text); only remove successfully-trashed ids from the images store; wrap all call sites in try/catch like menu.ts:365-371 already does.
- **Refutation notes:** Not runtime-reproduced (would require inducing a trash failure, e.g. permissions); the swallow path is, however, traced across all three layers (Rust command → API wrapper → UI call sites) with no error channel anywhere. Treated as verified-by-trace; the worst defect class for a product that manages people's files.

#### SEC-001 — MCP export_images writes to arbitrary output_dir with no path confinement
- **Lens:** security · **Effort:** S · **Runtime-verified:** no
- **Evidence:**
  - src-tauri/src/services/export.rs:60-67 — output_dir taken verbatim from params and created with fs::create_dir_all, no home/temp confinement and no '..' rejection
  - src-tauri/src/mcp/tools.rs:2633-2692 — export_images scope-checks the SOURCE images but never the destination; reachable by any curator/operator/admin token (capability export:read) over the MCP HTTP server
  - src-tauri/src/commands/static_publishing.rs:816-867 — resolve_export_root shows the project's own confinement pattern (reject '..', canonicalize, require under $HOME or temp) that export_images does not apply
  - SECURITY.md:48-49 claims 'Export paths are validated against the user's home directory using the same rules as deep links' — true for static publishing, false for export_images
- **Proposed fix:** Route export_images output_dir through the same confinement as resolve_export_root (or path_policy::validate_path); add unit tests mirroring static_publishing's. Damage is bounded (image-format content, sanitized filenames per export.rs:321-332) and HTTP MCP is off by default — hence pre-launch, not blocker.
- **Refutation notes:** Severity challenged downward and the downgrade accepted into the finding itself: bounded damage + HTTP-off-by-default keep this out of the blocker tier. The gap itself (and the contradiction with SECURITY.md's claim) is uncontested. Not exploited live.

#### SEC-002 — CSP connect-src whitelists three AI provider hosts the frontend never calls (ready-made exfiltration channel)
- **Lens:** security · **Effort:** S · **Runtime-verified:** no
- **Evidence:**
  - src-tauri/tauri.conf.json:27 — connect-src includes https://generativelanguage.googleapis.com, https://api.openai.com, https://openrouter.ai
  - All provider traffic is backend-side Rust reqwest: src-tauri/src/commands/embeddings.rs:11,885-903, src-tauri/src/db_core/gemini.rs:28; grep of src/ shows no fetch/XHR to these hosts (only UI labels in McpSettings.svelte:46-48)
  - src-tauri/Cargo.toml:31-50 — no tauri-plugin-http, so these CSP entries serve no IPC purpose; any renderer XSS could POST library data or pasted-key input to openrouter.ai/api.openai.com unimpeded
- **Proposed fix:** Remove the three external hosts from connect-src, leaving `'self' ipc: http://ipc.localhost`. Run the E2E smoke suite + a manual embeddings/validate-key flow to confirm nothing in the webview depended on them.
- **Refutation notes:** Refutation attempted via grep for any frontend consumer of these hosts — none found, which is itself the evidence. Requires the post-removal smoke run as final confirmation (scheduled into the fix), since absence-of-grep-hit is not proof of absence at runtime.

#### SEC-003 — Developer-personal directory $HOME/.codex/generated_images baked into shipped asset-protocol scope
- **Lens:** security · **Effort:** S · **Runtime-verified:** no
- **Evidence:**
  - src-tauri/tauri.conf.json:41 — assetProtocol scope ships `$HOME/.codex/generated_images/**/*` to every user install
  - SECURITY.md:50-56 documents it as one of only three asset-scope directories, so the posture doc canonizes a personal workflow path
  - Unlike $APPDATA/thumbnails and $APPDATA/generated, this grants the webview read access to a third-party tool's output directory that Cull does not own
- **Proposed fix:** Remove the hardcoded .codex scope from tauri.conf.json (and SECURITY.md); if the codex-import integration is wanted, surface it via the import/thumbnail pipeline or a user-configured watched folder.
- **Refutation notes:** Counter-evidence considered: src/lib/asset-protocol-config.test.ts:27 pins the path, proving it is deliberate, not accidental (see HYG-005). Deliberateness does not refute the finding — it converts it from "bug" to "ship/trim decision", recorded in the decision sheet.

#### HYG-002 — Copyright-year drift: uncommitted NOTICE edit, About dialog still says 2025-present
- **Lens:** release-hygiene · **Effort:** S · **Runtime-verified:** yes
- **Evidence:**
  - git status: NOTICE modified, uncommitted; git diff NOTICE: 'Copyright 2025-present' -> 'Copyright 2026-present'
  - src/lib/components/AboutDialog.svelte:125 hardcodes '(c) 2025-present Gleb Kalinin'
  - git log --reverse: first commit 9361d6129 dated 2026-05-07, so 2026 is the correct first year
  - src/lib/open-source-release-contract.test.ts:36-46 checks NOTICE content but not year alignment with the About dialog, so this drift passes the contract test (vitest run: 5/5 pass)
- **Proposed fix:** Update AboutDialog.svelte:125 to '(c) 2026-present', commit with the pending NOTICE change, and extend the contract test to assert year alignment between NOTICE and AboutDialog.svelte.
- **Refutation notes:** None — the contract test was run (5/5 pass) specifically to confirm the drift escapes the existing gate. Stands.

#### HYG-003 — npm run audit:supply-chain cannot pass: required tooling absent, no deny.toml, not wired into CI
- **Lens:** release-hygiene · **Effort:** M · **Runtime-verified:** yes
- **Evidence:**
  - `npm run audit:supply-chain` exits 1: 'missing required tool: cargo-deny' (cargo-audit was also missing until installed during this audit)
  - scripts/supply-chain-audit.sh:21-28 runs 'cargo deny check advisories licenses bans sources' but no deny.toml exists anywhere in the repo, so even with cargo-deny installed the check has no committed policy
  - Neither ci.yml nor release.yml runs any supply-chain audit (grep: no matches)
  - Manual results during this audit: npm audit = 0 vulnerabilities (prod and dev); cargo audit (v0.22.2, 678 crates) = 0 vulnerabilities, 17 warnings — unmaintained gtk3-stack/unic crates and glib 0.18.5 unsoundness RUSTSEC-2024-0429, all Linux-only Tauri transitive deps
  - docs/OPEN_SOURCE_AUDIT.md:63-66 documents cargo-deny and CycloneDX SBOM as part of the release posture, so docs promise more than the repo can execute
- **Proposed fix:** Commit src-tauri/deny.toml encoding the documented license policy, add a CI/release-gate step running scripts/supply-chain-audit.sh, record the Linux-only cargo-audit warnings as accepted ignores with rationale.
- **Refutation notes:** Partially self-refuting in the good direction: the underlying dependency health is clean (0 vulns both ecosystems, measured live). The finding is about the *gate*, not the deps — and the gate's failure was reproduced on this machine. Stands.

#### HYG-004 — Repo-going-public content review: tracked bd issue database, internal audit docs, and absolute personal paths ship publicly
- **Lens:** release-hygiene · **Effort:** M · **Runtime-verified:** yes
- **Evidence:**
  - .beads/issues.jsonl (238 lines) and .beads/interactions.jsonl (112 lines) are git-tracked despite .gitignore:40 '.beads/'; they contain the full internal issue history including security-issue descriptions and owner email glebis@gmail.com on every record
  - docs/cull-audit-2026-06-03.md — a full external security/UX audit — is tracked and will ship with the public repo (presence noted only; not read per fresh-eyes rule)
  - docs/superpowers/plans/2026-06-03-release-skill.md:497,503,550,641 and docs/superpowers/specs/2026-05-30-clipboard-monitor-design.md:69 contain absolute personal paths ($HOME/...)
  - AGENTS.md:148-159 ships personal machine references (Obsidian vault ~/Brains/brain/, ~/.Codex/refs/* email/telegram rule files)
  - docs/ also carries internal working artifacts a stranger gains nothing from: 2026-05-10-vision-brainstorm-raw.md, dev-workflow-audit-2026-06-02.md, tooling-research-2026-06-03.md, settings-mockup-*.json, oss-strategy-explorer.html
- **Proposed fix:** Make an explicit ship/trim decision before flipping the repo public: accept the bd jsonl files as transparency (scrubbing closed security-issue detail + per-record email) or stop tracking them; archive or path-rewrite personal-path-bearing plans/specs and internal audit docs; trim AGENTS.md personal references.
- **Refutation notes:** One scope narrowing applied deliberately: docs/cull-audit-2026-06-03.md was *not read* (fresh-eyes rule for this audit), so its content sensitivity is unassessed — only its tracked presence is asserted. Everything else verified via git ls-files. Stands as a decision item, not a defect.

#### HYG-005 — Personal Codex workflow path ($HOME/.codex/generated_images) baked into every user's asset-protocol scope
- **Lens:** release-hygiene · **Effort:** S · **Runtime-verified:** no
- **Evidence:**
  - src-tauri/tauri.conf.json:41 assetProtocol allow includes '$HOME/.codex/generated_images/**/*' alongside the two $APPDATA scopes
  - src/lib/view-utils.ts:126 hardcodes CODEX_GENERATED_IMAGES_SEGMENT = '/.codex/generated_images/'
  - SECURITY.md:48-52 documents this directory as one of only three asset-protocol scopes
  - src/lib/asset-protocol-config.test.ts:27 pins the path in a contract test, so it is deliberate, not accidental
- **Proposed fix:** Decide whether the Codex generated-images integration is a product feature for strangers; if yes, document it as a first-class integration; if not, remove it from the default asset scope (and the contract test) or make it an opt-in configurable library root, then update SECURITY.md.
- **Refutation notes:** Same deliberateness counter-argument as SEC-003 — accepted and folded in. The hygiene angle (hidden hardcoded scope in a public security model) stands regardless of which way the product decision goes. Static-traced only.

#### HYG-006 — README and SECURITY.md version staleness: pinned to v0.1.0 / 0.1.x while shipping 0.2.1, and no install path for non-developers
- **Lens:** release-hygiene · **Effort:** S · **Runtime-verified:** yes
- **Evidence:**
  - README.md:30 '## Current Status (v0.1.0)' while package.json:3, src-tauri/Cargo.toml:3, and src-tauri/tauri.conf.json all say 0.2.1
  - SECURITY.md:55-59 Supported Versions table lists only '0.1.x | Yes'
  - README.md:222-229 Quick Start is developer-only (git clone + npm run tauri dev); no download/install section for a stranger, even though the release definition is an installable signed macOS app
  - README otherwise adequate for a stranger: clear pitch, docs index, keyboard reference, pinned toolchain, license section
- **Proposed fix:** Bump the README status header (or drop the version from the heading), update SECURITY.md to 0.2.x, add a Download/Install section once HYG-001 is fixed; consider asserting README/SECURITY version freshness in the release skill's versionFiles gate.
- **Refutation notes:** None — three-way version comparison performed across manifests. Stands.

#### UX-01 — Global Tab hijack makes every control unreachable by keyboard (no focus order at all)
- **Lens:** ux · **Effort:** M · **Runtime-verified:** yes
- **Evidence:**
  - src/lib/keys.ts:338-342 — `if (e.key === 'Tab' ...) { e.preventDefault(); cycleViewMode(...) }` runs for all non-editable targets, so Tab never moves focus
  - Runtime: with focus on BODY, pressing real Tab twice cycled Grid→Loupe→Compare while document.activeElement stayed BODY both times
  - src/app.css:85-87 defines a :focus-visible outline that keyboard users can never reach for buttons like '+ Import Folder' (Sidebar.svelte:736)
  - Spec scope explicitly includes 'focus order through import → grid → loupe → export' — there is no traversable focus order anywhere
- **Proposed fix:** Move view-cycling off bare Tab (keep Ctrl+Tab / [ ] / existing Cmd+1-7) or only intercept Tab when document.activeElement is body, restoring native focus traversal.
- **Refutation notes:** None. Reproduced live with real key events. Note from the identity panel: README already promises "keyboard-first", so this contradicts shipped copy under *any* identity — fix is identity-neutral and mandatory.

#### UX-02 — Backend/init failure is silently rendered as a healthy empty library
- **Lens:** ux · **Effort:** S · **Runtime-verified:** yes
- **Evidence:**
  - src/routes/+page.svelte:359 — `init().catch(e => console.error(...))` with no UI surface
  - src/lib/components/Sidebar.svelte:34-43 — listFolders/listCollections failures are console.error only
  - Runtime: with all invokes failing (browser tab on :1420), the UI showed 'All Images (0)' and the normal 'No images loaded' empty state with zero error indication — identical to a genuinely empty library
  - A user with a corrupt/locked cull.db would conclude their library is gone and possibly re-import or churn
- **Proposed fix:** Show an error state distinct from the empty state (banner/toast with retry + DB path), and gate the 'No images loaded' copy on a successful first query.
- **Refutation notes:** Refutation considered: the runtime repro used a browser tab where *all* invokes fail, harsher than a real partial failure. Accepted as representative anyway — the code path has no error surface for any failure mode. Stands.

#### UX-03 — First-run sidebar leads with AI-model jargon and a dead-end 'Install model manually' instruction
- **Lens:** ux · **Effort:** S · **Runtime-verified:** yes
- **Evidence:**
  - Runtime (first-run, 0 images): sidebar prominently shows 'AI MODELS / YOLO manual install / nano 6MB…medium 50MB / Install model manually / NudeNet manual install / Ollama offline' before the user has imported anything
  - src/lib/components/Sidebar.svelte:551,562,571,576 — 'manual install' / 'Install model manually' have no link, no target path, no docs reference; nothing is clickable
  - src/lib/components/Sidebar.svelte:338 — `aiExpanded = $state(true)` so the section is expanded by default on first run
- **Proposed fix:** Collapse AI MODELS by default until the library has images; replace the dead-end text with a help affordance; soften 'offline' for Ollama to optional-integration framing.
- **Refutation notes:** None. Observed on a genuine first-run state. Stands.

#### UX-04 — Empty-library state is not actionable and undersells import paths
- **Lens:** ux · **Effort:** S · **Runtime-verified:** yes
- **Evidence:**
  - src/lib/components/Grid.svelte:197-198 — 'No images loaded / Use the sidebar to import a folder' with no button
  - Runtime: the referenced '+ Import Folder' button (Sidebar.svelte:736-737) sits at the very bottom of the sidebar, ~700px below the section the eye scans first, below AI MODELS/CLIPBOARD MONITOR noise
  - Drag-and-drop import exists (+page.svelte:510-511 'Drop to import') but the empty state never mentions it
- **Proposed fix:** Make the empty state the onboarding: Import Folder button in the empty-state body plus a drop-anywhere hint; consider moving Import Folder to the top of the sidebar.
- **Refutation notes:** None. Stands.

#### UX-05 — Toasts carry all success/error feedback but have no aria-live/status role
- **Lens:** ux · **Effort:** S · **Runtime-verified:** no (static grep; not screen-reader-tested)
- **Evidence:**
  - src/lib/components/Toast.svelte — grep for aria-live/role returns nothing (exit 1), while Canvas/Grid/Export/Sidebar all set aria-live on their regions
  - Import success/failure, trash, permanent delete, and collection errors are reported exclusively via showToast
  - Screen-reader users get no notification of import results or destructive-action outcomes
- **Proposed fix:** Add `role="status" aria-live="polite"` (and `aria-live="assertive"` for error type) to the toast container.
- **Refutation notes:** Not verified with an actual screen reader — the inference rests on missing ARIA attributes plus the codebase's own convention elsewhere. Low refutation risk; flagged in Completeness.

#### UX-06 — Search bar has two adjacent unlabeled '×' buttons with different behaviors (clear vs close)
- **Lens:** ux · **Effort:** S · **Runtime-verified:** yes
- **Evidence:**
  - Runtime aria scan of the open command bar: two ×s with no aria-label and no title (alongside labeled dictation buttons)
  - src/lib/components/CommandBar.svelte:464 (.clear-btn '×') and :466 (.close-btn '×'); also :430 (.pill-close '×' on filter chips, unlabeled)
  - Visually identical side-by-side ×s force trial-and-error even for sighted users
- **Proposed fix:** aria-label + title ('Clear query' / 'Close search' / 'Remove filter'), and visually differentiate clear from close.
- **Refutation notes:** None. Stands.

#### UX-07 — Shortcut help and the command palette are themselves undiscoverable (no '?' key, Cmd+P advertised nowhere)
- **Lens:** ux · **Effort:** S · **Runtime-verified:** yes
- **Evidence:**
  - Runtime: dispatching '?' opens nothing; no shortcuts overlay appears
  - src/lib/keys.ts:301-311 — palette opens only on Cmd+P/Cmd+Shift+P; no 'k'-metaKey or '?' handler exists in keys.ts
  - src/lib/command-palette.ts:638 — `shortcutsOpen.set(true)` is reachable only as a palette command, so the shortcuts panel requires the palette the user doesn't know about
  - src/lib/components/StatusBar.svelte:79-94 — the hint strip lists 13 chord hints but never mentions Cmd+P or help
- **Proposed fix:** Bind '?' (and Shift+/) to shortcutsOpen, add '?:help' and 'Cmd+P:commands' to the status-bar hints, consider Cmd+K as a palette alias.
- **Refutation notes:** Note the inventory's command_palette entry says "(Cmd+K)" — runtime check shows Cmd+K is NOT bound; the palette is Cmd+P only. The inventory note is the refuted artifact here, not the finding. Stands.

#### PERF-01 — Embedding job throughput: sequential single-threaded pipeline decodes full-res originals for 224px CLIP input
- **Lens:** performance · **Effort:** M · **Runtime-verified:** yes (live-DB observation + code trace; throughput estimated, not benchmarked)
- **Evidence:**
  - src-tauri/src/services/model_pipeline.rs:142-257 — strict per-image loop: DB lookup, decode, inference, 2 DB writes per image, no batching or parallelism
  - model_pipeline.rs:375-385 — resolve_image_path_for_ml returns the ORIGINAL full-resolution path for all non-RAW images, ignoring the existing 800px thumbnail
  - embeddings.rs:301-309 — generate_embedding_for decodes the full image then resize_exact to 224x224
  - model_pipeline.rs:210-213 — engine mutex held across each inference; commands/embeddings.rs:279-298 runs the whole synchronous run inline in the async command; mcp/tools.rs:2083-2097 spawns the same loop on async_runtime::spawn (not spawn_blocking)
  - Runtime observation: live library has 279/2,892 images embedded for clip-vit-b32 (read-only sqlite query)
  - Complexity estimate at 10k images: full-res decode dominates at ~0.3-2s/image single-threaded = 1-5+ hours per full run (no hard spec threshold exists)
- **Proposed fix:** Feed the 800px thumbnail to the embedding decode path for non-RAW images, wrap in spawn_blocking, batch DB writes per transaction, optionally parallelize decode with a small worker pool keeping ONNX inference serialized.
- **Refutation notes:** Throughput numbers are estimates, not benchmarks — flagged in Completeness. The structural facts (full-res decode for 224px input; no parallelism; tokio worker pinned) are uncontested.

#### PERF-02 — Import + thumbnail pipeline is fully sequential on one core inside an async command (first 10k import takes hours)
- **Lens:** performance · **Effort:** M · **Runtime-verified:** no (code-traced; thumbnail-store size observed)
- **Evidence:**
  - src-tauri/src/commands/import.rs:62-85 — entire import loop (per file: sha256 + decode + thumbnails) runs inline in the async import_folder command
  - db_core/import.rs:106-143 — import_file decodes and generates thumbnails inline per file; thumbnails.rs:37-56/71-90 — four Lanczos3 resizes + four JPEG encodes per image (64/128/256/800)
  - services/import.rs:105-114 — same sequential loop on the MCP/service path; no transaction batching, each insert its own WAL commit
  - Observed thumbnail store: 11,502 files / 287MB for 2,892 images
  - Mitigating: progress events emitted per file, so the UI shows progress
- **Proposed fix:** Parallelize decode+thumbnail across a rayon pool, batch DB inserts per chunk, run under spawn_blocking.
- **Refutation notes:** "Takes hours" is extrapolated, not measured. Under the chosen identity (AI art library, run-by-run accretion) the judge explicitly accepted this as survivable for the soft launch; it remains a top post-soft-launch fix per the judge's keep-anyway list.

#### PERF-03 — Cold start: session restore refetches ALL previously loaded pages serially before the grid paints anything
- **Lens:** performance · **Effort:** S · **Runtime-verified:** no (page-query timing measured; full restore path not measured)
- **Evidence:**
  - src/lib/image-loading.ts:242-252 — do/while loop awaits pages of 200 sequentially and only calls images.set(loaded) after the loop completes
  - src/routes/+page.svelte:328-333 — restored minItems = max(loadedImageCount, focusedIndex+1), so a previous deep-scroll session forces up to 50 serial IPC round-trips at 10k before first paint
  - Measured: single page query 29ms for 200 rows at 2,887 live images; fresh start passes the <3s bar comfortably; a restored 10k-deep session at ~30-60ms/page x 50 pages + IPC serialization risks breaching 3s
  - check_library_health (2 stat() calls x N images) runs after loadImages so it does not gate first paint, but does full-library file stats on every launch
- **Proposed fix:** Paint after the first page (set images incrementally), clamp restored minItems to a small multiple of the viewport.
- **Refutation notes:** Partially refuted for the fresh-start case (measured: passes). Stands only for the deep-restore case, which is extrapolated; flagged in Completeness.

#### PERF-04 — Loupe never loads the original full-resolution image — the 'loupe open < 1s full-res' threshold is unmeetable by design
- **Lens:** performance · **Effort:** M · **Runtime-verified:** no (explicitly code-traced, not runtime-measured)
- **Evidence:**
  - src/lib/view-utils.ts:218-226 — chooseLoupeImagePath ignores isRaw/sourceLoadFailed and delegates to safeAssetPreviewPath
  - view-utils.ts:200-216 — safeAssetPreviewPath returns thumbnail_path whenever asset-protocol-safe; the original path is only used if it lives under thumbnails/ or generated/ dirs, which user library folders never do
  - thumbnails.rs:8 — largest generated thumbnail is 800px, so loupe zoom (up to 20x) magnifies an 800px JPEG
  - 'cached thumb < 300ms' passes trivially; 'full-res < 1s' fails categorically — full-res is never requested
- **Proposed fix:** Decide deliberately: (a) progressive thumbnail→original swap with an explicit security boundary, or (b) ship 800px-preview-only as documented v1 behavior and rewrite the threshold.
- **Refutation notes:** Cannot be refuted by measurement — the code never issues the full-res request, so the threshold is unmeetable *by construction*. The open question is a product decision (option a vs b), recorded in the triage rationale for loupe_view.

#### PERF-05 — File-watcher images:changed triggers full reload + cache invalidation + focus reset on every debounce flush
- **Lens:** performance · **Effort:** S · **Runtime-verified:** no
- **Evidence:**
  - src/routes/+page.svelte:377-379 — listen('images:changed') calls loadImages({force:true, invalidateCache:true}) with default resetFocus=true, resetting focus/scroll to 0 and dropping loaded pages back to 200
  - src-tauri/src/watcher.rs:148-196 — sync thread flushes every 500ms with a 1.5s debounce and emits images:changed whenever any file changed; during a bulk copy/generation into a watched folder this fires repeatedly
  - Impact at 10k: a user mid-triage at scroll depth N loses position and the app refetches from offset 0 on every flush; repeated invalidation defeats the scope cache
- **Proposed fix:** Pass resetFocus:false on watcher-driven reloads and preserve scroll/loaded-count, or apply incremental inserts for synced files.
- **Refutation notes:** Not reproduced live (would need a bulk write into a watched folder during triage); the event chain is fully traced. Particularly identity-relevant: the chosen identity's accretion story (generation tools writing into watched folders) is exactly the trigger. Stands.

#### SEC-005 (pre-launch carve-out) — note
SEC-005 is filed post-launch overall, but its proposed fix explicitly carves out an S-sized pre-launch action: replace the hardcoded personal home-directory paths (scrubbed to $HOME in this report) in the two tracked docs before the repo goes public. The history-rewrite question is decided NO (would invalidate clones/tags for username-level disclosure only). See §1.3.

### 1.3 Post-Launch

#### CQ-3 — Inconsistent Mutex strategy: std::sync::Mutex + lock().unwrap() in services while db.rs uses parking_lot
- **Lens:** code-quality/rust · **Effort:** M · **Runtime-verified:** no
- **Evidence:** db.rs:8 uses parking_lot::Mutex (no poisoning); services/jobs.rs:3,64 (~15 `.lock().unwrap()` sites), services/undo.rs:5,81, db_core/embeddings.rs:6,299 (panic-while-locked exposure mirrors CQ-1), db_core/detection.rs:6,160 (the concrete trigger).
- **Proposed fix:** Migrate remaining std Mutex uses to parking_lot (already a dependency, Cargo.toml:56), or `lock().unwrap_or_else(PoisonError::into_inner)`.
- **Refutation notes:** Class-level finding; the only demonstrated trigger is CQ-1 (fixed pre-launch), which is why the broader migration is post-launch. Stands as hardening.

#### CQ-4 — Store subscription leaks in Grid and TabBar: manual .subscribe() with no unsubscribe
- **Lens:** code-quality/svelte · **Effort:** S · **Runtime-verified:** no
- **Evidence:** Grid.svelte:26,29 and TabBar.svelte:12 discard the unsubscriber; Svelte only auto-unsubscribes `$store` syntax; Grid remounts on every view switch, keeping destroyed-component closures alive for the app lifetime.
- **Proposed fix:** `$thumbnailSize`/`$gridGap` auto-subscription, or capture the unsubscriber and call it in onDestroy.
- **Refutation notes:** Leak growth not measured at runtime; mechanism is unambiguous from Svelte semantics. Stands.

#### CQ-5 — Dead component: SessionTimeline.svelte is referenced nowhere
- **Lens:** code-quality/svelte · **Effort:** S · **Runtime-verified:** no (grep-verified, exit 1)
- **Evidence:** `grep -rn SessionTimeline src/ tests/` returns zero matches outside the file; introduced in 45b60846b and never wired into any route; still calls listSessionEvents from $lib/api so it reads as live code.
- **Proposed fix:** Wire it into SessionSwitcher or delete the file. Triage verdict: CUT (delete).
- **Refutation notes:** None — grep refutation attempt confirmed absence. Note this CONTRADICTS the inventory's session_timeline entry, which lists Sidebar.svelte as an entry point; the inventory entry is the refuted artifact (see Completeness).

#### CQ-6 — PromptResubmitDialog cost-estimate effect lacks the stale-response guard used everywhere else
- **Lens:** code-quality/svelte · **Effort:** S · **Runtime-verified:** no
- **Evidence:** PromptResubmitDialog.svelte:50-57 — `$effect` writes `costEstimate = c` with no sequence token; rapid param changes can resolve out of order on a dialog that submits *paid* API generations. The codebase's own convention proves the gap: Loupe.svelte:164-176 (histogramRequestSeq) and EmbeddingExplorer.svelte:245-257 (selectedGenerationLoadSeq).
- **Proposed fix:** Same seq-token guard as sibling components.
- **Refutation notes:** Race not reproduced (timing-dependent); pattern divergence from the codebase's own convention is the evidence. Feeds the prompt_resubmit DEMOTE verdict.

#### CQ-7 — Import pipeline silently discards metadata/side-effect errors
- **Lens:** code-quality/rust · **Effort:** S · **Runtime-verified:** no
- **Evidence:** commands/import.rs:94 (`let _ = db.set_image_batch`), :98 (add_to_collection — imported images silently missing from active session), :100-101 (lineage + session events, duplicated at :186-193), :648/:692 (store_detections during import-time auto-detection).
- **Proposed fix:** Route through safe_eprintln with image ids; ideally count failures into the import result so the banner can say 'imported N, M metadata steps failed'.
- **Refutation notes:** None. Stands; pairs with UX-13.

#### SEC-004 — Audit-log gaps: UI-side token ops and failed HTTP auth not logged; tokens cannot expire
- **Lens:** security · **Effort:** S · **Runtime-verified:** no
- **Evidence:** commands/mcp.rs:6-36 (token create/revoke/rotate with no log_audit, while MCP-tool equivalents are logged); mcp/http.rs:352-377 (401s leave no queryable trace); services/tokens.rs:171-209 (expires_at always None though validate_token honors expiry at :248-254).
- **Proposed fix:** log_audit on the Tauri commands; synthetic '_auth_failed' tool_name for auth failures; optional expires_at on create paths.
- **Refutation notes:** None. Tolerable post-launch because the highest-blast-radius MCP surfaces (publish/serve) are demoted default-off in triage.

#### SEC-005 — Personal absolute paths in two tracked docs and throughout git history (no secrets found)
- **Lens:** security · **Effort:** S · **Runtime-verified:** no
- **Evidence:** Personal paths in 2026-06-03-release-skill.md and 2026-05-30-clipboard-monitor-design.md; history commits embed personal photo/project paths (dec06d40c, 8b7b00b32); credential scan of full history (AIza/sk-/ghp_/PRIVATE KEY patterns) surfaced only intentional test sentinels from 9b5698f43; no .env files in working tree, .gitignore excludes them.
- **Proposed fix:** Fix the two tracked docs pre-launch (S); do NOT rewrite history — username-level disclosure only, and a rewrite would invalidate all clones/tags; record the decision in release notes.
- **Refutation notes:** The secrets hypothesis was explicitly tested and refuted (full-history credential scan: clean). What remains is path/username disclosure, deliberately accepted.

#### SEC-006 — Default window capability grants opener:allow-open-path over all of $HOME plus process:default
- **Lens:** security · **Effort:** M · **Runtime-verified:** no
- **Evidence:** capabilities/default.json:11-17 ($HOME/**/* and $APPDATA/**/* opener grants on main and window-*), :23 (process:default to the renderer). Mitigating: strict CSP, narrower preview-display capability set — exploitation requires renderer compromise first.
- **Proposed fix:** Route reveal/open through a Rust command that checks the path is library-registered, then drop the blanket grant and process permission.
- **Refutation notes:** Mitigations are real (folded into the severity); the over-grant itself is uncontested. Stands as post-launch tightening.

#### HYG-007 — Verified-clean baseline (informational): license alignment, license audit, lockfiles, vuln scans, migration upgrade path all pass
- **Lens:** release-hygiene · **Effort:** S · **Runtime-verified:** yes
- **Evidence:** License identifier aligned across package.json/Cargo.toml/lockfile/README/NOTICE/AboutDialog + full Apache-2.0 text in LICENSE and LICENSE.md; `npm run audit:licenses` exit 0 (144 npm + 677 cargo packages); lockfiles committed and pinned in release.config.json:9; npm audit and cargo audit: 0 vulnerabilities; `cargo test db_core::db::migration_safety_tests` = 7 passed (legacy→21, v20→v21, re-open, future-version rejection, pre-migration backup, failed-step rollback; CURRENT_SCHEMA_VERSION=21; v21.db fixture exists); SECURITY.md threat model present; CHANGELOG current to 0.2.1; updater pubkey + endpoint configured with rotated signing key.
- **Proposed fix:** No action — recorded so Track A does not re-spend time on these checks.
- **Refutation notes:** This IS the refutation record for a class of suspected findings that did not materialize.

#### UX-08 — Status-bar shortcut hints are static and wrong outside Grid
- **Lens:** ux · **Effort:** S · **Runtime-verified:** yes
- **Evidence:** StatusBar.svelte:79-94 renders hints unconditionally for every mode; runtime: identical hint strip in Export/Canvas/Embeddings; in Loupe the NSFW overlay says 'hold Space to peek' while the status bar says 'space:select'.
- **Proposed fix:** Derive the hint list from viewMode.
- **Refutation notes:** None. Stands.

#### UX-09 — Loupe and Embeddings empty states name actions without offering them
- **Lens:** ux · **Effort:** S · **Runtime-verified:** yes
- **Evidence:** Loupe.svelte:541 bare 'No image selected'; EmbeddingExplorer.svelte:2102-2108 'Model Required'/'API Key Required' with no button (actual affordance is an unlabeled gear); Embeddings with 0 images still renders the full projection slider stack.
- **Proposed fix:** Loupe: 'Press Cmd+1 to pick an image in Grid'. Embeddings: Download/Settings button inside the empty state; disable projection controls until points exist.
- **Refutation notes:** None. Stands.

#### UX-10 — Raw classifier class names leak into the loupe info panel
- **Lens:** ux · **Effort:** S · **Runtime-verified:** yes (mock data)
- **Evidence:** Runtime loupe info panel: 'NSFW EXPOSED_BREAST_F 0.88' — raw snake_case NudeNet label truncated mid-token; terminology elsewhere is humanized (context-menu-labels.test.ts).
- **Proposed fix:** Map detector class IDs to human labels at display time.
- **Refutation notes:** Verified with mock data, not a live detection run — same render path. Stands.

#### UX-11 — Boilerplate document title 'Tauri + SvelteKit + Typescript App'
- **Lens:** ux · **Effort:** S · **Runtime-verified:** yes
- **Evidence:** src/app.html:7; runtime-confirmed via document.title on :1420. Production window title is fine (tauri.conf.json:15 'Cull') but the HTML title is what assistive tech announces.
- **Proposed fix:** Set the title to 'Cull'.
- **Refutation notes:** None. Stands.

#### UX-12 — Minor Tokyo Night token drift (off-palette red, raw #fff)
- **Lens:** ux · **Effort:** S · **Runtime-verified:** yes
- **Evidence:** Thumbnail.svelte:305 `#f87171` instead of --red #f7768e; Loupe.svelte:888,934 `#fff` instead of --text. Canvas-drawn hexes are justifiable (ctx can't read CSS vars) and use Tokyo Night values; runtime contrast scan found zero AA failures on real text.
- **Proposed fix:** var(--red)/var(--text); optionally getComputedStyle for canvas contexts.
- **Refutation notes:** The broader "theme drift" hypothesis was largely refuted by the contrast scan (clean); only the two literals remain. Stands narrowly.

#### UX-13 — Import error detail is summarized to a count with no way to see which files failed
- **Lens:** ux · **Effort:** M · **Runtime-verified:** no
- **Evidence:** Sidebar.svelte:319-321 — 'N errors' appended to an 8-second toast; result.errors contents never shown anywhere. Otherwise the import flow is good (button progress, toasts, post-import collection banner).
- **Proposed fix:** Clickable error count or a diagnostics panel listing failed paths and reasons.
- **Refutation notes:** None. Stands; pairs with CQ-7.

#### PERF-06 — generate_similarity_groups is O(n²) brute-force cosine clustering in memory
- **Lens:** performance · **Effort:** M · **Runtime-verified:** no
- **Evidence:** services/ai.rs:67-104 nested loop, up to n² cosine ops on 512-dim vectors single-threaded; ~100M ops at 10k ≈ tens of seconds to minutes; runs via async command (embeddings.rs:396-411) without spawn_blocking. Not covered by a named 10k threshold (find-similar is a separate O(n) path — PERF-07).
- **Proposed fix:** spawn_blocking + rayon + pre-normalized vectors; longer term, perceptual-hash banding or ANN candidate pruning.
- **Refutation notes:** Complexity-derived, not benchmarked. Stands.

#### PERF-07 — Measured: query/search thresholds PASS at 10k by measurement + extrapolation (record for synthesis)
- **Lens:** performance · **Effort:** S · **Runtime-verified:** yes
- **Evidence:** Live cull.db measurements (2,887 images, 4,218 files, 18MB): list_images page 29ms; deep OFFSET 2600: 6ms; smart-collection 6-LEFT-JOIN rating filter 1ms; 13-way LIKE search 15ms; list_folders 57ms. Smart-collection/NL p95 < 500ms: PASS (worst 57ms, ≤~200ms extrapolated at 10k). CLIP find-similar < 2s: PASS (3ms @ 279 vectors, ~0.1-0.3s extrapolated at 10k; live MCP find_similar instant; batched hydration, no N+1). Thumbnail p95 < 200ms: expected PASS, not browser-measured. Resident memory < 1.5GB: expected PASS, not measured. Caveats: f.path LIKE with bound parameter cannot use image_files_path_idx (full scan per folder page — first index-fix if libraries grow); raw_metadata shipped on every list row.
- **Proposed fix:** No action for the 10k bar. Post-launch: drop raw_metadata from list payloads; make folder scoping index-friendly.
- **Refutation notes:** This is the positive-verification record. Two sub-claims were originally extrapolated/expected rather than measured (thumbnail p95, RSS) — flagged in Completeness.
- **Measurement update (2026-06-10, bd imageview-dkz.31):** both remaining thresholds measured — thumbnail load-path p95 at a synthetic 10k library is 0.0011–0.0041ms per thumbnail / 0.54–1.95ms per scroll batch (vitest, regression-guarded); resident memory after an MCP-driven full-library browse of the running app measured at 2,687 images (real library size): worst 1.17GB transient, 0.80GB right after browsing, settling to 0.17GB — under the 1.5GB bar at measured size (not a 10k claim). Full numbers and caveats in §0.

#### PERF-08 — Dead dependency: virtua is declared but never imported
- **Lens:** performance · **Effort:** S · **Runtime-verified:** yes (grep, zero imports)
- **Evidence:** package.json:53 declares virtua ^0.49.1; zero imports across src/**/*.{ts,svelte}; the grid uses hand-rolled windowing (Grid.svelte:46-53, view-utils.ts:258-296).
- **Proposed fix:** Remove from package.json and lockfile — misleads contributors and adds supply-chain surface for nothing.
- **Refutation notes:** None. Stands.

#### PERF-09 — Embedding Explorer transfers up to 5,000 × 512 float vectors as one JSON IPC payload
- **Lens:** performance · **Effort:** S · **Runtime-verified:** no
- **Evidence:** EmbeddingExplorer.svelte:74,1234 (PROJECTION_EMBEDDING_LIMIT=5000, single getEmbeddingPage call); db.rs:1779-1827 returns flat f32 vector — ≈25-40MB of JSON through Tauri IPC. Mitigating: capped at 5000, worker-based projection, bounded thumbnail cache.
- **Proposed fix:** Chunked fetch feeding the worker incrementally, or base64-packed Float32 buffers.
- **Refutation notes:** Payload size computed, not profiled. Stands.

---

## 2. Identity Panel Summary

Three advocates argued competing release identities; a judge scored each on differentiation, maturity, audience fit, and budget fit (5-day pre-launch budget), then issued a recommendation.

### 2.1 Advocate A — "Fast culling tool: keyboard-driven triage of large AI-generation batches"

Core argument: the app is literally named Cull and the codebase agrees. (1) The triage spine is the tested, load-bearing core — essentially every feature that is both `tested` and `high`-coupling is a triage primitive (decision_tagging, star_rating, undo_redo, selection_management, command_palette, keyboard_shortcuts_ui, grid_view, image_loading, smart_collections, command_bar, image_search, collection_management), while the alternative identities rest on `partial`-coverage satellites (embeddings_view, publish_view, lineage_view, compare_view, loupe_view). (2) "AI-generation batches" is earned, not decoration: loupe generation metadata, lineage_view's run browser, generation_results, prompt_resubmit, C2PA source detection, and the MCP ai-processing group's redaction-tested metadata tools. (3) "Fast" and "keyboard-driven" are the design language — tinder_view, collect_mode, view_cycling, zen/presentation modes, chord ratings, scroll-direction-aware prefetch. (4) The audit's own performance thresholds (loupe <1s, 10k import, 10k query) presuppose large-batch culling, and the culling core is the part that clears its bar. (5) Narrowing to this identity sheds release risk: the worst security findings cluster on non-culling satellites, and the dormant features prove breadth beyond triage is aspiration.

### 2.2 Advocate B — "AI art library: browse, CLIP semantic search, collections, generation metadata" (WINNER)

Core argument: this is the identity the codebase already declares about itself. (1) Self-description: the MCP server self-describes as "browse, curate, and manage an AI art image library" (src-tauri/src/mcp/tools.rs:2818); README.md:3 opens with "review and curation for AI-generated image workflows"; PRODUCT.md names AI artists with large local archives; prompt/negative_prompt/model/seed are first-class columns (models.rs:255-260), ai_prompt on the core image record (models.rs:13). (2) The four pillars are the tested, high-coupling center of gravity of the inventory — browse (grid_view, image_loading), CLIP search (similarity_search, image_search, command_bar, embeddings_view, search-quality-group), collections (collection_management, smart_collections, browse-curate-group), generation metadata (loupe_view, lineage_view, generation_results, ai-processing-group with privacy-redaction tests) — and the generation-metadata pillar is the thing no generic DAM offers. (3) The findings actively eliminate rival identities while leaving this one intact: pro-photo culling is killed by PERF-04 (full-res never loads, by design) and PERF-02 (sequential 10k import) — fatal for a wedding shoot, survivable for run-by-run accretion; client proofing carries SEC-001 and partial coverage; generic image manager would make the dormant features matter. (4) Even the security findings testify to what Cull is — the dogfooding trail (AI provider CSP hosts, $HOME/.codex scope) is an AI-generation workflow, and the one measured performance PASS (query/search at 10k) validates exactly the browse + search pillars. (5) It is the narrowest true story that keeps the moat; everything else survives as features inside the library story rather than promises the findings show Cull cannot yet keep.

### 2.3 Advocate C — "Agent-native image tool: the MCP surface as headline differentiator"

Core argument: the MCP surface is the deepest, best-tested, hardest-to-copy subsystem. (1) A production-grade server fronting ~57 tools in ten groups, eight of ten marked tested at the level that matters (scope enforcement, pagination clamping, redaction, module gating). (2) The security architecture is the marketing story: roles, per-token scopes, one-time secrets, rotate/revoke, full audit trail, remote path redaction, privacy dashboard — "give an agent scoped, audited, revocable access to your image library" is a sentence nobody else can say. (3) Uniquely agent-shaped capabilities already work: agent_snapshots, display-ui-group view-driving, headless_cli proving the tool layer is the real product API with UI and CLI as clients. (4) The findings disqualify the polished-UI identity (Tab hijack, no full-res loupe, hours-long import, silent failures) while the agent path is structurally tolerant of those defects, and the one measured bright spot is the read path agents hammer. (5) MCP-specific findings are few and bounded (one pre-launch confinement fix) versus eleven pre-launch UX/perf findings on the human surface. (6) Strategically, the UI becomes the human-in-the-loop cockpit around the agent workflow, with the audit-logged end-to-end demo loop as the launch story.

### 2.4 Judge Scores

| Identity | Differentiation | Maturity | Audience | Budget fit |
|---|---|---|---|---|
| A — Fast culling tool (keyboard-driven triage) | 7 | 6 | 6 | 4 |
| **B — AI art library (browse, CLIP search, collections, generation metadata)** | **7** | **8** | **9** | **8** |
| C — Agent-native image tool (MCP headline) | 9 | 6 | 5 | 6 |

### 2.5 Judge Rationale and Recommendation

**Recommendation: B — "AI art library — browse, CLIP semantic search, collections, generation metadata."**

The judge verified B's citations against the repo and they all hold (README.md:3, tools.rs:2818, PRODUCT.md audience, GenerationRun fields at models.rs:253-262 + ai_prompt at models.rs:13). B wins on the interaction of three constraints. (1) **Maturity:** its four pillars are the tested, high-coupling center of the inventory, and the only measured performance PASS (query/search at 10k) lands exactly on them; its weakest pillar (generation metadata, partial coverage) is a feature-depth risk, not identity-falsifying. (2) **Audience:** a soft launch to friends who per PRODUCT.md are AI artists means libraries that accrete run-by-run — the hours-long bulk import that kills identity A on first run is survivable under B, and B makes no speed promise the audit contradicts. (3) **Budget:** B's remaining pre-launch work is identity-neutral hygiene (CI signed artifact, export_images confinement, CSP, NOTICE, onboarding jargon) — feasible in 5 days. Identity A stakes itself on the two adjectives the audit falsifies — "keyboard-driven" (global Tab hijack) and "fast" (fully sequential import, serial session restore) — plus silent delete failures, the worst defect class for a culling tool; fixing all of that plus CI in 5 days is not credible (budget_fit 4). Identity C is the most differentiated claim (9) and the findings asymmetry is genuine, but it headlines the smaller surface (10 MCP groups vs 60+ UI features) to friends who will judge the app by opening it, its most agent-valuable groups (search-quality, ai-processing) are only partial coverage, and the agent-native headline raises the security bar exactly where pre-launch findings sit (export confinement, non-expiring tokens, unlogged UI token ops). B is also the only identity the codebase literally declares about itself, so positioning and product cannot drift apart during a rushed launch. PRODUCT.md's own phrasing — "local-first, keyboard-first, agent-native image curation tool" — confirms the synthesis: **B is the noun; A and C are the adjectives, and adjectives become features, not headlines, for the soft launch.**

**Keep-anyway list (salvaged from losing identities):**
1. From A: the tested, high-coupling triage spine (decision_tagging A/R/U, star_rating chords, undo_redo, selection_management, command_palette, tinder_view) stays the lead *feature* inside the library story, just not the falsifiable headline.
2. From A: the global Tab hijack must be fixed before launch regardless of identity — README already promises "keyboard-first", so the defect contradicts the shipped copy, not just identity A.
3. From A: keep the large-batch thresholds (loupe <1s, 10k import, 10k query) as the post-launch performance bar; sequential import and silent trash/delete failure-swallowing are the top two post-soft-launch fixes.
4. From C: the scoped/audited/revocable MCP token architecture (roles, folder/collection/tag scopes, one-time secrets, full audit trail) is the hardest-to-copy claim in the product — stated secondary differentiator and the centerpiece of the next positioning beat.
5. From C: agent_snapshots plus display-ui-group (agents capturing annotated views and selecting inside the live UI) is category-unique and should survive any scope cut.
6. From C: headless_cli reusing the MCP tool layer via call_tool proves the tool layer is the real product API with UI and CLI as clients — preserve this architectural invariant in all future work.
7. From C: the export_images arbitrary output_dir path-confinement fix is a bounded pre-launch item needed under any identity.
8. From C: the end-to-end demo loop (agent imports/rates via scoped token, human speed-reviews in tinder_view, agent snapshots and publishes, every step audit-logged) is the best future marketing demo — every component is marked tested; bank it for the public launch.

---

## 3. Triage Table

> **Note:** Re-triaged 2026-06-10 against user-chosen identity 'agent-native image tool' per the override rework rule; original AI-art-library triage preserved in git history (commit 96b903da0).

Verdicts: **CORE** (identity pillar, gets polish budget) · **KEEP** (ship as-is) · **DEMOTE** (keep code, hide behind a settings toggle / default-off) · **PLUGIN** (extract via Track C) · **CUT** (remove).

| item_id | verdict | effort | rationale |
|---|---|---|---|
| grid_view | CORE | S | "Browse" is a verb in the identity sentence and the UI half of the co-equal promise. Tested, high coupling, and the measured query/search PASS at 10k (PERF-07) lands here. Polish: fix the Grid store-subscription leak (CQ-4) opportunistically. |
| loupe_view | CORE | M | The human inspection cockpit of the human-in-the-loop workflow — under "UI and agent surface co-equal" the review surface keeps its polish budget. High coupling, partial coverage; loupe-local findings (UX-10 raw classifier labels, PERF-04 never-loads-full-res) get worked. |
| compare_view | KEEP | S | High coupling, partial coverage, stable; no pre-launch findings. Useful for choosing among generation variants but neither agent-facing nor headline surface. Ship as-is. |
| canvas_view | KEEP | S | Tested, high coupling, 885 LOC. It is the workspace the MCP canvas tools (get_canvas_layout, list_session_canvases) read, so it supports the agent surface — but it is healthy and needs no polish; extraction would be L for no gain. Ship as-is. |
| lineage_view | KEEP | S | Was CORE as the AI-art-library generation-metadata pillar; under agent-native it is a UI convenience over data agents read via get_generation_run (ai-processing-group). Medium coupling, partial coverage, 504 LOC — ship as-is, no polish budget. |
| embeddings_view | KEEP | M | Showcases the sacred CLIP pillar but is the largest view (3041 LOC), partial coverage, with a 5000x512-float JSON IPC payload (PERF-09, post-launch). Sacred status forbids CUT/PLUGIN; ship as-is. |
| tinder_view | KEEP | S | The human speed-review beat of the agent demo loop (agent rates via scoped token, human reviews in tinder). Medium coupling, 552 LOC, partial coverage — cheap to ship, no polish budget. |
| export_view | KEEP | S | Tested, medium coupling, recently invested (commit bc5d40c44). Slide export is a human convenience; agents export via the import-export-group. Ship as-is. |
| publish_view | PLUGIN | M | Stays PLUGIN per the user-accepted plugin scope (proof plugin cull-publish). Coherent under agent-native: agents publish through the MCP publish-clipboard-group and export_static_publish_* tools, which stay in core (see their CORE verdicts); only this low-coupling, 1122-LOC settings UI extracts, already module-gated behind module_static_publishing. |
| command_palette | CORE | S | Tested, high coupling, the keyboard front door to every surface — co-equal UI demands it work for strangers. Pre-launch UX-07 (palette advertised nowhere) is the polish. |
| sidebar | CORE | M | High coupling navigation hub. Two pre-launch first-run findings land here (UX-03 AI-model jargon dead end, UX-04 buried import); the human first-run is half the co-equal promise, so polish is mandatory. |
| tab_bar | KEEP | S | Low coupling, tested, 280 LOC. Known subscription leak (CQ-4) is a post-launch two-line fix. Ship as-is. |
| status_bar | KEEP | S | Low coupling, tested, 180 LOC. Static-and-wrong-outside-Grid hints (UX-08) are post-launch. Ship as-is. |
| keyboard_shortcuts | KEEP | S | Tested, low coupling dialog. The '?' discoverability fix (UX-07) lands in keyboard_shortcuts_ui (CORE), not here. Ship as-is. |
| context_menu | KEEP | S | Tested, medium coupling, 858 LOC of table-stakes library interactions. No identity tension; ship as-is. |
| thumbnail | KEEP | S | High coupling render cell for the CORE grid; partial coverage but stable. Polish flows to it indirectly via grid_view. |
| star_rating | KEEP | S | 80 LOC, tested, high coupling; the UI twin of the MCP set_rating tool. Healthy, no findings. Ship as-is. |
| decision_tagging | KEEP | S | 60 LOC, tested, high coupling; the UI twin of set_decision — the curation verb agents and humans share. Healthy; ship as-is. |
| undo_redo | KEEP | S | Tested, high coupling, trust-critical and currently healthy — no findings. Ship as-is. |
| selection_management | KEEP | S | Tested, high coupling; also drivable by agents via select_images_in_view (display-ui-group). Healthy; ship as-is. |
| grid_customization | KEEP | S | Low coupling, tested, 80 LOC quality-of-life. Zero maintenance cost; ship as-is. |
| collection_management | CORE | S | "Curate" is in the identity sentence and collections are how agents and humans both do it — create_collection/add_to_collection in browse-curate-group mirror this UI exactly. Tested, high coupling, persisted. |
| collect_mode | KEEP | S | Medium coupling, partial coverage, 120 LOC human accelerator for the collections pillar. Ship as-is; coverage debt is small. |
| smart_collections | CORE | S | SACRED and doubly identity-central: NL/rule filters humans build are also agent-creatable via create_smart_collection. Tested, high coupling. Polish: remove the dead color_label filter from RuleBuilder (see color_analysis CUT). |
| import_banner | CORE | M | The library's human front door. Tested, medium coupling, but pre-launch findings cluster here (UX-02 silent init failure renders as healthy-empty, UX-04 undersold import paths); first-run polish is mandatory under co-equal UI. Bulk-import speed (PERF-02) remains a backend item. |
| job_progress | KEEP | S | Medium coupling, partial coverage, 447 LOC. Human visibility into agent-spawned async jobs (embeddings, import, export). Ship as-is; remove the OCR job row when that feature is cut. |
| generation_results | KEEP | S | Tested, low coupling, 140 LOC strip with agent-snapshot integration already noted in inventory. Cheap to keep. |
| export_folder_dialog | KEEP | S | Medium coupling, partial coverage. Batch export is table-stakes for humans; agents use export_images instead. Ship as-is. |
| contact_sheet | KEEP | S | Medium coupling, partial coverage, 240 LOC. Low maintenance; ship as-is. |
| delivery_csv | DEMOTE | S | Client-delivery/proofing workflow, orthogonal to the agent-native identity. Low coupling and only 80 LOC, so hiding the palette command behind a 'client tools' settings toggle is S; cutting would orphan client_feedback data it exports. |
| client_feedback | DEMOTE | S | Pro-photographer proofing concept (client favorites/comments vs curator ratings), outside the agent-native identity. High coupling (loupe, CSV) makes PLUGIN extraction L-cost, so DEMOTE behind the same client-tools toggle as delivery_csv is the cheap correct move. |
| group_ranking | KEEP | S | Tested, medium coupling, leans on the sacred CLIP grouping. Useful to both surfaces; ship as-is. |
| similarity_search | CORE | S | SACRED (find-similar) and the UI twin of the find_similar MCP tool. Tested, medium coupling, 90 LOC; the measured 10k search PASS (PERF-07) is a proof point for both surfaces. |
| detection_overlays | KEEP | S | Medium coupling, partial coverage. The NaN-unsafe NMS panic (CQ-1) is fixed in the backend detection path shared with search-quality-group (CORE); the overlay UI itself ships as-is. |
| image_search | CORE | S | SACRED (NL search). Tested, high coupling, creates temporary smart collections — the same NL query machinery agents hit through smart-collection tools. |
| command_bar | CORE | S | Tested, high coupling, the visible home of NL search and presets. Pre-launch UX-06 (two adjacent unlabeled '×' buttons) gets fixed. |
| view_cycling | KEEP | S | 50 LOC, tested. The pre-launch Tab-hijack finding (UX-01) is fixed in keyboard_shortcuts_ui (CORE); cycling itself stays, likely remapped. |
| loupe_pan_zoom | KEEP | S | Low coupling, partial coverage, standard inspection mechanics inside the CORE loupe. Ships under loupe's umbrella. |
| crop_tool | KEEP | S | Low coupling, 200 LOC, partial coverage, no findings. Cheap to keep. |
| nsfw_blur | KEEP | S | Low coupling, tested, 60 LOC. Ship as-is. |
| zen_mode | KEEP | S | 40 LOC, tested, low coupling. Zero-cost presentation nicety. |
| compare_presentation | KEEP | S | 60 LOC, tested, low coupling. Ships free with compare_view. |
| export_presentation | KEEP | S | 50 LOC, tested, low coupling. Ships free with export_view. |
| modal_dialogs | KEEP | S | Shared tested low-coupling infrastructure (trash confirm, text input, collection target ride on it). Ship as-is. |
| text_input_dialog | KEEP | S | Tested, low coupling, 80 LOC shared primitive. Ship as-is. |
| trash_confirm | KEEP | S | Tested, low coupling. The pre-launch silent-delete-failure fix (CQ-2) lands in file_operations (CORE); this dialog then reflects real per-file outcomes. |
| collection_target_dialog | KEEP | S | Tested, medium coupling, serves the CORE collections pillar. Ship as-is. |
| toast_notifications | KEEP | S | Tested, low coupling. The pre-launch aria-live fix (UX-05) is an S-sized attribute change that rides along before launch. |
| update_banner | KEEP | S | Tested, low coupling, and the only ship-fixes-fast channel for a soft launch. Depends on the release pipeline finding (HYG-001) being fixed. |
| preview_display | DEMOTE | S | Client-review window for humans-showing-humans (image-only/client-review/metadata-review modes), not part of the agent loop. Low coupling, partial coverage, 500 LOC — hide behind the client-tools settings toggle. |
| mcp_settings | CORE | M | Was KEEP under AI-art-library; rises because this 1052-LOC panel is the cockpit of the headline — token management, module gates, privacy dashboard — and the first screen an agent-curious user opens. Low coupling, but partial coverage is no longer acceptable on the headline's config surface; token-expiry UX from SEC-004 lands here. |
| about_dialog | KEEP | S | Tested, low coupling. Carries the copyright-year drift (HYG-002) — a one-line fix, not a verdict changer. |
| session_switcher | KEEP | S | Tested, medium coupling. Folder-based sessions map to run-by-run workflows; ship as-is. |
| session_timeline | CUT | S | Dead code, identity-independent (CQ-5): grep finds zero references anywhere in src. 160 LOC of pure maintenance cost with no UX surface. Delete the file. |
| prompt_resubmit | DEMOTE | M | Partial coverage, carries the stale cost-estimate bug before paid generation (CQ-6), and is the only feature justifying the pre-launch CSP exfiltration whitelist (SEC-002). Low coupling — gate behind settings until both are fixed; agent-side generation-metadata flows (ai-processing-group) are unaffected. |
| keyboard_shortcuts_ui | CORE | M | High coupling global key system carrying the worst pre-launch a11y finding (UX-01 global Tab hijack makes every control keyboard-unreachable) plus undiscoverable help (UX-07). Co-equal UI means the human surface must actually be keyboard-operable. |
| file_menu | KEEP | S | Medium coupling, partial coverage, standard macOS expectation. Ship as-is. |
| image_context_menu | KEEP | S | High coupling native menu mirroring context_menu actions; partial coverage but table-stakes. Ship as-is. |
| agent_snapshots | CORE | S | Was KEEP; rises because annotated view snapshots for agent analysis are category-unique agent surface — exactly the headline. Tested, low coupling, 280 LOC; polish is demo-loop integration and documentation, not rework. |
| workflows | KEEP | S | Tested, low coupling, 120 LOC power-user nicety with destructive-step confirmation built. Near-zero cost. |
| import_export | KEEP | S | Tested, high coupling. Full library backup/restore is trust infrastructure for any library product. Ship as-is. |
| deeplinks | KEEP | S | Tested, medium coupling. cull:// routing supports agent and inter-app workflows cheaply; healthy, no polish needed. |
| privacy_dashboard | CORE | S | Was KEEP; rises because "scoped, audited, revocable" is now the headline sentence and this 140-LOC low-coupling panel is where users verify it. Partial coverage on a headline trust surface gets polish (audit-log visibility, SEC-004 follow-through). |
| view_state_persistence | KEEP | S | Tested, low coupling. The serial-restore cold-start finding (PERF-03) is fixed in image_loading's restore path, not this store. Ship as-is. |
| image_loading | CORE | M | High coupling, tested, the substrate of browse and of first impressions. Two pre-launch perf findings get polish: PERF-03 (serial session-restore refetch before first paint) and PERF-05 (watcher-triggered full reload + focus reset). |
| min_size_filter | KEEP | S | 50 LOC, tested, medium coupling. Ship as-is. |
| show_missing_filter | KEEP | S | 50 LOC, tested, medium coupling. Diagnostic value for broken imports; ship as-is. |
| mcp-server | CORE | M | SACRED and now THE headline. 3865 LOC, tested, high coupling: auth context, tool dispatch, path redaction, audit logging. Polish: SEC-001 export confinement and SEC-004 hardening — the agent-native headline raises the security bar exactly where these findings sit, so they move up the queue. |
| browse-curate-group | CORE | S | 13 tested tools that are the literal identity sentence ("agents can browse, curate"): scope enforcement, pagination clamping, and validation all test-covered. High coupling, minimal work needed — bullish showcase. |
| search-quality-group | CORE | M | find_similar is SACRED (CLIP, cosine similarity). Partial coverage on a headline agent surface is the gap: polish is test coverage for the quality/detection read paths plus the CQ-1 NMS NaN-panic fix that can poison the ONNX session this group depends on. |
| import-export-group | CORE | M | Tested, high coupling, and hosts the export_static_publish_* tools agents publish through. The pre-launch SEC-001 finding (export_images arbitrary output_dir) lands precisely here and must be fixed before strangers point agents at it. |
| display-ui-group | CORE | S | Was KEEP; rises because agent view-driving (snapshot capture, label/image selection inside the live UI) is the category-unique demo of the headline. 9 tested local-only tools, medium coupling, no pre-launch findings — cheap bullishness. |
| publish-clipboard-group | CORE | S | Was DEMOTE under AI-art-library; "publish" is now in the identity sentence, so the agent publish path is headline surface, not hidden surface. 150 LOC, tested, module-gated with path-redaction tests. Polish: keep the module gate as an explicit consent step and harden the serve path — the security posture stays, the visibility rises. |
| ai-processing-group | CORE | M | Sacred CLIP embedding generation plus generation metadata with tested remote redaction of prompts. Partial coverage on the async job paths is the gap to close; PERF-01 (sequential single-threaded embedding throughput) lands in its backend. |
| jobs-management-group | KEEP | S | 3 admin-only tested tools, high coupling, 100 LOC. Necessary plumbing for agent-spawned jobs and already healthy — KEEP because no polish is needed, not because it is peripheral. |
| tokens-management-group | CORE | M | Was KEEP; rises because per-token roles, scopes, one-time secrets, and rotate/revoke ARE the "give an agent scoped, audited, revocable access" claim. Tested, medium coupling, 150 LOC. SEC-004 (no token expiry, unlogged UI-side token ops) is promoted from post-launch hardening to polish budget under this headline. |
| audit-logging-group | CORE | S | Was KEEP; the audit trail is the trust half of the headline. Tested, 80 LOC, every tool call already logged via log_tool_call(). Polish: close the SEC-004 gaps (UI-side token ops and failed HTTP auth unlogged) so the audit story ships without asterisks. |
| voice_dictation | DEMOTE | S | Low coupling, no behavioral tests (only indirect contract coverage), macOS-only, and orthogonal to both the agent surface and the core UI loop. Hide behind a settings toggle, default off. |
| system_tray | KEEP | S | Tested (5 unit tests), medium coupling; surfaces MCP connection count and clipboard-monitor state — ambient trust signals for the agent loop. Ship as-is. |
| file_watcher | CORE | M | High coupling, heavily tested (34 Rust tests), identity-critical: agents and generation tools write into watched folders, so live sync is how the agent-fed library accretes. PERF-05 (full reload + cache invalidation + focus reset per debounce flush) gets polish jointly with image_loading. |
| headless_cli | CORE | S | Was KEEP; rises because it proves the identity's architectural claim — the MCP tool layer is the product API with UI and CLI as clients (reuses call_tool, src-tauri/src/cli/mod.rs:154). Tested (~30 tests), medium coupling; polish is documenting it in the launch story, not code changes. |
| raw_support | DEMOTE | S | RAW decode (cr2/nef/arw...) serves camera photographers, not agent-fed AI archives. Already opt-in (module_raw, default off) with libraw.rs untested — DEMOTE ratifies the existing posture, labeled experimental. |
| drag_drop_import | KEEP | S | Medium coupling, thin (120 LOC), only a 16-line contract test — but drag-drop is the most discoverable human import path. Ship as-is; onboarding polish happens in import_banner/sidebar. |
| clipboard_monitor | KEEP | S | Tested (two dedicated test files), medium coupling. Auto-capture into a collection is agent-adjacent ingestion whose status is already exposed over MCP (get_clipboard_monitor_status); healthy, ships as-is while its publish tools rise to CORE separately. |
| native_menu_bar | KEEP | S | High coupling, tested (three contract test files), 2200 LOC of platform table-stakes. macOS users expect a complete menu bar; ship as-is. |
| file_operations | CORE | M | Trust in file ops underpins both surfaces — agents mutate libraries through the same backend humans do. The worst pre-launch data-integrity finding (CQ-2: trash/permanent-delete swallow per-file failures while the UI claims success) lands here; surfacing per-file outcomes is mandatory polish. files.rs has 11 tests to build on. |
| ocr_text_extraction | CUT | M | Dormant regardless of identity: start_ocr_batch is registered but grep confirms no frontend caller beyond the api.ts wrapper. 600 LOC with 2 tests of maintenance cost and zero UX surface. M because the lib.rs registration and JobProgressPanel row must be unwound and search_text consumers checked. |
| near_duplicate_detection | CUT | S | Dormant and redundant: callers only in api.ts/tauri-mock, 0 tests in db_core/perceptual_hash.rs, low coupling. The sacred CLIP find_similar plus group_ranking already cover near-dup detection for AI variants. 160 LOC, clean removal. |
| color_analysis | CUT | S | Dormant: analyze_image_colors has no caller beyond api.ts/tauri-mock (grep-confirmed), 0 tests in db_core/color.rs, and its only UI trace is the orphan color_label filter inside sacred smart collections — a user-visible dead end. Cut the 410 LOC and the filter together. |
| image_tags | KEEP | S | 100 LOC, low coupling, with live consumers (smart-collections RuleBuilder, Preview Display info rail). Unlike OCR/color it is not dormant; ship as-is. |
| loupe_histogram | KEEP | S | Tested (two test files), low coupling, native menu integration already done. Inspection nicety inside the CORE loupe; ship as-is. |
| preview_web_stream | DEMOTE | S | HTTP streaming of the preview window is client-review persona surface and a standing network listener (its own security contract test says so); the agent publish path is the MCP static-publish flow, not this. Tested, medium coupling — keep the code, default off behind the client-tools toggle. |

**Verdict totals:** 27 CORE · 53 KEEP · 7 DEMOTE · 1 PLUGIN · 4 CUT — all 92 inventory items triaged.

---

## 4. Plugin Spec (Track C, verbatim)

# Cull Plugin Mechanism + Store — Design Spec (Track C)

Worked backwards from the single PLUGIN-verdict feature: **publish_view** (Static Publishing). The question is not "what is the most general plugin system" but "what is the simplest runtime that can host *this* candidate in ~12 h."

## 0. What the candidate actually is (evidence)

Publish view is already half-extracted by accident — it is the only feature in the app behind a runtime module gate:

- Frontend surface: `src/lib/components/StaticPublishingSettings.svelte` (1122 LOC, `wc -l` verified), mounted at `src/routes/+page.svelte:485` only when `$staticPublishingEnabled` is true.
- The gate is a plain app setting: `staticPublishingEnabled.set((await getAppSetting('module_static_publishing')) === 'true')` (`src/routes/+page.svelte:336`; store declared `src/lib/stores.ts:167`).
- Command palette entries carry `requiresStaticPublishing: true` (`src/lib/command-palette.ts:118,125`) and are filtered at `src/lib/command-palette.ts:719`.
- Backend: `src-tauri/src/commands/static_publishing.rs` (2561 LOC) declares `MODULE_KEY: &str = "module_static_publishing"` and a versioned schema `"cull.static_publishing.v1"` (`static_publishing.rs:30-31`).
- MCP tools are gated on the same key (`src-tauri/src/mcp/tools.rs:216`, plus 3759/3763/3767/3796).

So the seam is: **a frontend view, gated by a module key, calling backend commands that already enforce that key independently.** The plugin system v1 is the generalization of exactly this seam — nothing more.

## 1. Mechanism choice: frontend JS modules over a Rust-enforced permission bridge (hybrid), backend stays in core

**Chosen:** plugins are precompiled ESM bundles (frontend JS modules) downloaded by the Rust side, checksum-verified, loaded from the app-data plugins dir, and given a narrow `host` API whose privileged calls are enforced in Rust. The Rust backend for a plugin's feature stays compiled into core behind its module key (as `static_publishing.rs` already is).

**Rejected alternatives, against Tauri 2 constraints and the 5-day budget:**

- **External process plugins (sidecars):** dead on arrival for a notarized macOS app under this budget. Downloaded executables get Gatekeeper quarantine; unsigned binaries won't run; signing third-party plugin binaries is a distribution program, not a 12 h task. The release plan already treats `codesign/spctl/stapler` as gates — sidecars multiply that surface.
- **MCP tool packs as the plugin mechanism:** Cull already has a strong MCP server (`src-tauri/src/mcp/server.rs`, `socket.rs`, `http.rs`) and external MCP servers are a fine *v1.1* extension story for agent-side features. But the one PLUGIN-verdict candidate is a **UI view**; MCP cannot deliver a view. Choosing MCP-only would mean the proof plugin is impossible by construction.
- **Pure frontend modules with no Rust enforcement:** fails the security-consistency requirement. MCP enforcement lives in Rust (`require_capability`, `src-tauri/src/mcp/auth.rs:29-39`); plugin permission checks must live at the same trust boundary, not in webview JS a plugin can monkey-patch.

**Tauri 2 specifics that shape the design:**

- CSP is `default-src 'self'` with no `script-src` override (`src-tauri/tauri.conf.json:25-34`), so remote `<script>`/dynamic import of HTTPS URLs is blocked — good, keep it. Loading path: Rust reads the installed bundle from disk → frontend re-hashes the string against the manifest checksum → `import(blobUrl)`. Requires one CSP change: add `"script-src": "'self' blob:"` at `tauri.conf.json:25`. The blob: widening is compensated by the load-time hash check (only checksum-matching code reaches `import`).
- Registry/bundle fetch happens in **Rust** (reqwest), not the webview, so `connect-src` (`tauri.conf.json:27`) stays untouched.
- Capability files (`src-tauri/capabilities/default.json` etc.) already segment window permissions; the plugin host adds no new Tauri capabilities — plugins never call `invoke` directly, only `host.invoke()` which routes through one new `plugin_invoke` Tauri command.

**Plugin API surface (v1, deliberately tiny):** default-export `activate(host)` where `host` = `{ mountView(el), registerPaletteCommands([...]), invoke(cmd, args) }`. `invoke` is the only privileged path and is permission-checked in Rust.

## 2. Manifest format

One `manifest.json` per plugin:

```json
{
  "id": "cull-publish",
  "name": "Publish View (Static Site)",
  "version": "1.0.0",
  "description": "Build a static site package from a canvas or selection, with QR code and local preview server.",
  "entry": "dist/plugin.js",
  "permissions": ["library:read", "export:read", "module:static-publishing"],
  "minAppVersion": "0.2.1",
  "checksum": "sha256:<hex of entry bundle>",
  "repo": "https://github.com/glebis/cull-plugins"
}
```

- `permissions` reuse the **existing MCP capability vocabulary** from `tokens::capabilities_for_role` (`src-tauri/src/services/tokens.rs:16-49`: `library:read`, `library:search`, `curation:write`, `export:read`, `import:write`, `ai:run`, `display:navigate`, `tokens:manage`), extended with `module:<key>` permissions that map onto existing module gates like `module_static_publishing` (`tools.rs:216`). No new permission taxonomy is invented.
- `minAppVersion` checked against `tauri.conf.json` version (`0.2.1`, `tauri.conf.json:4`) at install time, semver compare.
- `checksum` covers the `entry` bundle bytes; verified at install **and** at every load.

## 3. Registry v1

A single `registry.json` at the root of a public `glebis/cull-plugins` GitHub repo, fetched over HTTPS from a pinned immutable-ish URL (raw content of a tagged release, falling back to `main`). No backend, no accounts.

```json
{
  "schema": "cull.plugins.registry.v1",
  "updated": "2026-06-10",
  "plugins": [
    {
      "id": "cull-publish",
      "name": "Publish View (Static Site)",
      "version": "1.0.0",
      "description": "...",
      "minAppVersion": "0.2.1",
      "permissions": ["library:read", "export:read", "module:static-publishing"],
      "download": "https://raw.githubusercontent.com/glebis/cull-plugins/cull-publish-v1.0.0/cull-publish/dist/plugin.js",
      "checksum": "sha256:<hex>",
      "repo": "https://github.com/glebis/cull-plugins/tree/main/cull-publish"
    }
  ]
}
```

Install flow (all in Rust): fetch registry → user picks plugin → fetch `download` URL → SHA-256 the bytes → reject on mismatch → write to `$APPDATA/plugins/<id>/<version>/plugin.js` + manifest → record grant rows. Download URLs point at **git tags**, never `main`, so a checksum in the registry always describes immutable bytes.

**Migration path (v1 choices that don't block it):**

1. **v1 (now):** static `registry.json`, schema-versioned, per-bundle SHA-256, tag-pinned URLs.
2. **v1.5 (signed index):** add detached ed25519/minisign signature `registry.json.sig`; embed the public key in the app binary; app verifies the signature before trusting checksums. Possible *because* the registry is a single canonical file with a `schema` field — old clients ignore the sig, new clients require it.
3. **v2 (API):** an index API serving the same schema (`cull.plugins.registry.v2`) with pagination/search; plugin `id`s are already globally stable, so nothing re-keys.

Nothing in v1 (no accounts, no mutable URLs, no per-client state) creates a liability for steps 2-3.

## 4. Security model — consistent with the MCP token/audit posture

The MCP posture Cull already ships: capability-checked tools (`auth.rs:13-39`), role-scoped tokens with peppered hashes (`tokens.rs:140-167`), an audit log with param redaction (`log_audit` `tokens.rs:325`, `redact_audit_params` `tokens.rs:389`), tables `api_audit_log` (`src-tauri/src/db_core/db.rs:382`) and `mcp_audit_log` (`db.rs:876`), and a token-management UI (`src/lib/components/McpSettings.svelte:248-274`). Plugins slot into the same posture:

- **Enforcement point:** one Tauri command `plugin_invoke(plugin_id, tool, args)` that resolves the plugin's granted permissions and calls the same check shape as `require_capability` (`auth.rs:29`) using `tokens::tool_capability` (`tokens.rs:52`). A plugin is, in effect, a locally-installed actor with a capability set — exactly what an MCP token is (`AuthContext::Authenticated`, `auth.rs:8-10`).
- **Consent surfacing:** install dialog lists the manifest `permissions` with human-readable descriptions *before* download; nothing auto-installs. Mirrors the explicit create-token flow in McpSettings.
- **Audit:** `plugin.install`, `plugin.remove`, and every `plugin_invoke` call are written through the existing `log_audit` path with actor `plugin:<id>`, inheriting redaction and `prune_audit_log` retention (`tokens.rs:349`).
- **Honest v1 limitation, stated up front:** plugins execute in the main webview — there is no iframe/realm sandbox in v1. Checksums establish *integrity* (you run the bytes the registry described), not *confinement*. The Rust permission gate confines privileged operations; DOM access is unconfined. This is the Obsidian/VS Code trust model, documented as such in `docs/plugins-design.md`. Sandboxing is a filed v1.1 item, not a silent gap.
- **No remote code at runtime:** fetch happens once at install, in Rust; load-time re-hash means a tampered on-disk bundle refuses to load.

## 5. Honest sizing (budget ~12 h, hard cap 16 h)

| Work item | Hours |
|---|---|
| **Runtime:** plugin loader (Rust read + hash + blob import), `plugin_invoke` command with capability check, grants table + migration, CSP `script-src` change, `activate(host)` contract + lifecycle | 5.0 |
| **Registry + install UX:** Rust fetch/verify/install commands, Settings → Plugins section (list registry, install with permission dialog, installed list), audit wiring | 4.0 |
| **Proof plugin:** extract `StaticPublishingSettings.svelte` (1122 LOC) into `cull-plugins/cull-publish` with its own svelte-compile build (host API externalized), replace the `module_static_publishing` gate (`+page.svelte:336,485`; `command-palette.ts:719`) with plugin presence, tag + checksum + registry entry | 4.5 |
| **Total** | **13.5** |

13.5 h > 12 h, so per the spec the cut comes from **plugin features, not track time**. Pre-committed cuts, in order, until ≤ 12 h:

1. Cut uninstall/disable UX (~1 h saved) — document "delete `$APPDATA/plugins/<id>`"; install = enabled.
2. Cut update-check / version-upgrade flow (~0.5 h saved) — reinstall is the upgrade path.

→ **12.0 h committed scope.** Riskiest line is the proof-plugin build setup (Svelte compiled outside the app tree); if it slips, that slippage hits the valve below, not Track A/B.

## 6. Day-4 fallback valve

**Trigger:** at end of Day 4, the proof plugin does not install from the live registry and render the publish view end-to-end (one command: fresh app-data dir → install `cull-publish` from `registry.json` → publish view opens and exports a package).

**Fallback:** Track C falls back; the release is not delayed. Publish view ships exactly as it does today — module-gated via `module_static_publishing` (`static_publishing.rs:30`, `+page.svelte:336`) — and all plugin-runtime UI is removed or flagged off. Track C never borrows Track A time.

**Valve-case acceptance (machine-checkable, all four required — "subset" is not interpretable loosely):**

1. `docs/plugins-design.md` exists and matches this spec — check: `test -f docs/plugins-design.md` plus reviewer diff against this document's section headings.
2. A Track C status note records, per component — `runtime bootstrap`, `registry fetch + checksum verify`, `plugin install`, `proof plugin` — one of `working | partial | not-started`; every `working` claim cites a passing test (e.g. `cargo test plugin_` / `npx vitest run src/lib/plugins`) or a reproducible command transcript.
3. Known blockers filed as bd issues tagged `v1.1` — check: `bd list --tag v1.1` returns the blockers named in the status note.
4. No partially-working plugin surface reachable from the released UI — checks: a contract test (pattern of `src/lib/publishing-navigation-contract.test.ts`) asserting the Plugins settings section and any plugin palette commands are absent when the runtime flag is off; `grep -rn "plugin_invoke\|pluginsEnabled" src/` shows every entry point behind the flag; the E2E smoke suite passes with the flag off.

---

## 5. Completeness

### 5.1 Findings not runtime-verified (code-traced / grep-verified only)

The following 21 findings carry `runtime_verified: false` in findings.json. Their mechanisms are traced in source but were not reproduced in a running app; each should be confirmed (or fall) during its fix's verification step:

- **Pre-launch:** CQ-2 (trash failure swallowing — not failure-injected), SEC-001 (export_images confinement — not exploited live), SEC-002 (CSP hosts — grep-absence, needs post-removal smoke run), SEC-003 (.codex asset scope), HYG-005 (.codex scope hygiene angle), UX-05 (toast aria-live — not screen-reader-tested), PERF-02 (sequential import — duration extrapolated), PERF-03 (deep-restore cold start — only single-page query measured), PERF-04 (loupe full-res — explicitly code-traced; unmeetable by construction), PERF-05 (watcher reload churn — event chain traced, not live-triggered).
- **Post-launch:** CQ-3, CQ-4, CQ-5 (grep-verified), CQ-6 (race not reproduced), CQ-7, SEC-004, SEC-005, SEC-006, UX-13, PERF-06 (complexity-derived), PERF-09 (payload computed, not profiled).

Per the decision-sheet rules, evidence_ids may be empty only for rows whose source finding is in this list; in practice every decision-sheet row still carries at least its inventory id.

### 5.2 Lenses that narrowed scope

- **Release-hygiene (fresh-eyes rule):** docs/cull-audit-2026-06-03.md was deliberately NOT read; HYG-004 asserts only its tracked presence, not its content sensitivity. A content pass is still owed before the repo flips public.
- **Performance:** two of the four 10k thresholds in PERF-07 were "expected PASS" by architecture at audit time — thumbnail load p95 (no browser-level measurement) and resident memory (webview RSS not attributable from ps). **Update 2026-06-10:** both measured (bd imageview-dkz.31; webview WebContent attribution solved behaviorally by RSS delta during a driven browse) — see §0 for numbers; resident memory is measured at the real 2,687-image library, not at 10k. PERF-01's hours-scale throughput and PERF-02's "takes hours" are estimates, not benchmarks. PERF-03's breach risk applies only to the deep-restore case; the fresh-start case measured PASS. The deferred 100k target was explicitly out of scope.
- **Security:** all SEC findings are static-analysis/config-review; no live exploitation, no fuzzing, and the MCP HTTP auth path was not probed at runtime. SEC-005's history scan covered credential patterns (AIza/sk-/ghp_/PRIVATE KEY) — other secret formats were not exhaustively patterned.
- **UX:** parts of the runtime verification ran in a browser tab against :1420 (dev server) rather than the packaged app — UX-02's "all invokes failing" condition and UX-11's document.title are dev-context observations; UX-10 used mock detection data. UX-05 was not tested with an actual screen reader.
- **Code-quality:** no scope narrowing beyond §5.1 — six of the seven CQ findings are code-traced/grep-verified rather than runtime-reproduced (CQ-2's trash-failure path was not failure-injected, CQ-6's race is timing-dependent and not reproduced, CQ-4's leak growth was not measured); only CQ-1 was reproduced at runtime.
- **Identity panel:** advocates argued from the inventory + findings corpus, not from independent codebase access; the judge spot-verified Advocate B's citations against the repo but did not re-verify every claim in arguments A and C.
- **Plugin spec (Track C):** deliberately scoped backwards from the single PLUGIN-verdict feature (publish_view); it is not a general plugin-system design and makes no claims about hosting other candidates (e.g. the DEMOTE'd client-tools cluster) without re-sizing.

### 5.3 Inventory contradictions surfaced and resolved during audit

- **Triage coverage:** all 92 inventory ids received triage verdicts (verified by id comparison between inventory.json and the triage table); the decision sheet enumerates all 39 non-KEEP verdicts (re-triage of 2026-06-10 against the agent-native identity).
- **Inventory self-contradictions resolved in favor of grep/runtime evidence:** session_timeline lists Sidebar.svelte as an entry point, but CQ-5's grep shows zero references anywhere in src — the entry point is stale and the CUT verdict stands; command_palette's inventory note says "(Cmd+K)" but UX-07's runtime check shows the palette opens only on Cmd+P/Cmd+Shift+P — the inventory note is wrong; clipboard_monitor and native_menu_bar inventory notes themselves document earlier inventory omissions (both now present as rows and triaged).
- **Dormant-feature confirmations:** ocr_text_extraction, near_duplicate_detection, and color_analysis were each independently grep-confirmed dormant (no UI callers beyond api wrappers/mocks) before receiving CUT verdicts.

## Gate log addendum
- Stage 3 (Track A): codex APPROVE WITH CHANGES — clipboard module_raw exception documented, dkz.12/.13 acceptance aligned to deny.toml. Resolved in 13ee9c8a5.
- Stage 4 (Track B): codex REWORK → APPROVE after evidence — alleged legacy mcp_tokens migration gap refuted (expires_at present since table introduction, 74b3b32d6; migration-safety test covers fresh+legacy); source-contract test style is repo policy with E2E runtime coverage (suite green post-Track-B).
- Stage 5 (Track C): codex CLI hung (CPU-frozen, same signature as prior session) — killed; feature-dev:code-reviewer ran as codex-substitute. Found 2 blockers (plugin_id path traversal; unrestricted file:// bundle download) + 1 major (privileged-capability deny-list). All three fixed (bd imageview-dkz.32, commit f5feeaa38) and re-verified APPROVE: is_safe_plugin_id guards validate_manifest + uninstall; file:// read branch compiled out of release via cfg(test) with https-only scheme_allowed_in_release; DENIED_PLUGIN_PERMISSIONS (tokens:manage/settings:manage/import:write) checked deny-first. Track C valve: GO (proof plugin end-to-end passes).
