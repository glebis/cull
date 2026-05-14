# Agent CLI Standards

Cull's headless CLI and MCP server should expose the same task model. Agents should not need one vocabulary for MCP and another for shell automation.

## Contract

- CLI command names must match MCP tool names, using snake_case.
- CLI parameter names must match MCP JSON schema fields, using snake_case.
- Every headless command must support `--json` and return one JSON value on stdout for one-shot commands.
- Errors in `--json` mode must be JSON objects with `event: "error"` and `message`.
- Human output may be pretty-printed JSON, but must not be the only supported output.
- Commands must be non-interactive. Do not prompt, open dialogs, or depend on a visible Tauri window.
- Long-running commands should return either a final JSON result or a `job_id` that is queryable through a matching job tool. Do not mix both patterns casually.
- CLI and MCP behavior should call shared service functions, not duplicate business logic.

## Module Layout

Headless CLI code lives under `src-tauri/src/cli/`:

- `mod.rs`: Clap argument shape and command-to-tool wiring only.
- `context.rs`: database and app-data setup.
- `output.rs`: JSON/human output and JSON parameter loading.
- `tools/mod.rs`: MCP-name registry and dispatch.
- `tools/library.rs`: read-only library/listing commands.
- `tools/import.rs`: import commands.
- `tools/export.rs`: export commands.

Shared behavior belongs under `src-tauri/src/services/`. A CLI module should be thin: parse JSON params, call a service, serialize the result.

## Adding A Tool

1. Add or reuse a typed params struct in the relevant service module. Derive `Deserialize` and `JsonSchema` when MCP uses it.
2. Implement the behavior in `src-tauri/src/services/<domain>.rs`.
3. Add the MCP tool in `src-tauri/src/mcp/tools.rs`, using the same params struct.
4. Add the CLI dispatcher in `src-tauri/src/cli/tools/<domain>.rs`.
5. Register the tool name in `src-tauri/src/cli/tools/mod.rs`.
6. Add a Clap convenience subcommand in `src-tauri/src/cli/mod.rs` only when it improves shell ergonomics.
7. Update capability mapping in `src-tauri/src/services/tokens.rs` and MCP auth tests when the tool is exposed remotely.
8. Update `docs/cli-and-url-scheme.md` and this standards file if the pattern changes.

## Dedicated Agent Ownership

Use dedicated agents or focused work slices by domain, not by random files:

- Library agent: `cli/tools/library.rs`, library read services, read-only MCP tools.
- Import agent: `cli/tools/import.rs`, `services/import.rs`, import MCP tools.
- Export agent: `cli/tools/export.rs`, `services/export.rs`, export MCP tools.
- AI/search agent: embeddings, detection, semantic search tools.
- Curation agent: rating, decision, collection mutation tools.

Each agent should own one domain module and shared service changes for that domain. Cross-domain edits should be limited to the registry, docs, and capability mapping.

## Verification

Minimum checks for CLI work:

```bash
cd src-tauri
cargo fmt
cargo test
cargo build
```

Smoke-test against a temporary DB, not the real user database:

```bash
tmpdir=$(mktemp -d /tmp/cull-cli-smoke.XXXXXX)
./target/debug/cull --app-data-dir "$tmpdir/appdata" --db "$tmpdir/cull.db" --json get_library_stats
```

Use `trash`, not `rm`, for cleaning smoke-test directories.
