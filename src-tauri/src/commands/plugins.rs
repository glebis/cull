// Copyright (c) 2026-present Gleb Kalinin. Architecture and design by author.
// Implementation assisted by Claude (Anthropic). See AUTHORSHIP.md.

//! Tauri commands for the plugin runtime. Both commands are no-ops unless the
//! `module_plugins` app setting is "true" (default OFF for v1), so no plugin
//! code can execute or invoke privileged operations when the module is off —
//! the same gating posture as `module_static_publishing`.

use crate::plugins::{install, invoke, loader, registry};
use crate::services::ServiceContext;
use crate::AppState;
use tauri::State;

pub const MODULE_KEY: &str = "module_plugins";

fn plugins_enabled(state: &AppState) -> bool {
    state.db.get_setting(MODULE_KEY).ok().flatten().as_deref() == Some("true")
}

fn require_plugins_enabled(state: &AppState) -> Result<(), String> {
    if plugins_enabled(state) {
        Ok(())
    } else {
        Err("Plugin runtime is disabled (module_plugins is off)".to_string())
    }
}

/// Registry URL: the `plugin_registry_url` app setting when set (a `file://`
/// fixture works for local testing), otherwise the default public registry.
fn registry_url(state: &AppState) -> String {
    state
        .db
        .get_setting(registry::REGISTRY_URL_SETTING)
        .ok()
        .flatten()
        .filter(|url| !url.trim().is_empty())
        .unwrap_or_else(|| registry::DEFAULT_REGISTRY_URL.to_string())
}

/// The shipping-binary scheme policy: only `https://` is ever fetchable in a
/// release build. This is cfg-independent so it can be unit-tested to assert
/// the production behavior (e.g. `file://` is rejected) even from a test
/// build. A `settings:manage` holder must not be able to read arbitrary local
/// files via the registry/bundle download path.
fn scheme_allowed_in_release(url: &str) -> bool {
    url.starts_with("https://")
}

/// Whether a download URL scheme is allowed for the current build. Equals the
/// release policy, plus a `cfg(test)`-only `file://` escape hatch for local
/// registry fixtures so the hatch can never reach a shipping binary.
fn is_allowed_download_scheme(url: &str) -> bool {
    if scheme_allowed_in_release(url) {
        return true;
    }
    #[cfg(test)]
    if url.starts_with("file://") {
        return true;
    }
    false
}

/// Fetch bytes for a registry or bundle URL. HTTPS only in shipping builds,
/// with a `cfg(test)`-only `file://` escape hatch for local registry
/// fixtures. The pure parse/install functions and the proof test exercise
/// install from bytes directly and never reach this path.
async fn fetch_url_bytes(url: &str) -> Result<Vec<u8>, String> {
    if !is_allowed_download_scheme(url) {
        return Err(format!("unsupported scheme for URL '{url}'"));
    }
    #[cfg(test)]
    if let Some(path) = url.strip_prefix("file://") {
        return std::fs::read(path).map_err(|e| format!("cannot read '{url}': {e}"));
    }
    let response = reqwest::get(url)
        .await
        .map_err(|e| format!("fetch '{url}' failed: {e}"))?;
    if !response.status().is_success() {
        return Err(format!("fetch '{url}' failed: HTTP {}", response.status()));
    }
    response
        .bytes()
        .await
        .map(|b| b.to_vec())
        .map_err(|e| format!("fetch '{url}' failed: {e}"))
}

async fn fetch_registry_plugins(state: &AppState) -> Result<Vec<registry::RegistryPlugin>, String> {
    let url = registry_url(state);
    let bytes = fetch_url_bytes(&url).await?;
    let text = String::from_utf8(bytes).map_err(|_| format!("registry at '{url}' is not UTF-8"))?;
    let parsed = registry::parse_registry(&text).map_err(|e| e.to_string())?;
    for warning in &parsed.warnings {
        crate::safe_eprintln!("[plugins] {warning}");
    }
    Ok(parsed.plugins)
}

/// Fetch and parse the plugin registry (registry schema + entry validation
/// in `plugins::registry`, unit-tested against local fixtures).
#[tauri::command]
pub async fn fetch_plugin_registry(
    state: State<'_, AppState>,
) -> Result<Vec<registry::RegistryPlugin>, String> {
    require_plugins_enabled(&state)?;
    fetch_registry_plugins(&state).await
}

fn app_version(app: &tauri::AppHandle) -> String {
    app.config()
        .version
        .clone()
        .unwrap_or_else(|| env!("CARGO_PKG_VERSION").to_string())
}

/// Install a plugin by registry id: fetch registry -> find entry -> download
/// bundle -> SHA-256 verify -> atomic write -> record grant rows. The
/// frontend shows the permission consent dialog BEFORE invoking this.
#[tauri::command]
pub async fn install_plugin(
    state: State<'_, AppState>,
    app: tauri::AppHandle,
    plugin_id: String,
) -> Result<(), String> {
    require_plugins_enabled(&state)?;
    let plugins = fetch_registry_plugins(&state).await?;
    let entry = plugins
        .into_iter()
        .find(|p| p.manifest.id == plugin_id)
        .ok_or_else(|| format!("plugin '{plugin_id}' is not in the registry"))?;
    let bundle = fetch_url_bytes(&entry.download).await?;
    let version = app_version(&app);
    let ctx = ServiceContext::from_app_state(&state, None);
    let dir = loader::plugins_dir(&state.app_data_dir);
    install::install_plugin_from_bytes(&ctx, &dir, &entry.manifest, &bundle, &version)
        .map_err(|e| e.to_string())
}

/// Uninstall: remove `$APPDATA/plugins/<id>` and revoke all grant rows.
#[tauri::command]
pub async fn uninstall_plugin(state: State<'_, AppState>, plugin_id: String) -> Result<(), String> {
    require_plugins_enabled(&state)?;
    let ctx = ServiceContext::from_app_state(&state, None);
    let dir = loader::plugins_dir(&state.app_data_dir);
    install::uninstall_plugin(&ctx, &dir, &plugin_id).map_err(|e| e.to_string())
}

#[derive(serde::Serialize)]
pub struct InstalledPluginInfo {
    pub manifest: crate::plugins::manifest::PluginManifest,
    pub granted: Vec<String>,
}

/// Installed plugins with their granted capabilities, for Settings ->
/// Plugins. Empty when the module is off.
#[tauri::command]
pub async fn list_installed_plugin_info(
    state: State<'_, AppState>,
) -> Result<Vec<InstalledPluginInfo>, String> {
    if !plugins_enabled(&state) {
        return Ok(Vec::new());
    }
    let dir = loader::plugins_dir(&state.app_data_dir);
    loader::list_installed_manifests(&dir)
        .into_iter()
        .map(|manifest| {
            let granted = state
                .db
                .granted_plugin_capabilities(&manifest.id)
                .map_err(|e| e.to_string())?;
            Ok(InstalledPluginInfo { manifest, granted })
        })
        .collect()
}

/// The single privileged path for plugins. Enforced in Rust via the plugin's
/// persisted grants and the same capability check MCP tools use.
#[tauri::command]
pub async fn plugin_invoke(
    state: State<'_, AppState>,
    plugin_id: String,
    tool: String,
    args: Option<serde_json::Value>,
) -> Result<serde_json::Value, String> {
    if !plugins_enabled(&state) {
        return Err("Plugin runtime is disabled (module_plugins is off)".to_string());
    }
    invoke::plugin_invoke_inner(&state, &plugin_id, &tool, args)
}

/// Read, validate, and hash-verify all installed plugin bundles. Returns an
/// empty list when the module is off, so the frontend loader has nothing to
/// import.
#[tauri::command]
pub async fn load_installed_plugins(
    state: State<'_, AppState>,
    app: tauri::AppHandle,
) -> Result<Vec<loader::LoadedPlugin>, String> {
    if !plugins_enabled(&state) {
        return Ok(Vec::new());
    }
    let dir = loader::plugins_dir(&state.app_data_dir);
    Ok(loader::load_installed_plugins(&dir, &app_version(&app)))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fetch_rejects_file_scheme_in_release() {
        // The shipping policy rejects file:// (and any non-https scheme) so a
        // settings:manage holder cannot read arbitrary local files via the
        // download path.
        assert!(!scheme_allowed_in_release("file:///etc/passwd"));
        assert!(!scheme_allowed_in_release("http://example.com/x"));
        assert!(!scheme_allowed_in_release("ftp://example.com/x"));
        // https:// stays allowed.
        assert!(scheme_allowed_in_release(
            "https://example.com/registry.json"
        ));
    }

    #[test]
    fn test_build_keeps_file_fixture_hatch() {
        // The cfg(test) escape hatch is available for local fixtures, but only
        // in test builds — scheme_allowed_in_release proves it is absent from
        // shipping.
        assert!(is_allowed_download_scheme("file:///tmp/fixture.json"));
        assert!(is_allowed_download_scheme("https://example.com/x"));
        assert!(!is_allowed_download_scheme("http://example.com/x"));
    }
}
