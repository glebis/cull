// Copyright (c) 2026-present Gleb Kalinin. Architecture and design by author.
// Implementation assisted by Claude (Anthropic). See AUTHORSHIP.md.

//! Checksum-verified plugin install/uninstall (Track C2).
//!
//! Install takes already-fetched bundle bytes (the command layer downloads;
//! tests inject local bytes), verifies the SHA-256 against the manifest
//! checksum BEFORE anything is written into the installed-plugins dir, and
//! stages into a temp dir + atomic rename so a failure never leaves a
//! partial install. Grants are recorded only after the bundle landed.
//! Install and uninstall are audit-logged with actor `plugin:<id>`, like
//! every `plugin_invoke` call.

use super::manifest::{check_min_app_version, validate_manifest, PluginError, PluginManifest};
use crate::services::{tokens, ServiceContext};
use std::path::Path;

pub fn sha256_hex(bytes: &[u8]) -> String {
    use sha2::{Digest, Sha256};
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    hex::encode(hasher.finalize())
}

fn audit(ctx: &ServiceContext, plugin_id: &str, tool: &str, params: Option<&str>, status: &str) {
    let actor = format!("plugin:{plugin_id}");
    let _ = tokens::log_audit(ctx, Some(&actor), tool, params, status);
}

/// Install a plugin from verified bytes: semver gate -> SHA-256 verify ->
/// stage to a temp dir -> atomic rename into `plugins_dir/<id>/` -> record
/// grant rows. Any failure leaves no partial install and no grants.
pub fn install_plugin_from_bytes(
    ctx: &ServiceContext,
    plugins_dir: &Path,
    manifest: &PluginManifest,
    bundle: &[u8],
    app_version: &str,
) -> Result<(), PluginError> {
    let result = install_inner(ctx, plugins_dir, manifest, bundle, app_version);
    let params = serde_json::json!({
        "plugin_id": manifest.id,
        "version": manifest.version,
        "permissions": manifest.permissions,
    })
    .to_string();
    let status = match &result {
        Ok(()) => "ok",
        Err(_) => "error",
    };
    audit(ctx, &manifest.id, "plugin.install", Some(&params), status);
    result
}

fn install_inner(
    ctx: &ServiceContext,
    plugins_dir: &Path,
    manifest: &PluginManifest,
    bundle: &[u8],
    app_version: &str,
) -> Result<(), PluginError> {
    validate_manifest(manifest)?;
    check_min_app_version(manifest, app_version)?;

    // The entry must stay inside the plugin's install dir (same rule the
    // loader enforces at load time).
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

    // Verify BEFORE writing anything: only checksum-matching bytes land.
    let expected = manifest
        .checksum
        .strip_prefix("sha256:")
        .unwrap_or(&manifest.checksum)
        .to_ascii_lowercase();
    if sha256_hex(bundle) != expected {
        return Err(PluginError::ChecksumMismatch(manifest.id.clone()));
    }

    let io_err = |what: &str, e: std::io::Error| {
        PluginError::Other(format!("{what} for '{}': {e}", manifest.id))
    };

    // Stage into a temp dir next to the final location, then atomically
    // rename, so a crash mid-write never leaves a partial install.
    let staging = plugins_dir.join(format!(".staging-{}", manifest.id));
    let target = plugins_dir.join(&manifest.id);
    if staging.exists() {
        std::fs::remove_dir_all(&staging).map_err(|e| io_err("cannot clear staging dir", e))?;
    }
    let stage = || -> Result<(), PluginError> {
        let entry_path = staging.join(entry);
        if let Some(parent) = entry_path.parent() {
            std::fs::create_dir_all(parent).map_err(|e| io_err("cannot create staging dir", e))?;
        }
        std::fs::write(&entry_path, bundle).map_err(|e| io_err("cannot write bundle", e))?;
        let manifest_json = serde_json::to_string_pretty(manifest)
            .map_err(|e| PluginError::Other(format!("cannot serialize manifest: {e}")))?;
        std::fs::write(staging.join("manifest.json"), manifest_json)
            .map_err(|e| io_err("cannot write manifest", e))?;
        // Reinstall is the upgrade path: replace any existing install.
        if target.exists() {
            std::fs::remove_dir_all(&target)
                .map_err(|e| io_err("cannot replace existing install", e))?;
        }
        std::fs::rename(&staging, &target).map_err(|e| io_err("cannot finalize install", e))?;
        Ok(())
    };
    let staged = stage();
    if staged.is_err() {
        let _ = std::fs::remove_dir_all(&staging);
        return staged;
    }

    ctx.db
        .set_plugin_grants(&manifest.id, &manifest.permissions)
        .map_err(|e| PluginError::Other(format!("cannot record grants: {e}")))?;
    Ok(())
}

/// Uninstall: remove the install dir AND revoke every grant row. Audited as
/// `plugin.remove`.
pub fn uninstall_plugin(
    ctx: &ServiceContext,
    plugins_dir: &Path,
    plugin_id: &str,
) -> Result<(), PluginError> {
    let result = (|| -> Result<(), PluginError> {
        let target = plugins_dir.join(plugin_id);
        if target.exists() {
            std::fs::remove_dir_all(&target).map_err(|e| {
                PluginError::Other(format!("cannot remove install dir for '{plugin_id}': {e}"))
            })?;
        }
        ctx.db
            .set_plugin_grants(plugin_id, &[])
            .map_err(|e| PluginError::Other(format!("cannot revoke grants: {e}")))?;
        Ok(())
    })();
    let params = serde_json::json!({ "plugin_id": plugin_id }).to_string();
    let status = match &result {
        Ok(()) => "ok",
        Err(_) => "error",
    };
    audit(ctx, plugin_id, "plugin.remove", Some(&params), status);
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db_core::db::Database;
    use crate::db_core::detection::DetectionEngine;
    use crate::db_core::embeddings::EmbeddingEngine;
    use crate::db_core::secrets::MemoryStore;
    use crate::plugins::loader;
    use parking_lot::Mutex;
    use std::path::PathBuf;

    const BUNDLE: &str = "export default { activate(host) { return host; } };";

    struct Fixture {
        db: Database,
        secrets: MemoryStore,
        app_data_dir: PathBuf,
        embedding_engine: Mutex<EmbeddingEngine>,
        detection_engine: Mutex<DetectionEngine>,
        safety_engine: Mutex<DetectionEngine>,
        _tmp: tempfile::TempDir,
    }

    impl Fixture {
        fn new() -> Self {
            let tmp = tempfile::tempdir().unwrap();
            let model_dir = tmp.path().join("models");
            Self {
                db: Database::open(std::path::Path::new(":memory:")).unwrap(),
                secrets: MemoryStore::new(),
                app_data_dir: tmp.path().to_path_buf(),
                embedding_engine: Mutex::new(EmbeddingEngine::new(&model_dir)),
                detection_engine: Mutex::new(DetectionEngine::new_yolo(&model_dir)),
                safety_engine: Mutex::new(DetectionEngine::new_nudenet(&model_dir)),
                _tmp: tmp,
            }
        }

        fn ctx(&self) -> ServiceContext<'_> {
            ServiceContext {
                db: &self.db,
                app_data_dir: &self.app_data_dir,
                embedding_engine: &self.embedding_engine,
                detection_engine: &self.detection_engine,
                safety_engine: &self.safety_engine,
                secrets: &self.secrets,
                app_handle: None,
            }
        }

        fn plugins_dir(&self) -> PathBuf {
            loader::plugins_dir(&self.app_data_dir)
        }
    }

    fn manifest_for(bundle: &[u8]) -> PluginManifest {
        PluginManifest {
            id: "cull-publish".to_string(),
            name: "Publish View (Static Site)".to_string(),
            version: "1.0.0".to_string(),
            description: "test".to_string(),
            entry: "plugin.js".to_string(),
            permissions: vec!["library:read".to_string(), "export:read".to_string()],
            min_app_version: "0.2.1".to_string(),
            checksum: format!("sha256:{}", sha256_hex(bundle)),
            repo: "https://github.com/glebis/cull-plugins".to_string(),
        }
    }

    /// Everything under plugins_dir (install dirs, staging leftovers).
    fn dir_entries(dir: &Path) -> Vec<String> {
        let Ok(entries) = std::fs::read_dir(dir) else {
            return Vec::new();
        };
        entries
            .flatten()
            .map(|e| e.file_name().to_string_lossy().to_string())
            .collect()
    }

    #[test]
    fn registry_install_good_bundle_lands_and_is_loadable() {
        let f = Fixture::new();
        let ctx = f.ctx();
        let manifest = manifest_for(BUNDLE.as_bytes());

        install_plugin_from_bytes(
            &ctx,
            &f.plugins_dir(),
            &manifest,
            BUNDLE.as_bytes(),
            "0.2.1",
        )
        .expect("good install must succeed");

        // Loadable by the C1 loader (re-hash passes).
        let loaded = loader::load_plugin_bundle(&f.plugins_dir(), "cull-publish", "0.2.1")
            .expect("installed plugin must be loadable");
        assert_eq!(loaded.source, BUNDLE);
        assert_eq!(loaded.manifest, manifest);

        // Grant rows recorded for exactly the manifest permissions.
        let grants = f.db.granted_plugin_capabilities("cull-publish").unwrap();
        assert_eq!(grants, vec!["export:read", "library:read"]);

        // No staging leftovers.
        assert_eq!(dir_entries(&f.plugins_dir()), vec!["cull-publish"]);

        // Audited as plugin.install with the plugin actor marker.
        let entries = tokens::get_recent_audit(&ctx, 10).unwrap();
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].tool_name, "plugin.install");
        assert_eq!(entries[0].result_status, "ok");
        assert_eq!(entries[0].token_id.as_deref(), Some("plugin:cull-publish"));
    }

    #[test]
    fn registry_install_rejects_sha256_mismatch() {
        let f = Fixture::new();
        let ctx = f.ctx();
        // Checksum describes different bytes than what is being installed.
        let manifest = manifest_for(b"the bytes the registry promised");

        let err = install_plugin_from_bytes(
            &ctx,
            &f.plugins_dir(),
            &manifest,
            BUNDLE.as_bytes(),
            "0.2.1",
        )
        .unwrap_err();
        assert!(
            matches!(err, PluginError::ChecksumMismatch(ref id) if id == "cull-publish"),
            "mismatched bytes must be rejected, got {err:?}"
        );

        // Nothing written: no install dir, no staging leftovers.
        assert!(!f.plugins_dir().join("cull-publish").exists());
        assert!(dir_entries(&f.plugins_dir()).is_empty());

        // No grant rows.
        assert!(f
            .db
            .granted_plugin_capabilities("cull-publish")
            .unwrap()
            .is_empty());

        // The failed install is still audited.
        let entries = tokens::get_recent_audit(&ctx, 10).unwrap();
        assert_eq!(entries[0].tool_name, "plugin.install");
        assert_eq!(entries[0].result_status, "error");
    }

    #[test]
    fn registry_install_rejects_min_app_version_above_current() {
        let f = Fixture::new();
        let ctx = f.ctx();
        let mut manifest = manifest_for(BUNDLE.as_bytes());
        manifest.min_app_version = "9.9.9".to_string();

        let err = install_plugin_from_bytes(
            &ctx,
            &f.plugins_dir(),
            &manifest,
            BUNDLE.as_bytes(),
            "0.2.1",
        )
        .unwrap_err();
        assert!(matches!(err, PluginError::AppVersionTooOld(..)));

        assert!(dir_entries(&f.plugins_dir()).is_empty());
        assert!(f
            .db
            .granted_plugin_capabilities("cull-publish")
            .unwrap()
            .is_empty());
    }

    #[test]
    fn registry_install_replaces_existing_install() {
        let f = Fixture::new();
        let ctx = f.ctx();
        let v1 = manifest_for(BUNDLE.as_bytes());
        install_plugin_from_bytes(&ctx, &f.plugins_dir(), &v1, BUNDLE.as_bytes(), "0.2.1").unwrap();

        let new_bundle = "export default { activate() { return 2; } };";
        let mut v2 = manifest_for(new_bundle.as_bytes());
        v2.version = "2.0.0".to_string();
        v2.permissions = vec!["library:read".to_string()];
        install_plugin_from_bytes(&ctx, &f.plugins_dir(), &v2, new_bundle.as_bytes(), "0.2.1")
            .expect("reinstall is the upgrade path");

        let loaded = loader::load_plugin_bundle(&f.plugins_dir(), "cull-publish", "0.2.1").unwrap();
        assert_eq!(loaded.manifest.version, "2.0.0");
        assert_eq!(loaded.source, new_bundle);
        // Grants replaced, not unioned.
        assert_eq!(
            f.db.granted_plugin_capabilities("cull-publish").unwrap(),
            vec!["library:read"]
        );
    }

    #[test]
    fn registry_uninstall_removes_dir_and_grants() {
        let f = Fixture::new();
        let ctx = f.ctx();
        let manifest = manifest_for(BUNDLE.as_bytes());
        install_plugin_from_bytes(
            &ctx,
            &f.plugins_dir(),
            &manifest,
            BUNDLE.as_bytes(),
            "0.2.1",
        )
        .unwrap();

        uninstall_plugin(&ctx, &f.plugins_dir(), "cull-publish").expect("uninstall must succeed");

        assert!(!f.plugins_dir().join("cull-publish").exists());
        assert!(f
            .db
            .granted_plugin_capabilities("cull-publish")
            .unwrap()
            .is_empty());

        let entries = tokens::get_recent_audit(&ctx, 10).unwrap();
        assert_eq!(entries[0].tool_name, "plugin.remove");
        assert_eq!(entries[0].result_status, "ok");
        assert_eq!(entries[0].token_id.as_deref(), Some("plugin:cull-publish"));
    }
}
