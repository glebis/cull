# Agent-Native Image Workspace Standards

**Date:** 2026-06-16
**Status:** Draft for review
**Owner:** Gleb Kalinin

## Goal

Define Cull as an agent-native local image workspace and establish the first
standards that make that identity real. The standards come before a preset
generation API: agents should be able to read documented schemas, generate
valid recipes, and ask Cull to validate or apply them through the existing
CLI/MCP surface.

## Product Thesis

Cull is not just an image viewer. It is a local, private, scriptable image
workspace where humans visually curate image libraries and agents safely operate
on those libraries through documented contracts.

The useful analogy is "Obsidian for images," but the important part is not a
plugin marketplace. Obsidian works because notes are durable local artifacts
with conventions, links, templates, and automation hooks. Cull's equivalent is
images plus sidecars, collections, generation metadata, transform recipes,
export packages, publishing bundles, and agent-readable standards.

## Non-Goals

- Do not build a preset-generation API before the standards are stable.
- Do not make Cull a general creative-suite or cloud DAM.
- Do not require agents to mutate the user's original files.
- Do not make third-party model weights, fonts, or assets appear covered by
  Cull's Apache-2.0 source license.
- Do not add broad destructive operations to the standards v1 surface. Headless
  overwrite-like behavior must be controlled by explicit recipe fields,
  dry-run previews, path confinement, and audit events; any human confirmation
  happens in the app UI, MCP client, or operator workflow before invocation.

## Standards Stack

Cull standards should reuse existing standards where they fit and define small
Cull-specific schemas only where the product has domain-specific needs.

| Layer | Standard | Cull use |
|---|---|---|
| Normative language | RFC 2119 / RFC 8174 keywords | Use MUST, SHOULD, MAY, MUST NOT, and SHOULD NOT only when the document is intentionally normative. |
| Machine validation | JSON Schema 2020-12 | Validate recipe, package, sidecar, and preset files. |
| Compatibility | Semantic Versioning 2.0.0 | Version every published Cull schema with `schema_version`; breaking changes require a major version. |
| Media type identity | Vendor media type strings | Use stable identifiers such as `application/vnd.cull.export-recipe+json`. |
| Metadata interoperability | EXIF, IPTC, XMP, C2PA where available | Preserve and expose existing image metadata instead of inventing replacements. |
| Linked workspace concepts | URI/URL references plus Cull IDs | Recipes may reference images by stable Cull image id, file path, collection id, or `cull://` URL where appropriate. |
| Package integrity | Manifest plus checksums | Publish/export packages include a manifest and SHA-256 checksums for generated assets. |
| Auditability | Append-only operation events | Agent-applied standards record who/what invoked the operation, inputs, outputs, and affected paths. |

## Schema Governance

Every Cull standard document MUST contain:

- `schema`: one of the vendor media type strings in the registry below.
- `schema_version`: a SemVer 2.0.0 version string.

Cull validators MUST select the validator by the tuple
`(schema, schema_version)`. A validator MUST reject unknown media types. A
validator MUST reject unknown major versions unless an explicit migration path
is implemented. Minor and patch versions within a known major version MUST
remain backward-readable by validators for that major version.
Published Cull schemas MUST start at `1.0.0`; v1 will not use SemVer `0.x`
schema versions.

The initial media type registry is:

| Standard | Media type |
|---|---|
| Image set | `application/vnd.cull.image-set+json` |
| Export recipe | `application/vnd.cull.export-recipe+json` |
| Package recipe | `application/vnd.cull.package-recipe+json` |
| Generation sidecar | `application/vnd.cull.generation-sidecar+json` |
| Preset | `application/vnd.cull.preset+json` |

Schema tests SHOULD use Ajv's JSON Schema 2020-12 support for the TypeScript
test suite. Rust validation MAY use a different validator, but the v1 schemas
SHOULD avoid advanced 2020-12 features that are poorly supported across
validators when simpler `oneOf`, `const`, `enum`, `pattern`, and `if`/`then`
rules are enough.

## First Cull Standards

### Image Set Standard

Defines a portable selection of images. An image set MUST use the media type
`application/vnd.cull.image-set+json`. It is the common input type for
conversion, packaging, contact sheets, and publishing.

Required fields:

- `schema` MUST be `application/vnd.cull.image-set+json`.
- `schema_version` MUST be present.
- `id` MUST be a local identifier scoped to the current Cull library unless the
  document explicitly says it is portable.
- `created_at` MUST be an RFC 3339 timestamp with an explicit timezone offset.
- `source` MUST be one of `folder`, `collection`, `smart_collection`,
  `image_ids`, `search`, or `view_scope`.

The schema MUST model `source` as a discriminated union:

- `folder` MUST include `folder_path`.
- `collection` MUST include `collection_id`.
- `smart_collection` MUST include `smart_collection_id`.
- `image_ids` MUST include `items`, an array of Cull image ids.
- `search` MUST include `query`, a text query plus optional search options.
- `view_scope` MUST include `view`, and MAY include the active folder,
  collection, filters, sort order, and selection state.

Cull image ids and collection ids are local library identifiers. Portable image
sets SHOULD prefer paths, package manifests, or future `cull://` references
when the set must survive import into another library.

### Export Recipe Standard

Defines deterministic image export and conversion work. This is the first
standards target because the README already describes export and batch
operations as an existing/future agent surface.

Required fields:

- `schema` MUST be `application/vnd.cull.export-recipe+json`.
- `schema_version` MUST be present.
- `input` MUST be an inline image set or a reference to an image set document.
- `output` MUST include `root_dir`, `filename_template`, and
  `collision_policy`.
- `format` MUST be one of `original`, `jpeg`, `png`, `webp`, or `avif` for v1.
- `resize` MAY be omitted. If present, it MUST be an object with `mode`,
  optional `width`, optional `height`, optional `scale`, and `no_upscale`.
- `metadata_policy` MUST be one of `preserve`, `strip`, or `preserve_safe`.
- `dry_run` MUST be a boolean.
- `allow_overwrite` MUST be a boolean and MUST default to `false`.

`resize.mode` MUST be one of `fit`, `fill`, `width`, `height`, or `scale`.
`fit` and `fill` MUST include both `width` and `height`. `width` MUST include
`width`; `height` MUST include `height`; `scale` MUST include `scale`.
`no_upscale` MUST default to `true` when omitted.

`collision_policy` MUST be one of:

- `error`: fail if an output file already exists.
- `skip`: leave an existing output file unchanged.
- `rename`: generate a unique non-conflicting filename.
- `overwrite`: replace an existing output file only when `allow_overwrite` is
  also `true`.

Safety rules:

- Cull MUST validate output paths with the same path confinement policy used by
  existing export/static publishing code.
- Cull MUST validate each expanded `filename_template` result after parameter
  substitution. Expanded names MUST NOT be absolute paths, contain `..`
  traversal, or escape `root_dir` through separators, normalization, or
  symlink resolution.
- Cull MUST support dry-run reporting before writing files.
- Cull MUST NOT overwrite files unless the recipe explicitly opts into a
  collision policy that allows it and `allow_overwrite` is `true`.
- Validators and appliers MUST treat a missing `allow_overwrite` as `false`.
  The separate `collision_policy` plus `allow_overwrite` controls are
  intentional defense in depth.
- Cull MUST re-validate a recipe immediately before applying it, even if it was
  previously validated by `preview_recipe`.

Metadata rules:

- `preserve` MUST preserve metadata supported by the output encoder when doing
  so is technically possible.
- `strip` MUST remove EXIF, IPTC, XMP, and provider metadata from generated
  output. It SHOULD preserve color profile and orientation needed for correct
  display. C2PA handling MUST be explicit in the dry-run output because
  stripping provenance can break content authenticity.
- `preserve_safe` MUST remove location, camera serial, owner/author contact
  fields, and other common personally identifying metadata listed in the v1
  safe-strip registry. The initial registry MUST include GPS EXIF tags, camera
  serial/body serial fields, owner name, artist/author/creator fields,
  copyright contact fields, IPTC contact/location fields, XMP creator/contact
  fields, and software account/user identifiers when detected. It SHOULD
  preserve color profile, orientation, and non-sensitive generation metadata.

C2PA handling MUST distinguish original provenance from transformed output.
When pixels are resized, re-encoded, cropped, or otherwise changed, Cull MUST
NOT silently copy C2PA metadata as if the transformed file retained the original
cryptographic claim. The dry-run and apply result MUST report one of:

- `c2pa_preserved_original`: original bytes were exported unchanged and the C2PA
  claim remains valid.
- `c2pa_dropped`: C2PA data was removed.
- `c2pa_carried_historical`: provenance was carried only as historical metadata
  and marked invalidated by transform.
- `c2pa_resigned`: Cull or a plugin created a new valid claim for the output.

### Package Recipe Standard

Defines generated bundles for client delivery, editorial review, datasets,
archives, static sites, and future publishing plugins.

Required fields:

- `schema` MUST be `application/vnd.cull.package-recipe+json`.
- `schema_version` MUST be present.
- `input` MUST be an inline image set or a reference to an image set document.
- `package_type` MUST be one of `folder`, `zip`, `static_site`,
  `contact_sheet`, or `dataset` for v1. `custom` is out of scope for v1.
- `assets` MUST declare generated renditions, format, resize policy, and
  whether originals are included.
- `manifest` MUST declare which image metadata, generation metadata, licensing
  fields, and attribution fields are included.
- `integrity` MUST declare a checksum algorithm and MUST default to SHA-256.

Package manifests SHOULD include optional `license`, `source_attribution`, and
`rights_notes` fields for bundled/generated assets when that information is
available. Cull MUST NOT imply that third-party model weights, fonts, artwork,
or user-supplied assets are licensed under Cull's Apache-2.0 source license.

### Generation Sidecar Standard

Documents the shape Cull already partially supports through adjacent JSON
sidecars and `generation_runs`. This standard should not replace provider raw
JSON. It defines the normalized layer Cull can rely on while preserving the raw
payload for forward compatibility.

Required fields:

- `schema` MUST be `application/vnd.cull.generation-sidecar+json`.
- `schema_version` MUST be present.
- `provider`, `model`, and `prompt` MUST be present.
- `seed` MAY be present.
- `settings` MUST be an object. Provider-specific keys are allowed for forward
  compatibility.
- `raw_metadata` MAY contain the preserved original provider payload.
- `license`, `source_attribution`, and `rights_notes` MAY be present when known.

### Preset Standard

A preset is a named recipe template, not a code plugin. It may parameterize an
image set, export recipe, package recipe, or smart collection.

Required fields:

- `schema` MUST be `application/vnd.cull.preset+json`.
- `schema_version` MUST be present.
- `name` MUST be present.
- `kind` MUST be one of `image_set`, `export_recipe`, `package_recipe`, or
  `smart_collection` for v1. `workflow` is out of scope for v1.
- `target_schema` MUST be the media type of the standard document produced
  after substitution.
- `parameters` MUST declare named inputs. Each parameter MUST include `type`,
  `required`, and MAY include `default`, `enum`, `minimum`, `maximum`, and
  `description`.
- Parameter `type` MUST be one of `string`, `number`, `integer`, `boolean`,
  `path`, `enum`, `image_set_ref`, `collection_id`, or `format`.
- `template` MUST be a JSON object that becomes the target standard document
  after parameter substitution.

Preset substitution MUST use string placeholders of the form
`${parameter_name}`. A placeholder MAY occupy a whole JSON string value or a
substring within a JSON string. Substring placeholders MAY only reference
string-like parameters (`string`, `path`, `enum`, `image_set_ref`,
`collection_id`, or `format`). Non-string values MUST be supplied by making the
entire string value a placeholder and substituting the typed JSON value. Cull
MUST reject unknown placeholders, missing required parameters, invalid parameter
types, and unescaped literal placeholder syntax. Literal `${` MUST be escaped as
`$${`. Cull MUST validate the fully substituted template against
`target_schema` before preview or apply.

Agents can generate presets by writing JSON that validates against this schema.
Cull does not need a preset-generation API until repeated user workflows prove
that schema-only generation is insufficient.

## Agent Surface

The existing CLI/MCP rule remains: command names and parameter names match MCP
tool names and JSON schemas. Standards add these future tools without changing
the command model:

- `validate_standard_document` validates a file against a named Cull schema.
- `preview_recipe` returns a dry-run plan and warnings.
- `apply_recipe` executes a validated recipe.
- `list_standard_schemas` returns supported schema ids and versions.
- `explain_standard_errors` maps validation failures to human-readable fixes.

These commands should call shared Rust services rather than duplicate CLI/MCP
logic. They should never depend on a visible Tauri window.

`apply_recipe` MUST validate the recipe, compute a plan, enforce path
confinement, and emit audit events. Every mutating recipe application MUST emit
at least one audit event with: invoker identity, surface (`cli`, `mcp`, `ui`, or
`plugin`), schema, schema version, recipe hash, input summary, output paths,
operation result, and timestamp.

## Claude Opus Audit Protocol

Claude Opus can be used as an external reviewer, but it is not the standard.
The audit target is the written standards draft. The pass/fail rubric is:

1. **Requirement quality:** each MUST/SHOULD/MAY statement is testable,
   necessary, and scoped.
2. **Schema validity:** each proposed standard can become JSON Schema 2020-12
   without relying on prose-only constraints.
3. **Compatibility:** versioning, media type ids, and migration rules are clear.
4. **Safety:** path confinement, destructive operations, metadata stripping,
   licensing boundaries, and audit logging are explicit.
5. **Interoperability:** existing EXIF/IPTC/XMP/C2PA/provider metadata is
   preserved where possible rather than replaced.
6. **Agent ergonomics:** an agent can generate, validate, preview, and apply a
   recipe without hidden UI state.
7. **Human ergonomics:** a user can understand what a recipe will do before
   approving it.
8. **Scope control:** the standards do not imply a plugin marketplace,
   cloud service, or preset-generation API in the first implementation.

Audit verdicts use four levels:

- `APPROVE`: ready to turn into implementation issues.
- `APPROVE_WITH_CHANGES`: small edits needed, no re-architecture.
- `REWORK`: concept is sound but standards are ambiguous or too broad.
- `REJECT`: the direction is unsafe, incompatible with Cull, or not useful.

The audit prompt should ask Opus for findings ordered by severity, with file
and section references, followed by a verdict. Valid critique must be folded
back into this spec before implementation planning.

## Implementation Phasing

### Phase 1: Documented Standards

- Write JSON Schema files for image sets, export recipes, package recipes, and
  presets.
- Add examples for common workflows: client delivery folder, webp conversion,
  static review site, contact sheet, and dataset package.
- Add tests that every example validates.

### Phase 2: Validation Surface

- Add CLI/MCP validation tools.
- Validate recipe files without writing output.
- Return structured errors suitable for agents.

### Phase 3: Preview and Apply

- Add dry-run planning for export and package recipes.
- Add apply commands that route through existing export/static publishing
  services.
- Record audit events for agent-applied operations.

### Phase 4: Preset Ecosystem

- Treat presets as recipe templates first.
- Add a small in-app preset picker only after the schema-backed CLI path works.
- Consider plugin/API extensions only when schema-only presets prove too weak.

## Acceptance Criteria

- The first standard schemas are written as JSON Schema 2020-12 documents.
- Every schema has at least two valid examples and one invalid fixture.
- `npm test` or a dedicated schema test validates all examples.
- The standards include SemVer compatibility rules and media type ids.
- Export/package recipes include path safety, dry-run, collision, metadata, and
  checksum behavior.
- The standards are audited with Claude Opus using the rubric above, and the
  audit verdict plus accepted changes are recorded in the spec or a companion
  audit note.
- Follow-up implementation work is tracked as bd issues after the standards are
  approved.

## File Layout Decisions

- JSON Schema source files live under `schemas/`.
- Human-readable standards docs live under `docs/standards/`.
- Examples live beside the schema they exercise under `schemas/examples/`.
- Cull recipe files use descriptive suffixes such as `.cull-export.json`,
  `.cull-package.json`, and `.cull-preset.json`; media type strings remain the
  canonical in-file identifiers.
- Cull defines a smaller package manifest first. BagIt compatibility may be
  added later as an adapter if dataset/archive workflows require it.

## External Audit Status

Claude Opus audit was run on 2026-06-16 after unsetting `ANTHROPIC_API_KEY` so
the Claude CLI used the logged-in subscription. The first verdict was `REWORK`;
after fixes, the second verdict was `APPROVE_WITH_CHANGES`; after addressing
those findings, the final targeted verdict was `APPROVE`. Accepted fixes from
those audits were folded into this draft: normative field requirements,
discriminated image-set sources, media type registry, SemVer dispatch rules,
metadata policy semantics, collision policy and overwrite controls,
filename-template confinement, audit event requirements, licensing fields, v1
removal of `custom` package type and `workflow` preset kind, RFC 3339
timestamps, C2PA transform semantics, resize shape, and concrete preset
placeholder validation. User review is required before implementation planning.
