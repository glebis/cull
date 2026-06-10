// Copyright (c) 2026-present Gleb Kalinin. Architecture and design by author.
// Implementation assisted by Claude (Anthropic). See AUTHORSHIP.md.

//! Plugin registry parsing (pure, no IO) — Track C2.
//!
//! The registry is a single schema-versioned `registry.json`
//! (`cull.plugins.registry.v1`) hosted in the public `glebis/cull-plugins`
//! repo. Each entry is a plugin manifest plus a tag-pinned HTTPS download
//! URL, so the per-bundle SHA-256 checksum always describes immutable bytes.
//! Fetching happens in the command layer; everything here is testable
//! against local fixture strings, never the network.

use super::manifest::{validate_manifest, PluginError, PluginManifest};
use serde::{Deserialize, Serialize};

/// Default registry location. Overridable via the `plugin_registry_url`
/// app setting (e.g. a `file://` fixture for manual testing).
pub const DEFAULT_REGISTRY_URL: &str =
    "https://raw.githubusercontent.com/glebis/cull-plugins/main/registry.json";

/// App-setting key that overrides [`DEFAULT_REGISTRY_URL`].
pub const REGISTRY_URL_SETTING: &str = "plugin_registry_url";

/// The only registry schema this client understands.
pub const REGISTRY_SCHEMA_V1: &str = "cull.plugins.registry.v1";

/// One registry entry: a fully validated manifest plus where to download
/// the bundle bytes the manifest checksum describes.
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct RegistryPlugin {
    pub manifest: PluginManifest,
    pub download: String,
}

/// Parse result: valid entries plus warnings for skipped malformed ones.
#[derive(Debug, Default)]
pub struct ParsedRegistry {
    pub plugins: Vec<RegistryPlugin>,
    pub warnings: Vec<String>,
}

/// Raw registry entry shape: manifest fields (without `entry` — the install
/// step always writes the bundle as `plugin.js`) plus the download URL.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct RawRegistryEntry {
    id: String,
    name: String,
    version: String,
    #[serde(default)]
    description: String,
    #[serde(default)]
    permissions: Vec<String>,
    min_app_version: String,
    checksum: String,
    #[serde(default)]
    repo: String,
    download: String,
}

/// Installed bundles are always written under this name; the synthesized
/// manifest's `entry` points at it.
pub const INSTALLED_ENTRY: &str = "plugin.js";

/// Parse and validate a registry document. An unknown `schema` rejects the
/// whole document; malformed entries are skipped with a warning, never a
/// panic, so one bad entry cannot brick the registry for everyone.
pub fn parse_registry(json: &str) -> Result<ParsedRegistry, PluginError> {
    let value: serde_json::Value = serde_json::from_str(json)
        .map_err(|e| PluginError::InvalidRegistry(format!("not valid JSON: {e}")))?;

    let schema = value
        .get("schema")
        .and_then(|s| s.as_str())
        .ok_or_else(|| PluginError::InvalidRegistry("missing 'schema' field".to_string()))?;
    if schema != REGISTRY_SCHEMA_V1 {
        return Err(PluginError::InvalidRegistry(format!(
            "unsupported schema '{schema}' (expected '{REGISTRY_SCHEMA_V1}')"
        )));
    }

    let entries = value
        .get("plugins")
        .and_then(|p| p.as_array())
        .ok_or_else(|| PluginError::InvalidRegistry("missing 'plugins' array".to_string()))?;

    let mut parsed = ParsedRegistry::default();
    for (index, entry) in entries.iter().enumerate() {
        match parse_registry_entry(entry) {
            Ok(plugin) => parsed.plugins.push(plugin),
            Err(e) => {
                let id = entry
                    .get("id")
                    .and_then(|v| v.as_str())
                    .unwrap_or("<unknown>");
                parsed
                    .warnings
                    .push(format!("skipping registry entry {index} ('{id}'): {e}"));
            }
        }
    }
    Ok(parsed)
}

fn parse_registry_entry(entry: &serde_json::Value) -> Result<RegistryPlugin, PluginError> {
    let raw: RawRegistryEntry = serde_json::from_value(entry.clone())
        .map_err(|e| PluginError::InvalidManifest(e.to_string()))?;

    if !raw.download.starts_with("https://") {
        return Err(PluginError::InvalidManifest(format!(
            "download URL '{}' must be https",
            raw.download
        )));
    }

    let manifest = PluginManifest {
        id: raw.id,
        name: raw.name,
        version: raw.version,
        description: raw.description,
        entry: INSTALLED_ENTRY.to_string(),
        permissions: raw.permissions,
        min_app_version: raw.min_app_version,
        checksum: raw.checksum,
        repo: raw.repo,
    };
    // Same structural validation installed manifests get (permissions
    // vocabulary, checksum format, semver minAppVersion).
    validate_manifest(&manifest)?;

    Ok(RegistryPlugin {
        manifest,
        download: raw.download,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn valid_registry_json() -> String {
        r#"{
            "schema": "cull.plugins.registry.v1",
            "updated": "2026-06-10",
            "plugins": [
                {
                    "id": "cull-publish",
                    "name": "Publish View (Static Site)",
                    "version": "1.0.0",
                    "description": "Build a static site package from a canvas or selection.",
                    "minAppVersion": "0.2.1",
                    "permissions": ["library:read", "export:read", "module:static-publishing"],
                    "download": "https://raw.githubusercontent.com/glebis/cull-plugins/cull-publish-v1.0.0/cull-publish/dist/plugin.js",
                    "checksum": "sha256:0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef",
                    "repo": "https://github.com/glebis/cull-plugins/tree/main/cull-publish"
                }
            ]
        }"#
        .to_string()
    }

    #[test]
    fn registry_parses_valid_registry() {
        let parsed = parse_registry(&valid_registry_json()).expect("valid registry must parse");
        assert_eq!(parsed.plugins.len(), 1);
        assert!(parsed.warnings.is_empty());

        let plugin = &parsed.plugins[0];
        assert_eq!(plugin.manifest.id, "cull-publish");
        assert_eq!(plugin.manifest.version, "1.0.0");
        assert_eq!(plugin.manifest.entry, INSTALLED_ENTRY);
        assert_eq!(
            plugin.manifest.permissions,
            vec!["library:read", "export:read", "module:static-publishing"]
        );
        assert!(plugin
            .download
            .starts_with("https://raw.githubusercontent.com/"));
    }

    #[test]
    fn registry_rejects_unknown_schema_version() {
        let mut v: serde_json::Value = serde_json::from_str(&valid_registry_json()).unwrap();
        v["schema"] = serde_json::Value::String("cull.plugins.registry.v9".to_string());
        let err = parse_registry(&v.to_string()).unwrap_err();
        assert!(
            matches!(err, PluginError::InvalidRegistry(ref msg) if msg.contains("v9")),
            "unknown schema must reject the whole document, got {err:?}"
        );
    }

    #[test]
    fn registry_rejects_missing_schema_or_plugins() {
        assert!(matches!(
            parse_registry("{}"),
            Err(PluginError::InvalidRegistry(_))
        ));
        assert!(matches!(
            parse_registry(r#"{"schema": "cull.plugins.registry.v1"}"#),
            Err(PluginError::InvalidRegistry(_))
        ));
        assert!(matches!(
            parse_registry("not json"),
            Err(PluginError::InvalidRegistry(_))
        ));
    }

    #[test]
    fn registry_skips_malformed_entries_with_warning() {
        let mut v: serde_json::Value = serde_json::from_str(&valid_registry_json()).unwrap();
        let good = v["plugins"][0].clone();

        // Entry 0: unknown permission. Entry 1: missing checksum.
        // Entry 2: non-https download. Entry 3: the good one.
        let mut bad_permission = good.clone();
        bad_permission["id"] = serde_json::json!("bad-permission");
        bad_permission["permissions"] = serde_json::json!(["filesystem:write"]);

        let mut missing_checksum = good.clone();
        missing_checksum["id"] = serde_json::json!("missing-checksum");
        missing_checksum.as_object_mut().unwrap().remove("checksum");

        let mut http_download = good.clone();
        http_download["id"] = serde_json::json!("http-download");
        http_download["download"] = serde_json::json!("http://example.com/plugin.js");

        v["plugins"] = serde_json::json!([bad_permission, missing_checksum, http_download, good]);

        let parsed =
            parse_registry(&v.to_string()).expect("malformed entries must not poison parse");
        assert_eq!(parsed.plugins.len(), 1);
        assert_eq!(parsed.plugins[0].manifest.id, "cull-publish");
        assert_eq!(parsed.warnings.len(), 3);
        assert!(parsed.warnings[0].contains("bad-permission"));
        assert!(parsed.warnings[1].contains("missing-checksum"));
        assert!(parsed.warnings[2].contains("http-download"));
    }

    #[test]
    fn registry_default_url_is_https_github_raw() {
        assert!(DEFAULT_REGISTRY_URL
            .starts_with("https://raw.githubusercontent.com/glebis/cull-plugins/"));
        assert!(DEFAULT_REGISTRY_URL.ends_with("registry.json"));
    }
}
