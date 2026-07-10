# AI Settings and Agent Access Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Move AI setup out of the sidebar into focused Settings tabs, expose pending-only library AI jobs in the command palette, document Cull skill installation, and ship a rebuilt/reinstalled app.

**Architecture:** SQLite selects pending image IDs by exact model/source; focused Rust commands expose those IDs through the existing Tauri API. `McpSettings.svelte` becomes a tab shell around focused General, AI, and Agent Access components, while a shared settings-navigation store supports deep links. Command-palette orchestration owns library-wide AI jobs and the sidebar retains only detected-object navigation.

**Tech Stack:** Rust, rusqlite, Tauri 2 IPC/events, SvelteKit 5 runes/stores, Vitest, agent-browser E2E, npm/Tauri macOS build tooling.

## Global Constraints

- Never delete, trash, reset, or recreate `~/Library/Application Support/com.glebkalinin.cull/cull.db`.
- API calls use the real Tauri backend; `tauri-mock.ts` remains browser-E2E-only.
- Use Svelte 5 runes and `onclick`/`onkeydown` handlers.
- Use only app.css design tokens; do not hardcode product colors.
- Built-in YOLO and NudeNet downloads stay disabled; weights remain user-supplied and separately licensed.
- Provider secrets remain write-only from Settings and stored through the existing keychain API.
- The Cull UI copies skill-install commands; it never executes package managers or agent installers.
- `Appearance`, `PrivacyDashboard`, and `PluginsSettings` preserve their current feature behavior.
- Run `cargo fmt` inside `src-tauri/`.
- Browser smoke coverage is required for the changed Settings/sidebar/command-palette surfaces.

---

## File Structure

- `src-tauri/src/db_core/schema.sql`, `db.rs`: durable per-image/model completion markers and migration 26.
- `src-tauri/src/db_core/queries/misc.rs`: exact-model pending detection/vision ID queries.
- `src-tauri/src/services/ai.rs`: service boundary for pending-work queries.
- `src-tauri/src/commands/detection.rs`, `vision.rs`, `lib.rs`: Tauri commands and registration.
- `src/lib/api.ts`: typed frontend wrappers.
- `src/lib/settings-navigation.ts`: tab type, selected-tab store, and `openSettings(tab)` helper.
- `src/lib/components/McpSettings.svelte`: modal shell, accessible tab routing, Appearance/Privacy/Plugins mounts.
- `src/lib/components/GeneralSettings.svelte`: app behavior and built-in module toggles.
- `src/lib/components/AiSettings.svelte`: provider credentials, local model readiness, YOLO variant, embedding settings.
- `src/lib/components/AgentAccessSettings.svelte`: skill install copy, optional MCP connection, tokens, MCP config.
- `src/lib/ai-library-jobs.ts`: prerequisite checks, pending-only job orchestration, duplicate-run protection.
- `src/lib/command-palette.ts`: three AI commands and deep-linked Settings subtitle/action.
- `src/lib/components/JobProgressPanel.svelte`: `nsfw-progress` listener and completion labeling.
- `src/lib/components/Sidebar.svelte`: detected-object filters only; remove model setup/actions/state.
- `src/lib/tauri-mock.ts`: E2E responses for new commands/settings.
- `src/lib/*test.ts`: focused unit/source-contract coverage.
- `tests/e2e/smoke.py`: Settings and command discovery journeys.
- `docs/USER_GUIDE.md`: Settings, skill install, optional MCP, and AI job documentation.

---

### Task 1: Pending-only backend queries

**Files:**
- Modify: `src-tauri/src/db_core/schema.sql`
- Modify: `src-tauri/src/db_core/db.rs`
- Modify: `src-tauri/src/db_core/queries/detection.rs`
- Modify: `src-tauri/src/db_core/queries/misc.rs`
- Modify: `src-tauri/src/services/ai.rs`
- Modify: `src-tauri/src/commands/detection.rs`
- Modify: `src-tauri/src/commands/vision.rs`
- Modify: `src-tauri/src/lib.rs`
- Modify: `src/lib/api.ts`
- Test: `src-tauri/src/services/ai.rs`

**Interfaces:**
- Produces: `image_analysis_status(image_id, analysis_kind, model_name, completed_at)` with a composite primary key.
- Produces: `Database::mark_image_analysis_complete(&self, image_id: &str, analysis_kind: &str, model_name: &str) -> Result<()>`.
- Produces: `Database::list_image_ids_missing_detection(&self, model: &str) -> Result<Vec<String>>`.
- Produces: `Database::list_image_ids_missing_vision(&self, source: &str) -> Result<Vec<String>>`.
- Produces: Tauri commands `list_image_ids_missing_detection(model)` and `list_image_ids_missing_vision(source)`.
- Produces: TypeScript wrappers with matching camelCase arguments.

- [ ] **Step 1: Add failing Rust service tests**

Create migrated fixtures with three images, mark a result for only one exact model, and assert exact-model behavior. Include a successful empty detection result to prove that zero-object images are not reprocessed:

```rust
#[test]
fn pending_detection_ids_are_exact_model_and_stable() {
    let (_dir, db) = test_db();
    seed_images(&db, &["a", "b", "c"]);
    db.store_detections("a", "yolo11m", &[]).unwrap();
    db.mark_image_analysis_complete("a", "detection", "yolo11m").unwrap();
    db.mark_image_analysis_complete("b", "detection", "yolo11s").unwrap();

    assert_eq!(
        get_pending_detection_ids(&ctx(&db), "yolo11m").unwrap(),
        vec!["b".to_string(), "c".to_string()],
    );
}

#[test]
fn pending_vision_ids_are_exact_source() {
    let (_dir, db) = test_db();
    seed_images(&db, &["a", "b"]);
    db.mark_image_analysis_complete("a", "vision", "minicpm-v").unwrap();
    assert_eq!(
        get_pending_vision_ids(&ctx(&db), "llava").unwrap(),
        vec!["a".to_string(), "b".to_string()],
    );
}
```

- [ ] **Step 2: Run the new Rust tests and confirm failure**

Run: `cd src-tauri && cargo test --lib services::ai::tests::pending_ -- --nocapture`  
Expected: FAIL because pending-query functions do not exist.

- [ ] **Step 3: Add migration 26 and completion-marker persistence**

Increment `CURRENT_SCHEMA_VERSION` to 26, append migration name `image_analysis_status`, add the table to `schema.sql` and schema invariants, and run this idempotent migration:

```sql
CREATE TABLE IF NOT EXISTS image_analysis_status (
    image_id TEXT NOT NULL REFERENCES images(id) ON DELETE CASCADE,
    analysis_kind TEXT NOT NULL CHECK (analysis_kind IN ('detection', 'vision')),
    model_name TEXT NOT NULL,
    completed_at TEXT NOT NULL,
    PRIMARY KEY (image_id, analysis_kind, model_name)
);
CREATE INDEX IF NOT EXISTS idx_image_analysis_status_lookup
    ON image_analysis_status (analysis_kind, model_name, image_id);

INSERT OR IGNORE INTO image_analysis_status (image_id, analysis_kind, model_name, completed_at)
SELECT image_id, 'detection', model_name, MAX(created_at)
FROM detections GROUP BY image_id, model_name;

INSERT OR IGNORE INTO image_analysis_status (image_id, analysis_kind, model_name, completed_at)
SELECT image_id, 'vision', source, strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
FROM image_metadata GROUP BY image_id, source;
```

Pre-existing scans that produced zero detections cannot be inferred and will be processed once; all future successful zero-result scans receive a marker.

- [ ] **Step 4: Implement SQLite pending queries and service wrappers**

Use `NOT EXISTS` so images with zero stored detections still count as processed when a model row exists:

```rust
pub fn list_image_ids_missing_detection(&self, model: &str) -> Result<Vec<String>> {
    let conn = self.conn.lock().unwrap();
    let mut stmt = conn.prepare(
        "SELECT i.id FROM images i
         WHERE NOT EXISTS (
           SELECT 1 FROM image_analysis_status s
           WHERE s.image_id = i.id AND s.analysis_kind = 'detection' AND s.model_name = ?1
         )
         ORDER BY i.imported_at, i.id",
    )?;
    stmt.query_map([model], |row| row.get(0))?
        .collect::<Result<Vec<_>>>()
}
```

For vision, query the same status table with `analysis_kind = 'vision'` and preserve the same stable ordering. Update successful YOLO, NudeNet, and vision command paths to call `mark_image_analysis_complete` after result persistence, including when the result list/map is empty. Do not mark failed images complete.

- [ ] **Step 5: Add Tauri commands, registration, and TypeScript wrappers**

```rust
#[tauri::command]
pub async fn list_image_ids_missing_detection(
    state: State<'_, AppState>,
    model: String,
) -> Result<Vec<String>, String> {
    let ctx = crate::services::ServiceContext::from_app_state(&state, None);
    crate::services::ai::get_pending_detection_ids(&ctx, &model).map_err(|e| e.to_string())
}
```

```ts
export async function listImageIdsMissingDetection(model: string): Promise<string[]> {
    return invoke('list_image_ids_missing_detection', { model });
}
export async function listImageIdsMissingVision(source: string): Promise<string[]> {
    return invoke('list_image_ids_missing_vision', { source });
}
```

- [ ] **Step 6: Run migration, query, and command tests and format**

Run: `cd src-tauri && cargo fmt && cargo test --lib db_core::db::migration_safety_tests && cargo test --lib services::ai::tests::pending_`  
Expected: PASS.

- [ ] **Step 7: Commit the backend slice**

```bash
git add src-tauri/src/db_core/schema.sql src-tauri/src/db_core/db.rs src-tauri/src/db_core/queries/detection.rs src-tauri/src/db_core/queries/misc.rs src-tauri/src/services/ai.rs src-tauri/src/commands/detection.rs src-tauri/src/commands/vision.rs src-tauri/src/lib.rs src/lib/api.ts
git commit -m "feat(ai): query pending library model work"
```

---

### Task 2: Settings navigation and component boundaries

**Files:**
- Create: `src/lib/settings-navigation.ts`
- Create: `src/lib/components/GeneralSettings.svelte`
- Modify: `src/lib/components/McpSettings.svelte`
- Modify: `src/lib/stores.ts`
- Modify: Settings callers in `src/lib/command-palette.ts`, `src/lib/components/EmbeddingExplorer.svelte`, and `src/routes/+page.svelte`
- Test: `src/lib/settings-navigation.test.ts`
- Test: `src/lib/components/settings-tabs-contract.test.ts`

**Interfaces:**
- Produces: `type SettingsTab = 'general' | 'appearance' | 'ai' | 'agent-access' | 'privacy' | 'plugins'`.
- Produces: `settingsTab: Writable<SettingsTab>` and `openSettings(tab?: SettingsTab): void`.
- Consumes: existing `settingsOpen` store.

- [ ] **Step 1: Write failing navigation tests**

```ts
it('opens Settings at an explicit tab', () => {
  openSettings('ai');
  expect(get(settingsOpen)).toBe(true);
  expect(get(settingsTab)).toBe('ai');
});

it('defaults generic opens to General', () => {
  openSettings();
  expect(get(settingsTab)).toBe('general');
});
```

- [ ] **Step 2: Run the tests and confirm failure**

Run: `npx vitest run src/lib/settings-navigation.test.ts`  
Expected: FAIL because the module does not exist.

- [ ] **Step 3: Implement the navigation helper**

```ts
export type SettingsTab = 'general' | 'appearance' | 'ai' | 'agent-access' | 'privacy' | 'plugins';
export const settingsTab = writable<SettingsTab>('general');
export function openSettings(tab: SettingsTab = 'general') {
    settingsTab.set(tab);
    settingsOpen.set(true);
}
```

- [ ] **Step 4: Extract General settings**

Move only ordinary behavior/module state, loading, and handlers into `GeneralSettings.svelte`. It receives no MCP, token, provider, or model state. Preserve existing store updates and toasts for module flags.

- [ ] **Step 5: Turn `McpSettings.svelte` into the accessible shell**

Render the six tabs in approved order. Use `role="tablist"`, `role="tab"`, `aria-selected`, associated `role="tabpanel"`, and Left/Right/Home/End keyboard movement. Keep Appearance, Privacy, and Plugins mounts. Mount `GeneralSettings` for General.

- [ ] **Step 6: Update generic callers to use `openSettings()`**

Embedding prerequisites must call `openSettings('ai')`; menu and generic app Settings commands call `openSettings()`.

- [ ] **Step 7: Run navigation and Settings contract tests**

Run: `npx vitest run src/lib/settings-navigation.test.ts src/lib/components/settings-tabs-contract.test.ts`  
Expected: PASS.

- [ ] **Step 8: Commit the shell refactor**

```bash
git add src/lib/settings-navigation.ts src/lib/settings-navigation.test.ts src/lib/components/GeneralSettings.svelte src/lib/components/McpSettings.svelte src/lib/components/settings-tabs-contract.test.ts src/lib/stores.ts src/lib/command-palette.ts src/lib/components/EmbeddingExplorer.svelte src/routes/+page.svelte
git commit -m "refactor(settings): split tab shell and general settings"
```

---

### Task 3: AI Settings tab

**Files:**
- Create: `src/lib/components/AiSettings.svelte`
- Modify: `src/lib/components/McpSettings.svelte`
- Modify: `src/lib/onboarding.ts`
- Test: `src/lib/components/ai-settings-contract.test.ts`

**Interfaces:**
- Consumes: existing key APIs, embedding settings APIs, `isYoloAvailable`, `isNudenetAvailable`, `checkOllama`.
- Produces: persisted `yolo_variant` with values `nano | small | medium`, default `medium`.

- [ ] **Step 1: Write failing AI ownership tests**

Assert the component contains blocks in this source order: `Provider Credentials`, `Local Models`, `Embedding Models`; contains all providers; reads/writes `yolo_variant`; links `MODEL_SETUP_GUIDE_URL`; and contains no `detectObjects`, `detectNsfw`, or `analyzeImages` calls.

- [ ] **Step 2: Run the contract test and confirm failure**

Run: `npx vitest run src/lib/components/ai-settings-contract.test.ts`  
Expected: FAIL because `AiSettings.svelte` does not exist.

- [ ] **Step 3: Implement Provider Credentials**

Move the current write-only key state and validation/removal behavior intact. Keep secure-storage copy and explicit Connected/Invalid/Validating/Error states.

- [ ] **Step 4: Implement Local Models**

Load the saved variant and readiness in parallel. Persist variant changes through `setAppSetting('yolo_variant', variant)`. Render text states `Ready`, `Not installed`, and `Service unavailable`, with setup guidance and separately-licensed weight copy.

- [ ] **Step 5: Implement Embedding Models**

Move existing Cohere/OpenAI/Ollama embedding settings and blur persistence intact. Do not conflate `ollama_embedding_*` with vision’s `ollama_url`/`ollama_model`.

- [ ] **Step 6: Mount the AI tab and remove moved state from the shell**

`McpSettings.svelte` must contain `<AiSettings />` only in the `ai` panel and no longer load provider/model values itself.

- [ ] **Step 7: Run AI Settings tests and Svelte check**

Run: `npx vitest run src/lib/components/ai-settings-contract.test.ts && npm run check`  
Expected: PASS with zero Svelte errors/warnings.

- [ ] **Step 8: Commit AI Settings**

```bash
git add src/lib/components/AiSettings.svelte src/lib/components/McpSettings.svelte src/lib/components/ai-settings-contract.test.ts src/lib/onboarding.ts
git commit -m "feat(settings): add AI configuration tab"
```

---

### Task 4: Agent Access tab and skill installation guidance

**Files:**
- Create: `src/lib/components/AgentAccessSettings.svelte`
- Modify: `src/lib/components/McpSettings.svelte`
- Test: `src/lib/components/agent-access-settings.test.ts`

**Interfaces:**
- Consumes: existing MCP/token APIs and `MCP_CONFIG_SNIPPET`.
- Produces: copyable npx, Claude Code, Codex, and generic-agent instructions.

- [ ] **Step 1: Write failing source/copy tests**

Assert exact presence of:

```text
npx skills add glebis/claude-skills --skill cull
claude plugin marketplace add glebis/claude-skills
claude plugin install cull@glebis-skills
Use $skill-installer to install the Cull skill from https://github.com/glebis/claude-skills/tree/main/cull
```

Also assert the source `SKILL.md` URL, “Optional” MCP copy, token controls, and absence of shell/process execution APIs.

- [ ] **Step 2: Run the test and confirm failure**

Run: `npx vitest run src/lib/components/agent-access-settings.test.ts`  
Expected: FAIL because the component does not exist.

- [ ] **Step 3: Implement the skill-install block**

Use a local selected-method rune, a constant method array, and one copy button with method-specific `aria-label`. Copy only via `navigator.clipboard.writeText`; never invoke commands.

- [ ] **Step 4: Move MCP connection, tokens, and config**

Preserve HTTP port validation, token expiry defaults, one-time secret reveal, rotate/revoke error handling, and config copy behavior. Label MCP Connection as optional and retain loopback security language.

- [ ] **Step 5: Mount Agent Access and remove moved state from the shell**

Render `<AgentAccessSettings />` only for `agent-access`.

- [ ] **Step 6: Run tests and Svelte check**

Run: `npx vitest run src/lib/components/agent-access-settings.test.ts src/lib/components/mcp-settings-expiry.test.ts src/lib/components/mcp-settings-token-errors.test.ts && npm run check`  
Expected: PASS.

- [ ] **Step 7: Commit Agent Access**

```bash
git add src/lib/components/AgentAccessSettings.svelte src/lib/components/McpSettings.svelte src/lib/components/agent-access-settings.test.ts src/lib/components/mcp-settings-expiry.test.ts src/lib/components/mcp-settings-token-errors.test.ts
git commit -m "feat(settings): add agent access and Cull skill setup"
```

---

### Task 5: Command-palette AI library jobs

**Files:**
- Create: `src/lib/ai-library-jobs.ts`
- Create: `src/lib/ai-library-jobs.test.ts`
- Modify: `src/lib/command-palette.ts`
- Modify: `src/lib/components/JobProgressPanel.svelte`
- Modify: `src/lib/stores.ts`
- Test: `src/lib/command-palette.test.ts`

**Interfaces:**
- Produces: `runObjectDetectionJob()`, `runSensitiveContentJob()`, `runImageDescriptionJob()`.
- Consumes: pending-ID APIs from Task 1 and `openSettings('ai')` from Task 2.

- [ ] **Step 1: Write failing orchestration tests with injected dependencies**

Cover exact-model pending lookup, no-work toast, missing prerequisite with Open AI Settings action, processing only pending IDs, partial failure message, and duplicate-run rejection. Use a dependency interface rather than mocking `invoke` globally.

- [ ] **Step 2: Run orchestration tests and confirm failure**

Run: `npx vitest run src/lib/ai-library-jobs.test.ts`  
Expected: FAIL because the module does not exist.

- [ ] **Step 3: Implement job orchestration**

Map variants exactly:

```ts
export const yoloModelName = (variant: YoloVariant) =>
    variant === 'nano' ? 'yolo11n' : variant === 'small' ? 'yolo11s' : 'yolo11m';
```

Use module-level per-kind running guards with `finally` cleanup. Query pending IDs before calling existing processors. Refresh the current scope and dispatch a `detected-classes-changed` event after YOLO completion.

- [ ] **Step 4: Register three AI commands**

Add exact titles:

- `Detect Objects in Library`
- `Scan Library for Sensitive Content`
- `Describe Images in Library`

All use category `AI`, descriptive subtitles, searchable provider/model keywords, and the focused helpers.

- [ ] **Step 5: Add `nsfw-progress` to the Jobs panel**

Listen for `nsfw-progress`, upsert kind `nsfw`, and unsubscribe with the existing listeners. Preserve textual status semantics.

- [ ] **Step 6: Run focused frontend tests**

Run: `npx vitest run src/lib/ai-library-jobs.test.ts src/lib/command-palette.test.ts src/lib/components/job-progress-panel.test.ts`  
Expected: PASS.

- [ ] **Step 7: Commit the action surface**

```bash
git add src/lib/ai-library-jobs.ts src/lib/ai-library-jobs.test.ts src/lib/command-palette.ts src/lib/command-palette.test.ts src/lib/components/JobProgressPanel.svelte src/lib/components/job-progress-panel.test.ts src/lib/stores.ts
git commit -m "feat(ai): add pending-only library commands"
```

---

### Task 6: Sidebar cleanup and detected-object filters

**Files:**
- Modify: `src/lib/components/Sidebar.svelte`
- Modify: `src/lib/onboarding.ts`
- Modify: `src/lib/sidebar-audit-contract.test.ts`
- Modify: `src/lib/audit-ui-contract.test.ts`
- Modify: `src/lib/components/first-run-onboarding.test.ts`

**Interfaces:**
- Consumes: existing `detectedClasses` store and class filtering behavior.
- Removes: all sidebar model readiness, setup links, progress counts, and batch handlers.

- [ ] **Step 1: Rewrite failing sidebar contracts**

Assert `Sidebar.svelte` contains no `AI MODELS`, `Detect objects`, `Describe images`, `isYoloAvailable`, `isNudenetAvailable`, or `checkOllama`. Assert `Detected objects` appears inside the Filters section and class buttons still call `filterByClass`.

- [ ] **Step 2: Run sidebar tests and confirm failure**

Run: `npx vitest run src/lib/sidebar-audit-contract.test.ts src/lib/audit-ui-contract.test.ts src/lib/components/first-run-onboarding.test.ts`  
Expected: FAIL against the old block.

- [ ] **Step 3: Remove model state/actions and relocate detected filters**

Keep `loadDetectedClasses` and `filterByClass`, but decouple them from `loadAiState`. Load classes on mount and refresh on `detected-classes-changed`. Render the conditional subsection below minimum-size/show-missing controls.

- [ ] **Step 4: Remove obsolete onboarding expansion helpers**

Delete `resolveAiSectionExpanded` and its tests if no other caller exists; retain `MODEL_SETUP_GUIDE_URL` for AI Settings.

- [ ] **Step 5: Run sidebar tests and Svelte check**

Run: `npx vitest run src/lib/sidebar-audit-contract.test.ts src/lib/audit-ui-contract.test.ts src/lib/components/first-run-onboarding.test.ts && npm run check`  
Expected: PASS.

- [ ] **Step 6: Commit sidebar cleanup**

```bash
git add src/lib/components/Sidebar.svelte src/lib/onboarding.ts src/lib/sidebar-audit-contract.test.ts src/lib/audit-ui-contract.test.ts src/lib/components/first-run-onboarding.test.ts
git commit -m "refactor(sidebar): keep AI filters and remove model controls"
```

---

### Task 7: E2E mock, browser smoke, and documentation

**Files:**
- Modify: `src/lib/tauri-mock.ts`
- Modify: `tests/e2e/smoke.py`
- Modify: `docs/USER_GUIDE.md`
- Modify: relevant source contracts referencing Settings tabs/subtitles.

**Interfaces:**
- Produces browser-only deterministic readiness/pending responses.
- Documents the final user workflow.

- [ ] **Step 1: Add mock responses for pending queries and readiness**

Return stable IDs for each model/source and preserve the rule that `tauri-mock.ts` is never imported by production code.

- [ ] **Step 2: Extend browser smoke journeys**

Add a journey that opens Settings, selects AI, verifies block order, selects Agent Access, verifies skill/MCP sections, closes Settings, opens Command-K, and finds all three command titles. Extend sidebar coverage to verify AI Models is absent and Detected objects remains usable when mock counts exist.

- [ ] **Step 3: Update the User Guide**

Document the six tabs, provider-first AI layout, exact skill installation alternatives, optional MCP, and three pending-only library commands. State that YOLO/NudeNet weights remain separately licensed and user-supplied.

- [ ] **Step 4: Run frontend and E2E suites**

Run: `npm run preflight:quick`  
Expected: PASS.

Run prerequisites from `AGENTS.md`, then: `bash tests/e2e/run-e2e.sh`  
Expected: PASS including the new Settings/command/sidebar journey.

- [ ] **Step 5: Commit E2E and docs**

```bash
git add src/lib/tauri-mock.ts tests/e2e/smoke.py docs/USER_GUIDE.md src/lib/*test.ts
git commit -m "test: cover AI settings and library commands"
```

---

### Task 8: Full verification, build, reinstall, and landing

**Files:**
- Verify: all changed files
- Artifact: `src-tauri/target/release/bundle/macos/Cull.app`
- Install: `/Applications/Cull.app`

**Interfaces:**
- Produces: pushed implementation branch and installed current build.

- [ ] **Step 1: Run spec coverage audit**

Compare every section of `docs/superpowers/specs/2026-07-10-ai-settings-and-agent-access-design.md` with code/tests. Use `rg` to prove old sidebar copy/actions are absent and exact skill/command copy is present.

- [ ] **Step 2: Run full repository gates**

Run: `CARGO_TARGET_DIR=/Users/glebkalinin/ai_projects/cull/src-tauri/target npm run preflight:full`  
Expected: PASS. Existing clippy warnings may print but cannot introduce compilation/test/fmt failures.

- [ ] **Step 3: Run the license audit and production build gate**

Run: `npm run audit:licenses`  
Expected: PASS.

Run: `CARGO_TARGET_DIR=/Users/glebkalinin/ai_projects/cull/src-tauri/target npm run clean-machine-dmg-gate:build-install`  
Expected: build succeeds, DMG verification succeeds, `/Applications/Cull.app` is replaced from the built DMG, and launch smoke succeeds.

- [ ] **Step 4: Runtime-smoke the installed app**

Launch `/Applications/Cull.app`, verify its bundle/version and process path, then use the browser/Tauri UI where available to confirm Settings opens with AI and Agent Access and the three commands are discoverable. Do not mutate or reset `cull.db`.

- [ ] **Step 5: Commit any final verification-only corrections**

If fixes were required, rerun their focused tests and commit them. Otherwise leave the tree clean.

- [ ] **Step 6: Land and push**

Run: `CARGO_TARGET_DIR=/Users/glebkalinin/ai_projects/cull/src-tauri/target CULL_PREFLIGHT_SKIP_E2E=1 npm run land`  
Expected: clean worktree, full gates pass, branch rebases, and push succeeds. E2E is skipped only here because it was run explicitly in Step 4.

- [ ] **Step 7: Final completion audit**

Verify:

```bash
git status -sb
git rev-list --left-right --count @{upstream}...HEAD
test -d /Applications/Cull.app
mdls -name kMDItemVersion /Applications/Cull.app
```

Expected: clean/synchronized branch, `0 0`, installed app exists, and version is reported.
