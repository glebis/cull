// Copyright (c) 2026-present Gleb Kalinin. Architecture and design by author.
// Implementation assisted by Claude (Anthropic). See AUTHORSHIP.md.

//! Tauri commands for the plugin runtime. Both commands are no-ops unless the
//! `module_plugins` app setting is "true" (default OFF for v1), so no plugin
//! code can execute or invoke privileged operations when the module is off —
//! the same gating posture as `module_static_publishing`.

use crate::plugins::{invoke, loader};
use crate::services::ServiceContext;
use crate::AppState;
use tauri::State;

pub const MODULE_KEY: &str = "module_plugins";

fn plugins_enabled(state: &AppState) -> bool {
    state.db.get_setting(MODULE_KEY).ok().flatten().as_deref() == Some("true")
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
    let ctx = ServiceContext::from_app_state(&state, None);
    invoke::plugin_invoke_inner(&ctx, &plugin_id, &tool, args)
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
    let app_version = app
        .config()
        .version
        .clone()
        .unwrap_or_else(|| env!("CARGO_PKG_VERSION").to_string());
    let dir = loader::plugins_dir(&state.app_data_dir);
    Ok(loader::load_installed_plugins(&dir, &app_version))
}
