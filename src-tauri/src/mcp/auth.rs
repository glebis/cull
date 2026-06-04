// Copyright (c) 2025-present Gleb Kalinin. Architecture and design by author.
// Implementation assisted by Claude (Anthropic). See AUTHORSHIP.md.

use crate::services::tokens;

#[derive(Debug, Clone)]
pub enum AuthContext {
    Local,
    Authenticated(crate::db_core::models::McpToken),
}

impl AuthContext {
    pub fn has_capability(&self, capability: &str) -> bool {
        match self {
            AuthContext::Local => true,
            AuthContext::Authenticated(token) => tokens::has_capability(&token.role, capability),
        }
    }

    #[allow(dead_code)]
    pub fn token_id(&self) -> Option<&str> {
        match self {
            AuthContext::Local => None,
            AuthContext::Authenticated(t) => Some(&t.id),
        }
    }
}

pub fn require_capability(auth: &AuthContext, tool_name: &str) -> Result<(), String> {
    let capability = tokens::tool_capability(tool_name);
    if auth.has_capability(capability) {
        Ok(())
    } else {
        Err(format!(
            "Permission denied: '{}' requires '{}' capability",
            tool_name, capability
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db_core::models::McpToken;
    use crate::services::tokens;

    fn make_token(role: &str) -> McpToken {
        McpToken {
            id: format!("tok_{}", role),
            name: format!("{} token", role),
            role: role.to_string(),
            scope_json: None,
            created_at: "2026-01-01T00:00:00Z".to_string(),
            expires_at: None,
            last_used_at: None,
            revoked: false,
        }
    }

    fn viewer_auth() -> AuthContext {
        AuthContext::Authenticated(make_token("viewer"))
    }
    fn curator_auth() -> AuthContext {
        AuthContext::Authenticated(make_token("curator"))
    }
    fn operator_auth() -> AuthContext {
        AuthContext::Authenticated(make_token("operator"))
    }
    fn admin_auth() -> AuthContext {
        AuthContext::Authenticated(make_token("admin"))
    }

    const ALL_TOOLS: &[&str] = &[
        "get_library_stats",
        "list_images",
        "get_image",
        "list_folders",
        "list_collections",
        "list_folder_images",
        "list_session_canvases",
        "get_canvas_layout",
        "set_rating",
        "set_decision",
        "create_collection",
        "add_to_collection",
        "delete_collection",
        "list_collection_images",
        "create_smart_collection",
        "find_similar",
        "search_by_object",
        "get_detections",
        "get_vision_metadata",
        "get_image_quality",
        "get_quality_count",
        "show_image",
        "navigate_to_folder",
        "show_collection",
        "capture_current_view_snapshot",
        "get_last_view_snapshot",
        "select_snapshot_labels",
        "select_images_in_view",
        "import_folder",
        "import_files",
        "download_embedding_model",
        "generate_embeddings",
        "analyze_image_quality",
        "detect_objects",
        "analyze_images",
        "create_token",
        "list_tokens",
        "revoke_token",
        "rotate_token",
        "get_audit_log",
        "prune_audit_log",
        "export_images",
        "list_export_presets",
        "export_static_publish_package",
        "export_static_publish_canvas",
    ];

    const ADMIN_ONLY_TOOLS: &[&str] = &[
        "get_job",
        "list_jobs",
        "cancel_job",
        "pause_job",
        "resume_job",
        "rescan_sources",
        "serve_static_publish_package",
    ];

    const READ_TOOLS: &[&str] = &[
        "get_library_stats",
        "list_images",
        "get_image",
        "list_folders",
        "list_collections",
        "list_folder_images",
        "list_session_canvases",
        "get_canvas_layout",
        "get_detections",
        "get_vision_metadata",
        "get_image_quality",
        "get_quality_count",
    ];

    const SEARCH_TOOLS: &[&str] = &["find_similar", "search_by_object"];

    const CURATION_TOOLS: &[&str] = &[
        "set_rating",
        "set_decision",
        "create_collection",
        "add_to_collection",
        "delete_collection",
        "create_smart_collection",
    ];

    const IMPORT_TOOLS: &[&str] = &["import_folder", "import_files"];
    const EXPORT_TOOLS: &[&str] = &[
        "export_images",
        "list_export_presets",
        "assemble_pdf",
        "export_static_publish_package",
        "export_static_publish_canvas",
    ];
    const DISPLAY_TOOLS: &[&str] = &[
        "show_image",
        "navigate_to_folder",
        "show_collection",
        "capture_current_view_snapshot",
        "get_last_view_snapshot",
        "select_snapshot_labels",
        "select_images_in_view",
    ];
    const AI_TOOLS: &[&str] = &[
        "download_embedding_model",
        "generate_embeddings",
        "analyze_image_quality",
        "detect_objects",
        "analyze_images",
    ];
    const TOKEN_TOOLS: &[&str] = &[
        "create_token",
        "list_tokens",
        "revoke_token",
        "rotate_token",
        "get_audit_log",
        "prune_audit_log",
    ];

    // --- Basic auth context tests ---

    #[test]
    fn test_local_has_all_capabilities() {
        let auth = AuthContext::Local;
        assert!(auth.has_capability("tokens:manage"));
        assert!(auth.has_capability("anything_at_all"));
        assert!(auth.has_capability("library:read"));
        assert!(auth.has_capability("display:navigate"));
    }

    #[test]
    fn test_local_token_id_is_none() {
        assert!(AuthContext::Local.token_id().is_none());
    }

    #[test]
    fn test_authenticated_token_id() {
        let auth = AuthContext::Authenticated(make_token("viewer"));
        assert_eq!(auth.token_id(), Some("tok_viewer"));
    }

    // --- Capability mapping completeness ---

    #[test]
    fn test_all_defined_tools_have_explicit_capability_mapping() {
        for tool in ALL_TOOLS {
            let cap = tokens::tool_capability(tool);
            assert_ne!(
                cap, "settings:manage",
                "Tool '{}' fell through to default 'settings:manage' — add it to tool_capability()",
                tool
            );
        }
    }

    #[test]
    fn test_token_management_tools_mapped() {
        for tool in TOKEN_TOOLS {
            assert_eq!(
                tokens::tool_capability(tool),
                "tokens:manage",
                "Token tool '{}' should map to 'tokens:manage'",
                tool
            );
        }
    }

    #[test]
    fn test_unknown_tool_defaults_to_settings_manage() {
        assert_eq!(
            tokens::tool_capability("nonexistent_tool"),
            "settings:manage"
        );
        assert_eq!(tokens::tool_capability(""), "settings:manage");
        assert_eq!(tokens::tool_capability("drop_database"), "settings:manage");
    }

    // --- Local context bypasses everything ---

    #[test]
    fn test_local_passes_all_tools() {
        let auth = AuthContext::Local;
        for tool in ALL_TOOLS {
            assert!(
                require_capability(&auth, tool).is_ok(),
                "Local should pass for tool '{}'",
                tool
            );
        }
        for tool in TOKEN_TOOLS {
            assert!(
                require_capability(&auth, tool).is_ok(),
                "Local should pass for token tool '{}'",
                tool
            );
        }
        for tool in ADMIN_ONLY_TOOLS {
            assert!(
                require_capability(&auth, tool).is_ok(),
                "Local should pass for admin-only tool '{}'",
                tool
            );
        }
        assert!(require_capability(&auth, "nonexistent_tool").is_ok());
    }

    // --- Viewer access matrix ---

    #[test]
    fn test_viewer_can_read() {
        let auth = viewer_auth();
        for tool in READ_TOOLS {
            assert!(
                require_capability(&auth, tool).is_ok(),
                "Viewer should access read tool '{}'",
                tool
            );
        }
    }

    #[test]
    fn test_viewer_can_search() {
        let auth = viewer_auth();
        for tool in SEARCH_TOOLS {
            assert!(
                require_capability(&auth, tool).is_ok(),
                "Viewer should access search tool '{}'",
                tool
            );
        }
    }

    #[test]
    fn test_viewer_cannot_curate() {
        let auth = viewer_auth();
        for tool in CURATION_TOOLS {
            assert!(
                require_capability(&auth, tool).is_err(),
                "Viewer should NOT access curation tool '{}'",
                tool
            );
        }
    }

    #[test]
    fn test_viewer_cannot_import() {
        let auth = viewer_auth();
        for tool in IMPORT_TOOLS {
            assert!(
                require_capability(&auth, tool).is_err(),
                "Viewer should NOT access import tool '{}'",
                tool
            );
        }
    }

    #[test]
    fn test_viewer_cannot_navigate() {
        let auth = viewer_auth();
        for tool in DISPLAY_TOOLS {
            assert!(
                require_capability(&auth, tool).is_err(),
                "Viewer should NOT access display tool '{}'",
                tool
            );
        }
    }

    #[test]
    fn test_viewer_cannot_manage_tokens() {
        let auth = viewer_auth();
        for tool in TOKEN_TOOLS {
            assert!(
                require_capability(&auth, tool).is_err(),
                "Viewer should NOT access token tool '{}'",
                tool
            );
        }
    }

    // --- Curator access matrix ---

    #[test]
    fn test_curator_can_read_and_search() {
        let auth = curator_auth();
        for tool in READ_TOOLS.iter().chain(SEARCH_TOOLS.iter()) {
            assert!(
                require_capability(&auth, tool).is_ok(),
                "Curator should access read/search tool '{}'",
                tool
            );
        }
    }

    #[test]
    fn test_curator_can_curate() {
        let auth = curator_auth();
        for tool in CURATION_TOOLS {
            assert!(
                require_capability(&auth, tool).is_ok(),
                "Curator should access curation tool '{}'",
                tool
            );
        }
    }

    #[test]
    fn test_curator_can_export() {
        let auth = curator_auth();
        for tool in EXPORT_TOOLS {
            assert!(
                require_capability(&auth, tool).is_ok(),
                "Curator should access export tool '{}'",
                tool
            );
        }
    }

    #[test]
    fn test_curator_cannot_import() {
        let auth = curator_auth();
        for tool in IMPORT_TOOLS {
            assert!(
                require_capability(&auth, tool).is_err(),
                "Curator should NOT access import tool '{}'",
                tool
            );
        }
    }

    #[test]
    fn test_curator_cannot_navigate() {
        let auth = curator_auth();
        for tool in DISPLAY_TOOLS {
            assert!(
                require_capability(&auth, tool).is_err(),
                "Curator should NOT access display tool '{}'",
                tool
            );
        }
    }

    #[test]
    fn test_curator_cannot_run_ai() {
        let auth = curator_auth();
        for tool in AI_TOOLS {
            assert!(
                require_capability(&auth, tool).is_err(),
                "Curator should NOT access AI tool '{}'",
                tool
            );
        }
    }

    // --- Operator access matrix ---

    #[test]
    fn test_operator_inherits_curator() {
        let auth = operator_auth();
        for tool in READ_TOOLS
            .iter()
            .chain(SEARCH_TOOLS.iter())
            .chain(CURATION_TOOLS.iter())
            .chain(EXPORT_TOOLS.iter())
        {
            assert!(
                require_capability(&auth, tool).is_ok(),
                "Operator should access inherited tool '{}'",
                tool
            );
        }
    }

    #[test]
    fn test_operator_can_import() {
        let auth = operator_auth();
        for tool in IMPORT_TOOLS {
            assert!(
                require_capability(&auth, tool).is_ok(),
                "Operator should access import tool '{}'",
                tool
            );
        }
    }

    #[test]
    fn test_operator_can_run_ai() {
        let auth = operator_auth();
        for tool in AI_TOOLS {
            assert!(
                require_capability(&auth, tool).is_ok(),
                "Operator should access AI tool '{}'",
                tool
            );
        }
    }

    #[test]
    fn test_operator_cannot_navigate() {
        let auth = operator_auth();
        for tool in DISPLAY_TOOLS {
            assert!(
                require_capability(&auth, tool).is_err(),
                "Operator should NOT access display tool '{}'",
                tool
            );
        }
    }

    #[test]
    fn test_operator_cannot_manage_tokens() {
        let auth = operator_auth();
        for tool in TOKEN_TOOLS {
            assert!(
                require_capability(&auth, tool).is_err(),
                "Operator should NOT access token tool '{}'",
                tool
            );
        }
    }

    // --- Admin access matrix ---

    #[test]
    fn test_admin_can_do_everything() {
        let auth = admin_auth();
        let all_tool_lists: Vec<&&str> = READ_TOOLS
            .iter()
            .chain(SEARCH_TOOLS.iter())
            .chain(CURATION_TOOLS.iter())
            .chain(IMPORT_TOOLS.iter())
            .chain(EXPORT_TOOLS.iter())
            .chain(DISPLAY_TOOLS.iter())
            .chain(AI_TOOLS.iter())
            .chain(TOKEN_TOOLS.iter())
            .chain(ADMIN_ONLY_TOOLS.iter())
            .collect();

        for tool in all_tool_lists {
            assert!(
                require_capability(&auth, tool).is_ok(),
                "Admin should access tool '{}'",
                tool
            );
        }
    }

    #[test]
    fn test_admin_can_access_unknown_tools() {
        let auth = admin_auth();
        assert!(require_capability(&auth, "nonexistent_tool").is_ok());
    }

    // --- Error message quality ---

    #[test]
    fn test_denied_error_includes_tool_and_capability() {
        let auth = viewer_auth();
        let err = require_capability(&auth, "set_rating").unwrap_err();
        assert!(err.contains("set_rating"), "Error should mention tool name");
        assert!(
            err.contains("curation:write"),
            "Error should mention required capability"
        );
        assert!(
            err.contains("Permission denied"),
            "Error should say permission denied"
        );
    }

    // --- Capability coverage: all 9 capabilities are reachable ---

    #[test]
    fn test_all_capabilities_are_reachable_from_tools() {
        let all_capabilities: Vec<&str> = vec![
            "library:read",
            "library:search",
            "curation:write",
            "import:write",
            "export:read",
            "display:navigate",
            "ai:run",
            "tokens:manage",
        ];

        let all_tools_extended: Vec<&str> = ALL_TOOLS
            .iter()
            .copied()
            .chain(IMPORT_TOOLS.iter().copied())
            .chain(EXPORT_TOOLS.iter().copied())
            .chain(AI_TOOLS.iter().copied())
            .chain(TOKEN_TOOLS.iter().copied())
            .collect();

        for cap in &all_capabilities {
            let has_tool = all_tools_extended
                .iter()
                .any(|t| tokens::tool_capability(t) == *cap);
            assert!(has_tool, "Capability '{}' has no tool mapping", cap);
        }
    }
}
