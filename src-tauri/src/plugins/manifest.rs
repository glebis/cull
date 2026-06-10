// Copyright (c) 2026-present Gleb Kalinin. Architecture and design by author.
// Implementation assisted by Claude (Anthropic). See AUTHORSHIP.md.

//! Plugin manifest parsing and validation (pure, no IO).
//!
//! The manifest `permissions` reuse the existing MCP capability vocabulary
//! from `tokens::capabilities_for_role` — no new permission taxonomy —
//! extended only with `module:<key>` permissions that map onto existing
//! module gates (e.g. `module:static-publishing`).

use serde::{Deserialize, Serialize};

#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum PluginError {
    #[error("invalid plugin manifest: {0}")]
    InvalidManifest(String),
    #[error("invalid plugin registry: {0}")]
    InvalidRegistry(String),
    #[error("plugin '{0}' requires app version >= {1} (current: {2})")]
    AppVersionTooOld(String, String, String),
    #[error("checksum mismatch for plugin '{0}': installed bundle does not match manifest")]
    ChecksumMismatch(String),
    #[error("plugin error: {0}")]
    Other(String),
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct PluginManifest {
    pub id: String,
    pub name: String,
    pub version: String,
    #[serde(default)]
    pub description: String,
    pub entry: String,
    #[serde(default)]
    pub permissions: Vec<String>,
    pub min_app_version: String,
    pub checksum: String,
    #[serde(default)]
    pub repo: String,
}

/// MCP capabilities that are valid for tokens but must NEVER be granted to a
/// plugin manifest: they let a plugin manage tokens, mutate app settings, or
/// write into the library. Checked inside [`is_known_permission`].
pub(crate) const DENIED_PLUGIN_PERMISSIONS: &[&str] =
    &["tokens:manage", "settings:manage", "import:write"];

/// A plugin id is safe iff it is a single, non-empty path component made up
/// only of ASCII alphanumerics, `-`, or `_`. This blocks path traversal
/// (`../evil`), separators (`a/b`, `a\b`), absolute paths (`/abs`), empty
/// ids, and NUL bytes before the id is ever joined into a filesystem path.
pub fn is_safe_plugin_id(id: &str) -> bool {
    !id.is_empty()
        && id
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
}

/// Parse and structurally validate a plugin manifest. Does NOT check
/// `min_app_version` against the running app — see [`check_min_app_version`].
pub fn parse_manifest(json: &str) -> Result<PluginManifest, PluginError> {
    let manifest: PluginManifest =
        serde_json::from_str(json).map_err(|e| PluginError::InvalidManifest(e.to_string()))?;
    validate_manifest(&manifest)?;
    Ok(manifest)
}

/// Structural validation shared by installed manifests and registry entries:
/// required fields non-empty, permissions in the known vocabulary, checksum
/// format, semver `minAppVersion`.
pub fn validate_manifest(manifest: &PluginManifest) -> Result<(), PluginError> {
    for (field, value) in [
        ("id", &manifest.id),
        ("name", &manifest.name),
        ("version", &manifest.version),
        ("entry", &manifest.entry),
        ("minAppVersion", &manifest.min_app_version),
        ("checksum", &manifest.checksum),
    ] {
        if value.trim().is_empty() {
            return Err(PluginError::InvalidManifest(format!(
                "required field '{field}' is empty"
            )));
        }
    }

    if !is_safe_plugin_id(&manifest.id) {
        return Err(PluginError::InvalidManifest(format!(
            "id '{}' must be a single path segment of alphanumerics, '-', or '_'",
            manifest.id.escape_debug()
        )));
    }

    for permission in &manifest.permissions {
        if !is_known_permission(permission) {
            return Err(PluginError::InvalidManifest(format!(
                "unknown permission '{permission}' (must be an MCP capability or 'module:<key>')"
            )));
        }
    }

    validate_checksum_format(&manifest.checksum)?;

    if parse_semver(&manifest.min_app_version).is_none() {
        return Err(PluginError::InvalidManifest(format!(
            "minAppVersion '{}' is not a semver version",
            manifest.min_app_version
        )));
    }

    Ok(())
}

/// True when `permission` is part of the known capability vocabulary: the
/// MCP capability set (sourced from `tokens::capabilities_for_role`, not a
/// forked list) or a `module:<key>` permission with a non-empty key.
pub fn is_known_permission(permission: &str) -> bool {
    // Explicit deny-list: these capabilities are valid for MCP tokens but must
    // never appear in a plugin manifest.
    if DENIED_PLUGIN_PERMISSIONS.contains(&permission) {
        return false;
    }
    if let Some(module_key) = permission.strip_prefix("module:") {
        return !module_key.trim().is_empty();
    }
    crate::services::tokens::capabilities_for_role(crate::services::tokens::ROLE_ADMIN)
        .contains(&permission)
}

/// Semver check of the manifest's `min_app_version` against the running app
/// version (e.g. `tauri.conf.json` "version").
pub fn check_min_app_version(
    manifest: &PluginManifest,
    app_version: &str,
) -> Result<(), PluginError> {
    let required = parse_semver(&manifest.min_app_version).ok_or_else(|| {
        PluginError::InvalidManifest(format!(
            "minAppVersion '{}' is not a semver version",
            manifest.min_app_version
        ))
    })?;
    let current = parse_semver(app_version).ok_or_else(|| {
        PluginError::Other(format!(
            "app version '{app_version}' is not a semver version"
        ))
    })?;
    if current < required {
        return Err(PluginError::AppVersionTooOld(
            manifest.id.clone(),
            manifest.min_app_version.clone(),
            app_version.to_string(),
        ));
    }
    Ok(())
}

fn validate_checksum_format(checksum: &str) -> Result<(), PluginError> {
    let invalid = || {
        PluginError::InvalidManifest(format!(
            "checksum '{checksum}' must be 'sha256:<64 hex chars>'"
        ))
    };
    let hex = checksum.strip_prefix("sha256:").ok_or_else(invalid)?;
    if hex.len() != 64 || !hex.chars().all(|c| c.is_ascii_hexdigit()) {
        return Err(invalid());
    }
    Ok(())
}

/// Parse `major.minor.patch` (numeric components only; pre-release tags are
/// rejected). Component-wise comparison comes from the tuple ordering.
fn parse_semver(version: &str) -> Option<(u64, u64, u64)> {
    let mut parts = version.trim().split('.');
    let major = parts.next()?.parse().ok()?;
    let minor = parts.next()?.parse().ok()?;
    let patch = parts.next()?.parse().ok()?;
    if parts.next().is_some() {
        return None;
    }
    Some((major, minor, patch))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn valid_manifest_json() -> String {
        r#"{
            "id": "cull-publish",
            "name": "Publish View (Static Site)",
            "version": "1.0.0",
            "description": "Build a static site package from a canvas or selection.",
            "entry": "dist/plugin.js",
            "permissions": ["library:read", "export:read", "module:static-publishing"],
            "minAppVersion": "0.2.1",
            "checksum": "sha256:0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef",
            "repo": "https://github.com/glebis/cull-plugins"
        }"#
        .to_string()
    }

    #[test]
    fn plugin_manifest_parses_valid_manifest() {
        let m = parse_manifest(&valid_manifest_json()).expect("valid manifest must parse");
        assert_eq!(m.id, "cull-publish");
        assert_eq!(m.name, "Publish View (Static Site)");
        assert_eq!(m.version, "1.0.0");
        assert_eq!(m.entry, "dist/plugin.js");
        assert_eq!(m.min_app_version, "0.2.1");
        assert_eq!(
            m.permissions,
            vec!["library:read", "export:read", "module:static-publishing"]
        );
        assert!(m.checksum.starts_with("sha256:"));
    }

    #[test]
    fn plugin_manifest_rejects_invalid_json() {
        assert!(matches!(
            parse_manifest("not json at all"),
            Err(PluginError::InvalidManifest(_))
        ));
    }

    #[test]
    fn plugin_manifest_rejects_missing_required_fields() {
        // Missing `id`, `entry`, `checksum`, `minAppVersion` each fail at parse.
        for omit in [
            "id",
            "entry",
            "checksum",
            "minAppVersion",
            "version",
            "name",
        ] {
            let mut v: serde_json::Value = serde_json::from_str(&valid_manifest_json()).unwrap();
            v.as_object_mut().unwrap().remove(omit);
            let result = parse_manifest(&v.to_string());
            assert!(
                matches!(result, Err(PluginError::InvalidManifest(_))),
                "manifest missing '{}' must be rejected, got {:?}",
                omit,
                result
            );
        }
    }

    #[test]
    fn plugin_manifest_rejects_empty_required_fields() {
        for field in [
            "id",
            "entry",
            "checksum",
            "version",
            "name",
            "minAppVersion",
        ] {
            let mut v: serde_json::Value = serde_json::from_str(&valid_manifest_json()).unwrap();
            v[field] = serde_json::Value::String(String::new());
            let result = parse_manifest(&v.to_string());
            assert!(
                matches!(result, Err(PluginError::InvalidManifest(_))),
                "manifest with empty '{}' must be rejected, got {:?}",
                field,
                result
            );
        }
    }

    #[test]
    fn plugin_manifest_rejects_unknown_permission() {
        let mut v: serde_json::Value = serde_json::from_str(&valid_manifest_json()).unwrap();
        v["permissions"] = serde_json::json!(["library:read", "filesystem:write"]);
        let err = parse_manifest(&v.to_string()).unwrap_err();
        match err {
            PluginError::InvalidManifest(msg) => {
                assert!(
                    msg.contains("filesystem:write"),
                    "error must name the unknown permission: {msg}"
                );
            }
            other => panic!("expected InvalidManifest, got {other:?}"),
        }
    }

    #[test]
    fn plugin_manifest_permissions_use_mcp_capability_vocabulary() {
        // Every non-privileged capability from the token vocabulary is
        // accepted; the list is sourced from tokens::capabilities_for_role,
        // not forked here. Privileged caps are deny-listed (see
        // is_known_permission_denies_privileged_caps).
        for cap in
            crate::services::tokens::capabilities_for_role(crate::services::tokens::ROLE_ADMIN)
        {
            if DENIED_PLUGIN_PERMISSIONS.contains(&cap) {
                continue;
            }
            assert!(
                is_known_permission(cap),
                "MCP capability '{}' must be a valid plugin permission",
                cap
            );
        }
        assert!(is_known_permission("module:static-publishing"));
        assert!(!is_known_permission("module:"));
        assert!(!is_known_permission("filesystem:write"));
        assert!(!is_known_permission(""));
        assert!(!is_known_permission("library:write"));
    }

    #[test]
    fn plugin_manifest_rejects_malformed_checksum() {
        let bad_checksums = [
            "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef", // no prefix
            "sha256:short",
            "sha256:zzzz456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef", // non-hex
            "md5:0123456789abcdef0123456789abcdef",
            "sha256:0123456789ABCDEF0123456789abcdef0123456789abcdef0123456789abcde", // 63 chars
        ];
        for checksum in bad_checksums {
            let mut v: serde_json::Value = serde_json::from_str(&valid_manifest_json()).unwrap();
            v["checksum"] = serde_json::Value::String(checksum.to_string());
            assert!(
                matches!(
                    parse_manifest(&v.to_string()),
                    Err(PluginError::InvalidManifest(_))
                ),
                "checksum '{}' must be rejected",
                checksum
            );
        }
    }

    #[test]
    fn plugin_manifest_accepts_uppercase_hex_checksum() {
        let mut v: serde_json::Value = serde_json::from_str(&valid_manifest_json()).unwrap();
        v["checksum"] = serde_json::Value::String(format!("sha256:{}", "A".repeat(64)));
        assert!(parse_manifest(&v.to_string()).is_ok());
    }

    #[test]
    fn plugin_manifest_min_app_version_semver_check() {
        let m = parse_manifest(&valid_manifest_json()).unwrap();

        // App is newer or equal -> ok.
        assert!(check_min_app_version(&m, "0.2.1").is_ok());
        assert!(check_min_app_version(&m, "0.3.0").is_ok());
        assert!(check_min_app_version(&m, "1.0.0").is_ok());

        // App is older -> rejected with a useful error.
        let err = check_min_app_version(&m, "0.2.0").unwrap_err();
        assert!(matches!(err, PluginError::AppVersionTooOld(..)));
        let msg = err.to_string();
        assert!(
            msg.contains("0.2.1"),
            "error must include required version: {msg}"
        );
        assert!(
            msg.contains("0.2.0"),
            "error must include current version: {msg}"
        );

        // Component-wise, not lexicographic: 0.10.0 >= 0.2.1.
        assert!(check_min_app_version(&m, "0.10.0").is_ok());
    }

    #[test]
    fn validate_manifest_rejects_id_with_path_separators() {
        for bad_id in ["../evil", "../../x", "a/b", "a\\b", "/abs", "", "evil\0id"] {
            let mut v: serde_json::Value = serde_json::from_str(&valid_manifest_json()).unwrap();
            v["id"] = serde_json::Value::String(bad_id.to_string());
            let result = parse_manifest(&v.to_string());
            assert!(
                matches!(result, Err(PluginError::InvalidManifest(_))),
                "id '{}' must be rejected, got {:?}",
                bad_id.escape_debug(),
                result
            );
        }
    }

    #[test]
    fn validate_manifest_accepts_safe_id() {
        for good_id in ["cull-publish", "my_plugin", "Plugin1"] {
            let mut v: serde_json::Value = serde_json::from_str(&valid_manifest_json()).unwrap();
            v["id"] = serde_json::Value::String(good_id.to_string());
            assert!(
                parse_manifest(&v.to_string()).is_ok(),
                "id '{good_id}' must be accepted"
            );
        }
    }

    #[test]
    fn is_known_permission_denies_privileged_caps() {
        for denied in ["tokens:manage", "settings:manage", "import:write"] {
            assert!(
                !is_known_permission(denied),
                "privileged capability '{denied}' must never be a plugin permission"
            );
        }
        for allowed in ["library:read", "export:read", "module:static-publishing"] {
            assert!(
                is_known_permission(allowed),
                "capability '{allowed}' must remain a valid plugin permission"
            );
        }
    }

    #[test]
    fn plugin_manifest_rejects_unparseable_min_app_version() {
        let mut v: serde_json::Value = serde_json::from_str(&valid_manifest_json()).unwrap();
        v["minAppVersion"] = serde_json::Value::String("latest".to_string());
        assert!(matches!(
            parse_manifest(&v.to_string()),
            Err(PluginError::InvalidManifest(_))
        ));
    }
}
