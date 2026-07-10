# User-Facing Accessibility Test Coverage Design

**Date:** 2026-07-10

**Issue:** `imageview-5uqv.6`

**Status:** Approved direction; implementation pending

## Goal

Replace the weakest source-substring checks around destructive and approval flows with behavioral tests that exercise the interface as a user does. The first pass must cover keyboard accessibility as behavior, including Escape dismissal, focus management, accessible names, and protection against accidental destructive actions.

This work improves confidence in existing behavior; it does not redesign the dialogs or broaden destructive-operation policy.

## Evidence and scope

The repository currently has 124 frontend test files, but 56 inspect component source text and Vitest runs without a DOM environment. The browser smoke suite covers many navigation shortcuts, but it has stale failures and does not cover the Trash confirmation journey. Existing tests validate the shared modal keyboard helper directly, yet they do not prove that rendered dialogs wire the helper, focus, semantics, and callbacks together correctly.

The implementation will cover:

1. The shared `ModalDialog` accessibility contract in a rendered DOM.
2. The rendered `TrashConfirmDialog` cancellation and confirmation boundary.
3. The rendered `ActionProposalReviewDialog` cancellation, candidate selection, and explicit apply boundary.
4. A browser journey proving that Trash can be opened from the grid, cancelled with Escape without mutation, confirmed explicitly, and undone.
5. Context-menu Escape behavior where it can be added without expanding the change into a general menu rewrite.
6. Stale browser assertions encountered on the changed journey, limited to expectations required to make the relevant smoke path trustworthy.

Permanent deletion, session-folder deletion, collection workflows, export success, and wholesale repair of every pre-existing browser failure remain separate tracked work. This prevents an accessibility coverage change from silently becoming a broad product refactor.

## Testing architecture

### Rendered component layer

Add the smallest DOM test setup compatible with Svelte 5 and Vitest. Prefer `@testing-library/svelte`, `@testing-library/user-event`, and `jsdom` if they work with the current toolchain. Tests query by role, accessible name, and visible label rather than CSS implementation details.

The shared modal test matrix is:

| Behavior | Evidence |
| --- | --- |
| Dialog semantics | `role="dialog"`, `aria-modal="true"`, and resolvable name/description |
| Initial focus | Explicit initial target receives focus after mount |
| Forward focus trap | Tab from the last control wraps to the first |
| Reverse focus trap | Shift+Tab from the first control wraps to the last |
| Escape | Close callback fires once and the key does not leak to the app |
| Background isolation | The app shell becomes inert while mounted and is restored on unmount |
| Focus restoration | Focus returns to the connected opener after dismissal |
| Overlay click | Clicking the overlay closes; clicking the dialog content does not |

Rendered Trash tests prove that Escape and Cancel call only `oncancel`, confirmation calls `onconfirm` with the selected suppression scope, and the destructive action has intentional initial focus. Rendered proposal tests prove that Escape cancels without apply, candidates have accessible checkbox names, deselection changes the approved set, and apply sends only the remaining approved identifiers.

### Browser journey layer

Extend the mock-backed browser suite with a stateful Trash path:

1. Focus an image in Grid.
2. Press Backspace and assert a named confirmation dialog opens.
3. Press Escape and assert the dialog closes while the image remains.
4. Reopen, confirm, and assert only the intended image disappears.
5. Invoke undo and assert the image returns.

The Tauri browser mock may be extended to track trash and undo state deterministically. It must remain isolated from the real filesystem and `cull.db`.

For context menus, the browser test will assert that Escape closes the active menu and returns keyboard control to the underlying grid. Focus restoration will be asserted if the current menu architecture exposes a stable opener; otherwise that missing behavior will be recorded as a follow-up rather than hidden behind a weak assertion.

## Red-green implementation order

1. Add a rendered test that fails because the current Vitest setup cannot mount a Svelte component.
2. Add the minimal DOM test dependencies/configuration and make the shared modal tests pass.
3. Add failing rendered tests for Trash, then satisfy them without weakening the assertions.
4. Add failing rendered tests for proposal review, then satisfy them.
5. Add the failing browser Trash/Escape/undo journey, then add only the mock behavior or production correction needed for it to pass.
6. Add or tighten the context-menu Escape assertion.
7. Run focused tests, the complete frontend suite, Svelte checks, the browser smoke suite, and license audit because test dependencies are added.

Production changes are allowed only when a behavioral test exposes a real accessibility defect. Each such change must be the smallest correction that makes the desired user behavior true.

## Failure handling and safety

- Tests must never access or reset the real Cull database.
- Browser tests run only with `CULL_E2E_MOCK=1` through the existing runner.
- Destructive-path tests assert non-mutation before asserting successful mutation.
- If the full browser suite still contains unrelated stale failures, the handoff will name each failure and separately report the new journey's result. Unrelated expectations will not be silently changed.
- Dependency licenses will be checked with `npm run audit:licenses`.

## Acceptance criteria

- Shared modal accessibility behavior is verified by rendered tests, including Escape, Tab/Shift+Tab trapping, initial focus, focus restoration, semantics, and background inertness.
- Trash confirmation is tested as a rendered component and as a browser journey; Escape and Cancel preserve the image.
- Agent proposal review is rendered and tested for accessible candidate controls, cancellation, and explicit approved-item application.
- Context-menu Escape behavior has a behavioral browser assertion or a documented follow-up if the current architecture prevents a meaningful focus assertion.
- The new tests do not rely on reading Svelte source strings.
- Focused tests, full `npm test`, `npm run check`, relevant browser E2E, and `npm run audit:licenses` have fresh recorded results.
- Existing user data and the real database are untouched.
