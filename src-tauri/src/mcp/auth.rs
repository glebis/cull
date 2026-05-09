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

    #[test]
    fn test_local_has_all_capabilities() {
        let auth = AuthContext::Local;
        assert!(auth.has_capability("tokens:manage"));
        assert!(auth.has_capability("anything_at_all"));
    }

    #[test]
    fn test_authenticated_viewer_limited() {
        let token = McpToken {
            id: "tok_test".to_string(),
            name: "test".to_string(),
            role: "viewer".to_string(),
            scope_json: None,
            created_at: "2026-01-01".to_string(),
            expires_at: None,
            last_used_at: None,
            revoked: false,
        };
        let auth = AuthContext::Authenticated(token);
        assert!(auth.has_capability("library:read"));
        assert!(auth.has_capability("library:search"));
        assert!(!auth.has_capability("curation:write"));
        assert!(!auth.has_capability("tokens:manage"));
    }

    #[test]
    fn test_require_capability_viewer_blocked() {
        let token = McpToken {
            id: "tok_test".to_string(),
            name: "test".to_string(),
            role: "viewer".to_string(),
            scope_json: None,
            created_at: "2026-01-01".to_string(),
            expires_at: None,
            last_used_at: None,
            revoked: false,
        };
        let auth = AuthContext::Authenticated(token);
        assert!(require_capability(&auth, "list_images").is_ok());
        assert!(require_capability(&auth, "set_rating").is_err());
        assert!(require_capability(&auth, "show_image").is_err());
    }

    #[test]
    fn test_require_capability_admin_all() {
        let token = McpToken {
            id: "tok_admin".to_string(),
            name: "admin".to_string(),
            role: "admin".to_string(),
            scope_json: None,
            created_at: "2026-01-01".to_string(),
            expires_at: None,
            last_used_at: None,
            revoked: false,
        };
        let auth = AuthContext::Authenticated(token);
        assert!(require_capability(&auth, "set_rating").is_ok());
        assert!(require_capability(&auth, "show_image").is_ok());
        assert!(require_capability(&auth, "create_token").is_ok());
    }
}
