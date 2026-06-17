# Art Catalog Implementation Resolution Notes (v1)

Date: 2026-06-17
Status: Draft — ready for execution

These notes resolve the open blockers from the Opus catalog implementation audit.

## 1) Capability model (resolved)

We will align with the current token role model and avoid a parallel permission
system until a dedicated catalog RBAC migration is needed.

Required token capabilities:

- `catalog:read` — read catalog records, presets, fields, and approved values
- `catalog:write` — create/update draft catalog values and work/image links
- `catalog:approve` — approve or reject draft values
- `catalog:admin` — manage presets and field definitions

Token role mapping in v1:

- `viewer` → `catalog:read`
- `curator` → `catalog:read`, `catalog:write`
- `operator` → `catalog:read`, `catalog:write`
- `admin` → `catalog:read`, `catalog:write`, `catalog:approve`, `catalog:admin`

Policy:

- Agents are provisioned through tokens with `curator`/`operator` and therefore can
  only draft.
- Local app context may expose approval actions directly because it is the human
  control surface.
- If we discover requirement for third-party non-human approve automation, we do
  that as a separate RBAC design task, not v1.

## 2) Auth-independent safety guard (resolved)

- Agent-originated suggestion writes are always stored as drafts regardless of auth
  context.
- `source_type = agent` is enforced as `status = draft`.
- `catalog:approve` is never accepted implicitly by jobs.
- `catalog:approve` also requires either `admin` token role or local context.

## 3) Search and smart-collection queryability (resolved)

There is no generic catalog filter yet in the current search engine. v1 will ship the
following explicit additions in this order:

1. Add `Filter::CatalogField` and parsing in `smart_collections.rs`.
2. Map field keys with approved-only semantics:
   - default: value exists for `subject = image | work` and `status = approved`
   - support operator `=` and `contains` on rendered scalar text.
3. For v1 numeric/date/dimension filtering:
   - if field definition is `number/integer/money/date`, query path uses
     `CAST(json_extract(value_json, '$.value') AS REAL)` or `date()` matching.
   - add conversion-robust test coverage for invalid values.
4. Do not promise full arbitrary operators before query support exists. The UI will
   expose only supported operators from the schema parser.

## 4) Typed field model and dedupe (resolved)

- Draft dedupe is required to prevent suggestion flood:
  unique key on `(subject_type, subject_id, field_def_id, source_type, source_id)`
  with latest-write wins by updated timestamp.
- For typed filtering, numeric/date fields are treated as render-and-cast candidates.
  If cast fails, row is excluded from numeric/date operators.
- `image_metadata` remains machine metadata; catalog is curated business metadata.
  Duplicate imports into both stores are disallowed by design.

## 5) Plugin command surface governance (resolved)

- The command allowlist in the plugin host invoke path will explicitly include all
  catalog Tauri commands.
- Catalog commands are not exposed through ad-hoc paths.
- MCP and CLI use only these commands:
  - read: `list_catalog_presets`, `get_catalog_preset`, `list_catalog_fields`,
    `get_catalog_record`, `list_catalog_values`, `list_catalog_drafts`
  - write: `create_catalog_work`, `attach_images_to_catalog_work`,
    `set_catalog_draft_value`, `set_catalog_draft_values`
  - approval: `approve_catalog_values`, `reject_catalog_values`
  - admin: `create_catalog_field_def`, `deprecate_catalog_field_def`,
    `create_catalog_preset`, `update_catalog_preset`
  - jobs: `suggest_catalog_values`, `get_catalog_suggestion_job`, `cancel_job`

## 6) Searchability and sensitive fields (resolved)

- `catalog:read` remains sensitive-capable and not equivalent to `library:read`.
- No field-level ACL for `private/commercial` is implemented in v1.
- For v1 behavior:
  - `catalog:read` with token includes all catalog fields.
  - if this is too open, we will gate sensitive modes explicitly in a follow-up issue
    before product rollout.

## 7) What changes in execution order

1. Lock capability mapping + token model first (`tokens.rs` + tests).
2. Add catalog migration + schemas with dedupe/constraints.
3. Add command layer and module allowlisting.
4. Add job suggestion + forced-draft constraints.
5. Add smart collection catalog filters (text/scalar first, typed operators next).
6. Build plugin surface with explicit draft/approved states.
7. Add export hooks and finalize.

No remaining Opus blocker is considered unresolved for v1 planning.
