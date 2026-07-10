# User-Facing Accessibility Test Coverage Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Replace weak source contracts with rendered and browser-level tests for modal accessibility, Trash safety, agent approval, and Escape behavior.

**Architecture:** Add a jsdom-backed Svelte test layer for component behavior while retaining the existing Node utility tests. Exercise complete user journeys in the existing Playwright/Tauri-mock smoke suite, adding only deterministic mock state needed for Trash and undo.

**Tech Stack:** Svelte 5, Vitest 4, jsdom, Testing Library for Svelte, Testing Library user-event, Testing Library jest-dom matchers, Python Playwright, Tauri browser mock.

## Global Constraints

- Never import `src/lib/tauri-mock.ts` from `src/lib/api.ts` or a component.
- Browser tests must run with `CULL_E2E_MOCK=1` and must not access the real filesystem or `cull.db`.
- New component tests must query by role, accessible name, or visible label rather than source substrings.
- Production changes are allowed only when a failing behavioral test exposes a real accessibility defect.
- Run `npm run audit:licenses` after adding test dependencies.
- Preserve the Tokyo Night token system; this plan does not require visual styling changes.

---

## File map

- `package.json`, `package-lock.json`: DOM test dependencies only.
- `src/lib/components/ModalDialog.test-harness.svelte`: minimal consumer used to exercise snippets, focus, and callbacks.
- `src/lib/components/modal-dialog.behavior.test.ts`: rendered shared-modal accessibility matrix.
- `src/lib/components/trash-confirm-dialog.behavior.test.ts`: rendered destructive confirmation boundary.
- `src/lib/components/action-proposal-review-dialog.behavior.test.ts`: rendered proposal selection and approval boundary.
- `src/lib/components/ModalDialog.svelte`: only the minimal focusability correction exposed by the failing test.
- `src/lib/tauri-mock.ts`: deterministic in-memory Trash and undo state for browser tests.
- `tests/e2e/smoke.py`: user-facing Trash/Escape/undo and context-menu Escape journeys.

---

### Task 1: Render and verify the shared modal accessibility contract

**Files:**
- Modify: `package.json`
- Modify: `package-lock.json`
- Create: `src/lib/components/ModalDialog.test-harness.svelte`
- Create: `src/lib/components/modal-dialog.behavior.test.ts`
- Modify: `src/lib/components/ModalDialog.svelte`

**Interfaces:**
- Consumes: `ModalDialog` props `titleId`, `descriptionId`, `onclose`, `initialFocus`, `closeOnEscape`, `trapFocus`.
- Produces: a rendered-test toolchain and a verified shared modal contract used by Tasks 2 and 3.

- [ ] **Step 1: Install the DOM test dependencies**

Run:

```bash
npm install --save-dev @testing-library/svelte @testing-library/user-event @testing-library/jest-dom jsdom
```

Expected: `package.json` and `package-lock.json` add the four development dependencies without changing runtime dependencies.

- [ ] **Step 2: Add the modal test harness**

Create `src/lib/components/ModalDialog.test-harness.svelte`:

```svelte
<script lang="ts">
    import ModalDialog from './ModalDialog.svelte';

    let { onclose }: { onclose: () => void } = $props();
</script>

<div class="app-shell">
    <button type="button">Background action</button>
</div>

<ModalDialog
    titleId="test-modal-title"
    descriptionId="test-modal-description"
    {onclose}
>
    <h2 id="test-modal-title">Accessible test dialog</h2>
    <p id="test-modal-description">Modal behavior under test</p>
    <button type="button" data-modal-initial-focus>First action</button>
    <button type="button">Last action</button>
</ModalDialog>
```

- [ ] **Step 3: Write the failing rendered modal tests**

Create `src/lib/components/modal-dialog.behavior.test.ts` with a jsdom pragma, Testing Library cleanup, and tests that render the harness:

```ts
// @vitest-environment jsdom
import { afterEach, describe, expect, it, vi } from 'vitest';
import { cleanup, fireEvent, render, screen, waitFor } from '@testing-library/svelte';
import userEvent from '@testing-library/user-event';
import Harness from './ModalDialog.test-harness.svelte';

afterEach(() => cleanup());

describe('ModalDialog rendered accessibility behavior', () => {
    it('exposes a named and described modal and isolates the app shell', async () => {
        render(Harness, { onclose: vi.fn() });
        const dialog = screen.getByRole('dialog', { name: 'Accessible test dialog' });
        expect(dialog).toHaveAttribute('aria-modal', 'true');
        expect(dialog).toHaveAccessibleDescription('Modal behavior under test');
        await waitFor(() => expect(screen.getByRole('button', { name: 'First action' })).toHaveFocus());
        expect(document.querySelector('.app-shell')).toHaveAttribute('aria-hidden', 'true');
    });

    it('wraps Tab and Shift+Tab inside the modal', async () => {
        const user = userEvent.setup();
        render(Harness, { onclose: vi.fn() });
        const first = screen.getByRole('button', { name: 'First action' });
        const last = screen.getByRole('button', { name: 'Last action' });
        await waitFor(() => expect(first).toHaveFocus());
        last.focus();
        await user.tab();
        expect(first).toHaveFocus();
        await user.tab({ shift: true });
        expect(last).toHaveFocus();
    });

    it('closes once on Escape and restores focus after unmount', async () => {
        const user = userEvent.setup();
        const opener = document.createElement('button');
        document.body.append(opener);
        opener.focus();
        const onclose = vi.fn();
        const view = render(Harness, { onclose });
        await user.keyboard('{Escape}');
        expect(onclose).toHaveBeenCalledOnce();
        view.unmount();
        expect(opener).toHaveFocus();
        opener.remove();
    });

    it('closes from the overlay but not from dialog content', async () => {
        const onclose = vi.fn();
        const view = render(Harness, { onclose });
        await fireEvent.click(screen.getByRole('dialog'));
        expect(onclose).not.toHaveBeenCalled();
        await fireEvent.click(view.container.querySelector('.modal-overlay')!);
        expect(onclose).toHaveBeenCalledOnce();
    });
});
```

Import `@testing-library/jest-dom/vitest` immediately after the Vitest imports so `toHaveFocus`, `toHaveAttribute`, and accessible-description matchers are registered.

- [ ] **Step 4: Run the modal tests and confirm the focus-trap failure**

Run:

```bash
npx vitest run src/lib/components/modal-dialog.behavior.test.ts
```

Expected: the Tab wrapping assertion fails because `findFocusableWithin` currently rejects buttons whose closest ancestor has the panel's `tabindex="-1"`.

- [ ] **Step 5: Apply the minimal focusability correction**

In `ModalDialog.svelte`, keep the `isFocusable` filter and remove the ancestor filter:

```ts
function findFocusableWithin(root: ParentNode): HTMLElement[] {
    return Array.from(root.querySelectorAll<HTMLElement>(FOCUSABLE_SELECTOR))
        .filter(isFocusable);
}
```

- [ ] **Step 6: Re-run the focused test**

Run:

```bash
npx vitest run src/lib/components/modal-dialog.behavior.test.ts
```

Expected: all shared modal behavior tests pass.

- [ ] **Step 7: Commit the shared modal test layer**

```bash
git add package.json package-lock.json src/lib/components/ModalDialog.svelte src/lib/components/ModalDialog.test-harness.svelte src/lib/components/modal-dialog.behavior.test.ts
git commit -m "test: render modal accessibility behavior"
```

---

### Task 2: Render Trash and proposal approval boundaries

**Files:**
- Create: `src/lib/components/trash-confirm-dialog.behavior.test.ts`
- Create: `src/lib/components/action-proposal-review-dialog.behavior.test.ts`
- Possible RED-supported correction: `src/lib/components/TrashConfirmDialog.svelte`
- Possible RED-supported correction: `src/lib/components/ActionProposalReviewDialog.svelte`

**Interfaces:**
- Consumes: rendered test setup from Task 1.
- Produces: user-level evidence that Escape/Cancel never apply destructive actions and that approval submits only checked candidates.

- [ ] **Step 1: Write Trash confirmation behavior tests**

Create a jsdom test that renders `TrashConfirmDialog` with `visible: true`, then:

```ts
const onconfirm = vi.fn();
const oncancel = vi.fn();
render(TrashConfirmDialog, { visible: true, fileName: 'portrait.png', onconfirm, oncancel });
const dialog = screen.getByRole('dialog', { name: 'Move to Trash' });
expect(dialog).toHaveAccessibleDescription('Move portrait.png to Trash?');
await userEvent.setup().keyboard('{Escape}');
expect(oncancel).toHaveBeenCalledOnce();
expect(onconfirm).not.toHaveBeenCalled();
```

Add separate tests for Cancel, default confirmation (`'none'`), and the labelled “Don't ask again” checkbox followed by “Always” (`'always'`). Query buttons, checkbox, and radio by accessible name.

- [ ] **Step 2: Run the Trash tests**

Run:

```bash
npx vitest run src/lib/components/trash-confirm-dialog.behavior.test.ts
```

Expected: tests either pass against the shared modal implementation or expose a specific wiring/accessibility defect. Correct only the observed defect and rerun to green.

- [ ] **Step 3: Write proposal review behavior tests**

Use this local proposal fixture and render without visible images so no native asset conversion is needed:

```ts
const proposal: AgentActionProposal = {
    id: 'proposal-1',
    kind: 'trash_images',
    status: 'pending',
    persona: 'curator',
    lens: null,
    criteria: 'Remove weak variants',
    visual_level: 'tiny',
    selection_preset_id: null,
    estimated_input_tokens: 100,
    estimated_output_tokens: 20,
    estimated_cost_eur: 0.002,
    source_context_json: '{}',
    items_json: JSON.stringify([
        { image_id: 'image-1', reason: 'soft focus' },
        { image_id: 'image-2', reason: 'duplicate' },
    ]),
    guard_results_json: '{}',
    apply_result_json: null,
    undo_journal_json: null,
    created_at: '2026-07-10T09:00:00Z',
    updated_at: '2026-07-10T09:00:00Z',
    applied_at: null,
};
```

Assert:

```ts
expect(screen.getByRole('dialog', { name: 'Review Trash proposal' })).toBeVisible();
expect(screen.getByRole('checkbox', { name: 'Include image-1' })).toBeChecked();
await user.click(screen.getByRole('checkbox', { name: 'Include image-2' }));
await user.click(screen.getByRole('button', { name: 'Move approved to Trash' }));
expect(onapplyproposal).toHaveBeenCalledWith('proposal-1', ['image-1']);
```

In a separate render, press Escape and assert `oncancelreview` fires once while `onapplyproposal` remains untouched.

- [ ] **Step 4: Run the proposal tests**

Run:

```bash
npx vitest run src/lib/components/action-proposal-review-dialog.behavior.test.ts
```

Expected: tests pass or expose a concrete rendered behavior defect. Apply only the smallest production correction supported by the failing assertion.

- [ ] **Step 5: Run all rendered behavior tests together**

Run:

```bash
npx vitest run src/lib/components/modal-dialog.behavior.test.ts src/lib/components/trash-confirm-dialog.behavior.test.ts src/lib/components/action-proposal-review-dialog.behavior.test.ts
```

Expected: all rendered accessibility and approval tests pass without source-string assertions.

- [ ] **Step 6: Commit the rendered flow tests**

```bash
git add src/lib/components/trash-confirm-dialog.behavior.test.ts src/lib/components/action-proposal-review-dialog.behavior.test.ts src/lib/components/TrashConfirmDialog.svelte src/lib/components/ActionProposalReviewDialog.svelte
git commit -m "test: cover accessible approval dialogs"
```

---

### Task 3: Exercise Trash, Escape, undo, and context-menu dismissal in the browser

**Files:**
- Modify: `src/lib/tauri-mock.ts`
- Modify: `tests/e2e/smoke.py`

**Interfaces:**
- Consumes: existing `invoke` handler names `list_images`, `get_image_count`, `trash_images`, and `undo`.
- Produces: deterministic in-memory removal/restoration behavior visible through the same frontend API as Tauri.

- [ ] **Step 1: Add the failing Trash browser scenario**

Add `test_trash_escape_confirm_and_undo` near the other Grid curation tests. It must:

```py
press(page, "Meta+1")
wait_mode(page, "grid")
press(page, "Home")
before = page.locator(".thumb").count()
page.keyboard.press("Backspace")
dialog = page.get_by_role("dialog", name="Move to Trash")
expect(dialog).to_be_visible()
page.keyboard.press("Escape")
expect(dialog).to_be_hidden()
assert page.locator(".thumb").count() == before
page.keyboard.press("Backspace")
page.get_by_role("button", name="Move to Trash", exact=True).click()
expect(page.locator(".thumb")).to_have_count(before - 1)
press(page, "Meta+z")
expect(page.locator(".thumb")).to_have_count(before)
```

Register it as `S26 Trash Escape/confirm/undo` after the other curation scenarios.

- [ ] **Step 2: Run only the new browser scenario and verify RED**

Register the new step at the end of the smoke sequence, then run the complete suite:

```bash
npm run test:e2e
```

Expected for `S26`: confirmation opens and Escape cancels, but confirmed removal does not reduce the grid because the mock returns a count without mutating `list_images` state. Existing failures from `imageview-5uqv.7` may also appear and must be reported separately.

- [ ] **Step 3: Make Trash and undo stateful in the browser mock**

Create module-local state initialized from the existing 20-image fixture:

```ts
let mockImages = Array.from({ length: 20 }, (_, i) => makeMockImage(i));
let lastTrashedImages: ReturnType<typeof makeMockImage>[] = [];
```

Update `list_images` and `get_image_count` to read this state. Update `trash_images` to remove matching IDs and retain the removed items in original order. Update `undo` to restore the retained items, clear the undo buffer, and return the existing action label. Do not touch files or use the real database.

- [ ] **Step 4: Re-run the Trash browser scenario**

Run:

```bash
npm run test:e2e
```

Expected for the new step: Escape preserves the count, explicit confirmation decrements it once, and Meta+Z restores it.

- [ ] **Step 5: Tighten context-menu Escape behavior**

In `test_context_menu`, replace click-away-only dismissal with a keyboard assertion:

```py
page.locator(".thumb").first.click(button="right")
expect(page.locator(".context-menu")).to_be_visible()
page.keyboard.press("Escape")
expect(page.locator(".context-menu")).to_be_hidden()
```

Then press an Arrow key and assert the focused thumbnail changes, proving the closed menu no longer captures Grid keyboard input.

- [ ] **Step 6: Run the browser smoke suite and classify unrelated failures**

Run:

```bash
npm run test:e2e
```

Expected: the new Trash and context-menu steps pass. Fix only stale assertions directly encountered by those steps. Record unrelated existing failures under `imageview-5uqv.7` rather than changing them without rationale.

- [ ] **Step 7: Commit the browser journeys**

```bash
git add src/lib/tauri-mock.ts tests/e2e/smoke.py
git commit -m "test: cover Trash and Escape browser journeys"
```

---

### Task 4: Full verification, issue closure, and landing

**Files:**
- Modify: `.beads/issues.jsonl`

**Interfaces:**
- Consumes: all tests and changes from Tasks 1–3.
- Produces: verified branch, closed coverage issue, and pushed remote state.

- [ ] **Step 1: Run frontend behavioral and type gates**

```bash
npm test
npm run check
```

Expected: all Vitest files pass; Svelte diagnostics report 0 errors and 0 warnings.

- [ ] **Step 2: Run browser and license gates**

```bash
npm run test:e2e
npm run audit:licenses
```

Expected: new accessibility journeys pass; license audit accepts all new test dependencies. Any unrelated browser failures are listed exactly and left on the existing `imageview-5uqv.7` issue.

- [ ] **Step 3: Run the project full preflight**

```bash
npm run preflight:full
```

Expected: hook checks, frontend checks/tests, Rust formatting, Clippy, and Rust tests complete successfully; pre-existing Clippy warnings may be reported but do not fail the command.

- [ ] **Step 4: Close the completed issue**

```bash
npm run bd -- close imageview-5uqv.6
npm run bd -- vc status
```

Expected: `imageview-5uqv.6` is closed and tracked export state reflects the change.

- [ ] **Step 5: Commit issue state and land**

```bash
git add .beads/issues.jsonl
git commit -m "chore: close accessibility coverage task"
npm run land
```

Expected: the worktree is clean, all required checks pass, and the branch is pushed successfully.
