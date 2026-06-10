# Cull Plugin Mechanism + Store — Design Spec (Track C)

> Source of truth: the Track C plugin spec in
> `docs/release-audit-2026-06-09/report.md` (section 4, "Plugin Spec"). This
> document mirrors that spec and records implementation status.
>
> **Status (Track C1, bd imageview-dkz.23):**
> - runtime bootstrap — **working** (`cargo test --lib plugins::`,
>   `npx vitest run src/lib/plugins`)
> - registry fetch + checksum verify — **not-started** (Track C2; load-time
>   checksum re-hash IS implemented and tested)
> - plugin install — **not-started** (Track C2)
> - proof plugin — **not-started** (Track C2)

## 0. What the candidate actually is (evidence)

Worked backwards from the single PLUGIN-verdict feature: **publish_view**
(Static Publishing) — the only feature behind a runtime module gate
(`module_static_publishing`): a frontend view, gated by a module key, calling
backend commands that already enforce that key independently. The plugin
system v1 is the generalization of exactly this seam — nothing more.

## 1. Mechanism choice: frontend JS modules over a Rust-enforced permission bridge (hybrid), backend stays in core

Plugins are precompiled ESM bundles (frontend JS modules) downloaded by the
Rust side, checksum-verified, loaded from the app-data plugins dir
(`$APPDATA/plugins/<id>/`), and given a narrow `host` API whose privileged
calls are enforced in Rust. The Rust backend for a plugin's feature stays
compiled into core behind its module key.

Rejected alternatives (full reasoning in the audit report):

- **External process plugins (sidecars):** Gatekeeper/notarization makes
  downloaded executables a distribution program, not a feature.
- **MCP tool packs:** MCP cannot deliver a UI view; the one PLUGIN-verdict
  candidate is a view.
- **Pure frontend modules without Rust enforcement:** permission checks must
  live at the same trust boundary as MCP enforcement (`require_capability`,
  `src-tauri/src/mcp/auth.rs`), not in webview JS a plugin can monkey-patch.

Loading path (implemented in C1): Rust reads the installed bundle and
re-hashes it against the manifest checksum
(`src-tauri/src/plugins/loader.rs`) → the frontend re-hashes the string again
(`src/lib/plugins/loader.ts`) → only checksum-matching code reaches
`import(blobUrl)`. This required exactly one CSP change:
`"script-src": "'self' blob:"` (`src-tauri/tauri.conf.json`), pinned by
contract tests in `src-tauri/src/config_contract.rs` and
`src/lib/plugins-runtime-contract.test.ts`.

**Plugin API surface (v1, deliberately tiny):** default-export
`activate(host)` where `host = { mountView(el), registerPaletteCommands([...]),
invoke(tool, args) }` (`src/lib/plugins/host.ts`). `invoke` is the only
privileged path and is permission-checked in Rust by the `plugin_invoke`
command.

## 2. Manifest format

One `manifest.json` per plugin (parsing/validation:
`src-tauri/src/plugins/manifest.rs`):

```json
{
  "id": "cull-publish",
  "name": "Publish View (Static Site)",
  "version": "1.0.0",
  "description": "Build a static site package from a canvas or selection.",
  "entry": "dist/plugin.js",
  "permissions": ["library:read", "export:read", "module:static-publishing"],
  "minAppVersion": "0.2.1",
  "checksum": "sha256:<hex of entry bundle>",
  "repo": "https://github.com/glebis/cull-plugins"
}
```

- `permissions` reuse the **existing MCP capability vocabulary** from
  `tokens::capabilities_for_role` (`src-tauri/src/services/tokens.rs`),
  extended only with `module:<key>` permissions mapping onto existing module
  gates. No new permission taxonomy.
- `minAppVersion` is semver-checked against the app version at install and at
  load.
- `checksum` covers the `entry` bundle bytes; verified at install **and** at
  every load (Rust side and webview side).

## 3. Registry v1 (Track C2)

A single schema-versioned `registry.json` (`cull.plugins.registry.v1`) in a
public `glebis/cull-plugins` GitHub repo, fetched over HTTPS in Rust
(reqwest), download URLs pinned to git tags so checksums always describe
immutable bytes. Install flow: fetch registry → user consents to the listed
permissions → fetch bundle → SHA-256 verify → write to
`$APPDATA/plugins/<id>/` → record grant rows. Migration path: v1.5 adds a
detached signature over the registry; v2 serves the same schema from an API.

## 4. Security model — consistent with the MCP token/audit posture

- **Enforcement point:** one Tauri command `plugin_invoke(plugin_id, tool,
  args)` (`src-tauri/src/plugins/invoke.rs`) resolves the plugin's persisted
  grants (`plugin_grants` table, migration 22) and runs the same check shape
  as MCP: `tokens::tool_capability` + `require_capability` via
  `AuthContext::Plugin`. A plugin is a locally-installed actor with a
  capability set — exactly what an MCP token is.
- **Consent surfacing:** the install dialog lists manifest `permissions` with
  human-readable descriptions (`grantPromptModel`,
  `src/lib/plugins/loader.ts`) *before* download; nothing auto-installs.
- **Audit:** every `plugin_invoke` call (allowed, denied, unsupported) is
  written through the existing `log_audit` path with actor `plugin:<id>`,
  inheriting param redaction and `prune_audit_log` retention. Install/remove
  events are audited in Track C2.
- **Default off:** the whole runtime is behind the `module_plugins` app
  setting (default OFF). When off, the Rust commands refuse/return empty, no
  plugin code loads, and no plugin surface (palette commands, views) is
  reachable — pinned by `src/lib/plugins-runtime-contract.test.ts`.
- **Honest v1 limitation, stated up front:** plugins execute in the main
  webview — there is no iframe/realm sandbox in v1. Checksums establish
  *integrity* (you run the bytes the manifest described), not *confinement*.
  The Rust permission gate confines privileged operations; DOM access is
  unconfined. This is the Obsidian/VS Code trust model. Sandboxing is a filed
  v1.1 item, not a silent gap.
- **No remote code at runtime:** fetch happens once at install, in Rust;
  load-time re-hash means a tampered on-disk bundle refuses to load.

## 5. Honest sizing

Per the audit spec: runtime 5.0 h (this track), registry + install UX 4.0 h,
proof plugin 4.5 h; uninstall/disable UX and update-check flow are
pre-committed cuts (reinstall is the upgrade path; uninstall = delete
`$APPDATA/plugins/<id>`).

## 6. Day-4 fallback valve

If the proof plugin does not install from the live registry and render the
publish view end-to-end by end of Day 4, Track C falls back: publish view
ships exactly as today (module-gated via `module_static_publishing`) and all
plugin-runtime UI stays flagged off. The release is never delayed by Track C.
