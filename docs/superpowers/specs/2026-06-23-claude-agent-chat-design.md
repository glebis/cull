# Claude Agent Chat And Proposal Workflow Design

## Purpose

Cull should add a first-party Claude Agent chat surface for image curation. The
agent should behave like a curator, copilot, and operator without turning
destructive file operations into opaque model actions.

The first release focuses on agent-assisted curation in the live Cull UI:

- Explain and rank images in the current context.
- Propose selections, accept/reject decisions, manual collection changes, canvas
  ordering/removal, and system Trash cleanup.
- Apply approved proposals through Cull-native review, undo, and audit paths.
- Keep model cost and visual context resolution understandable while defaulting
  to the smallest useful context.

Permanent delete is out of scope for v1.

## Product Model

The chat is a first-party Cull panel, not a plugin-hosted UI. A plugin-first
implementation would make the destructive boundary harder to control, because
Trash, canvas layout changes, and collection membership all need native review,
undo, audit, and access to live UI state.

The panel supports three visible personas over the same runtime:

- **Curator**: analyzes, explains, ranks, and suggests. It cannot apply changes.
- **Copilot**: creates action proposals and can select/highlight candidates.
- **Operator**: can apply approved proposals through Cull's native review gate.

Persona is separate from the active lens. Lenses describe the curation task, such
as near duplicates, low focus, trash review, select, collection, or ordering.

## Interaction Prototype Decisions

The accepted interaction pattern is a right-side agent dock that can also become
a floating panel.

Default behavior:

- A thin persistent right rail makes Claude discoverable.
- Hovering over the right edge can reveal a floating panel as an accelerator.
- Pinning the panel turns it into a persistent right dock.
- In pinned mode, the grid reflows around the dock instead of being covered.
- Floating mode may overlay the grid, but it must not be the only available
  state.

The final hierarchy should prioritize curation work over metering:

1. Current task and proposal status.
2. Candidate reasoning and next action.
3. Context, token, and estimated cost as a secondary but always visible control.

The prototype should evolve toward:

- Compact context chip in the agent header.
- Primary CTA named "Open review gate" or equivalent, never "Apply" for Trash.
- Candidate source cards highlighted in the grid.
- Native review sheet for destructive proposals.
- Post-apply undo state.

## Visual Context Policy

Cull should be thumbnail-first with explicit visual escalation. The agent must
not receive full-size images by default.

Visual levels:

- **Text-only**: image IDs, paths if allowed, dimensions, ratings, decisions,
  prompts, source labels, quality scores, hash/similarity metadata, collections,
  canvas layout summaries.
- **Tiny**: smallest thumbnails or contact sheets. This is the default.
- **Preview**: medium previews for ambiguous candidates.
- **Full**: full images. This requires explicit confirmation or a session-level
  permission.

The visual level is an interactive secondary chip, for example:

```text
Context: Tiny  -  EUR0.014 est  -  2.1k tokens
```

Clicking the visual level cycles:

```text
Text-only -> Tiny -> Preview -> Full -> Text-only
```

Guardrails:

- `Tiny` is the default for new sessions and proposals.
- `Preview` shows the estimated incremental cost before loading.
- `Full` opens a confirmation gate unless the user has explicitly allowed full
  images for the session.
- If a requested level would exceed budget, Cull shows a blocked state instead
  of silently escalating.
- Agent output must disclose the basis of the decision, such as "reasoned from
  Tiny thumbnails" or "2 candidates need Preview before Trash recommendation."

## Cost And Token Tracking

Token tracking is a first-class product feature, not a settings afterthought.
Users need to understand what kind of operation spends tokens.

Cull should record and display:

- Per chat turn usage.
- Per operation type: curator analysis, candidate search, proposal generation,
  visual escalation, and proposal apply.
- Per model usage.
- Input, output, and cache tokens when available.
- Estimated cost before work and reconciled cost after work.
- Cumulative session cost.
- Proposal cost before apply.

The panel shows this as a quiet header chip by default. Expanded details can show
the ledger by turn, operation, and visual escalation.

Budget controls:

- Session budget.
- Warning threshold.
- Hard stop threshold.
- Ask before Preview.
- Ask before Full.
- Ask before subagents or parallel review.
- Prefer thumbnails enabled by default.

Claude Agent SDK cost fields are estimates and should be labeled as such. The
runtime should persist both estimate and observed SDK result usage when
available.

## Built-In Agent Skills

Cull should not rely on the user's personal Claude skill/plugin set at runtime.
The installed `agent-sdk-dev` plugin is useful as a development and verification
reference, but runtime behavior must be reproducible.

Cull should ship a Cull-owned Claude plugin, loaded explicitly by the app:

```text
cull-agent-plugin/
  .claude-plugin/plugin.json
  skills/
    cull-curator/
      SKILL.md
    cull-near-duplicate-review/
      SKILL.md
    cull-trash-review/
      SKILL.md
    cull-visual-escalation/
      SKILL.md
    cull-collection-builder/
      SKILL.md
    cull-canvas-ordering/
      SKILL.md
  agents/
    cull-curator.md
    cull-dedupe-reviewer.md
```

Initial skill responsibilities:

- **cull-curator**: general selection, accept/reject, and collection advice.
- **cull-near-duplicate-review**: use exact hash, perceptual hash, embedding
  similarity, prompt/seed lineage, quality metrics, ratings, and decisions
  before asking Claude to visually arbitrate.
- **cull-trash-review**: enforce no-trash guards for accepted, high-rated, or
  client-favorited images unless the user explicitly overrides them.
- **cull-visual-escalation**: prefer text and Tiny context, request Preview or
  Full only with cost/need justification.
- **cull-collection-builder**: turn selected or proposed sets into manual
  collections.
- **cull-canvas-ordering**: propose ordering/layout changes without directly
  mutating the canvas.

The runtime should load only this Cull plugin and explicit Cull skills by
default.

## Claude Agent SDK Runtime

Use the Claude Agent SDK in TypeScript, aligned with official SDK patterns and
verified with an SDK-focused review before implementation lands.

Runtime shape:

- `@anthropic-ai/claude-agent-sdk` powers the streaming agent loop.
- Cull streams SDK messages into the Svelte chat panel.
- Cull supplies a controlled system prompt and Cull-owned plugin/skills.
- Cull exposes MCP or in-process custom tools for context and proposals.
- `canUseTool` and the native app review UI handle sensitive actions.
- SDK permissions deny direct destructive execution tools in v1.
- Model/provider credentials use explicit product settings. Do not depend on a
  developer's local Claude Code login for shipped behavior.

The product label should be "Claude Agent" or "Claude", not "Claude Code", even
if the SDK uses Claude Code's agent loop internally.

## Action Proposal Model

Claude must not directly perform destructive mutations. It creates persisted
Action Proposals that Cull can inspect, review, apply, audit, and undo.

Proposal kinds:

- `select_images`
- `set_decisions`
- `create_collection`
- `add_to_collection`
- `remove_from_collection`
- `reorder_canvas`
- `remove_from_canvas`
- `trash_images`

Each proposal stores:

- Stable proposal ID.
- Persona and lens.
- User prompt and interpreted criteria.
- Source context and visual level used.
- Estimated token/cost ledger.
- Candidate image IDs.
- Per-candidate reason.
- Confidence or review-required status.
- Guard results.
- Target operation.
- Created/applied/dismissed timestamps.
- Apply result and undo journal when applied.

The review UI lets the user deselect items, inspect reasons, escalate visual
level, and approve the final subset.

## Removal And Trash Scope

V1 supports three removal targets:

- Remove from a manual collection.
- Remove or hide items from a canvas/current layout.
- Move original files to system Trash.

In All Images, folders, and smart collections, "remove from current working set"
is ambiguous, so the only real removal target is Trash unless the user first
creates or uses a manual collection/canvas.

Permanent delete is excluded.

Trash proposal rules:

- The agent proposes a resolved candidate set and reasons.
- Cull opens a native Trash Proposal Review gate.
- The user approves the exact final subset.
- Cull moves approved files to system Trash.
- Per-file success/failure is returned and surfaced.
- Audit and undo entries are recorded.

Existing `trash_images` behavior only returns a count and hides per-file errors.
The implementation must add reliable per-file results before exposing Trash to
agent proposals.

## Undo Contract

Undo is batch-scoped at the proposal level.

- Collection removal undo re-adds the exact image IDs to the original
  collection.
- Canvas removal/order undo restores the prior canvas layout JSON.
- Trash undo attempts to restore successfully trashed files and reports
  per-file success/failure.
- Mixed proposals undo successful operations in reverse order and report partial
  failures.

Undo availability should be visible in the panel after apply, not only in global
status.

## API Surface Needed

New or expanded APIs:

- `list_active_context`
- `get_current_selection`
- `get_active_canvas_layout`
- `preview_candidate_set`
- `create_action_proposal`
- `get_action_proposal`
- `list_action_proposals`
- `dismiss_action_proposal`
- `apply_action_proposal`
- `undo_action_proposal`
- `estimate_agent_operation_cost`
- `get_agent_usage_ledger`

MCP/tool policy:

- Read/context tools are available to the agent.
- Proposal creation tools are available to the agent.
- Direct destructive tools are not available to Claude in v1.
- Applying a proposal is a Cull UI/native command, not a free-form agent tool
  call.

## Near-Duplicate Workflow

Near-duplicate cleanup should use cheap deterministic passes before model visual
inspection.

Pipeline:

1. Build groups with exact hash, perceptual hash, embeddings, prompt/seed
   lineage, and source metadata.
2. Rank within groups using ratings, decisions, client favorites, quality
   metrics, focus score, and representativeness.
3. Apply guards: never auto-trash accepted, high-rated, or client-favorited
   images unless explicitly allowed.
4. Use Tiny contact sheets for visual arbitration.
5. Escalate ambiguous cases to Preview.
6. Create a Trash proposal with safe candidates and review-required candidates.
7. Let the user apply the final subset through the review gate.

## Required States To Prototype Next

- Persistent right rail with unread/proposal badges.
- Native Trash Proposal Review gate.
- Preview escalation pending, approved, denied, and over-budget states.
- Budget exhausted state.
- Empty/no-proposal state.
- Streaming/thinking state.
- Model unavailable/offline state.
- Post-apply undo state.
- Multi-proposal queue.
- Pinned dock on Loupe, Compare, and Canvas.

## Testing And Verification

Design-level acceptance:

- User can discover Claude without hover-only interaction.
- Pinned dock never covers thumbnails in Grid.
- Floating mode is optional and dismissible.
- Token/cost and visual level remain visible but secondary.
- Visual level can be changed explicitly.
- Destructive proposals cannot apply without native review.
- Agent output identifies whether it used text, Tiny, Preview, or Full context.

Implementation verification later:

- TypeScript SDK adapter typechecks.
- Agent SDK integration is reviewed against the official TypeScript SDK docs.
- Proposal model tests cover persistence, guard results, apply, partial failure,
  and undo.
- Trash command returns per-file results.
- Svelte tests cover panel state transitions and visual-level controls.
- Rust tests cover proposal apply and undo.
- Browser E2E covers Grid reflow, review gate, and Trash proposal review.

## References

- Existing agent surface: `docs/agents.md`
- Current research note: `docs/agent-first-chat-research.md`
- Snapshot MCP tools: `src-tauri/src/mcp/tools/sessions.rs`
- Curation MCP tools: `src-tauri/src/mcp/tools/curation.rs`
- Collections MCP tools: `src-tauri/src/mcp/tools/collections.rs`
- Trash command gap: `src-tauri/src/commands/library.rs`
- Canvas document model: `src-tauri/src/db_core/canvas_document.rs`
- Claude Agent SDK overview: <https://code.claude.com/docs/en/agent-sdk/overview>
- Claude Agent SDK TypeScript: <https://code.claude.com/docs/en/agent-sdk/typescript>
- Claude Agent SDK cost tracking: <https://code.claude.com/docs/en/agent-sdk/cost-tracking>
- Claude Agent SDK skills: <https://code.claude.com/docs/en/agent-sdk/skills>
- Claude Agent SDK plugins: <https://code.claude.com/docs/en/agent-sdk/plugins>
