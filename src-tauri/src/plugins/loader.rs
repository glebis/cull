// Copyright (c) 2026-present Gleb Kalinin. Architecture and design by author.
// Implementation assisted by Claude (Anthropic). See AUTHORSHIP.md.

//! Plugin bundle loading: read the installed manifest + ESM bundle from the
//! app-data plugins dir, re-hash the bundle bytes against the manifest
//! checksum, and hand the verified source to the frontend for blob-import.
//!
//! Checksums establish integrity (you run the bytes the manifest described),
//! not confinement: a tampered on-disk bundle refuses to load and never
//! reaches `import()`.

use super::manifest::{check_min_app_version, parse_manifest, PluginError, PluginManifest};
use serde::Serialize;
use std::path::{Path, PathBuf};

/// Installed-plugins directory under the app data dir.
pub fn plugins_dir(app_data_dir: &Path) -> PathBuf {
    app_data_dir.join("plugins")
}

#[derive(Debug, Clone, Serialize)]
pub struct LoadedPlugin {
    pub manifest: PluginManifest,
    /// Verified ESM source, ready for blob-URL import in the webview.
    pub source: String,
}

/// Load one installed plugin: parse + validate the manifest, check
/// `minAppVersion`, read the entry bundle, and verify its SHA-256 against
/// the manifest checksum. A mismatch never reaches the webview.
pub fn load_plugin_bundle(
    plugins_dir: &Path,
    plugin_id: &str,
    app_version: &str,
) -> Result<LoadedPlugin, PluginError> {
    let plugin_dir = plugins_dir.join(plugin_id);
    let manifest_json = std::fs::read_to_string(plugin_dir.join("manifest.json"))
        .map_err(|e| PluginError::Other(format!("cannot read manifest for '{plugin_id}': {e}")))?;
    let manifest = parse_manifest(&manifest_json)?;

    if manifest.id != plugin_id {
        return Err(PluginError::InvalidManifest(format!(
            "manifest id '{}' does not match install directory '{}'",
            manifest.id, plugin_id
        )));
    }

    check_min_app_version(&manifest, app_version)?;

    // The entry must stay inside the plugin's install dir.
    let entry = Path::new(&manifest.entry);
    if entry.is_absolute()
        || entry
            .components()
            .any(|c| matches!(c, std::path::Component::ParentDir))
    {
        return Err(PluginError::InvalidManifest(format!(
            "entry '{}' must be a relative path inside the plugin directory",
            manifest.entry
        )));
    }

    let bundle = std::fs::read(plugin_dir.join(entry))
        .map_err(|e| PluginError::Other(format!("cannot read bundle for '{plugin_id}': {e}")))?;

    // Load-time re-hash: only checksum-matching bytes reach import().
    let expected = manifest
        .checksum
        .strip_prefix("sha256:")
        .unwrap_or(&manifest.checksum)
        .to_ascii_lowercase();
    let actual = {
        use sha2::{Digest, Sha256};
        let mut hasher = Sha256::new();
        hasher.update(&bundle);
        hex::encode(hasher.finalize())
    };
    if actual != expected {
        return Err(PluginError::ChecksumMismatch(plugin_id.to_string()));
    }

    let source = String::from_utf8(bundle)
        .map_err(|_| PluginError::Other(format!("bundle for '{plugin_id}' is not valid UTF-8")))?;

    Ok(LoadedPlugin { manifest, source })
}

/// Load every installed plugin, skipping (and logging) invalid ones rather
/// than failing the whole load.
pub fn load_installed_plugins(plugins_dir: &Path, app_version: &str) -> Vec<LoadedPlugin> {
    let Ok(entries) = std::fs::read_dir(plugins_dir) else {
        return Vec::new();
    };
    let mut loaded = Vec::new();
    for entry in entries.flatten() {
        let Ok(file_type) = entry.file_type() else {
            continue;
        };
        if !file_type.is_dir() {
            continue;
        }
        let plugin_id = entry.file_name().to_string_lossy().to_string();
        match load_plugin_bundle(plugins_dir, &plugin_id, app_version) {
            Ok(plugin) => loaded.push(plugin),
            Err(e) => {
                crate::safe_eprintln!("[plugins] skipping '{}': {}", plugin_id, e);
            }
        }
    }
    loaded
}

#[cfg(test)]
mod tests {
    use super::*;
    use sha2::{Digest, Sha256};

    const BUNDLE: &str = "export default { activate(host) { return host; } };";

    fn sha256_hex(bytes: &[u8]) -> String {
        let mut hasher = Sha256::new();
        hasher.update(bytes);
        hex::encode(hasher.finalize())
    }

    fn install_plugin(dir: &Path, id: &str, bundle: &str, checksum: &str) {
        let plugin_dir = dir.join(id);
        std::fs::create_dir_all(plugin_dir.join("dist")).unwrap();
        let manifest = serde_json::json!({
            "id": id,
            "name": "Test Plugin",
            "version": "1.0.0",
            "description": "test",
            "entry": "dist/plugin.js",
            "permissions": ["library:read"],
            "minAppVersion": "0.2.1",
            "checksum": format!("sha256:{checksum}"),
            "repo": "https://example.com"
        });
        std::fs::write(
            plugin_dir.join("manifest.json"),
            serde_json::to_string_pretty(&manifest).unwrap(),
        )
        .unwrap();
        std::fs::write(plugin_dir.join("dist/plugin.js"), bundle).unwrap();
    }

    #[test]
    fn plugin_load_returns_verified_source() {
        let tmp = tempfile::tempdir().unwrap();
        install_plugin(
            tmp.path(),
            "good-plugin",
            BUNDLE,
            &sha256_hex(BUNDLE.as_bytes()),
        );

        let loaded = load_plugin_bundle(tmp.path(), "good-plugin", "0.2.1")
            .expect("untampered bundle must load");
        assert_eq!(loaded.source, BUNDLE);
        assert_eq!(loaded.manifest.id, "good-plugin");
    }

    #[test]
    fn plugin_load_rejects_checksum_mismatch() {
        let tmp = tempfile::tempdir().unwrap();
        install_plugin(
            tmp.path(),
            "tampered",
            BUNDLE,
            &sha256_hex(BUNDLE.as_bytes()),
        );
        // Tamper with the on-disk bundle after "install".
        std::fs::write(
            tmp.path().join("tampered/dist/plugin.js"),
            "export default { activate() { fetch('https://evil.example'); } };",
        )
        .unwrap();

        let err = load_plugin_bundle(tmp.path(), "tampered", "0.2.1").unwrap_err();
        assert!(
            matches!(err, PluginError::ChecksumMismatch(ref id) if id == "tampered"),
            "tampered bundle must fail the load-time re-hash, got {err:?}"
        );
    }

    #[test]
    fn plugin_load_rejects_min_app_version_above_current() {
        let tmp = tempfile::tempdir().unwrap();
        install_plugin(tmp.path(), "future", BUNDLE, &sha256_hex(BUNDLE.as_bytes()));

        let err = load_plugin_bundle(tmp.path(), "future", "0.1.0").unwrap_err();
        assert!(matches!(err, PluginError::AppVersionTooOld(..)));
    }

    #[test]
    fn plugin_load_rejects_entry_path_traversal() {
        let tmp = tempfile::tempdir().unwrap();
        // A bundle outside the plugin dir that a malicious entry tries to reach.
        std::fs::write(tmp.path().join("outside.js"), "evil").unwrap();
        let plugin_dir = tmp.path().join("sneaky");
        std::fs::create_dir_all(&plugin_dir).unwrap();
        let manifest = serde_json::json!({
            "id": "sneaky",
            "name": "Sneaky",
            "version": "1.0.0",
            "entry": "../outside.js",
            "permissions": [],
            "minAppVersion": "0.2.1",
            "checksum": format!("sha256:{}", sha256_hex(b"evil")),
        });
        std::fs::write(plugin_dir.join("manifest.json"), manifest.to_string()).unwrap();

        let err = load_plugin_bundle(tmp.path(), "sneaky", "0.2.1").unwrap_err();
        assert!(
            matches!(err, PluginError::InvalidManifest(_)),
            "entry escaping the plugin dir must be rejected, got {err:?}"
        );
    }

    #[test]
    fn plugin_load_missing_plugin_errors() {
        let tmp = tempfile::tempdir().unwrap();
        assert!(load_plugin_bundle(tmp.path(), "ghost", "0.2.1").is_err());
    }

    #[test]
    fn plugin_load_installed_skips_broken_plugins() {
        let tmp = tempfile::tempdir().unwrap();
        install_plugin(
            tmp.path(),
            "good-plugin",
            BUNDLE,
            &sha256_hex(BUNDLE.as_bytes()),
        );
        install_plugin(
            tmp.path(),
            "tampered",
            BUNDLE,
            &sha256_hex(b"different bytes"),
        );

        let loaded = load_installed_plugins(tmp.path(), "0.2.1");
        assert_eq!(loaded.len(), 1);
        assert_eq!(loaded[0].manifest.id, "good-plugin");
    }

    #[test]
    fn plugin_load_installed_empty_dir_is_empty() {
        let tmp = tempfile::tempdir().unwrap();
        assert!(load_installed_plugins(tmp.path(), "0.2.1").is_empty());
        assert!(load_installed_plugins(&tmp.path().join("missing"), "0.2.1").is_empty());
    }
}
