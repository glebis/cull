# Smart Collections — Design Brief v2

*Updated with Codex review feedback*

## Context

ImageView is a Tauri 2 + SvelteKit desktop image viewer focused on AI-generated art. Currently supports: manual collections, folder browsing, star ratings (1-5), color labels, pick/reject decisions, CLIP embeddings (ViT-B/32), and dimension-based filtering.

**Goal**: Design a smart collections system that feels natural, voice-friendly, and goes beyond what Capture One / Lightroom offer — leveraging our CLIP embeddings and optional LLM integration.

---

## Design Decisions (Resolved)

### Input Paradigm: Option C — Command Bar + Rule Builder

A Spotlight/Raycast-style command bar at the top of the content area. User types or speaks natural language. Below it, a rule builder panel shows the parsed interpretation as editable rows. Each row reads like a sentence: `Source / is / Midjourney`.

The command bar is **not a chat box** — it is an input method for constructing visible rules. Users always see exactly why images matched and can edit any individual rule via dropdowns.

**Why**: NL-first (Option B) causes trust problems in a desktop cataloging app. Users need deterministic, visible state. Voice benefits from visible confirmation: user says "show recent Midjourney portraits rated four or better," app fills rules, user corrects one dropdown if needed.

### Query Explainability

Every NL query shows a preview: "I interpreted this as: rating >= 4, imported last week, source != Midjourney." Users confirm or edit before the query executes on first use. Saved smart collections skip the confirmation.

---

## Research Findings

### Industry UX Patterns (Capture One, Lightroom, Apple Photos)

All use a **row-based rule builder**: each row reads like a sentence (`Rating / is greater than / 3 stars`). Rules combine with a top-level AND/OR toggle. Capture One adds **nested search groups** for complex boolean logic.

What makes them work:
- Progressive disclosure (start with 1 rule, add more)
- Live results (matches update instantly)
- Sentence-like reading order

What none offer: natural language input.

### AI Image Source Detection (Evidence-Based)

Detection stores **evidence, not just labels**. Each detector adds signals; the UI shows confidence.

| Method | Reliability | Survives re-save? | Role |
|--------|------------|-------------------|------|
| **Metadata/C2PA** (PNG tEXt, EXIF, content credentials) | High when present | No (stripped) | Primary — highest trust |
| **Filename patterns** (DALL-E timestamps, ComfyUI_*, SD seeds) | Low (hint only) | N/A | Weak signal, never decisive |
| **Invisible watermarks** (SD's DwtDctSvd) | Moderate | Survives mild compression | Supporting signal |
| **CLIP similarity** | Visual only | Yes | "Looks like" — NOT source attribution |

**Key insight from Codex review**: CLIP clustering cannot reliably attribute source/vendor — it can detect visual style similarity but not "this is Midjourney." It should be labeled as visual similarity, not factual detection. "Photo" is not simply the negation of "AI" — edited, upscaled, composited, and AI-assisted images make this messy.

### Specific Metadata by Tool

- **DALL-E/ChatGPT**: C2PA manifests, filenames with "DALL·E" + timestamp
- **Stable Diffusion**: PNG tEXt chunks (`parameters`, `prompt`, `workflow`), AUTOMATIC1111 writes `.txt` sidecars
- **Midjourney**: EXIF `ImageDescription`/`UserComment`, Discord filename patterns
- **ComfyUI**: PNG `prompt` and `workflow` JSON chunks
- **Nanobanana**: Typically `Software` EXIF field

### NL Parsing Architecture (Deterministic-First)

**Revised per Codex feedback**: The fast path is a deterministic parser, not a ML classifier.

```
text → deterministic parse → validated FilterQuery
                                   ↓ (if incomplete)
                              local LLM parse (optional)
                                   ↓ (schema validate)
                              user-visible preview
                                   ↓ (if LLM unavailable)
                              cloud API (user permission required)
```

**Deterministic parser**: Regex + synonyms + field aliases + confidence scoring. Handles structured phrases like "5 stars", "recent", "midjourney", "landscape", "png", "wide", "not rejects". Faster, easier to debug, smaller, and less surprising than a ML classifier. Covers 90%+ of queries.

**Local LLM** (optional): Qwen2.5-1.5B via llama-cpp-rs with grammar-constrained decoding. For ambiguous or compound queries. Cold start may be 3-5s on consumer machines; warm latency 1-2s. Grammar-constrained decoding ensures valid JSON output.

**Cloud API** (last resort): Only with explicit user permission. Provider configurable in settings.

**Why not MiniLM**: Codex review pointed out that a 22MB classifier is overkill when the common queries are structured phrases with a fixed vocabulary. A deterministic parser is faster (<1ms), easier to debug, smaller, and less surprising.

### LLM Provider Architecture (~300-400 lines Rust)

```rust
trait LlmProvider {
    async fn parse_query(&self, request: QueryRequest) -> Result<FilterQuery>;
}

struct QueryRequest {
    text: String,
    available_fields: Vec<FieldDef>,
    locale: String,
    timezone: String,
    current_context: Option<CollectionContext>,
    existing_labels: Vec<String>,
}
```

- `OpenAiCompatibleProvider` → OpenAI, Groq, OpenRouter, Cerebras, DeepSeek, Qwen (swap base_url, but need provider capability flags for JSON schema support, grammar, max context, latency class)
- `AnthropicProvider` → separate (Messages API)
- `OllamaProvider` → local, OpenAI-compatible interface

**Provider capability flags** (per Codex):
- supports_json_schema
- supports_grammar
- supports_local_offline
- max_context
- expected_latency
- privacy_class

### Default Smart Collection Presets (~20)

**Rating & Triage**: 5 Stars, 4 Stars+, Picks, Rejects, Unrated
**Color Labels**: One per color (Red, Yellow, Green, Blue, Purple)
**Recency**: Imported Today, This Week, This Month
**Format**: Large (>4K), Small (<1024px), PNG, WebP, Square, Panoramic, Portrait, Landscape
**CLIP-Powered** (unique to us): Near-Duplicates, Outliers, Visual Clusters

**By Source** (sidebar section, not just filter rules):
Midjourney, Stable Diffusion, DALL-E / ChatGPT, Nanobanana, Photos, Unknown

---

## Technical Specification

### DB Schema Additions (on import)

```sql
-- Evidence-based source detection
ALTER TABLE images ADD COLUMN source_label TEXT;           -- 'midjourney', 'stable_diffusion', 'dalle', etc.
ALTER TABLE images ADD COLUMN source_confidence REAL;      -- 0.0 to 1.0
ALTER TABLE images ADD COLUMN source_evidence_json TEXT;   -- JSON: which detectors fired, what they found
ALTER TABLE images ADD COLUMN source_detected_at DATETIME;
ALTER TABLE images ADD COLUMN source_detector_version TEXT;
ALTER TABLE images ADD COLUMN is_ai_generated BOOLEAN;     -- separate from generator identity

-- Additional metadata
ALTER TABLE images ADD COLUMN ai_prompt TEXT;              -- extracted generation prompt if available
ALTER TABLE images ADD COLUMN aspect_ratio REAL;           -- computed from width/height
ALTER TABLE images ADD COLUMN orientation TEXT;            -- 'landscape', 'portrait', 'square'
ALTER TABLE images ADD COLUMN original_date DATETIME;     -- from EXIF if available
ALTER TABLE images ADD COLUMN megapixels REAL;            -- computed
```

### Smart Collection Storage

```sql
CREATE TABLE smart_collections (
    id INTEGER PRIMARY KEY,
    name TEXT NOT NULL,
    icon TEXT,
    rules_json TEXT NOT NULL,      -- serialized FilterNode tree
    nl_query TEXT,                 -- original natural language if used
    auto_update BOOLEAN DEFAULT TRUE,
    is_preset BOOLEAN DEFAULT FALSE,
    sort_order INTEGER DEFAULT 0,
    created_at DATETIME,
    updated_at DATETIME,
    schema_version INTEGER DEFAULT 1
);
```

### FilterQuery Schema (nested-capable from day one)

```typescript
// Internal storage — supports nesting even if v1 UI is flat
type FilterNode =
  | { type: 'group'; op: 'and' | 'or'; children: FilterNode[] }
  | { type: 'not'; child: FilterNode }
  | { type: 'rule'; field: Field; op: Operator; value?: Value };

type Field =
  | 'rating' | 'color_label' | 'decision'
  | 'format' | 'width' | 'height' | 'aspect_ratio' | 'orientation' | 'megapixels'
  | 'source_label' | 'source_confidence' | 'is_ai_generated' | 'ai_prompt'
  | 'folder' | 'imported_at' | 'original_date' | 'file_size'
  | 'clip_similar_to' | 'clip_text_match';

type Operator =
  | 'eq' | 'neq' | 'gt' | 'gte' | 'lt' | 'lte'
  | 'contains' | 'not_contains'
  | 'in' | 'not_in' | 'between'
  | 'is_empty' | 'is_not_empty'
  | 'before' | 'after' | 'last_n_days' | 'this_week' | 'this_month';

type Value =
  | string | number | boolean
  | string[]
  | { image_id: number; threshold?: number }   // for clip_similar_to
  | { text: string; threshold: number }         // for clip_text_match
  | { from: string; to: string };               // for between/date ranges
```

### v1 UI shows flat builder on top of nested-capable schema

The first version renders a single top-level group with an AND/OR toggle. The storage model supports full nesting so we don't have to migrate later. Capture One-style nested search groups can be added in v2 UI without schema changes.

---

## v1 Scope (Codex-recommended)

1. Option C UI: command bar + flat rule builder
2. Nested-capable FilterNode schema (flat UI on top)
3. Deterministic parser first, local LLM optional
4. Metadata-based source detection with evidence storage
5. CLIP-powered similarity rules clearly labeled as "visual similarity" not "factual detection"
6. Query explainability: "Parsed as..." preview
7. ~20 preset smart collections
8. Privacy controls: cloud parsing requires explicit user opt-in

### Deferred to v2+
- Nested rule groups in UI
- Smart collections referencing other collections
- CLIP clustering for source detection
- Schema versioning / migration tooling
- Import-time reprocessing when detector version changes
