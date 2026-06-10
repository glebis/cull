// Copyright (c) 2026-present Gleb Kalinin. Architecture and design by author.
// Implementation assisted by Claude (Anthropic). See AUTHORSHIP.md.

//! Contract tests for the shipped `tauri.conf.json` security posture.
//!
//! All AI-provider HTTP traffic is backend `reqwest` (see
//! `commands/embeddings.rs`, `db_core/gemini.rs`), so the webview CSP must not
//! whitelist provider hosts, and the asset-protocol scope must contain only
//! app-owned directories — never developer-personal paths.

#[cfg(test)]
mod tests {
    fn config() -> serde_json::Value {
        serde_json::from_str(include_str!("../tauri.conf.json"))
            .expect("tauri.conf.json must be valid JSON")
    }

    #[test]
    fn csp_connect_src_allows_only_self_and_ipc() {
        let conf = config();
        let connect_src = conf["app"]["security"]["csp"]["connect-src"]
            .as_str()
            .expect("connect-src must be a string");

        let sources: Vec<&str> = connect_src.split_whitespace().collect();
        assert_eq!(
            sources,
            vec!["'self'", "ipc:", "http://ipc.localhost"],
            "connect-src must contain only Tauri IPC sources; provider calls are backend-only"
        );
    }

    #[test]
    fn asset_protocol_scope_contains_no_personal_paths() {
        let conf = config();
        let allow = conf["app"]["security"]["assetProtocol"]["scope"]["allow"]
            .as_array()
            .expect("assetProtocol scope allow must be an array");

        let entries: Vec<&str> = allow
            .iter()
            .map(|v| v.as_str().expect("scope entries must be strings"))
            .collect();
        assert_eq!(
            entries,
            vec!["$APPDATA/thumbnails/**/*", "$APPDATA/generated/**/*"],
            "asset-protocol scope must contain exactly the app-owned $APPDATA dirs"
        );
        for entry in &entries {
            assert!(
                !entry.contains(".codex") && !entry.starts_with("$HOME"),
                "asset-protocol scope must not ship developer-personal paths: {entry}"
            );
        }
    }
}
