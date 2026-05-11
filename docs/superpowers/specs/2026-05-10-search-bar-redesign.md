# Search Bar Redesign

## Overview

Redesign the CommandBar search component from an always-visible input to a toggleable, three-state search bar with voice input and interactive rule editing.

## Current State

- `CommandBar.svelte` is always visible in grid view
- Search input with `/` prefix, Enter to parse, natural language query → FilterNode
- `RuleBuilder.svelte` displays parsed rules as read-only dropdowns
- "+ Add rule" and "× Remove" buttons are non-functional (empty `onclick`)
- No keyboard shortcut to activate/dismiss

## State Model

```typescript
// In stores.ts
export const searchOpen = writable(false);

// In CommandBar.svelte
let query = $state('');
let parsedFilter: FilterNode | null = $state(null);
let matchCount = $state(0);
let showRules = $state(false);
let applied = $state(false);
let isCollapsed = $state(false);
let isDirtyFromManualEdit = $state(false);
let isApplying = $state(false);
let isListening = $state(false);

let saving = $state(false);
let collectionName = $state('');
let savedMessage = $state('');
```

## Design

### Three States

#### State 1: Hidden (default)

- No search bar rendered
- A faint hint `/ to search` displayed in the command bar area (subtle, secondary text color)
- Activated by pressing `/` or `⌘F` when no text input is focused
- Keyboard handling integrated into `src/lib/keys.ts` (existing shortcut pipeline), not a separate `document` listener

#### State 2: Active

The search bar slides in below the tab bar with a CSS transition (`max-height` + `opacity`, ~200ms ease-out).

**Layout (left to right):**
- `/` prefix icon (gray, 16px, font-weight 600)
- Text input (flex: 1, 14px, placeholder: "landscape midjourney 4 stars or more...")
- Mic toggle button (24px circle, blue tint when inactive, pulsing blue ring when recording)
- `esc` keyboard hint badge (small, muted)
- `×` close button (clears query and hides search bar)

**Below the input (when query is parsed):**
- Interactive rule chips displayed inline (horizontal flex-wrap)
- Each chip shows: field, operator, value (e.g. `Rating ≥ 4`, `Source = midjourney`)
- Click a chip to edit its field/op/value via inline dropdowns
- `×` on each chip to remove that rule
- `+ Add rule` button appends a new empty rule
- Result count (`112 images`), Refresh button, Save Collection button

**Keyboard shortcuts (integrated into `src/lib/keys.ts`):**
- `Enter` — parse query and apply
- `Escape` — priority chain: (1) close editing dropdown if open, (2) close search bar, (3) allow global handlers. Calls `stopPropagation`/`preventDefault` when handled.
- `/` or `⌘F` (when search is closed) — open and focus. Ignored when any text input/select/textarea is focused.

**Animation:**
- **Open:** `max-height: 0 → auto` (use measured height, not fixed 60px since rules area is variable), `opacity: 0 → 1`, `200ms ease-out`. Input and rules area animate separately — input slides first, rules fade in after parse.
- **Close:** reverse, `150ms ease-in`

#### State 3: Collapsed Sticky

When the user scrolls down, the active search bar collapses to a slim sticky pill at the top of the grid area.

**Scroll detection:** Listen to `.grid-container` `scrollTop` (the grid scrolls inside `Grid.svelte`, not the viewport). Use a scroll threshold (~50px) to trigger collapse.

**Pill layout:**
- Height: ~28px, border-radius: 999px
- `🔍` icon, truncated query text, result count badge, `×` close
- Background: `var(--surface)`, border: 1px solid `var(--border)`
- Position: sticky, top: 0, z-index: 10

**Interactions:**
- Click pill → expand back to full active state, scroll grid to top
- Press `/` → same as click
- `×` on pill → clear search entirely, return to hidden state

**Collapse animation:** Full bar fades out with `opacity 150ms`, pill fades in with `opacity 150ms`

### Voice Input

Uses the Web Speech API (`webkitSpeechRecognition` / `SpeechRecognition`).

**Behavior:**
- Click mic button → toggle listening on/off
- While listening: mic button shows pulsing blue ring animation (CSS `@keyframes pulse`, `box-shadow` blue glow, `1.5s infinite`)
- Transcribed text is appended to the current input value
- On speech end: auto-trigger parse (`handleParse()`)
- If Speech API unavailable (non-Chromium): hide mic button entirely

**Error handling:**
- `onerror` handler: show brief error state on mic button (red flash), reset `isListening`
- Permission denied: hide mic button for the session, show brief toast "Microphone access denied"
- Cleanup: stop recognition on component destroy (`onDestroy`) and when search closes
- Guard `onend`: only auto-parse if not manually stopped (track `manualStop` flag)

**TypeScript:** Add global type declarations for `SpeechRecognition` / `webkitSpeechRecognition` in `src/app.d.ts`

### Interactive Rule Chips

Replace the current `RuleBuilder.svelte` dropdown-based display with inline editable chips.

**RuleBuilder contract (Svelte 5):**
```typescript
interface Props {
  filter: FilterNode;
  onchange: (next: FilterNode) => void;
}
let { filter, onchange }: Props = $props();
```
Parent owns the state. RuleBuilder calls `onchange(newFilter)` with an immutable copy. No direct prop mutation.

**Filter structure — v1 flat only:**
- Only supports a flat root group: `{ type: 'group', op: 'and'|'or', children: FilterRule[] }`
- If `parsedFilter` is a single `FilterRule`, normalize to `{ type: 'group', op: 'and', children: [rule] }`
- Nested `group` and `not` nodes from NL parse are flattened: `not` wrapping a rule shows a "NOT" badge on the chip; nested groups are flattened into the root group
- Unsupported deep nesting: display a "Complex filter — edit via query" message, disable chip editing

**Each chip:**
- Rendered as a `<span>` with background color coded by field type
- Click to enter edit mode: shows inline `<select>` for field and operator, `<input>` for value
- `×` button to remove the rule from the filter
- Click outside or press Enter to confirm edit

**Validation matrix:**

| Field | Type | Allowed Operators | Value Editor |
|-------|------|-------------------|-------------|
| `rating` | number | eq, gte, lte, gt, lt | number input (0-5) |
| `color_label` | enum | eq, neq | select: red, yellow, green, blue, purple |
| `decision` | enum | eq, neq | select: accepted, rejected, pending |
| `format` | string | eq, neq, contains | text input |
| `width`, `height` | number | eq, gt, gte, lt, lte | number input |
| `orientation` | enum | eq | select: landscape, portrait, square |
| `source_label` | string | eq, neq, contains | text input |
| `is_ai_generated` | boolean | eq | select: yes, no |
| `imported_at` | date | last_n_days, this_week, this_month | number input (for days) |
| `ai_prompt` | string | contains, is_empty | text input |
| `aspect_ratio` | number | gt, gte, lt, lte, eq | number input |

When field changes, reset operator to first allowed and clear value. Hide inapplicable operators.

**"+ Add rule" button:**
- Appends a new `FilterRule` with defaults: `{ type: 'rule', field: 'rating', op: 'gte', value: 0 }`
- Immediately enters edit mode on the new chip
- Calls `onchange` with updated filter

**Apply behavior:**
- After any chip edit: debounce 300ms, then call `handleApply()` to refresh results
- Track request IDs: ignore stale responses when a newer apply is in flight
- Set `isDirtyFromManualEdit = true` so save behavior is adjusted

### Empty States

- **Removing last rule:** Remove the rule, clear filter, show empty search input (stay in Active state)
- **Adding rule before parse:** Allowed — creates a default rule chip, user edits it
- **Clearing search (×):** Reset all state, return to Hidden
- **Parse returns empty/invalid:** Show "Couldn't parse query" inline message, keep input for editing
- **Zero results after apply:** Show "No images match" with result count `0 images`

### Save Collection After Manual Edits

When `isDirtyFromManualEdit` is true:
- Save uses current `filter_json` from chips (not the stale NL query)
- `nl_query` field stores the original query as provenance, suffixed with " (edited)"
- Name generation derives from chip fields: e.g. "Rating 4+ MJ Landscape"

## Files to Modify

| File | Changes |
|------|---------|
| `src/lib/components/CommandBar.svelte` | Three-state logic, voice input, sticky collapse, slide animation, debounced apply |
| `src/lib/components/RuleBuilder.svelte` | Replace with interactive chip-based rule editing, `onchange` callback |
| `src/routes/+page.svelte` | Remove always-visible CommandBar, render conditionally on `$searchOpen` |
| `src/lib/stores.ts` | Add `searchOpen` writable store |
| `src/lib/keys.ts` | Add `/` and `⌘F` handlers for search open |
| `src/app.d.ts` | Add `SpeechRecognition` / `webkitSpeechRecognition` type declarations |

## Accessibility

- Search input gets `role="searchbox"`, `aria-label="Search images"`
- Mic button: `aria-label="Toggle voice input"`, `aria-pressed={isListening}`
- Rule chips: `role="group"`, `aria-label="Active filters"`
- Each chip: `role="button"`, `aria-label="Edit filter: {field} {op} {value}"`
- Escape key closes search from any focused element within (priority chain as specified)

## Tests

- Unit: rule mutation (add/remove/edit), validation matrix, filter normalization (single rule → group), stale apply protection
- E2E: `/` opens search, `⌘F` opens search, `Escape` closes, chip edit/remove/add flow, voice button hidden when unsupported
