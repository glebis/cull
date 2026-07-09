# cull-publish

The Track C3 proof plugin: the static publishing settings UI extracted from
core (`src/lib/components/StaticPublishingSettings.svelte`) into a standalone
ESM bundle that installs from the plugin registry and renders through the
plugin host API.

What stays in core: the Rust static-publishing backend
(`src-tauri/src/commands/static_publishing.rs`) and the MCP publish tools.
Only the settings UI extracts.

## Permissions

| Permission | Used for |
| --- | --- |
| `library:read` | `get_library_stats`, `list_collections`, `list_collection_images` |
| `export:read` | `export_static_publish_package` (capability check) |
| `module:static-publishing` | `export_static_publish_package` (module gate — plugin presence substitutes for the `module_static_publishing` setting) |

All privileged calls go through `host.invoke(tool, args)`, enforced in Rust by
the `plugin_invoke` capability bridge and written to the audit log as actor
`plugin:cull-publish`.

## Build

```bash
node scripts/build-plugin.mjs cull-publish
```

This writes the `entry.mjs` SHA-256 into `manifest.json` and regenerates the local registry fixture at
`tests/fixtures/plugin-registry/registry.json` (a `file://` registry used by
the end-to-end Rust proof test `proof_plugin_cull_publish_installs_end_to_end`
and by manual testing via the `plugin_registry_url` app setting).

Publishing to the public `glebis/cull-plugins` registry (git tag + HTTPS
download URL) is soft-launch scope and is not done from this repo.
