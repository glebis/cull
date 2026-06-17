# Art Catalog Metadata Layer Design

**Date:** 2026-06-16
**Status:** Draft for user review
**Owner:** Gleb Kalinin

## Goal

Build a generative-art-first catalog layer for Cull that turns imported images
and existing generation metadata into curated artwork records. The layer should
also support physical artist inventory, photo/DAM metadata, and future delivery
plugins without making the first version a full museum CMS.

The catalog must be agent-native: MCP, CLI, the app UI, and first-party plugin
views all use the same Rust service, capability checks, and audit path. Agents
may suggest catalog data, but human approval remains the trust boundary.

## Context

Cull already stores:

- Imported images, file paths, dimensions, format, hashes, and thumbnails.
- `image_metadata` key/value/source rows for enrichment and vision metadata.
- Normalized tags promoted from source, generation, vision, and file facts.
- `generation_runs` with prompt, negative prompt, provider, model, settings,
  seed, raw sidecar/API metadata, and image links.
- Quality, color, perceptual-hash, source-detection, and curation data.

The plugin runtime is not a safe place for arbitrary database mutation. Plugins
are frontend ESM bundles with a narrow `host.invoke` bridge. Rust enforces
plugin permissions and audit logging. The catalog therefore belongs in core
Rust, with plugin/UI surfaces calling whitelisted commands.

## Standards Position

Cull should use external standards as crosswalk and export targets, not as the
internal editing schema.

Relevant standards:

- VRA Core and CCO/CDWA inform the Work/Image split and artist inventory
  vocabulary.
- IPTC Photo Metadata, XMP, and C2PA inform embedded image/provenance export,
  including generative AI prompt/system/source fields.
- Dublin Core, schema.org `VisualArtwork`, and IIIF Presentation are practical
  lightweight publication/export targets.
- Linked Art and CIDOC CRM are useful for future JSON-LD export, but too heavy
  for the internal v1 editing model.

Internal schema rule: native, typed, preset-driven tables first. Standards
crosswalk hints live on field definitions and are consumed by serializers.

## Non-Goals

- No direct third-party plugin database writes.
- No C2PA/XMP/IPTC write-back to original files in v1.
- No Linked Art/CIDOC CRM internal graph model in v1.
- No automatic approval of agent-generated catalog values.
- No forced work record for every imported image.
- No Google Drive or cloud delivery implementation in this catalog v1.
- No full authority/vocabulary management for artists, materials, or clients in
  v1.

## Core Model

The catalog has two subject levels:

1. `image`: a Cull image asset. It can hold file/asset-specific catalog values.
2. `work`: an optional curated artwork/inventory record. It can link to one or
   more images.

Works are created lazily when a user or authorized tool starts curation. An
import does not create one work per image.

### Work/Image Resolution

Fields may be defined as work-level, image-level, or both. Consumers resolve
values by field policy:

- `work_default`: image inherits the work value unless the image has its own
  approved override.
- `image_only`: value belongs to the image and is never inherited.
- `work_only`: value belongs to the work and does not appear as an image value.
- `derived`: value is computed from an existing core source, such as
  `generation_runs`, and is not duplicated as editable catalog data.

This keeps generative-image records, physical-work inventory, and photo DAM
records in one model without pretending every image is the same kind of object.

## Data Model

Suggested v1 tables:

### `catalog_works`

- `id`
- `primary_image_id`
- `created_at`
- `updated_at`
- `deleted_at`

The table stays narrow. Most data lives in typed field values.

### `catalog_work_images`

- `work_id`
- `image_id`
- `role`: `primary`, `alternate`, `detail`, `source`, `reference`,
  `rendition`, or `other`
- `ordinal`
- `edition_label`
- `created_at`

V1 uses the link row for simple edition/version labels. A separate edition table
is deferred.

### `catalog_field_defs`

- `id`
- `stable_key`: for example `artist_inventory.height`
- `label`
- `description`
- `subject_scope`: `image`, `work`, or `both`
- `value_type`: `text`, `long_text`, `number`, `integer`, `money`,
  `dimension`, `date`, `boolean`, `enum`, `reference`, `json`
- `cardinality`: `single` or `multi`
- `unit_kind`: optional, such as `length`, `weight`, `currency`
- `validation_json`
- `sensitivity`: `normal`, `private`, or `commercial`
- `derived_source`: optional source pointer, such as
  `generation_runs.prompt`
- `crosswalk_json`: optional mappings to DC, schema.org, IPTC, VRA, IIIF,
  Linked Art
- `version`
- `supersedes_field_def_id`
- `created_at`
- `deprecated_at`

Field definitions are append-mostly. Renames create aliases. Type changes create
new field definitions rather than coercing existing values.

### `catalog_presets`

- `id`
- `name`
- `description`
- `preset_kind`: `generative_art`, `artist_inventory`, `photo_dam`,
  `asset_delivery`, or `custom`
- `field_def_ids_json`
- `layout_json`
- `version`
- `created_at`
- `updated_at`

Presets define which fields are visible, their order, grouping, and lightweight
layout preferences. They do not own the values.

### `catalog_field_values`

- `id`
- `subject_type`: `image` or `work`
- `subject_id`
- `field_def_id`
- `value_json`
- `display_value`
- `source_type`: `user`, `agent`, `derived`, `import`, `plugin`, `mcp`,
  `cli`
- `source_id`: token id, plugin id, model run id, or local user marker
- `confidence`
- `status`: `draft`, `approved`, `rejected`, `superseded`
- `approved_by`
- `approved_at`
- `created_at`
- `updated_at`

The table stores typed JSON so dimensions, money, dates, and multi-values are
not flattened into ambiguous strings.

### `catalog_value_events`

- `id`
- `value_id`
- `event_type`: `created`, `updated`, `approved`, `rejected`, `superseded`,
  `deleted`
- `actor_type`
- `actor_id`
- `before_json`
- `after_json`
- `created_at`

The existing audit table remains the top-level MCP/plugin audit path. This table
is the field-level history needed by the catalog UI.

## Capabilities

The initial v1 capability set uses these names and maps to token roles as follows:

| Capability | Purpose |
|---|---|
| `catalog:read` | Read works, presets, field definitions, and field values. |
| `catalog:write` | Create works, attach images, and write draft field values. |
| `catalog:approve` | Promote drafts to approved values or reject drafts. |
| `catalog:admin` | Create/edit/deprecate field definitions and presets. |

Role mapping in v1:

- `viewer`: `catalog:read`
- `curator`: `catalog:read`, `catalog:write`
- `operator`: `catalog:read`, `catalog:write`
- `admin`: `catalog:read`, `catalog:write`, `catalog:approve`, `catalog:admin`

`catalog:write` never implies approval. This separation is the main safety
boundary for agentic curation.

Agents normally receive `catalog:read` plus `catalog:write`. They do not receive
`catalog:approve` or `catalog:admin` by default. Chat-driven preset changes can
be drafted, but applying them requires a human/admin command.

## Agentic Suggestion Flow

Agent curation is a job-based workflow:

1. User selects images or a collection.
2. User chooses a preset, such as Artist Inventory or Generative Art.
3. Agent reads allowed image facts, generation runs, tags, color/quality metrics,
   and optional vision metadata.
4. Agent writes draft catalog values through `catalog:write`.
5. UI presents a review queue with confidence, source, and per-field diffs.
6. Human approval promotes selected drafts through `catalog:approve`.

Rules:

- Agents never silently overwrite approved user values.
- Approved user values outrank approved agent values.
- Drafts are deduplicated by subject, field, source, and run id so reruns do not
  flood the queue.
- Batch suggestion jobs must be cancellable and may write drafts incrementally.
- Prompt injection from filenames, EXIF, OCR, or user notes is contained because
  agent output remains draft until approved.

## Generative Art Behavior

Generation metadata is not retyped as editable catalog values. The catalog reads
prompt, model, provider, seed, settings, and raw sidecar/API metadata from
`generation_runs`.

The Generative Art preset should include derived read-only fields:

- prompt
- negative prompt
- provider
- model
- seed
- settings/workflow summary
- source image links when available

It should include authored catalog fields:

- title
- series
- edition or version label
- description
- display notes
- rights statement
- intended use
- sale or delivery notes

This prevents drift between the catalog and the existing generation-run source
of truth.

## Initial Presets

### Generative Art

Purpose: catalog AI-generated or computational artwork.

Fields:

- title
- series
- edition/version
- description
- rights statement
- display notes
- derived prompt
- derived provider/model/seed
- derived workflow/settings summary
- source images

### Artist Inventory

Purpose: practical inventory for physical works.

Fields:

- name
- series
- year
- materials
- height
- depth
- weight
- price
- description

Types:

- height/depth: dimension with length unit
- weight: number with weight unit
- price: money with currency
- year: date with support for year-only values

### Photo DAM

Purpose: organize photographic assets and raw/rendition workflows.

Fields:

- title
- capture date
- location
- creator
- keywords
- rights
- client/project
- delivery status

### Asset Delivery

Purpose: drive export and future Google Drive/client delivery workflows.

Fields:

- title
- approved rendition
- usage rights
- client-facing filename
- destination
- delivery note
- delivery status

The delivery preset should be designed so future cloud plugins consume catalog
metadata and export manifests instead of inventing their own naming/status
system.

## Commands And Surfaces

Tauri commands, MCP tools, and CLI tools should share service functions. Command
names and parameter names should follow the existing MCP-aligned CLI standard.

Suggested read tools:

- `list_catalog_presets`
- `get_catalog_preset`
- `list_catalog_fields`
- `get_catalog_record`
- `list_catalog_values`
- `list_catalog_drafts`

Suggested write tools:

- `create_catalog_work`
- `attach_images_to_catalog_work`
- `set_catalog_draft_value`
- `set_catalog_draft_values`

Suggested approval tools:

- `approve_catalog_values`
- `reject_catalog_values`

Suggested admin tools:

- `create_catalog_field_def`
- `deprecate_catalog_field_def`
- `create_catalog_preset`
- `update_catalog_preset`

Suggested agent/job tools:

- `suggest_catalog_values`
- `get_catalog_suggestion_job`
- `cancel_job` can reuse the existing job surface.

Plugin runtime whitelist:

- The first-party Catalog plugin should call only these tools through
  `host.invoke`.
- Third-party plugins may request catalog capabilities later, but still cannot
  bypass Rust enforcement.

## Search And Smart Collections

Catalog values must not become a silo. V1 must make approved values queryable
by smart collections and search.

Minimum:

- Approved values must be filterable by field key through a new `CatalogField` filter
  path; text operators (`=` / `contains`) are available in v1.
- Numeric/date fields use cast-based operators where safe and only after schema-level
  enabling.
- Draft values are hidden from normal search unless explicitly requested.
- Derived generation-run fields remain searchable through existing generation
  metadata paths.

## Export Crosswalk

Each field definition may carry crosswalk hints:

- Dublin Core: `title`, `creator`, `description`, `date`, `rights`,
  `subject`.
- schema.org: `VisualArtwork`, `CreativeWork`, `ImageObject`,
  `additionalProperty`.
- IPTC/XMP: creator, rights, description, keywords, digital source type, and
  future generative AI fields.
- IIIF: manifest metadata pairs and image annotations.
- VRA/Linked Art: future mapping for Work/Image, agent, material, measurements,
  rights, source, style/period, subject, and technique.

V1 should export simple JSON and schema.org/Dublin Core shaped metadata. IIIF
and Linked Art JSON-LD can follow once the internal field model is stable.

## Google Drive Follow-Up

Google Drive is a separate asset delivery plugin/module, tracked as
`imageview-59m`.

Catalog requirements for that future plugin:

- Delivery plugin reads approved catalog values with `catalog:read`.
- Delivery plugin reads export manifests with `export:read`.
- Delivery state writes go through narrow catalog commands and remain audited.
- Network transfer must be Rust-mediated and capability-gated. The plugin
  should not self-report delivery success without the Rust side authorizing or
  performing the transfer.
- Export naming should use the catalog delivery fields and existing export
  preset concepts, not a second naming system.

## Migration And Safety

- Migrations must not mutate or delete existing `cull.db` data beyond adding
  new tables/indexes.
- New tables should use foreign keys with conservative delete behavior.
- Image deletion or missing-file state must not silently erase catalog history.
- Work records should support soft delete via `deleted_at`.
- Catalog read is sensitive because it can include price, client, rights, and
  commercial delivery information. Treat `catalog:read` as separate from
  `library:read`.

## Testing

Rust unit tests:

- Migration creates all catalog tables and preserves existing images.
- Field definitions validate typed values and cardinality.
- Presets reference existing field definitions only.
- Derived generation fields read from `generation_runs` and do not duplicate
  editable values.
- Capability mapping distinguishes `catalog:read`, `catalog:write`,
  `catalog:approve`, and `catalog:admin`.
- Agent writes create drafts only.
- Approval changes status and writes field-level history.
- Approved user values are not overwritten by agent drafts.

CLI/MCP tests:

- CLI command names match MCP tool names.
- `catalog:read` can read records but cannot write drafts.
- `catalog:write` can write drafts but cannot approve.
- `catalog:approve` can approve/reject drafts.
- Plugin bridge dispatch allows only whitelisted catalog tools and audits every
  call.

Frontend tests:

- Preset field list renders the expected Artist Inventory fields.
- Draft suggestion review shows confidence/source/status.
- Approving a draft updates the visible approved value.
- Derived generation fields render read-only from `generation_runs`.

Manual checks:

- Large batch suggestion job can be cancelled.
- Artist Inventory form handles dimensions, weight, price, and year-only dates.
- Generative Art preset stays useful when only sidecar metadata is available.

## Phasing

### Phase 1: Core Catalog Schema And Permissions

- Add catalog tables, models, and migrations.
- Seed initial field definitions and presets.
- Add `catalog:read`/`catalog:write` and role-aligned capability mapping.
- Add hard rule: agent/plugin suggestions are draft-only, regardless of auth context.
- Add CLI/MCP/Tauri read and draft-write commands.

### Phase 2: Catalog UI Plugin

- Add first-party Catalog plugin/tab.
- Render preset forms and draft review queue.
- Show derived generation fields read-only.
- Support manual creation of works and image attachments.

### Phase 3: Agent Suggestions

- Add job-based `suggest_catalog_values`.
- Use existing generation, tag, color, quality, and vision metadata as inputs.
- Store suggestions as draft values with source/confidence.

### Phase 4: Export And Delivery Readiness

- Add schema.org/Dublin Core JSON export.
- Add catalog values to static publishing/export manifests.
- Prepare asset-delivery fields for the future Google Drive plugin.

### Deferred V2+

- Controlled vocabularies and authority records.
- Editions as first-class entities.
- Linked Art/CIDOC CRM JSON-LD export.
- IIIF manifests.
- C2PA/XMP/IPTC write-back.
- Field-level ACL.
- Multilingual values.

## Open Decisions

- Exact date representation for year, circa, and ranges. Recommendation:
  support year-only in v1 and evaluate EDTF before ranges/circa become UI
  requirements.
- Whether money/currency should use a strict ISO 4217 enum in v1.
- Whether field-level ACL (`private`/`commercial`) is required in v1.
- Whether strict typed numeric/date query casting should be expanded beyond simple
  operators.

## References

- VRA Core, Library of Congress:
  https://www.loc.gov/standards/vracore/
- Cataloging Cultural Objects:
  https://vraweb.org/resources/cco/
- Getty CDWA:
  https://www.getty.edu/publications/categories-description-works-art/
- Linked Art:
  https://linked.art/model/
- CIDOC CRM:
  https://cidoc-crm.org/
- IPTC Photo Metadata:
  https://www.iptc.org/std/photometadata/specification/IPTC-PhotoMetadata
- IPTC AI metadata fields:
  https://iptc.org/news/iptc-photo-metadata-standard-2025-1-adds-ai-properties/
- C2PA specification:
  https://spec.c2pa.org/specifications/specifications/2.4/specs/C2PA_Specification.html
- Dublin Core Terms:
  https://www.dublincore.org/specifications/dublin-core/dcmi-terms/
- schema.org VisualArtwork:
  https://schema.org/VisualArtwork
- IIIF Presentation API 3.0:
  https://iiif.io/api/presentation/3.0/
