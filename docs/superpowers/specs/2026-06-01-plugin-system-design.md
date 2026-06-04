# Cull Plugin System Design

## Status

Approved for spec drafting on 2026-06-01 and reviewed with Claude before
implementation planning. This document incorporates the review feedback on
staging, process lifecycle, permission grants, and MCP boundaries.

## Goals

- Let plugins add export formats, size presets, packaging steps, and delivery
  destinations without recompiling Cull.
- Let plugins participate in export-oriented workflows through typed extension
  points rather than arbitrary lifecycle interception.
- Keep Cull authoritative for database writes, file staging, job tracking,
  permission grants, and UI/MCP exposure.
- Default to the least access possible. Users must grant or revoke plugin access
  explicitly.
- Preserve a path to future hooks without making v1 depend on broad hooks.

## Non-Goals For v1

- No in-process native dynamic libraries.
- No JavaScript runtime embedded in Cull.
- No arbitrary import, curation, watcher, thumbnail, search, or app lifecycle
  hooks.
- No direct plugin database access.
- No direct writes into the source library.
- No Cull-mediated MCP client bridge in v1. A destination plugin may run its own
  MCP client process if the user grants the relevant file, metadata, and network
  scopes, but Cull will not proxy arbitrary MCP calls until a dedicated MCP
  client contract exists.
- No OS-level network sandboxing in v1. Cull enforces the data and effect
  boundary it controls, but it cannot prove a normal external process never opens
  a socket.

## Architecture

V1 uses external process plugins. A plugin is a folder containing a manifest and
an executable command. Cull discovers manifests, validates them, displays
actions in the UI and MCP tool surface, stages inputs into a job directory, and
invokes the command with a JSON payload over stdin. The plugin returns a JSON
result over stdout.

Cull owns all durable effects:

- Database writes.
- Job creation, cancellation, pause/resume where applicable, and failure state.
- Input file staging and output path validation.
- Capability checks and user grants.
- UI visibility and MCP exposure.

Plugins produce outputs and request effects. Cull validates those effects before
applying them. A plugin process never receives a database connection or a hidden
Tauri command channel.

## Extension Points

V1 extension points are explicit actions:

### `export.transform`

Produces files from selected images, staged assets, or an export manifest.
Examples:

- Resize selected images into multiple dimensions.
- Convert to WebP/JPEG/PNG with plugin-specific compression settings.
- Package outputs into a ZIP or static folder.
- Render a custom export format not built into Cull.

### `export.destination`

Receives produced files and metadata, then sends them somewhere outside Cull.
Examples:

- Copy to a folder structure.
- Run a local publishing script.
- Upload to an API if network access is granted.
- Run a plugin-owned MCP client or bridge command.

Destination actions should not modify source library files. If they need to add
generated output back to Cull later, that should be a separate typed effect in a
future version.

## Future Hook Strategy

Cull should have a typed extension-point registry from the start so hooks can be
added later without changing the plugin manifest shape.

Recommended progression:

1. V1: explicit actions only, manually invoked by UI or MCP tools.
2. V2: observe-only hooks such as `import.completed`, `selection.changed`,
   `export.completed`, and `collection.updated`. Observe-only hooks cannot block
   the originating flow.
3. V3: controlled blocking hooks such as `export.before_render` and
   `import.before_add`, with strict timeout, ordering, capability, and effect
   contracts.

The system should not support "run on any process" as an untyped hook. Each hook
must have a stable name, input schema, output schema, timeout policy, and allowed
effect set.

## Discovery

Cull discovers manifests from two roots:

1. User-installed plugins:
   `~/Library/Application Support/com.glebkalinin.cull/plugins/<plugin-id>/plugin.json`
2. Development plugins in dev builds only:
   `<repo>/plugins/<plugin-id>/plugin.json`

The user-installed root is the production source of truth. Development plugins
must be hidden or clearly labeled in release builds.

Discovery rules:

- The folder name and manifest `id` must match.
- Plugin IDs use reverse-DNS style ASCII identifiers, for example
  `com.example.webp-pack`.
- Cull ignores manifests with invalid IDs, unsupported schema versions, missing
  commands, duplicate action IDs, or unsupported extension-point kinds.
- Cull records manifest hash and last validation status for display.
- If process spawn fails on macOS because a binary is not executable, missing,
  quarantined, or blocked by Gatekeeper, Cull surfaces a specific diagnostic in
  the plugin settings panel and job result.

## Manifest

Example:

```json
{
  "schema_version": 1,
  "id": "com.example.webp-pack",
  "name": "WebP Pack Exporter",
  "version": "1.0.0",
  "description": "Exports selected images as WebP variants.",
  "command": ["./webp-pack"],
  "timeout_seconds": 120,
  "actions": [
    {
      "id": "webp-pack",
      "kind": "export.transform",
      "label": "WebP Pack",
      "inputs": ["selected_images"],
      "required_scopes": [
        "files:staged-read",
        "files:write-output"
      ],
      "optional_scopes": [
        "metadata:read"
      ],
      "options_schema": {
        "type": "object",
        "properties": {
          "quality": {
            "type": "integer",
            "minimum": 1,
            "maximum": 100,
            "default": 82
          },
          "max_edge_px": {
            "type": "integer",
            "minimum": 64,
            "maximum": 8192,
            "default": 2048
          }
        }
      }
    }
  ]
}
```

Manifest rules:

- Commands are resolved relative to the plugin folder unless absolute.
- Commands may not include shell interpolation. Cull uses direct process spawn,
  not `sh -c`.
- `timeout_seconds` defaults to 60 and may not exceed Cull's hard cap of 300.
- Actions may declare required and optional scopes. Required scopes are needed
  to run the action. Optional scopes unlock richer input or effects but must not
  be required for a basic run.
- `options_schema` uses JSON Schema and is validated before invocation.

## Permission Model

Grants are stored per plugin. Actions declare required scopes. If a user runs an
action and the plugin lacks a required scope, Cull shows an escalation prompt for
the missing scopes. If the user approves, Cull adds those scopes to the plugin's
grant set and runs the action.

Default state:

- Newly discovered plugins are disabled or have no granted scopes.
- First run prompts for the minimum required scopes.
- Optional scopes are off until the user enables them.
- Revocation is immediate. The next action run receives less data or is blocked.
- A manifest update that requests new scopes must trigger a fresh prompt.

Grant storage should include:

- `plugin_id`
- granted scopes as JSON
- manifest hash seen when the grant was last changed
- timestamps for created and updated grant state

The UI should group scopes by risk and explain them in user-facing language.

## Scope Taxonomy

V1 scopes:

- `metadata:read`: read basic metadata such as dimensions, format, rating,
  decision, tags, collection names, and file display names.
- `metadata:private-read`: read prompts, generation metadata, sidecar metadata,
  and other provenance that may contain private text.
- `files:staged-read`: read Cull-staged copies in the job input directory.
- `files:library-read`: receive source library file paths directly.
- `files:write-output`: write files into the Cull-managed job output directory.
- `network:access`: run a plugin that may make network requests. Cull cannot
  enforce this fully without OS sandboxing, so the grant prompt must be explicit.
- `effects:open-url`: request that Cull open a URL after validation and user
  confirmation where appropriate.

Deferred scopes:

- `files:write-library`: write into source library folders.
- `mcp:call`: request Cull-mediated MCP calls.
- `effects:metadata-patch`: request Cull to mutate image metadata.
- `effects:collection-add`: request Cull to change collection membership.
- `secrets:read`: access plugin-specific credentials or tokens.

Deferred scopes need their own typed schemas before implementation. They must not
be implemented as free-form JSON patches or database writes.

## File Staging

Plugins receive paths, not image bytes embedded in JSON.

Default `files:staged-read` behavior:

- Cull creates a per-job input directory under the app data directory.
- Cull copies selected source files or resolved preview assets into that
  directory.
- For RAW or platform-only decodable formats, Cull may stage the preview asset
  rather than the original unless the action explicitly needs originals and the
  user grants `files:library-read`.
- Staged files are treated as disposable job inputs.
- Cull passes only paths inside the job input directory.

`files:library-read` behavior:

- Cull includes source library paths in the invocation payload.
- The grant prompt must clearly state that the plugin can read original files
  from the library.
- This is the escape hatch for trusted plugins that need original files or want
  to avoid copy cost.

Cull must not use symlinks or hardlinks for staged files in v1 because they blur
the boundary between staged copies and source files. Copying is slower but gives
the clearest safety semantics.

## Invocation Payload

Cull sends `PluginInvocation` over stdin:

```json
{
  "schema_version": 1,
  "job_id": "job_abc123",
  "plugin_id": "com.example.webp-pack",
  "action_id": "webp-pack",
  "kind": "export.transform",
  "granted_scopes": ["files:staged-read", "files:write-output"],
  "options": {
    "quality": 82,
    "max_edge_px": 2048
  },
  "input_dir": "/app-data/plugin-jobs/job_abc123/input",
  "output_dir": "/app-data/plugin-jobs/job_abc123/output",
  "images": [
    {
      "id": "img_001",
      "staged_path": "/app-data/plugin-jobs/job_abc123/input/img_001.jpg",
      "library_path": null,
      "metadata": {
        "width": 4032,
        "height": 3024,
        "format": "jpg"
      }
    }
  ],
  "export_manifest": null
}
```

Rules:

- `library_path` is non-null only with `files:library-read`.
- `metadata` contains only fields allowed by granted metadata scopes.
- Private generation metadata appears only with `metadata:private-read`.
- The payload may include an export manifest when the action input requires it.
- The payload does not include API keys, MCP tokens, or application secrets in v1.

## Result Payload

The plugin returns `PluginResult` over stdout:

```json
{
  "schema_version": 1,
  "status": "ok",
  "outputs": [
    {
      "path": "/app-data/plugin-jobs/job_abc123/output/img_001-2048.webp",
      "mime": "image/webp",
      "label": "WebP 2048px",
      "bytes": 248102
    }
  ],
  "warnings": [],
  "effects": []
}
```

Rules:

- Output paths must be inside the job output directory.
- Cull rejects paths that escape the output directory after canonicalization.
- `status` is `ok`, `partial`, or `error`.
- Warnings are shown in the job result.
- Effects are validated against granted scopes and the v1 allowlist.
- Unknown fields are ignored for forward compatibility, but unknown effect kinds
  are rejected.

V1 effect allowlist:

- `open_url`: requires `effects:open-url`, URL must be `https://` unless the user
  confirms another scheme.

All metadata, collection, import, and library-write effects are deferred.

## Process Lifecycle

Plugin jobs integrate with the existing Rust job service.

Rules:

- Cull creates a job before staging inputs.
- Cull updates progress at least for queued, staging, running, validating,
  completed, cancelled, and failed states.
- Plugin stdout is reserved for the result JSON.
- Plugin stderr is captured as diagnostic log output with a size cap.
- On timeout, Cull kills the child process and marks the job failed.
- On user cancellation, Cull kills the child process and marks the job
  cancelled.
- On Unix/macOS, Cull should kill the process group where practical so child
  subprocesses do not outlive the plugin parent.
- Cull performs best-effort cleanup of job input directories after completion,
  cancellation, or failure. Output directories are retained until the user clears
  the job or export result.

Timeout policy:

- Manifest default: 60 seconds.
- Manifest maximum: 300 seconds.
- Cull hard cap: 300 seconds for v1.
- Future long-running plugins should become resumable jobs with progress
  reporting before the cap is raised.

## MCP Boundary

Cull is already an MCP server. V1 plugin support should expose plugin actions to
Cull's existing MCP tools as actions that can be listed and invoked, subject to
the same grants as the UI.

V1 does not make Cull an MCP client for plugins. Therefore:

- `mcp:call` is deferred.
- A destination plugin can run its own MCP client command, but Cull treats that
  as normal external process behavior.
- If that plugin needs network access, the user must grant `network:access`.
- If that plugin needs staged files or metadata, those scopes must also be
  granted.

This satisfies the "pipe exported files into MCP-like workflows" use case
without inventing an underspecified Cull-mediated MCP call path.

## UI Requirements

Plugin settings should show:

- Installed plugin name, version, ID, validation status, and manifest path.
- Enabled/disabled state.
- Granted scopes grouped by risk.
- Actions exposed by the plugin and the scopes each action requires.
- Manifest hash or "updated since last grant" warning when new scopes appear.
- Last run status and recent diagnostics.
- Buttons to grant, revoke, disable, reveal plugin folder, and run validation.

Grant prompts should state:

- Which plugin is asking.
- Which action triggered the request.
- Which new scopes are requested.
- What each scope means in plain language.
- Whether network access is involved.

## Data Model

Add a plugin grant table rather than overloading generic app settings:

```sql
CREATE TABLE plugin_grants (
  plugin_id TEXT PRIMARY KEY,
  enabled INTEGER NOT NULL DEFAULT 0,
  granted_scopes_json TEXT NOT NULL DEFAULT '[]',
  manifest_hash TEXT,
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL
);
```

Plugin manifests themselves remain file-backed. Cull can rebuild the discovered
plugin list from disk and join grant state by `plugin_id`.

Job results should use the existing job system where possible. Plugin-specific
result metadata can be stored as JSON in the job result payload or a narrow
plugin job result table if the existing schema cannot represent output files.

## API Surface

Tauri commands:

- `list_plugins`
- `validate_plugin`
- `set_plugin_enabled`
- `grant_plugin_scopes`
- `revoke_plugin_scopes`
- `list_plugin_actions`
- `run_plugin_action`
- `get_plugin_job_result`

MCP tools:

- `list_plugin_actions`
- `run_plugin_action`

MCP callers must not bypass plugin grants. Remote MCP access should receive the
same redacted paths and metadata policy already used by existing MCP tools.

## Error Handling

Manifest validation errors are non-fatal to app startup. Invalid plugins are
listed as invalid with diagnostics.

Runtime errors:

- Spawn failure: job failed with executable/path/quarantine diagnostic.
- Timeout: job failed and child killed.
- Cancelled: job cancelled and child killed.
- Invalid stdout JSON: job failed with parse diagnostic.
- Output path escape: job failed and escaping outputs ignored.
- Missing required output: job failed if the action contract requires output.
- Unknown requested effect: job failed or partial, depending on whether outputs
  are still valid.

## Testing

Rust unit tests:

- Manifest parsing and validation.
- Plugin ID and folder matching.
- Scope grant/effective scope calculation.
- Action escalation detection.
- Path containment and canonicalization.
- Result payload validation.
- Timeout and cancellation behavior with a fixture process.
- Metadata redaction by scope.

Integration tests:

- Fixture transform plugin exports resized WebP/JPEG variants.
- Fixture destination plugin writes a summary file to an output folder.
- Invalid plugin manifests appear in validation results without crashing.
- Revoked scopes remove metadata or block action execution.

Frontend tests:

- Plugin settings render validation status and grants.
- Escalation prompt appears for missing scopes.
- Revocation updates the displayed grant state.

Manual checks:

- macOS executable permission failure.
- macOS quarantined binary diagnostic.
- Large image staging performance.
- Cancelled plugin does not leave a running child process.

## Open Decisions Closed By This Spec

- Grants are per plugin, not per action.
- Action-triggered escalation adds plugin-level grants only after explicit user
  approval.
- V1 uses copied staged files by default, not symlinks, hardlinks, or bytes in
  JSON.
- Cull-mediated `mcp:call` is deferred until Cull has a typed MCP client bridge.
- Broad lifecycle hooks are deferred, but the manifest/action model leaves room
  for typed hooks later.
