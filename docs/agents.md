<!-- Copyright (c) 2026-present Gleb Kalinin. Architecture and design by author. -->
<!-- Implementation assisted by Claude (Anthropic). See AUTHORSHIP.md. -->

# The Agent Surface

Cull is an agent-native image tool: the tool layer **is** the product API. The
same tool names and JSON parameter fields are reachable two ways:

1. **Headless CLI** — `cull <tool>` runs a curated slice of tools directly
   against the SQLite library, no server and no token required. Good for
   scripted import/export/library/embedding/quality work.
2. **MCP server** — `cull --mcp-stdio` (or `cull --mcp-http`) exposes the full
   tool catalogue, including token management, the agent-snapshot loop, and the
   audit log. This is how an agent drives the *live* app.

This doc is meant to be runnable by a stranger with a token. Every flag and tool
name below is verified against the source and pinned by a docs-contract test
(`src/lib/open-source-release-contract.test.ts`) so it cannot silently drift.

> Verify CLI flags in `src-tauri/src/cli/mod.rs`, the headless tool slice in
> `src-tauri/src/cli/tools/mod.rs`, and MCP tool names in
> `src-tauri/src/mcp/tools.rs`.

## 1. Headless CLI

The CLI uses MCP tool names and MCP JSON field names so agents reuse one mental
model across CLI and MCP. Pass `--json` for machine-readable output.

```bash
# Library introspection
cull --json get_library_stats
cull --json list_folders
cull --json list_collections

# Import
cull --json import_folder --folder_path ~/renders
cull --json import_files --file_paths ~/renders/a.png,~/renders/b.png

# Export
cull --json export_images --collection_id <id> --output_dir ~/Desktop/export --format original

# Generic escape hatch: call any supported tool by name with raw JSON params
cull --json call_tool import_folder --params_json '{"folder_path":"/Users/me/renders"}'
```

Global flags: `--json` / `-j`, `--db <path>` (use a specific SQLite file), and
`--app-data-dir <path>` (thumbnails/exports location).

Supported headless tools: `get_library_stats`, `list_images`, `list_folders`,
`list_collections`, `import_folder`, `import_files`,
`get_embedding_model_download_info`, `download_embedding_model`,
`generate_embeddings`, `analyze_image_quality`, `get_image_quality`,
`get_quality_count`, `list_export_presets`, `export_images`.

The headless CLI is a **slice**, not the whole MCP surface. Token management,
snapshots, and the audit log below run over the MCP server, not the CLI.

## 2. Approval and confirmation boundary

Cull can expose MCP and CLI tools, but it cannot guarantee that the calling
agent can surface or answer its own confirmation prompts through those tools.
Claude Code's Agent SDK, for example, routes approval requests through the
client `canUseTool` callback and currently documents `AskUserQuestion` as not
available inside subagents spawned via the Agent tool:
<https://code.claude.com/docs/en/agent-sdk/user-input#limitations>.

Treat Cull tool calls as execution surfaces, not as the confirmation mechanism
for critical decisions. Do not rely on an agent asking itself "are you sure?"
before operations such as file removal, token revocation, audit-log pruning, or
broad destructive batch changes. Put those confirmations in the app UI, MCP
client, shell wrapper, or human operator workflow before the tool call is made.

## 3. Connecting an agent over MCP

Cull runs an MCP server inside the app over a local Unix socket. `cull
--mcp-stdio` is a thin stdio↔socket bridge that launches the app in tray mode if
it is not already running. Drop this into an MCP client config (the same snippet
the in-app Settings → Access Tokens panel copies):

```json
{
  "mcpServers": {
    "cull": {
      "command": "cull",
      "args": ["--mcp-stdio"]
    }
  }
}
```

For HTTP/SSE instead of stdio:

```bash
cull --mcp-http            # loopback, default port 9847
cull --mcp-http 8080       # custom port
```

HTTP binds to `127.0.0.1` by default. Binding to a non-loopback host requires
**both** `--mcp-http-host <host>` and the explicit `--mcp-http-allow-remote`
flag, and you should only do that with scoped, least-privilege tokens.

## 4. Creating a scoped token

Two ways:

- **UI**: Settings → Access Tokens → Create. Pick a role (viewer / curator /
  operator / admin) and an expiry window (90 days recommended). The secret is
  shown once.
- **MCP**: call the `create_token` tool over a connected admin session. It
  accepts `name`, `role`, an optional `scope` (collections / folders / tags),
  and an optional RFC 3339 `expires_at` (defaults to 90 days). It returns the
  token id and the one-time secret.

```jsonc
// MCP call_tool: create_token
{
  "name": "Demo Agent",
  "role": "curator",
  "expires_at": "2026-09-08T00:00:00Z"
}
```

Tokens expire; the Settings token list and the Privacy dashboard's **Agent
Access Log** show relative expiry ("expires in 12 days" / "expired 3 days ago")
and surface `_auth_failed` rows so failed-auth attempts are visible, not just
stored. Rotate before expiry with the "Rotate to renew" affordance.

## 5. The agent_snapshots demo loop

The snapshot tools turn the visible app into a multimodal observe→act loop. They
are **local stdio only** in v1 (they drive the live window, so they are not
exposed over HTTP).

1. **Capture** the current view. `capture_current_view_snapshot` writes
   `raw.png`, `annotated.png` (numbered overlays), and `manifest.json`, and
   returns their local paths. Pass `{"clipboard": true}` to also copy the
   annotated PNG to the clipboard.
2. **Inspect** with `get_last_view_snapshot` (optionally `{"snapshot_id": ...}`)
   to re-read the latest manifest and file paths — feed `annotated.png` to a
   multimodal model and reason over the numbered labels.
3. **Act** with `select_images_in_view`, passing the chosen `image_ids`, an
   optional `mode` (`replace` / `add` / `toggle`), and optional
   `focus_first`. The live grid selection updates.
4. **Audit**: `get_audit_log` (optional `{"limit": N}`, default 50) returns the
   recent tool-call trail, including the token-management and `_auth_failed`
   rows the Privacy dashboard renders.

```text
capture_current_view_snapshot   -> raw.png + annotated.png + manifest.json
        ↓ (multimodal reasoning over annotated.png)
get_last_view_snapshot          -> re-read manifest / labels
        ↓
select_images_in_view           -> drive the live grid selection
        ↓
get_audit_log                   -> confirm what the agent did
```

## Launch demo (the keep-anyway loop)

A stranger with an admin token can run the differentiator end to end:

1. `create_token` → scoped curator/admin token (or make one in Settings).
2. Connect via the `--mcp-stdio` config snippet above.
3. `capture_current_view_snapshot` → read `annotated.png`.
4. Reason over the labels; `select_images_in_view` to keep the good frames.
5. `get_audit_log` → show the scoped, audited, revocable trail in the Privacy
   dashboard's Agent Access Log.

That loop — a real agent driving the live UI through the same scoped, audited
tool layer the CLI reuses — is the launch demo.
