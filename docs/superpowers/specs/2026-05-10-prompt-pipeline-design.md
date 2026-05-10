# ImageView Prompt Pipeline — Design Spec

## Overview

Three-slice feature adding AI generation metadata support, canvas-based prompt evolution, and live generation to ImageView.

**JTBD:** When designers produce visual narratives using AI tools (especially GPT Image), they want one app that captures every generated image with its prompt DNA, lets them explore the prompt space visually, and exports platform-ready output — so nothing leaks from the pipeline.

**Reference:** `~/jtbd/imageview-prompt-pipeline/jtbd.json`

---

## Slice 1: Capture — Sidecar Import + Prompt Display

### Goal
Every imported image that has generation metadata (sidecar JSON, PNG text chunks, C2PA) gets a `generation_run` record. Prompt is visible in Loupe.

### Data Model

**`generation_runs` table** — canonical prompt artifact, one per image on import:
- `id, prompt, negative_prompt, provider, model, settings_json, seed`
- `parent_run_id` (nullable FK to self, for future branching)
- `source_type` ('sidecar' | 'png_metadata' | 'c2pa' | 'manual' | 'live')
- `source_path` (path to sidecar file)
- `raw_metadata_json` (full original JSON preserved for forward-compat)
- `created_at, imported_at`

**`images` table** gets `generation_run_id` FK.

### Sidecar Import Pipeline

Plugs into `import.rs` after source detection (line 70):

1. For each imported image, check for adjacent `{name}.json` sidecar file
2. Parse sidecar JSON — handle both OpenAI schema (`provider, quality, thinking, estimated_cost, n`) and Gemini schema (`model, duration_s, edit_source`)
3. Create a `generation_run` record with normalized fields + raw JSON
4. Link image to run via `generation_run_id`
5. Sidecar parsing is another evidence source in the existing detection pipeline, not a separate importer

### Loupe Prompt Display

- New collapsible "Generation" section in Loupe metadata panel
- Shows: prompt text (full, scrollable), model, provider, seed
- Muted secondary info: settings, cost, timestamp
- If no generation data: section hidden

### MCP Tools (new)

- `get_generation_run(image_id)` — returns run metadata for an image
- `set_generation_metadata(image_id, prompt, model, ...)` — manually attach prompt to image (creates a run with source_type='manual')

---

## Slice 2: Canvas Evolution — Column Layout + Prompt Provenance

### Goal
Canvas view becomes a visual evolution workspace with sticky columns representing prompt iteration stages.

### Design Decision: Hybrid Columns + Prompt Chain (Option C from brainstorm)

User-created free columns, but each column can optionally link to a prompt. Branching a prompt auto-spawns a new column. Column header shows prompt diff from parent.

### Design Decision: Composer + Column Provenance (Option D from brainstorm)

- **Composer drawer** at bottom: dictation input, AI-suggested refinements, queue controls
- **Column headers** are read-only provenance — receipts showing what generated the images, not editors
- **Branching** opens the composer prefilled with the source prompt
- **Remix** is an action mode within the composer (select image → Remix → composer opens with editable clauses highlighted)

### Data Model

**`canvas_layouts`** — named canvas workspaces (id, name, timestamps)
**`canvas_columns`** — sticky zones within a canvas (title, prompt_snapshot, parent_run_id, position, width)
**`canvas_items`** — images placed in columns (image_id, column_id, position)

Canvas is independent from lineage (Codex recommendation). A lineage group is factual/procedural; canvas layout is user-authored organization. One lineage group may appear across multiple canvases.

### Column Behavior

- Drag images between columns
- Adjustable column width — at minimum width, becomes a thin card stack
- At narrowest: first-to-last image of evolution shows as one smooth visual line
- Add/remove columns freely, name them anything
- Columns with a linked `parent_run_id` show the prompt diff from parent column

### Canvas Component (`Canvas.svelte`)

- Wired into existing TabBar (slot already declared, keyboard shortcut ⌘4)
- Renders in `+page.svelte` where the "Coming soon" placeholder currently lives
- Persistent layout saved to DB, restored on re-open

---

## Slice 3: Full Pipeline — Generation + Queue + Refinement

### Goal
Submit prompts from within ImageView, get results back automatically. AI refines prompts before submission. Queue manages concurrency.

### Composer Drawer

- Persistent bottom panel in Canvas view
- Text input with dictation support (microphone indicator)
- On submit: call Claude API to generate 3-6 refined prompt variants with highlighted diffs
- User checks which variants to submit
- "Submit All" / "Submit Checked" buttons
- Queue counter: "2/3 running"

### AI Prompt Refinement

- On prompt submit, call Claude API (or local LLM) to generate refined variants
- Each variant: full prompt text with green-highlighted additions
- Variants shown as checkboxes — user selects which to generate
- Support commands: "make it moodier", "more editorial", "three product-photo versions"

### Generation Queue (Tauri Backend)

Codex recommendation: queue lives in backend, not frontend.

- Concurrency limit (default 2, user-configurable)
- Queue survives frontend reloads
- Backend atomically connects submitted jobs to imported images and DB records
- Frontend: initiate, monitor, cancel, reorder
- Each submission creates a `generation_run` with source_type='live' and parent_run_id linking to source

### Generation Providers

- GPT Image 2 via OpenAI API (primary)
- Extensible provider interface for future: Gemini, Stability, local SD
- MCP as integration layer — provider adapters behind backend queue

### Auto-Import

- Generated images auto-import into library
- Auto-link to generation_run
- Auto-place in active canvas column (or spawn new linked column)
- Lineage detection auto-groups variants

### MCP Tools (new)

- `submit_generation(prompt, model, settings)` — queue a generation job
- `list_generation_queue()` — current queue state
- `cancel_generation(job_id)` — cancel queued/running job

---

## Cross-Cutting Concerns

### Sidecar Format Standardization

- Normalize both OpenAI and Gemini sidecar schemas into `generation_runs`
- Preserve raw JSON in `raw_metadata_json` for fields we don't understand yet
- Field mapping: `prompt` → `prompt`, `provider` → `provider`, `model` → `model`, `seed` → `seed`, `quality/thinking/n` → `settings_json`
- Consider aligning with IPTC 2025.1 field names for forward-compat

### gpt-image-2 Skill Updates

- Add `model` field to sidecar output (currently missing)
- Add `width/height` fields
- Add `schema_version` field
- Add `generation_id` from API response
- Consider adding iterate-from-sidecar functionality

### Keyboard Shortcuts (Canvas)

- `n` — new prompt (open composer)
- `⑂` or `b` — branch from selected column
- `⌘↵` — submit
- `Esc` — close composer
- Column width: `[` / `]` to narrow/widen

---

## Implementation Order

1. **Slice 1** (~1-2 sessions): generation_runs table + sidecar parser in import.rs + Loupe display + 2 MCP tools
2. **Slice 2** (~2-3 sessions): Canvas.svelte component + canvas DB tables + column drag/drop + width adjustment + prompt provenance headers
3. **Slice 3** (~3-4 sessions): Composer drawer + AI refinement + backend queue + provider adapters + auto-import

Each slice is independently shippable and testable.

---

## Design References

- Canvas mockup (draft): `site/public/images/imageview-canvas-composer.png`
- Canvas mockup (final): `site/public/images/imageview-canvas-composer-final.png`
- Visual companion mockups: `.superpowers/brainstorm/` session directory
- JTBD: `~/jtbd/imageview-prompt-pipeline/`
- Codex architecture review: confirmed A-lite approach, generation_runs as canonical entity
