use crate::db_core::models::{McpToken, TokenScope};
use crate::services::{ServiceContext, ServiceError};
use rand::Rng;
use sha2::{Digest, Sha256};
use subtle::ConstantTimeEq;

pub const ROLE_VIEWER: &str = "viewer";
pub const ROLE_CURATOR: &str = "curator";
pub const ROLE_OPERATOR: &str = "operator";
pub const ROLE_ADMIN: &str = "admin";

const VALID_ROLES: &[&str] = &[ROLE_VIEWER, ROLE_CURATOR, ROLE_OPERATOR, ROLE_ADMIN];
const TOKEN_PREFIX: &str = "tok_";
const SECRET_BYTES: usize = 32;

pub fn capabilities_for_role(role: &str) -> Vec<&'static str> {
    match role {
        ROLE_VIEWER => vec!["library:read", "library:search"],
        ROLE_CURATOR => vec![
            "library:read",
            "library:search",
            "curation:write",
            "export:read",
        ],
        ROLE_OPERATOR => vec![
            "library:read",
            "library:search",
            "curation:write",
            "export:read",
            "import:write",
            "ai:run",
        ],
        ROLE_ADMIN => vec![
            "library:read",
            "library:search",
            "curation:write",
            "export:read",
            "import:write",
            "ai:run",
            "display:navigate",
            "tokens:manage",
            "settings:manage",
        ],
        _ => vec![],
    }
}

pub fn has_capability(role: &str, capability: &str) -> bool {
    capabilities_for_role(role).contains(&capability)
}

pub fn tool_capability(tool_name: &str) -> &'static str {
    match tool_name {
        "list_images"
        | "get_image"
        | "list_folders"
        | "list_folder_images"
        | "list_collections"
        | "list_collection_images"
        | "list_session_canvases"
        | "get_canvas_layout"
        | "get_library_stats"
        | "get_clipboard_monitor_status"
        | "get_last_clipboard_publish"
        | "get_detections"
        | "get_vision_metadata"
        | "get_image_quality"
        | "get_quality_count" => "library:read",

        "search_images" | "find_similar" | "search_by_object" => "library:search",

        "set_rating"
        | "set_decision"
        | "create_collection"
        | "add_to_collection"
        | "delete_collection"
        | "create_smart_collection" => "curation:write",

        "import_folder" | "import_files" => "import:write",
        "rescan_sources" => "settings:manage",

        "export_images"
        | "list_export_presets"
        | "assemble_pdf"
        | "export_static_publish_package"
        | "export_static_publish_canvas"
        | "publish_clipboard_collection" => "export:read",

        "show_image" | "navigate_to_folder" | "show_collection" | "show_clipboard_collection" => {
            "display:navigate"
        }

        "download_embedding_model"
        | "generate_embeddings"
        | "analyze_image_quality"
        | "detect_objects"
        | "analyze_images"
        | "start_ocr_batch" => "ai:run",

        "create_token" | "list_tokens" | "revoke_token" | "rotate_token" | "get_audit_log"
        | "prune_audit_log" => "tokens:manage",

        "get_job"
        | "list_jobs"
        | "cancel_job"
        | "pause_job"
        | "resume_job"
        | "serve_static_publish_package" => "settings:manage",

        _ => "settings:manage",
    }
}

fn generate_token_id() -> String {
    let mut rng = rand::thread_rng();
    let chars: String = (0..12)
        .map(|_| {
            let idx = rng.gen_range(0..36u8);
            if idx < 10 {
                (b'0' + idx) as char
            } else {
                (b'a' + idx - 10) as char
            }
        })
        .collect();
    format!("{}{}", TOKEN_PREFIX, chars)
}

fn generate_secret() -> String {
    let mut rng = rand::thread_rng();
    let bytes: Vec<u8> = (0..SECRET_BYTES).map(|_| rng.gen()).collect();
    hex::encode(bytes)
}

fn hash_secret(pepper: &str, secret: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(pepper.as_bytes());
    hasher.update(secret.as_bytes());
    hex::encode(hasher.finalize())
}

fn verify_secret(pepper: &str, secret: &str, stored_hash: &str) -> bool {
    let computed = hash_secret(pepper, secret);
    let a = computed.as_bytes();
    let b = stored_hash.as_bytes();
    if a.len() != b.len() {
        return false;
    }
    a.ct_eq(b).into()
}

fn get_or_create_pepper(ctx: &ServiceContext) -> Result<String, ServiceError> {
    match ctx.secrets.get("mcp_pepper") {
        Ok(Some(p)) => Ok(p),
        Ok(None) => {
            let new_pepper = generate_secret();
            ctx.secrets
                .set("mcp_pepper", &new_pepper)
                .map_err(|e| ServiceError::Engine(e))?;
            Ok(new_pepper)
        }
        Err(e) => Err(ServiceError::Engine(e)),
    }
}

pub fn create_token(
    ctx: &ServiceContext,
    name: &str,
    role: &str,
    scope: Option<TokenScope>,
) -> Result<(McpToken, String), ServiceError> {
    if !VALID_ROLES.contains(&role) {
        return Err(ServiceError::InvalidInput(format!(
            "Invalid role: {}",
            role
        )));
    }

    let id = generate_token_id();
    let secret = generate_secret();
    let pepper = get_or_create_pepper(ctx)?;
    let secret_hash = hash_secret(&pepper, &secret);
    let scope_json = scope.as_ref().map(|s| serde_json::to_string(s).unwrap());
    let now = chrono::Utc::now().to_rfc3339();

    let conn = ctx.db.conn.lock();
    conn.execute(
        "INSERT INTO mcp_tokens (id, name, secret_hash, role, scope_json, created_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
        rusqlite::params![id, name, secret_hash, role, scope_json, now],
    )?;

    let token = McpToken {
        id,
        name: name.to_string(),
        role: role.to_string(),
        scope_json,
        created_at: now,
        expires_at: None,
        last_used_at: None,
        revoked: false,
    };

    Ok((token, secret))
}

pub fn validate_token(
    ctx: &ServiceContext,
    secret: &str,
) -> Result<Option<McpToken>, ServiceError> {
    let pepper = get_or_create_pepper(ctx)?;
    let computed_hash = hash_secret(&pepper, secret);
    let conn = ctx.db.conn.lock();

    let result = conn.query_row(
        "SELECT id, name, secret_hash, role, scope_json, created_at, expires_at, last_used_at
         FROM mcp_tokens WHERE secret_hash = ?1 AND revoked = 0",
        rusqlite::params![computed_hash],
        |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, String>(3)?,
                row.get::<_, Option<String>>(4)?,
                row.get::<_, String>(5)?,
                row.get::<_, Option<String>>(6)?,
                row.get::<_, Option<String>>(7)?,
            ))
        },
    );

    let (id, name, stored_hash, role, scope_json, created_at, expires_at, _last_used_at) =
        match result {
            Ok(row) => row,
            Err(rusqlite::Error::QueryReturnedNoRows) => return Ok(None),
            Err(e) => return Err(ServiceError::Database(e)),
        };

    if !verify_secret(&pepper, secret, &stored_hash) {
        return Ok(None);
    }

    if let Some(ref exp) = expires_at {
        if let Ok(exp_time) = chrono::DateTime::parse_from_rfc3339(exp) {
            if exp_time < chrono::Utc::now() {
                return Ok(None);
            }
        }
    }

    let now = chrono::Utc::now().to_rfc3339();
    let _ = conn.execute(
        "UPDATE mcp_tokens SET last_used_at = ?1 WHERE id = ?2",
        rusqlite::params![now, id],
    );

    Ok(Some(McpToken {
        id,
        name,
        role,
        scope_json,
        created_at,
        expires_at,
        last_used_at: Some(now),
        revoked: false,
    }))
}

pub fn list_tokens(ctx: &ServiceContext) -> Result<Vec<McpToken>, ServiceError> {
    let conn = ctx.db.conn.lock();
    let mut stmt = conn.prepare(
        "SELECT id, name, role, scope_json, created_at, expires_at, last_used_at, revoked
         FROM mcp_tokens WHERE revoked = 0",
    )?;
    let tokens = stmt
        .query_map([], |row| {
            Ok(McpToken {
                id: row.get(0)?,
                name: row.get(1)?,
                role: row.get(2)?,
                scope_json: row.get(3)?,
                created_at: row.get(4)?,
                expires_at: row.get(5)?,
                last_used_at: row.get(6)?,
                revoked: row.get::<_, i32>(7)? != 0,
            })
        })?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    Ok(tokens)
}

pub fn revoke_token(ctx: &ServiceContext, token_id: &str) -> Result<(), ServiceError> {
    let conn = ctx.db.conn.lock();
    let updated = conn.execute(
        "UPDATE mcp_tokens SET revoked = 1 WHERE id = ?1",
        rusqlite::params![token_id],
    )?;
    if updated == 0 {
        return Err(ServiceError::NotFound(format!("Token '{}'", token_id)));
    }
    Ok(())
}

pub fn rotate_token(ctx: &ServiceContext, token_id: &str) -> Result<String, ServiceError> {
    let pepper = get_or_create_pepper(ctx)?;
    let new_secret = generate_secret();
    let new_hash = hash_secret(&pepper, &new_secret);

    let conn = ctx.db.conn.lock();
    let updated = conn.execute(
        "UPDATE mcp_tokens SET secret_hash = ?1 WHERE id = ?2 AND revoked = 0",
        rusqlite::params![new_hash, token_id],
    )?;
    if updated == 0 {
        return Err(ServiceError::NotFound(format!("Token '{}'", token_id)));
    }
    Ok(new_secret)
}

pub fn log_audit(
    ctx: &ServiceContext,
    token_id: Option<&str>,
    tool_name: &str,
    params_json: Option<&str>,
    result_status: &str,
) -> Result<(), ServiceError> {
    let now = chrono::Utc::now().to_rfc3339();
    let redacted_params = params_json.and_then(redact_audit_params);
    let conn = ctx.db.conn.lock();
    conn.execute(
        "INSERT INTO mcp_audit_log (token_id, tool_name, params_json, result_status, timestamp)
         VALUES (?1, ?2, ?3, ?4, ?5)",
        rusqlite::params![
            token_id,
            tool_name,
            redacted_params.as_deref(),
            result_status,
            now
        ],
    )?;
    Ok(())
}

pub fn prune_audit_log(ctx: &ServiceContext, retention_days: u32) -> Result<u32, ServiceError> {
    let cutoff = chrono::Utc::now() - chrono::Duration::days(retention_days as i64);
    let conn = ctx.db.conn.lock();
    let deleted = conn.execute(
        "DELETE FROM mcp_audit_log WHERE timestamp < ?1",
        rusqlite::params![cutoff.to_rfc3339()],
    )?;
    Ok(deleted as u32)
}

pub fn get_recent_audit(
    ctx: &ServiceContext,
    limit: u32,
) -> Result<Vec<crate::db_core::models::AuditEntry>, ServiceError> {
    let conn = ctx.db.conn.lock();
    let mut stmt = conn.prepare(
        "SELECT id, token_id, tool_name, params_json, result_status, timestamp
         FROM mcp_audit_log ORDER BY id DESC LIMIT ?1",
    )?;
    let entries = stmt
        .query_map(rusqlite::params![limit], |row| {
            Ok(crate::db_core::models::AuditEntry {
                id: row.get(0)?,
                token_id: row.get(1)?,
                tool_name: row.get(2)?,
                params_json: row.get(3)?,
                result_status: row.get(4)?,
                timestamp: row.get(5)?,
            })
        })?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    Ok(entries)
}

pub fn parse_scope(scope_json: &Option<String>) -> Option<TokenScope> {
    scope_json
        .as_ref()
        .and_then(|s| serde_json::from_str(s).ok())
}

fn redact_audit_params(params_json: &str) -> Option<String> {
    let mut value: serde_json::Value = serde_json::from_str(params_json).ok()?;
    redact_json_value(&mut value, None);
    serde_json::to_string(&value).ok()
}

fn redact_json_value(value: &mut serde_json::Value, key: Option<&str>) {
    match value {
        serde_json::Value::Object(map) => {
            for (k, v) in map.iter_mut() {
                redact_json_value(v, Some(k));
            }
        }
        serde_json::Value::Array(items) => {
            for item in items {
                redact_json_value(item, key);
            }
        }
        serde_json::Value::String(s) => {
            if let Some(replacement) = redaction_for_string(key, s) {
                *s = replacement.to_string();
            }
        }
        _ => {}
    }
}

fn redaction_for_string<'a>(key: Option<&str>, value: &str) -> Option<&'a str> {
    let lower_key = key.unwrap_or_default().to_ascii_lowercase();
    if lower_key.contains("key")
        || lower_key.contains("secret")
        || lower_key.contains("token")
        || lower_key.contains("authorization")
    {
        Some("[redacted:secret]")
    } else if lower_key.contains("path")
        || lower_key.contains("dir")
        || lower_key.contains("folder")
        || looks_like_path(value)
    {
        Some("[redacted:path]")
    } else if lower_key.contains("prompt") {
        Some("[redacted:text]")
    } else {
        None
    }
}

fn looks_like_path(value: &str) -> bool {
    value.starts_with('/')
        || value.starts_with("~/")
        || value.starts_with("./")
        || value.starts_with("../")
        || value.contains(":\\")
}

fn normalized_path(path: &str) -> std::path::PathBuf {
    let path = std::path::Path::new(path);
    std::fs::canonicalize(path).unwrap_or_else(|_| normalize_path_components(path))
}

fn normalize_path_components(path: &std::path::Path) -> std::path::PathBuf {
    use std::path::{Component, PathBuf};

    let mut normalized = PathBuf::new();
    for component in path.components() {
        match component {
            Component::CurDir => {}
            Component::ParentDir => {
                if !normalized.pop() {
                    normalized.push(component.as_os_str());
                }
            }
            _ => normalized.push(component.as_os_str()),
        }
    }
    normalized
}

fn is_path_under(path: &str, ancestor: &str) -> bool {
    let path = normalized_path(path);
    let ancestor = normalized_path(ancestor);
    path.starts_with(ancestor)
}

pub fn image_in_scope(
    scope: &Option<TokenScope>,
    image_path: &str,
    image_collections: &[String],
) -> bool {
    let s = match scope {
        None => return true,
        Some(s) => s,
    };

    if let Some(folders) = &s.folders {
        for folder in folders {
            if is_path_under(image_path, folder) {
                return true;
            }
        }
    }

    if let Some(collections) = &s.collections {
        for col_id in collections {
            if image_collections.contains(col_id) {
                return true;
            }
        }
    }

    false
}

/// DB-backed per-image scope check shared by all MCP image-id tools.
///
/// Loads the image's path AND collection membership before evaluating the
/// token scope, so a collection-scoped token authorizes the same images for
/// per-image tools (get_image, set_rating, …) that it can reach via collection
/// listing. Previously callers passed an empty membership slice, which made
/// collection-scoped tokens reject in-scope images.
pub fn image_id_in_scope(
    db: &crate::db_core::db::Database,
    scope: &Option<TokenScope>,
    image_id: &str,
) -> Result<bool, String> {
    if scope.is_none() {
        return Ok(true);
    }
    let images = db
        .get_images_by_ids(&[image_id])
        .map_err(|e| e.to_string())?;
    let Some(img) = images.first() else {
        return Ok(false);
    };
    let collections = db
        .image_collection_ids(image_id)
        .map_err(|e| e.to_string())?;
    if image_in_scope(scope, &img.path, &collections) {
        return Ok(true);
    }

    // Tag scope (union with folder/collection). Match scope tag names against the
    // image's tags by normalized name so casing/whitespace differences still match.
    if let Some(scope_tags) = scope.as_ref().and_then(|s| s.tags.as_ref()) {
        let image_tags = db.list_image_tags(image_id).map_err(|e| e.to_string())?;
        let image_norms: std::collections::HashSet<&str> = image_tags
            .iter()
            .map(|t| t.normalized_name.as_str())
            .collect();
        for tag in scope_tags {
            if let Some(norm) = crate::db_core::tags::normalize_tag_name(tag) {
                if image_norms.contains(norm.as_str()) {
                    return Ok(true);
                }
            }
        }
    }

    Ok(false)
}

pub fn folder_in_scope(scope: &Option<TokenScope>, folder_path: &str) -> bool {
    let s = match scope {
        None => return true,
        Some(s) => s,
    };

    if let Some(folders) = &s.folders {
        for allowed in folders {
            if is_path_under(folder_path, allowed) {
                return true;
            }
        }
    }

    false
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db_core::db::Database;
    use crate::db_core::detection::DetectionEngine;
    use crate::db_core::embeddings::EmbeddingEngine;
    use crate::db_core::secrets::{MemoryStore, SecretStore};
    use parking_lot::Mutex;
    use std::path::PathBuf;

    fn test_context() -> (
        Database,
        MemoryStore,
        PathBuf,
        Mutex<EmbeddingEngine>,
        Mutex<DetectionEngine>,
        Mutex<DetectionEngine>,
        tempfile::TempDir,
    ) {
        let tmp = tempfile::tempdir().unwrap();
        let db = Database::open(std::path::Path::new(":memory:")).unwrap();
        let secrets = MemoryStore::new();
        let app_data_dir = tmp.path().to_path_buf();
        let model_dir = tmp.path().join("models");
        let embedding_engine = Mutex::new(EmbeddingEngine::new(&model_dir));
        let detection_engine = Mutex::new(DetectionEngine::new_yolo(&model_dir));
        let safety_engine = Mutex::new(DetectionEngine::new_nudenet(&model_dir));
        (
            db,
            secrets,
            app_data_dir,
            embedding_engine,
            detection_engine,
            safety_engine,
            tmp,
        )
    }

    fn make_ctx<'a>(
        db: &'a Database,
        secrets: &'a MemoryStore,
        app_data_dir: &'a PathBuf,
        embedding_engine: &'a Mutex<EmbeddingEngine>,
        detection_engine: &'a Mutex<DetectionEngine>,
        safety_engine: &'a Mutex<DetectionEngine>,
    ) -> ServiceContext<'a> {
        ServiceContext {
            db,
            app_data_dir,
            embedding_engine,
            detection_engine,
            safety_engine,
            secrets,
            app_handle: None,
        }
    }

    fn insert_test_image(db: &Database, id: &str, path: &str) {
        db.insert_image(&crate::db_core::models::Image {
            id: id.to_string(),
            sha256_hash: format!("hash-{id}"),
            width: 10,
            height: 10,
            format: "png".to_string(),
            file_size: 1,
            created_at: "2026-01-01T00:00:00Z".to_string(),
            imported_at: "2026-01-01T00:00:00Z".to_string(),
            ai_prompt: None,
            raw_metadata: None,
        })
        .unwrap();
        db.insert_image_file(&crate::db_core::models::ImageFile {
            id: format!("file-{id}"),
            image_id: id.to_string(),
            path: path.to_string(),
            last_seen_at: "2026-01-01T00:00:00Z".to_string(),
            missing_at: None,
            last_seen_size: None,
            last_seen_mtime: None,
        })
        .unwrap();
    }

    #[test]
    fn image_id_in_scope_honors_collection_membership() {
        let (db, ..) = test_context();
        insert_test_image(&db, "imgA", "/lib/a.png");
        insert_test_image(&db, "imgB", "/lib/b.png");
        let col = db.create_collection("C1").unwrap();
        db.add_to_collection(&col, &["imgA"]).unwrap();

        let scope = Some(TokenScope {
            collections: Some(vec![col.clone()]),
            folders: None,
            tags: None,
        });

        // imgA is in the scoped collection -> allowed; imgB is not -> denied.
        assert!(image_id_in_scope(&db, &scope, "imgA").unwrap());
        assert!(!image_id_in_scope(&db, &scope, "imgB").unwrap());
        // No scope -> always allowed; unknown id -> denied.
        assert!(image_id_in_scope(&db, &None, "imgB").unwrap());
        assert!(!image_id_in_scope(&db, &scope, "ghost").unwrap());
    }

    #[test]
    fn image_id_in_scope_honors_tag_membership() {
        let (db, ..) = test_context();
        insert_test_image(&db, "imgA", "/lib/a.png");
        insert_test_image(&db, "imgB", "/lib/b.png");
        db.add_image_tag("imgA", "public", "user", "manual", None)
            .unwrap();

        let scope = Some(TokenScope {
            collections: None,
            folders: None,
            tags: Some(vec!["public".to_string()]),
        });

        // Tagged image allowed; untagged denied.
        assert!(image_id_in_scope(&db, &scope, "imgA").unwrap());
        assert!(!image_id_in_scope(&db, &scope, "imgB").unwrap());
    }

    #[test]
    fn image_id_in_scope_tag_match_is_normalized() {
        let (db, ..) = test_context();
        insert_test_image(&db, "imgA", "/lib/a.png");
        db.add_image_tag("imgA", "Public", "user", "manual", None)
            .unwrap();
        // Scope spelled differently but normalizes to the same tag.
        let scope = Some(TokenScope {
            collections: None,
            folders: None,
            tags: Some(vec![" PUBLIC ".to_string()]),
        });
        assert!(image_id_in_scope(&db, &scope, "imgA").unwrap());
    }

    #[test]
    fn image_id_in_scope_empty_tags_grants_nothing() {
        let (db, ..) = test_context();
        insert_test_image(&db, "imgA", "/lib/a.png");
        db.add_image_tag("imgA", "public", "user", "manual", None)
            .unwrap();
        // An explicit empty tags list grants no tag-based access.
        let scope = Some(TokenScope {
            collections: None,
            folders: None,
            tags: Some(vec![]),
        });
        assert!(!image_id_in_scope(&db, &scope, "imgA").unwrap());
    }

    #[test]
    fn image_id_in_scope_unnormalizable_scope_tag_is_ignored() {
        let (db, ..) = test_context();
        insert_test_image(&db, "imgA", "/lib/a.png");
        db.add_image_tag("imgA", "public", "user", "manual", None)
            .unwrap();
        // A scope tag that normalizes to nothing is a safe no-op, not a grant.
        let scope = Some(TokenScope {
            collections: None,
            folders: None,
            tags: Some(vec!["!!!".to_string()]),
        });
        assert!(!image_id_in_scope(&db, &scope, "imgA").unwrap());
    }

    #[test]
    fn image_id_in_scope_union_across_folder_collection_tag() {
        let (db, ..) = test_context();
        insert_test_image(&db, "byFolder", "/art/x.png");
        insert_test_image(&db, "byCollection", "/other/y.png");
        insert_test_image(&db, "byTag", "/other/z.png");
        insert_test_image(&db, "none", "/other/n.png");
        let col = db.create_collection("C1").unwrap();
        db.add_to_collection(&col, &["byCollection"]).unwrap();
        db.add_image_tag("byTag", "public", "user", "manual", None)
            .unwrap();

        let scope = Some(TokenScope {
            collections: Some(vec![col]),
            folders: Some(vec!["/art".to_string()]),
            tags: Some(vec!["public".to_string()]),
        });

        // Any one of folder / collection / tag membership grants access (union).
        assert!(image_id_in_scope(&db, &scope, "byFolder").unwrap());
        assert!(image_id_in_scope(&db, &scope, "byCollection").unwrap());
        assert!(image_id_in_scope(&db, &scope, "byTag").unwrap());
        assert!(!image_id_in_scope(&db, &scope, "none").unwrap());
    }

    #[test]
    fn image_collection_ids_returns_memberships() {
        let (db, ..) = test_context();
        insert_test_image(&db, "imgA", "/lib/a.png");
        let c1 = db.create_collection("C1").unwrap();
        let c2 = db.create_collection("C2").unwrap();
        db.add_to_collection(&c1, &["imgA"]).unwrap();
        db.add_to_collection(&c2, &["imgA"]).unwrap();

        let mut ids = db.image_collection_ids("imgA").unwrap();
        ids.sort();
        let mut expected = vec![c1, c2];
        expected.sort();
        assert_eq!(ids, expected);
        assert!(db.image_collection_ids("imgB").unwrap().is_empty());
    }

    #[test]
    fn test_capabilities_for_roles() {
        let viewer_caps = capabilities_for_role(ROLE_VIEWER);
        assert_eq!(viewer_caps, vec!["library:read", "library:search"]);

        let curator_caps = capabilities_for_role(ROLE_CURATOR);
        assert!(curator_caps.contains(&"curation:write"));
        assert!(curator_caps.contains(&"library:read"));
        assert!(curator_caps.contains(&"export:read"));

        let operator_caps = capabilities_for_role(ROLE_OPERATOR);
        assert!(operator_caps.contains(&"import:write"));
        assert!(operator_caps.contains(&"ai:run"));

        let admin_caps = capabilities_for_role(ROLE_ADMIN);
        assert!(admin_caps.contains(&"tokens:manage"));
        assert!(admin_caps.contains(&"display:navigate"));
        assert!(admin_caps.contains(&"settings:manage"));
        assert_eq!(admin_caps.len(), 9);

        assert!(capabilities_for_role("bogus").is_empty());
    }

    #[test]
    fn test_has_capability() {
        assert!(has_capability(ROLE_ADMIN, "tokens:manage"));
        assert!(has_capability(ROLE_ADMIN, "library:read"));
        assert!(!has_capability(ROLE_VIEWER, "curation:write"));
        assert!(!has_capability(ROLE_VIEWER, "tokens:manage"));
        assert!(has_capability(ROLE_CURATOR, "library:read"));
        assert!(has_capability(ROLE_CURATOR, "curation:write"));
        assert!(!has_capability(ROLE_CURATOR, "import:write"));
        assert!(has_capability(ROLE_OPERATOR, "import:write"));
        assert!(has_capability(ROLE_OPERATOR, "ai:run"));
        assert!(!has_capability(ROLE_OPERATOR, "display:navigate"));
    }

    #[test]
    fn test_tool_capability_mapping() {
        assert_eq!(tool_capability("list_images"), "library:read");
        assert_eq!(tool_capability("get_image"), "library:read");
        assert_eq!(tool_capability("list_collections"), "library:read");
        assert_eq!(tool_capability("list_session_canvases"), "library:read");
        assert_eq!(tool_capability("get_canvas_layout"), "library:read");
        assert_eq!(tool_capability("get_library_stats"), "library:read");
        assert_eq!(tool_capability("search_images"), "library:search");
        assert_eq!(tool_capability("find_similar"), "library:search");
        assert_eq!(tool_capability("set_rating"), "curation:write");
        assert_eq!(tool_capability("create_collection"), "curation:write");
        assert_eq!(tool_capability("import_folder"), "import:write");
        assert_eq!(tool_capability("list_export_presets"), "export:read");
        assert_eq!(
            tool_capability("export_static_publish_package"),
            "export:read"
        );
        assert_eq!(
            tool_capability("export_static_publish_canvas"),
            "export:read"
        );
        assert_eq!(
            tool_capability("serve_static_publish_package"),
            "settings:manage"
        );
        assert_eq!(tool_capability("show_image"), "display:navigate");
        assert_eq!(tool_capability("download_embedding_model"), "ai:run");
        assert_eq!(tool_capability("generate_embeddings"), "ai:run");
        assert_eq!(tool_capability("analyze_image_quality"), "ai:run");
        assert_eq!(tool_capability("get_image_quality"), "library:read");
        assert_eq!(tool_capability("get_quality_count"), "library:read");
        assert_eq!(tool_capability("create_token"), "tokens:manage");
        assert_eq!(tool_capability("unknown_tool"), "settings:manage");
    }

    #[test]
    fn test_hash_and_verify() {
        let pepper = "test_pepper_value";
        let secret = "my_secret_token_123";
        let h = hash_secret(pepper, secret);
        assert!(verify_secret(pepper, secret, &h));
        assert!(!verify_secret(pepper, "wrong_secret", &h));
    }

    #[test]
    fn test_constant_time_wrong_pepper() {
        let pepper = "correct_pepper";
        let secret = "the_secret";
        let h = hash_secret(pepper, secret);
        assert!(!verify_secret("wrong_pepper", secret, &h));
    }

    #[test]
    fn test_generate_token_id_format() {
        let id = generate_token_id();
        assert!(id.starts_with("tok_"));
        assert_eq!(id.len(), 16);
        assert!(id[4..].chars().all(|c| c.is_ascii_alphanumeric()));
    }

    #[test]
    fn test_generate_secret_length() {
        let s = generate_secret();
        assert_eq!(s.len(), 64);
        assert!(s.chars().all(|c| c.is_ascii_hexdigit()));
    }

    #[test]
    fn test_create_token() {
        let (db, secrets, dir, ee, de, se, _tmp) = test_context();
        let ctx = make_ctx(&db, &secrets, &dir, &ee, &de, &se);

        let (token, secret) = create_token(&ctx, "Test Token", ROLE_VIEWER, None).unwrap();
        assert!(token.id.starts_with("tok_"));
        assert_eq!(token.name, "Test Token");
        assert_eq!(token.role, ROLE_VIEWER);
        assert!(!secret.is_empty());
        assert_eq!(secret.len(), 64);
        assert!(token.scope_json.is_none());
        assert!(!token.revoked);

        let tokens = list_tokens(&ctx).unwrap();
        assert_eq!(tokens.len(), 1);
        assert_eq!(tokens[0].id, token.id);
    }

    #[test]
    fn test_create_token_with_scope() {
        let (db, secrets, dir, ee, de, se, _tmp) = test_context();
        let ctx = make_ctx(&db, &secrets, &dir, &ee, &de, &se);

        let scope = TokenScope {
            collections: Some(vec!["col_abc".to_string()]),
            folders: None,
            tags: Some(vec!["public".to_string()]),
        };

        let (token, _) = create_token(&ctx, "Scoped", ROLE_CURATOR, Some(scope)).unwrap();
        assert!(token.scope_json.is_some());
        let parsed: TokenScope = serde_json::from_str(token.scope_json.as_ref().unwrap()).unwrap();
        assert_eq!(parsed.collections.unwrap(), vec!["col_abc"]);
        assert_eq!(parsed.tags.unwrap(), vec!["public"]);
        assert!(parsed.folders.is_none());
    }

    #[test]
    fn test_create_token_invalid_role() {
        let (db, secrets, dir, ee, de, se, _tmp) = test_context();
        let ctx = make_ctx(&db, &secrets, &dir, &ee, &de, &se);

        let result = create_token(&ctx, "Bad", "superadmin", None);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("Invalid role"));
    }

    #[test]
    fn test_validate_token() {
        let (db, secrets, dir, ee, de, se, _tmp) = test_context();
        let ctx = make_ctx(&db, &secrets, &dir, &ee, &de, &se);

        let (token, secret) = create_token(&ctx, "Auth Test", ROLE_ADMIN, None).unwrap();

        let validated = validate_token(&ctx, &secret).unwrap();
        assert!(validated.is_some());
        let v = validated.unwrap();
        assert_eq!(v.id, token.id);
        assert_eq!(v.role, ROLE_ADMIN);
        assert!(v.last_used_at.is_some());
    }

    #[test]
    fn test_validate_wrong_secret() {
        let (db, secrets, dir, ee, de, se, _tmp) = test_context();
        let ctx = make_ctx(&db, &secrets, &dir, &ee, &de, &se);

        let _ = create_token(&ctx, "Token", ROLE_VIEWER, None).unwrap();

        let result = validate_token(&ctx, "totally_wrong_secret_value").unwrap();
        assert!(result.is_none());
    }

    #[test]
    fn test_revoke_token() {
        let (db, secrets, dir, ee, de, se, _tmp) = test_context();
        let ctx = make_ctx(&db, &secrets, &dir, &ee, &de, &se);

        let (token, secret) = create_token(&ctx, "To Revoke", ROLE_CURATOR, None).unwrap();

        revoke_token(&ctx, &token.id).unwrap();

        let validated = validate_token(&ctx, &secret).unwrap();
        assert!(validated.is_none());

        let tokens = list_tokens(&ctx).unwrap();
        assert!(tokens.is_empty());
    }

    #[test]
    fn test_revoke_nonexistent() {
        let (db, secrets, dir, ee, de, se, _tmp) = test_context();
        let ctx = make_ctx(&db, &secrets, &dir, &ee, &de, &se);

        let result = revoke_token(&ctx, "tok_nonexistent");
        assert!(result.is_err());
    }

    #[test]
    fn test_rotate_token() {
        let (db, secrets, dir, ee, de, se, _tmp) = test_context();
        let ctx = make_ctx(&db, &secrets, &dir, &ee, &de, &se);

        let (token, old_secret) = create_token(&ctx, "Rotatable", ROLE_OPERATOR, None).unwrap();

        let new_secret = rotate_token(&ctx, &token.id).unwrap();
        assert_ne!(old_secret, new_secret);
        assert_eq!(new_secret.len(), 64);

        let old_result = validate_token(&ctx, &old_secret).unwrap();
        assert!(old_result.is_none());

        let new_result = validate_token(&ctx, &new_secret).unwrap();
        assert!(new_result.is_some());
        assert_eq!(new_result.unwrap().id, token.id);
    }

    #[test]
    fn test_list_tokens_multiple() {
        let (db, secrets, dir, ee, de, se, _tmp) = test_context();
        let ctx = make_ctx(&db, &secrets, &dir, &ee, &de, &se);

        create_token(&ctx, "Token A", ROLE_VIEWER, None).unwrap();
        create_token(&ctx, "Token B", ROLE_CURATOR, None).unwrap();
        create_token(&ctx, "Token C", ROLE_ADMIN, None).unwrap();

        let tokens = list_tokens(&ctx).unwrap();
        assert_eq!(tokens.len(), 3);

        let names: Vec<&str> = tokens.iter().map(|t| t.name.as_str()).collect();
        assert!(names.contains(&"Token A"));
        assert!(names.contains(&"Token B"));
        assert!(names.contains(&"Token C"));
    }

    #[test]
    fn test_audit_log() {
        let (db, secrets, dir, ee, de, se, _tmp) = test_context();
        let ctx = make_ctx(&db, &secrets, &dir, &ee, &de, &se);

        log_audit(
            &ctx,
            Some("tok_abc"),
            "list_images",
            Some(r#"{"limit":10}"#),
            "ok",
        )
        .unwrap();
        log_audit(&ctx, None, "get_library_stats", None, "ok").unwrap();
        log_audit(
            &ctx,
            Some("tok_abc"),
            "set_rating",
            Some(r#"{"image_id":"img1"}"#),
            "denied",
        )
        .unwrap();

        let entries = get_recent_audit(&ctx, 10).unwrap();
        assert_eq!(entries.len(), 3);
        assert_eq!(entries[0].tool_name, "set_rating");
        assert_eq!(entries[0].result_status, "denied");
        assert_eq!(entries[1].tool_name, "get_library_stats");
        assert!(entries[1].token_id.is_none());
        assert_eq!(entries[2].tool_name, "list_images");
    }

    #[test]
    fn test_prune_audit_log() {
        let (db, secrets, dir, ee, de, se, _tmp) = test_context();
        let ctx = make_ctx(&db, &secrets, &dir, &ee, &de, &se);

        // Insert an entry with an old timestamp
        {
            let conn = ctx.db.conn.lock();
            conn.execute(
                "INSERT INTO mcp_audit_log (token_id, tool_name, params_json, result_status, timestamp) VALUES (?1, ?2, ?3, ?4, ?5)",
                rusqlite::params![None::<String>, "old_tool", None::<String>, "ok", "2020-01-01T00:00:00+00:00"],
            ).unwrap();
        }

        log_audit(&ctx, None, "recent_tool", None, "ok").unwrap();

        let before = get_recent_audit(&ctx, 10).unwrap();
        assert_eq!(before.len(), 2);

        let pruned = prune_audit_log(&ctx, 30).unwrap();
        assert_eq!(pruned, 1);

        let after = get_recent_audit(&ctx, 10).unwrap();
        assert_eq!(after.len(), 1);
        assert_eq!(after[0].tool_name, "recent_tool");
    }

    #[test]
    fn test_pepper_auto_generated() {
        let (db, secrets, dir, ee, de, se, _tmp) = test_context();
        let ctx = make_ctx(&db, &secrets, &dir, &ee, &de, &se);

        assert!(secrets.get("mcp_pepper").unwrap().is_none());

        create_token(&ctx, "First", ROLE_VIEWER, None).unwrap();

        let pepper = secrets.get("mcp_pepper").unwrap();
        assert!(pepper.is_some());
        assert_eq!(pepper.unwrap().len(), 64);
    }

    #[test]
    fn test_parse_scope_none() {
        assert!(parse_scope(&None).is_none());
    }

    #[test]
    fn test_parse_scope_valid() {
        let json =
            Some(r#"{"collections":["col_a"],"folders":["/art"],"tags":["public"]}"#.to_string());
        let scope = parse_scope(&json).unwrap();
        assert_eq!(scope.collections.unwrap(), vec!["col_a"]);
        assert_eq!(scope.folders.unwrap(), vec!["/art"]);
        assert_eq!(scope.tags.unwrap(), vec!["public"]);
    }

    #[test]
    fn test_parse_scope_invalid_json() {
        let json = Some("not json".to_string());
        assert!(parse_scope(&json).is_none());
    }

    #[test]
    fn test_image_in_scope_unrestricted() {
        assert!(image_in_scope(&None, "/any/path/image.png", &[]));
    }

    #[test]
    fn test_image_in_scope_folder_match() {
        let scope = Some(TokenScope {
            collections: None,
            folders: Some(vec!["/art/midjourney".to_string()]),
            tags: None,
        });
        assert!(image_in_scope(&scope, "/art/midjourney/img001.png", &[]));
        assert!(image_in_scope(
            &scope,
            "/art/midjourney/subfolder/img.jpg",
            &[]
        ));
    }

    #[test]
    fn test_image_in_scope_folder_no_match() {
        let scope = Some(TokenScope {
            collections: None,
            folders: Some(vec!["/art/midjourney".to_string()]),
            tags: None,
        });
        assert!(!image_in_scope(&scope, "/art/dalle/img001.png", &[]));
        assert!(!image_in_scope(&scope, "/photos/vacation.jpg", &[]));
    }

    #[test]
    fn test_image_in_scope_collection_match() {
        let scope = Some(TokenScope {
            collections: Some(vec!["col_abc".to_string()]),
            folders: None,
            tags: None,
        });
        assert!(image_in_scope(
            &scope,
            "/any/path.png",
            &["col_abc".to_string()]
        ));
        assert!(!image_in_scope(
            &scope,
            "/any/path.png",
            &["col_other".to_string()]
        ));
        assert!(!image_in_scope(&scope, "/any/path.png", &[]));
    }

    #[test]
    fn test_image_in_scope_union_semantics() {
        let scope = Some(TokenScope {
            collections: Some(vec!["col_abc".to_string()]),
            folders: Some(vec!["/art/dalle".to_string()]),
            tags: None,
        });
        // Matches folder
        assert!(image_in_scope(&scope, "/art/dalle/img.png", &[]));
        // Matches collection
        assert!(image_in_scope(
            &scope,
            "/other/path.png",
            &["col_abc".to_string()]
        ));
        // Matches neither
        assert!(!image_in_scope(&scope, "/other/path.png", &[]));
    }

    #[test]
    fn test_image_in_scope_no_match() {
        let scope = Some(TokenScope {
            collections: Some(vec!["col_abc".to_string()]),
            folders: Some(vec!["/art".to_string()]),
            tags: Some(vec!["public".to_string()]),
        });
        assert!(!image_in_scope(
            &scope,
            "/photos/vacation.jpg",
            &["col_other".to_string()]
        ));
    }

    #[test]
    fn test_folder_in_scope_unrestricted() {
        assert!(folder_in_scope(&None, "/any/folder"));
    }

    #[test]
    fn test_folder_in_scope_match() {
        let scope = Some(TokenScope {
            folders: Some(vec!["/art/midjourney".to_string()]),
            collections: None,
            tags: None,
        });
        assert!(folder_in_scope(&scope, "/art/midjourney"));
        assert!(folder_in_scope(&scope, "/art/midjourney/subfolder"));
        assert!(!folder_in_scope(&scope, "/art"));
    }

    #[test]
    fn test_folder_in_scope_no_match() {
        let scope = Some(TokenScope {
            folders: Some(vec!["/art/midjourney".to_string()]),
            collections: None,
            tags: None,
        });
        assert!(!folder_in_scope(&scope, "/photos"));
        assert!(!folder_in_scope(&scope, "/art/dalle"));
    }

    #[test]
    fn test_path_traversal_prevention() {
        let scope = Some(TokenScope {
            folders: Some(vec!["/art".to_string()]),
            collections: None,
            tags: None,
        });
        // /art/image.jpg should match
        assert!(image_in_scope(&scope, "/art/image.jpg", &[]));
        // /art/sub/image.jpg should match
        assert!(image_in_scope(&scope, "/art/sub/image.jpg", &[]));
        // /artifacts/image.jpg should NOT match (path traversal)
        assert!(!image_in_scope(&scope, "/artifacts/image.jpg", &[]));
        // /artisan/image.jpg should NOT match
        assert!(!image_in_scope(&scope, "/artisan/image.jpg", &[]));
        // Same for folder_in_scope
        assert!(folder_in_scope(&scope, "/art/sub"));
        assert!(!folder_in_scope(&scope, "/artifacts"));
    }

    #[test]
    fn test_image_scope_rejects_canonical_parent_and_symlink_escape() {
        let tmp = tempfile::tempdir().unwrap();
        let allowed = tmp.path().join("allowed");
        let sibling = tmp.path().join("sibling");
        std::fs::create_dir_all(&allowed).unwrap();
        std::fs::create_dir_all(&sibling).unwrap();

        let sibling_image = sibling.join("secret.jpg");
        std::fs::write(&sibling_image, b"secret").unwrap();

        let scope = Some(TokenScope {
            folders: Some(vec![allowed.to_string_lossy().to_string()]),
            collections: None,
            tags: None,
        });

        let dotdot_escape = allowed.join("..").join("sibling").join("secret.jpg");
        assert!(!image_in_scope(
            &scope,
            dotdot_escape.to_string_lossy().as_ref(),
            &[]
        ));

        #[cfg(unix)]
        {
            use std::os::unix::fs::symlink;

            let link = allowed.join("external");
            symlink(&sibling, &link).unwrap();
            let symlink_escape = link.join("secret.jpg");
            assert!(!image_in_scope(
                &scope,
                symlink_escape.to_string_lossy().as_ref(),
                &[]
            ));
        }
    }

    #[test]
    fn test_folder_scope_rejects_parent_of_allowed_folder() {
        let scope = Some(TokenScope {
            folders: Some(vec!["/art/midjourney".to_string()]),
            collections: None,
            tags: None,
        });

        assert!(!folder_in_scope(&scope, "/art"));
    }

    #[test]
    fn test_audit_log_redacts_paths_prompts_and_secrets() {
        let (db, secrets, dir, ee, de, se, _tmp) = test_context();
        let ctx = make_ctx(&db, &secrets, &dir, &ee, &de, &se);

        log_audit(
            &ctx,
            Some("tok_abc"),
            "import_folder",
            Some(
                r#"{"folder_path":"/Users/alice/secret/art","prompt":"private prompt","api_key":"sk-test","limit":10}"#,
            ),
            "ok",
        )
        .unwrap();

        let entries = get_recent_audit(&ctx, 1).unwrap();
        let params = entries[0].params_json.as_deref().unwrap();
        assert!(!params.contains("/Users/alice/secret/art"));
        assert!(!params.contains("private prompt"));
        assert!(!params.contains("sk-test"));
        assert!(params.contains("\"limit\":10"));
        assert!(params.contains("[redacted:path]"));
        assert!(params.contains("[redacted:text]"));
        assert!(params.contains("[redacted:secret]"));
    }

    #[test]
    fn test_pepper_reused() {
        let (db, secrets, dir, ee, de, se, _tmp) = test_context();
        let ctx = make_ctx(&db, &secrets, &dir, &ee, &de, &se);

        let (_, secret1) = create_token(&ctx, "T1", ROLE_VIEWER, None).unwrap();
        let pepper1 = secrets.get("mcp_pepper").unwrap().unwrap();

        let (_, _secret2) = create_token(&ctx, "T2", ROLE_VIEWER, None).unwrap();
        let pepper2 = secrets.get("mcp_pepper").unwrap().unwrap();

        assert_eq!(pepper1, pepper2);

        let v = validate_token(&ctx, &secret1).unwrap();
        assert!(v.is_some());
    }
}
