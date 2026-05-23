# Rejected Visibility Phase 1 Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Hide rejected images from normal curation by default and add a persisted View -> Show Rejected toggle.

**Architecture:** Add a frontend `showRejected` view-state store, mirror it into the native View menu, and pass an `includeRejected` flag through Tauri APIs to Rust query methods. Filtering must happen in Rust queries so paging, counts, folder lists, collection counts, and smart collection counts match the visible working set. The only exception is explicit rejected scopes: smart collection filters that directly request `decision = reject` still include rejected images even when `showRejected` is false.

**Tech Stack:** SvelteKit 5 stores and menu bridge, TypeScript API wrapper, Tauri 2 commands, Rust rusqlite query methods, Vitest, Rust unit tests

---

## Local Audit Notes

- Do not implement this as render-only filtering. `src/lib/image-loading.ts` pages by backend offset, so frontend-only filtering would create short pages, incorrect `hasMore`, bad focus movement, and mismatched counts.
- Add `showRejected` to the cache key. Without that, toggling the menu can reuse the wrong cached image set.
- The Rejected preset already exists as `Rejects` in `seed_preset_collections`; keep that explicit rejected scope visible.
- Pressing `x` must remove the image from the current working set when `showRejected` is false. Query filtering alone only applies after a reload.
- `listFolders`, `listCollections`, `listSmartCollections`, `getImageCount`, detected-class counts, and loaded image queries all need the visibility flag or the sidebar/status counts will disagree with the grid.

### Task 1: Frontend State, Persistence, and Menu Contract

**Files:**
- Modify: `src/lib/stores.ts`
- Modify: `src/lib/persistence.ts`
- Modify: `src/lib/api.ts`
- Modify: `src-tauri/src/menu.rs`
- Modify: `src/lib/menu.ts`
- Test: `src/lib/stores.test.ts`
- Test: `src/lib/tauri-command-contract.test.ts`

- [ ] **Step 1: Add the frontend store**

In `src/lib/stores.ts`, add the store beside the existing view visibility stores:

```ts
export const sidebarVisible = writable<boolean>(true);
export const zenMode = writable<boolean>(false);
export const showRejected = writable<boolean>(false);
```

- [ ] **Step 2: Persist the store**

In `src/lib/persistence.ts`, import `showRejected`, add `showRejected: boolean` to `PersistedState`, save `showRejected: get(showRejected)`, and restore with a backward-compatible default:

```ts
showRejected.set(state.showRejected ?? false);
```

- [ ] **Step 3: Extend menu state types**

In `src/lib/api.ts`, extend `MenuStatePayload`:

```ts
export interface MenuStatePayload {
    viewMode: string;
    sidebarVisible: boolean;
    showRejected: boolean;
    hasFocusedImage: boolean;
    selectedCount: number;
}
```

In `src-tauri/src/menu.rs`, extend `MenuStatePayload`:

```rust
pub struct MenuStatePayload {
    view_mode: String,
    sidebar_visible: bool,
    show_rejected: bool,
    has_focused_image: bool,
    selected_count: usize,
}
```

- [ ] **Step 4: Add the native menu item**

In `src-tauri/src/menu.rs`, append the item after `toggle_sidebar`:

```rust
view_menu.append(&CheckMenuItem::with_id(
    app,
    "view_show_rejected",
    "Show Rejected",
    true,
    false,
    None::<&str>,
)?)?;
```

In `update_menu_state`, mirror the store:

```rust
set_menu_item_checked(&app, "view_show_rejected", state.show_rejected)?;
```

In `handle_menu_event`, include `view_show_rejected` in the forwarded action list.

- [ ] **Step 5: Wire the frontend menu handler**

In `src/lib/menu.ts`, import `showRejected`, `invalidateImageCache`, and `loadImagesForCurrentScope`. Handle the menu action:

```ts
case 'view_show_rejected':
    showRejected.update((value) => !value);
    invalidateImageCache();
    loadImagesForCurrentScope({ resetFocus: false, force: true }).catch((e) => {
        showToast('Failed to reload images', { detail: String(e), type: 'error', duration: 8000 });
    });
    break;
```

Include `showRejected: get(showRejected)` in `updateMenuState` and subscribe to `showRejected` in `initMenu`.

- [ ] **Step 6: Add a focused frontend state test**

In `src/lib/stores.test.ts`, add:

```ts
import { showRejected } from './stores';

describe('showRejected', () => {
    afterEach(() => {
        showRejected.set(false);
    });

    it('defaults to hiding rejected images', () => {
        expect(get(showRejected)).toBe(false);
    });
});
```

- [ ] **Step 7: Run frontend contract tests**

Run:

```bash
npm test -- src/lib/stores.test.ts src/lib/tauri-command-contract.test.ts
```

Expected: tests pass. If the contract test fails, register any newly invoked Tauri command in `src-tauri/src/lib.rs`; this phase should not need a new command.

- [ ] **Step 8: Commit task 1**

```bash
git add src/lib/stores.ts src/lib/persistence.ts src/lib/api.ts src-tauri/src/menu.rs src/lib/menu.ts src/lib/stores.test.ts
git commit -m "feat: add rejected visibility menu state"
```

### Task 2: Rust Query Filtering

**Files:**
- Modify: `src-tauri/src/db_core/smart_collections.rs`
- Modify: `src-tauri/src/db_core/db.rs`
- Modify: `src-tauri/src/services/library.rs`
- Modify: `src-tauri/src/services/curation.rs`
- Modify: `src-tauri/src/services/ai.rs`
- Modify: `src-tauri/src/commands/library.rs`
- Modify: `src-tauri/src/commands/collections.rs`
- Modify: `src-tauri/src/commands/smart_collections.rs`
- Modify: `src-tauri/src/commands/detection.rs`
- Test: `src-tauri/src/db_core/db.rs`

- [ ] **Step 1: Add smart filter intent helper**

In `src-tauri/src/db_core/smart_collections.rs`, add:

```rust
impl FilterNode {
    pub fn explicitly_requests_rejected(&self) -> bool {
        match self {
            FilterNode::Group { children, .. } => {
                children.iter().any(FilterNode::explicitly_requests_rejected)
            }
            FilterNode::Not { .. } => false,
            FilterNode::Rule { field, op, value } => {
                if !matches!(field, Field::Decision) {
                    return false;
                }
                match (op, value) {
                    (RuleOp::Eq, FilterValue::String(value)) => value == "reject",
                    (RuleOp::In, FilterValue::StringArray(values)) => {
                        values.iter().any(|value| value == "reject")
                    }
                    _ => false,
                }
            }
        }
    }
}
```

- [ ] **Step 2: Add DB visibility helpers**

In `src-tauri/src/db_core/db.rs`, add private helpers near other DB helpers:

```rust
fn rejected_visibility_clause(include_rejected: bool) -> &'static str {
    if include_rejected {
        "1=1"
    } else {
        "(s.decision IS NULL OR s.decision != 'reject')"
    }
}

fn should_include_rejected_for_filter(filter: &FilterNode, include_rejected: bool) -> bool {
    include_rejected || filter.explicitly_requests_rejected()
}
```

- [ ] **Step 3: Update image query signatures**

Update DB method signatures:

```rust
pub fn list_images(&self, limit: u32, offset: u32, include_rejected: bool) -> Result<Vec<ImageWithFile>>
pub fn list_images_by_folder(&self, folder: &str, limit: u32, offset: u32, include_rejected: bool) -> Result<Vec<ImageWithFile>>
pub fn list_images_filtered(&self, min_width: Option<u32>, min_height: Option<u32>, limit: u32, offset: u32, include_rejected: bool) -> Result<Vec<ImageWithFile>>
pub fn list_collection_images_page(&self, collection_id: &str, limit: u32, offset: u32, include_rejected: bool) -> Result<Vec<ImageWithFile>>
pub fn image_count(&self, include_rejected: bool) -> Result<u32>
pub fn list_folders(&self, include_rejected: bool) -> Result<Vec<(String, u32)>>
pub fn list_collections(&self, include_rejected: bool) -> Result<Vec<(String, String, u32)>>
pub fn list_smart_collections(&self, include_rejected: bool) -> Result<Vec<SmartCollection>>
pub fn count_smart_collection(&self, filter_json: &str, include_rejected: bool) -> Result<i64>
pub fn evaluate_smart_collection_page(&self, filter_json: &str, limit: Option<u32>, offset: Option<u32>, include_rejected: bool) -> Result<Vec<ImageWithFile>>
pub fn list_images_by_class(&self, class_name: &str, limit: u32, offset: u32, include_rejected: bool) -> Result<Vec<ImageWithFile>>
pub fn count_by_class(&self, class_name: &str, include_rejected: bool) -> Result<u32>
```

For each SQL query that joins `selections s`, add the clause:

```sql
AND (s.decision IS NULL OR s.decision != 'reject')
```

when `include_rejected` is false. For queries with an existing `WHERE`, append `AND {clause}`. For `list_images`, introduce `WHERE {clause}` before `GROUP BY`.

- [ ] **Step 4: Preserve explicit rejected smart collections**

In smart collection count/evaluation methods, parse the filter first, then compute:

```rust
let include_rejected = should_include_rejected_for_filter(&filter, include_rejected);
let decision_clause = rejected_visibility_clause(include_rejected);
```

Build the final `WHERE` as:

```rust
WHERE ({}) AND {}
```

with `where_clause` and `decision_clause`.

- [ ] **Step 5: Thread command/service flags**

Thread `include_rejected: Option<bool>` through Tauri commands and plain `bool` through services. Use `include_rejected.unwrap_or(false)` at command boundaries.

Example command boundary:

```rust
pub async fn list_images(
    state: State<'_, AppState>,
    limit: u32,
    offset: u32,
    include_rejected: Option<bool>,
) -> Result<Vec<ImageWithFile>, String> {
    let ctx = ServiceContext::from_app_state(&state, None);
    svc::list_images(
        &ctx,
        Pagination::clamped(offset, limit),
        include_rejected.unwrap_or(false),
    )
    .map_err(|e| e.to_string())
}
```

- [ ] **Step 6: Add Rust tests**

In `src-tauri/src/db_core/db.rs`, add tests using the existing test image helpers:

```rust
#[test]
fn test_list_images_hides_rejected_by_default() {
    let db = create_test_db();
    insert_test_image(&db, "accepted");
    insert_test_image(&db, "rejected");
    db.set_decision("accepted", "accept").unwrap();
    db.set_decision("rejected", "reject").unwrap();

    let visible = db.list_images(20, 0, false).unwrap();
    assert_eq!(visible.iter().map(|img| img.image.id.as_str()).collect::<Vec<_>>(), vec!["accepted"]);

    let all = db.list_images(20, 0, true).unwrap();
    assert_eq!(all.len(), 2);
}

#[test]
fn test_rejects_smart_collection_includes_rejected_when_hidden() {
    let db = create_test_db();
    insert_test_image(&db, "rejected");
    db.set_decision("rejected", "reject").unwrap();

    let filter = r#"{"type":"rule","field":"decision","op":"eq","value":"reject"}"#;
    let images = db.evaluate_smart_collection_page(filter, Some(20), Some(0), false).unwrap();
    assert_eq!(images.len(), 1);
    assert_eq!(images[0].image.id, "rejected");
}
```

- [ ] **Step 7: Run Rust tests**

Run:

```bash
cd src-tauri && cargo test db_core::db::tests::test_list_images_hides_rejected_by_default db_core::db::tests::test_rejects_smart_collection_includes_rejected_when_hidden
```

Expected: both tests pass.

- [ ] **Step 8: Commit task 2**

```bash
git add src-tauri/src/db_core/smart_collections.rs src-tauri/src/db_core/db.rs src-tauri/src/services src-tauri/src/commands
git commit -m "feat: filter rejected images in queries"
```

### Task 3: Frontend API and Loader Wiring

**Files:**
- Modify: `src/lib/api.ts`
- Modify: `src/lib/image-loading.ts`
- Modify: `src/lib/menu.ts`
- Modify: `src/lib/components/Sidebar.svelte`
- Modify: `src/lib/components/ContextMenu.svelte`
- Modify: `src/lib/tauri-mock.ts`
- Test: `src/lib/image-loading.test.ts`

- [ ] **Step 1: Update API wrappers**

In `src/lib/api.ts`, add optional `includeRejected = false` parameters and pass camelCase args:

```ts
export async function listImages(limit: number, offset: number, includeRejected = false): Promise<ImageWithFile[]> {
    return invoke<ImageWithFile[]>('list_images', { limit, offset, includeRejected });
}

export async function getImageCount(includeRejected = false): Promise<number> {
    return invoke<number>('get_image_count', { includeRejected });
}
```

Repeat the same pattern for `listFolders`, `listImagesByFolder`, `listImagesFiltered`, `listCollectionImages`, `listSmartCollections`, `countSmartCollection`, `evaluateSmartCollection`, `listImagesByDetectedClass`, and `countByDetectedClass`.

- [ ] **Step 2: Add rejected state to image-loading scope keys**

In `src/lib/image-loading.ts`, import `showRejected` and include it in `scopeKey`:

```ts
const rejectedKey = get(showRejected) ? 'with-rejected' : 'without-rejected';
```

Append `rejectedKey` to every returned key.

- [ ] **Step 3: Pass includeRejected during page fetches**

In `fetchPage`, compute:

```ts
const includeRejected = get(showRejected);
```

Pass it to every API call. Smart collection calls still receive `includeRejected`; Rust handles explicit rejected filters.

- [ ] **Step 4: Refresh count with visibility**

Update `refreshImageCount`:

```ts
export async function refreshImageCount() {
    totalCount.set(await getImageCount(get(showRejected)));
}
```

- [ ] **Step 5: Update sidebar and menu folder/list refreshes**

In `Sidebar.svelte`, import `showRejected` and pass `$showRejected` to count/list functions. In `ContextMenu.svelte`, call `listFolders(true)` for move destinations so target folders are not hidden merely because they currently contain rejected images.

- [ ] **Step 6: Update mock handlers**

In `src/lib/tauri-mock.ts`, add rejected mock data and respect `includeRejected`:

```ts
function visibleMockImages(includeRejected = false) {
    return Array.from({ length: 20 }, (_, i) => makeMockImage(i))
        .filter(img => includeRejected || img.selection?.decision !== 'reject');
}
```

Use it in `list_images`, `get_image_count`, and smart collection mock handlers.

- [ ] **Step 7: Add loader unit tests**

Create `src/lib/image-loading.test.ts` with mocked API functions and stores. Test that `loadImagesForCurrentScope()` calls `listImages(200, 0, false)` by default and `listImages(200, 0, true)` after `showRejected.set(true)`.

- [ ] **Step 8: Run frontend tests**

Run:

```bash
npm test -- src/lib/image-loading.test.ts src/lib/stores.test.ts src/lib/tauri-command-contract.test.ts
```

Expected: all tests pass.

- [ ] **Step 9: Commit task 3**

```bash
git add src/lib/api.ts src/lib/image-loading.ts src/lib/menu.ts src/lib/components/Sidebar.svelte src/lib/components/ContextMenu.svelte src/lib/tauri-mock.ts src/lib/image-loading.test.ts
git commit -m "feat: wire rejected visibility into image loading"
```

### Task 4: Reject Gesture Refresh and Verification

**Files:**
- Modify: `src/lib/keys.ts`
- Modify: `src/lib/command-palette.ts`
- Modify: `src/lib/components/ContextMenu.svelte`
- Test: `tests/e2e/smoke.py`

- [ ] **Step 1: Reload after hidden reject**

In each decision path that can set `reject`, after local selection state is updated, run:

```ts
if (decision === 'reject' && !get(showRejected)) {
    await loadImagesForCurrentScope({
        resetFocus: false,
        force: true,
        invalidateCache: true,
        minItems: Math.max(0, get(images).length - 1),
    });
    focusedIndex.update(index => Math.max(0, Math.min(index, get(images).length - 1)));
}
```

Apply this to `handleDecision` in `src/lib/keys.ts`, `setFocusedDecision` in `src/lib/command-palette.ts`, and `handleDecision` in `src/lib/components/ContextMenu.svelte`.

- [ ] **Step 2: Add E2E coverage**

In `tests/e2e/smoke.py`, update the accept/reject test so it records the focused image label, presses `x`, asserts the grid no longer shows that focused item in normal mode, then toggles **Show Rejected** through the app menu or frontend event path and asserts the rejected badge is visible again.

- [ ] **Step 3: Run quality gates**

Run:

```bash
npm test
npm run check
cd src-tauri && cargo test
```

Expected: all pass. If E2E dependencies are already running, also run:

```bash
bash tests/e2e/run-e2e.sh
```

Expected: smoke suite passes.

- [ ] **Step 4: Commit task 4**

```bash
git add src/lib/keys.ts src/lib/command-palette.ts src/lib/components/ContextMenu.svelte tests/e2e/smoke.py
git commit -m "feat: hide rejected images after rejection"
```

### Task 5: Final Integration

**Files:**
- Verify all modified files

- [ ] **Step 1: Run full status and review**

Run:

```bash
git status --short
git log --oneline -5
```

Expected: only intentional files are modified or committed; unrelated pre-existing changes are not reverted.

- [ ] **Step 2: Push**

Run:

```bash
git fetch origin
git rev-list --left-right --count HEAD...origin/main
git push
git status --short --branch
```

Expected: push succeeds and branch is up to date with `origin/main`, except for unrelated uncommitted user files.
