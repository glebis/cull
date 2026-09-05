use crate::db_core::db::Database;
use crate::db_core::models::{
    AgentActionProposal, AgentSelectionPreset, CreateActionProposalDb, UpsertAgentSelectionPresetDb,
};
use crate::services::selection_run::ShortlistDirection;
use crate::services::ServiceError;
use rusqlite::OptionalExtension;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateActionProposalRequest {
    pub kind: String,
    pub persona: String,
    pub lens: Option<String>,
    pub criteria: String,
    pub visual_level: String,
    pub selection_preset_id: Option<String>,
    pub estimated_input_tokens: Option<i64>,
    pub estimated_output_tokens: Option<i64>,
    pub estimated_cost_eur: Option<f64>,
    pub source_context_json: String,
    pub items_json: String,
    pub guard_results_json: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApplyActionProposalResult {
    pub proposal_id: String,
    pub status: String,
    pub applied_count: u32,
    pub failed_count: u32,
    pub result_json: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpsertAgentSelectionPresetRequest {
    pub id: Option<String>,
    pub name: String,
    pub purpose: String,
    pub prompt: String,
    pub criteria_json: String,
    pub sort_order: Option<i64>,
}

pub fn validate_create_request(request: &CreateActionProposalRequest) -> Result<(), ServiceError> {
    let valid_kind = matches!(
        request.kind.as_str(),
        "select_images"
            | "set_decisions"
            | "create_collection"
            | "add_to_collection"
            | "remove_from_collection"
            | "reorder_canvas"
            | "remove_from_canvas"
            | "trash_images"
            | "shortlist_add"
            | "shortlist_remove"
    );
    if !valid_kind {
        return Err(ServiceError::InvalidInput(format!(
            "Unsupported proposal kind '{}'",
            request.kind
        )));
    }
    if !matches!(request.persona.as_str(), "curator" | "copilot" | "operator") {
        return Err(ServiceError::InvalidInput(format!(
            "Unsupported persona '{}'",
            request.persona
        )));
    }
    if !matches!(
        request.visual_level.as_str(),
        "text" | "tiny" | "preview" | "full"
    ) {
        return Err(ServiceError::InvalidInput(format!(
            "Unsupported visual level '{}'",
            request.visual_level
        )));
    }
    serde_json::from_str::<serde_json::Value>(&request.source_context_json)
        .map_err(|e| ServiceError::InvalidInput(format!("Invalid source_context_json: {}", e)))?;
    if is_shortlist_kind(&request.kind) {
        // Immutable target contract: shortlist proposals are bound to the
        // exact selection run captured at proposal creation. Neither the
        // active UI run nor client-provided result data may retarget them.
        shortlist_target_from_context(&request.source_context_json)?;
    }
    serde_json::from_str::<serde_json::Value>(&request.guard_results_json)
        .map_err(|e| ServiceError::InvalidInput(format!("Invalid guard_results_json: {}", e)))?;
    let items: serde_json::Value = serde_json::from_str(&request.items_json)
        .map_err(|e| ServiceError::InvalidInput(format!("Invalid items_json: {}", e)))?;
    if request.kind != "create_collection" && items.as_array().map(|a| a.is_empty()).unwrap_or(true)
    {
        return Err(ServiceError::InvalidInput(
            "Proposal requires at least one candidate".to_string(),
        ));
    }
    Ok(())
}

pub fn create_action_proposal_db(
    db: &Database,
    request: CreateActionProposalRequest,
) -> Result<AgentActionProposal, ServiceError> {
    validate_create_request(&request)?;
    if let Some(preset_id) = request.selection_preset_id.as_deref() {
        if db.get_agent_selection_preset(preset_id)?.is_none() {
            return Err(ServiceError::NotFound(format!(
                "Selection preset '{}' was not found",
                preset_id
            )));
        }
    }
    if is_shortlist_kind(&request.kind) {
        let selection_id = shortlist_target_from_context(&request.source_context_json)?;
        validate_shortlist_target(db, &selection_id)?;
    }
    db.create_action_proposal(CreateActionProposalDb {
        kind: request.kind,
        persona: request.persona,
        lens: request.lens,
        criteria: request.criteria,
        visual_level: request.visual_level,
        selection_preset_id: request.selection_preset_id,
        estimated_input_tokens: request.estimated_input_tokens,
        estimated_output_tokens: request.estimated_output_tokens,
        estimated_cost_eur: request.estimated_cost_eur,
        source_context_json: request.source_context_json,
        items_json: request.items_json,
        guard_results_json: request.guard_results_json,
    })
    .map_err(ServiceError::Database)
}

pub fn list_action_proposals_db(
    db: &Database,
    status: Option<&str>,
    limit: u32,
) -> Result<Vec<AgentActionProposal>, ServiceError> {
    db.list_action_proposals(status, limit)
        .map_err(ServiceError::Database)
}

pub fn dismiss_action_proposal_db(db: &Database, proposal_id: &str) -> Result<(), ServiceError> {
    if db.dismiss_action_proposal(proposal_id)? {
        Ok(())
    } else {
        Err(ServiceError::InvalidInput(format!(
            "Proposal '{}' is not pending or does not exist",
            proposal_id
        )))
    }
}

pub fn apply_action_proposal_db(
    db: &Database,
    action_manager: &crate::services::undo::ActionManager,
    proposal_id: &str,
    approved_image_ids: &[String],
    result_json: &str,
) -> Result<ApplyActionProposalResult, ServiceError> {
    let proposal = db.get_action_proposal(proposal_id)?.ok_or_else(|| {
        ServiceError::NotFound(format!("Proposal '{}' was not found", proposal_id))
    })?;
    if proposal.status != "pending" {
        return Err(ServiceError::InvalidInput(format!(
            "Proposal '{}' is not pending",
            proposal_id
        )));
    }
    serde_json::from_str::<serde_json::Value>(result_json)
        .map_err(|e| ServiceError::InvalidInput(format!("Invalid result_json: {}", e)))?;

    if is_shortlist_kind(&proposal.kind) {
        return apply_shortlist_proposal_db(
            db,
            action_manager,
            proposal_id,
            &proposal,
            approved_image_ids,
            result_json,
        );
    }

    let undo_journal_json = serde_json::json!({
        "proposal_id": proposal_id,
        "kind": proposal.kind,
        "approved_image_ids": approved_image_ids,
    })
    .to_string();
    if !db.mark_action_proposal_applied(proposal_id, result_json, &undo_journal_json)? {
        return Err(ServiceError::InvalidInput(format!(
            "Proposal '{}' is not pending",
            proposal_id
        )));
    }
    Ok(ApplyActionProposalResult {
        proposal_id: proposal_id.to_string(),
        status: "applied".to_string(),
        applied_count: approved_image_ids.len() as u32,
        failed_count: 0,
        result_json: result_json.to_string(),
    })
}

/// Applies a reviewed shortlist proposal in ONE atomic backend operation:
/// membership mutation, undo record, proposal status/undo-journal update and
/// agent provenance all commit together or not at all. The mutation target is
/// the proposal's immutable `source_context_json.selection_id` — never the
/// active UI run or client `result_json`. Approved IDs are revalidated
/// against the stored proposal items inside the same transaction, and the
/// pending status is re-checked there so duplicate consumption is rejected.
fn apply_shortlist_proposal_db(
    db: &Database,
    action_manager: &crate::services::undo::ActionManager,
    proposal_id: &str,
    proposal: &AgentActionProposal,
    approved_image_ids: &[String],
    result_json: &str,
) -> Result<ApplyActionProposalResult, ServiceError> {
    if approved_image_ids.is_empty() {
        return Err(ServiceError::InvalidInput(
            "Approved proposal subset is empty".to_string(),
        ));
    }
    let selection_id = shortlist_target_from_context(&proposal.source_context_json)?;
    let direction = match proposal.kind.as_str() {
        "shortlist_add" => ShortlistDirection::Add,
        _ => ShortlistDirection::Remove,
    };
    let action = match direction {
        ShortlistDirection::Add => crate::services::undo::Action::ShortlistAdd {
            selection_id: selection_id.clone(),
            image_ids: approved_image_ids.to_vec(),
        },
        ShortlistDirection::Remove => crate::services::undo::Action::ShortlistRemove {
            selection_id: selection_id.clone(),
            image_ids: approved_image_ids.to_vec(),
        },
    };

    let proposal_id_owned = proposal_id.to_string();
    let kind = proposal.kind.clone();
    let result_json_owned = result_json.to_string();
    let approved_owned = approved_image_ids.to_vec();
    let applied_count = std::cell::Cell::new(0);
    let count = &applied_count;
    action_manager
        .execute_shortlist_transactional(db, action, move |tx, changed_count| {
            count.set(changed_count as u32);
            // Revalidate inside the mutation transaction: pending status,
            // approved subset ⊆ reviewed items, then consume the proposal.
            let status: String = tx
                .query_row(
                    "SELECT status FROM agent_action_proposals WHERE id = ?1",
                    rusqlite::params![proposal_id_owned],
                    |row| row.get(0),
                )
                .optional()
                .map_err(|e| e.to_string())?
                .ok_or_else(|| format!("Proposal '{proposal_id_owned}' no longer exists"))?;
            if status != "pending" {
                return Err(format!(
                    "Proposal '{proposal_id_owned}' was already consumed",
                ));
            }
            let items_json: String = tx
                .query_row(
                    "SELECT items_json FROM agent_action_proposals WHERE id = ?1",
                    rusqlite::params![proposal_id_owned],
                    |row| row.get(0),
                )
                .map_err(|e| e.to_string())?;
            let stored_items: Vec<String> = proposal_item_image_ids(&items_json)
                .map_err(|e| ServiceError::InvalidInput(e).to_string())?;
            for id in &approved_owned {
                if !stored_items.contains(id) {
                    return Err(format!(
                        "Approved image '{id}' is not part of the reviewed proposal",
                    ));
                }
            }
            let undo_journal_json = serde_json::json!({
                "proposal_id": proposal_id_owned,
                "kind": kind,
                "approved_image_ids": approved_owned,
            })
            .to_string();
            let updated = tx
                .execute(
                    "UPDATE agent_action_proposals
                     SET status = 'applied',
                         apply_result_json = ?2,
                         undo_journal_json = ?3,
                         applied_at = datetime('now'),
                         updated_at = datetime('now')
                     WHERE id = ?1 AND status = 'pending'",
                    rusqlite::params![proposal_id_owned, result_json_owned, undo_journal_json],
                )
                .map_err(|e| e.to_string())?;
            if updated != 1 {
                return Err(format!(
                    "Proposal '{proposal_id_owned}' was already consumed",
                ));
            }
            // Agent provenance is part of the same atomic commit.
            crate::db_core::activity::log_session_event_conn(
                tx,
                &crate::db_core::models::NewSessionEvent {
                    session_id: None,
                    event_type: "shortlist_proposal_applied".to_string(),
                    actor_type: "agent".to_string(),
                    actor_id: Some(proposal_id_owned.clone()),
                    subject_type: Some("selection_run".to_string()),
                    subject_id: Some(selection_id.clone()),
                    payload_json: serde_json::json!({
                        "proposal_id": proposal_id_owned,
                        "kind": kind,
                        "approved_image_ids": approved_owned,
                    })
                    .to_string(),
                },
            )
            .map_err(|e| e.to_string())?;
            Ok(())
        })
        .map_err(ServiceError::InvalidInput)?;

    Ok(ApplyActionProposalResult {
        proposal_id: proposal_id.to_string(),
        status: "applied".to_string(),
        applied_count: applied_count.get(),
        failed_count: 0,
        result_json: result_json.to_string(),
    })
}

fn is_shortlist_kind(kind: &str) -> bool {
    matches!(kind, "shortlist_add" | "shortlist_remove")
}

/// Extracts and validates the immutable selection run target from a
/// proposal's `source_context_json` (top-level `selection_id`).
fn shortlist_target_from_context(source_context_json: &str) -> Result<String, ServiceError> {
    let context: serde_json::Value = serde_json::from_str(source_context_json)
        .map_err(|e| ServiceError::InvalidInput(format!("Invalid source_context_json: {}", e)))?;
    let selection_id = context
        .get("selection_id")
        .and_then(serde_json::Value::as_str)
        .map(str::trim)
        .filter(|id| !id.is_empty())
        .ok_or_else(|| {
            ServiceError::InvalidInput(
                "Shortlist proposal is missing its top-level selection_id target".to_string(),
            )
        })?;
    Ok(selection_id.to_string())
}

/// Validates that the proposal target references an existing, active
/// selection run in the database.
fn validate_shortlist_target(db: &Database, selection_id: &str) -> Result<(), ServiceError> {
    let conn = db.conn.lock();
    let status =
        crate::db_core::queries::selection_runs::selection_run_status_conn(&conn, selection_id)
            .map_err(ServiceError::Database)?;
    drop(conn);
    match status.as_deref() {
        Some("active") => Ok(()),
        Some(other) => Err(ServiceError::InvalidInput(format!(
            "Selection run '{selection_id}' is {other}; shortlist proposals require an active run"
        ))),
        None => Err(ServiceError::NotFound(format!(
            "Selection run '{selection_id}' was not found"
        ))),
    }
}

/// Proposal items are objects with at least `image_id`.
fn proposal_item_image_ids(items_json: &str) -> Result<Vec<String>, String> {
    let items: serde_json::Value =
        serde_json::from_str(items_json).map_err(|e| format!("Invalid items_json: {}", e))?;
    let array = items
        .as_array()
        .ok_or_else(|| "Invalid items_json: expected an array".to_string())?;
    Ok(array
        .iter()
        .filter_map(|item| {
            item.get("image_id")
                .and_then(serde_json::Value::as_str)
                .map(str::to_string)
        })
        .collect())
}

pub fn list_agent_selection_presets_db(
    db: &Database,
) -> Result<Vec<AgentSelectionPreset>, ServiceError> {
    db.list_agent_selection_presets()
        .map_err(ServiceError::Database)
}

pub fn upsert_agent_selection_preset_db(
    db: &Database,
    request: UpsertAgentSelectionPresetRequest,
) -> Result<AgentSelectionPreset, ServiceError> {
    if request.name.trim().is_empty() {
        return Err(ServiceError::InvalidInput(
            "Preset name must not be empty".to_string(),
        ));
    }
    if request.prompt.trim().is_empty() {
        return Err(ServiceError::InvalidInput(
            "Preset prompt must not be empty".to_string(),
        ));
    }
    serde_json::from_str::<serde_json::Value>(&request.criteria_json)
        .map_err(|e| ServiceError::InvalidInput(format!("Invalid criteria_json: {}", e)))?;
    db.upsert_agent_selection_preset(UpsertAgentSelectionPresetDb {
        id: request.id,
        name: request.name,
        purpose: request.purpose,
        prompt: request.prompt,
        criteria_json: request.criteria_json,
        sort_order: request.sort_order,
    })
    .map_err(ServiceError::Database)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db_core::detection::DetectionEngine;
    use crate::db_core::embeddings::EmbeddingEngine;
    use crate::db_core::models::{Image, ImageFile};
    use crate::db_core::secrets::MemoryStore;
    use crate::{services, watcher, AppState};
    use std::path::Path;

    fn db() -> Database {
        let dir = tempfile::tempdir().unwrap();
        Database::open(&dir.path().join("test.db")).unwrap()
    }

    fn test_state(tmp: &Path) -> AppState {
        let db = Database::open(&tmp.join("test.db")).unwrap();
        let app_data_dir = tmp.join("app-data");
        let model_dir = tmp.join("models");
        std::fs::create_dir_all(&app_data_dir).unwrap();

        AppState {
            db,
            app_data_dir,
            embedding_engine: parking_lot::Mutex::new(EmbeddingEngine::new(&model_dir)),
            detection_engine: parking_lot::Mutex::new(DetectionEngine::new_yolo(&model_dir)),
            safety_engine: parking_lot::Mutex::new(DetectionEngine::new_nudenet(&model_dir)),
            secrets: Box::new(MemoryStore::new()),
            jobs: services::jobs::JobRegistry::default(),
            action_manager: services::undo::ActionManager::new(),
            file_watcher: parking_lot::Mutex::new(watcher::FileWatcher::new()),
            clipboard_monitor: parking_lot::Mutex::new(
                services::clipboard_monitor::ClipboardMonitorState::default(),
            ),
            static_publish_server: parking_lot::Mutex::new(
                crate::commands::static_publishing::StaticPublishServerState::default(),
            ),
            preview_state: crate::preview::state::PreviewStateStore::default(),
            preview_web_stream: crate::preview::web_stream::PreviewWebStreamController::default(),
            agent_snapshots: parking_lot::Mutex::new(
                services::agent_snapshots::AgentSnapshotRegistry::default(),
            ),
            agent_snapshot_requests: parking_lot::Mutex::new(std::collections::HashMap::new()),
        }
    }

    fn insert_test_image(db: &Database, image_id: &str, file_path: &Path) {
        let now = "2026-07-06T00:00:00Z".to_string();
        let file_size = std::fs::metadata(file_path).unwrap().len();
        db.insert_image(&Image {
            id: image_id.to_string(),
            sha256_hash: format!("hash-{image_id}"),
            width: 1,
            height: 1,
            format: "png".to_string(),
            file_size,
            created_at: now.clone(),
            imported_at: now.clone(),
            ai_prompt: None,
            raw_metadata: None,
        })
        .unwrap();
        db.insert_image_file(&ImageFile {
            id: format!("file-{image_id}"),
            image_id: image_id.to_string(),
            path: file_path.to_string_lossy().to_string(),
            last_seen_at: now,
            missing_at: None,
            last_seen_size: Some(file_size),
            last_seen_mtime: None,
        })
        .unwrap();
    }

    #[test]
    fn validate_proposal_rejects_destructive_direct_apply_without_candidates() {
        let err = validate_create_request(&CreateActionProposalRequest {
            kind: "trash_images".to_string(),
            persona: "copilot".to_string(),
            lens: Some("near_duplicates".to_string()),
            criteria: "cleanup".to_string(),
            visual_level: "tiny".to_string(),
            selection_preset_id: None,
            estimated_input_tokens: Some(1),
            estimated_output_tokens: Some(1),
            estimated_cost_eur: Some(0.001),
            source_context_json: "{}".to_string(),
            items_json: "[]".to_string(),
            guard_results_json: "{}".to_string(),
        })
        .unwrap_err();
        assert!(err.to_string().contains("at least one candidate"));
    }

    #[test]
    fn create_proposal_persists_pending_request_with_preset() {
        let db = db();
        let preset = db.list_agent_selection_presets().unwrap()[0].clone();
        let request = CreateActionProposalRequest {
            kind: "select_images".to_string(),
            persona: "copilot".to_string(),
            lens: Some("portfolio".to_string()),
            criteria: "select portfolio candidates".to_string(),
            visual_level: "text".to_string(),
            selection_preset_id: Some(preset.id.clone()),
            estimated_input_tokens: Some(300),
            estimated_output_tokens: Some(100),
            estimated_cost_eur: Some(0.002),
            source_context_json: "{}".to_string(),
            items_json: r#"[{"image_id":"img_a","reason":"strong"}]"#.to_string(),
            guard_results_json: "{}".to_string(),
        };

        let proposal = create_action_proposal_db(&db, request).unwrap();
        assert_eq!(proposal.status, "pending");
        assert_eq!(proposal.kind, "select_images");
        assert_eq!(
            proposal.selection_preset_id.as_deref(),
            Some(preset.id.as_str())
        );
    }

    #[test]
    fn apply_action_proposal_db_records_only_approved_subset() {
        let db = db();
        let proposal = create_action_proposal_db(
            &db,
            CreateActionProposalRequest {
                kind: "select_images".to_string(),
                persona: "copilot".to_string(),
                lens: Some("portfolio".to_string()),
                criteria: "select portfolio candidates".to_string(),
                visual_level: "text".to_string(),
                selection_preset_id: None,
                estimated_input_tokens: Some(300),
                estimated_output_tokens: Some(100),
                estimated_cost_eur: Some(0.002),
                source_context_json: "{}".to_string(),
                items_json: serde_json::json!([
                    {"image_id":"img_a","reason":"strong"},
                    {"image_id":"img_b","reason":"duplicate"},
                    {"image_id":"img_c","reason":"coherent"}
                ])
                .to_string(),
                guard_results_json: "{}".to_string(),
            },
        )
        .unwrap();
        let approved = vec!["img_a".to_string(), "img_c".to_string()];

        let result = apply_action_proposal_db(
            &db,
            &crate::services::undo::ActionManager::new(),
            &proposal.id,
            &approved,
            &serde_json::json!({"selected": 2, "missing": 1}).to_string(),
        )
        .unwrap();

        assert_eq!(result.status, "applied");
        assert_eq!(result.applied_count, 2);
        let applied = db.get_action_proposal(&proposal.id).unwrap().unwrap();
        assert_eq!(applied.status, "applied");
        let undo: serde_json::Value =
            serde_json::from_str(applied.undo_journal_json.as_deref().unwrap()).unwrap();
        assert_eq!(undo["kind"], "select_images");
        assert_eq!(undo["approved_image_ids"], serde_json::json!(approved));
        assert!(!undo["approved_image_ids"]
            .as_array()
            .unwrap()
            .iter()
            .any(|id| id == "img_b"));
    }

    #[test]
    fn apply_action_proposal_db_records_destructive_trash_after_actual_effect() {
        let dir = tempfile::tempdir().unwrap();
        let state = test_state(dir.path());
        let file_path = dir.path().join("proposal-trash.png");
        std::fs::write(&file_path, b"fake image data").unwrap();
        insert_test_image(&state.db, "img_trash", &file_path);
        let proposal = create_action_proposal_db(
            &state.db,
            CreateActionProposalRequest {
                kind: "trash_images".to_string(),
                persona: "copilot".to_string(),
                lens: Some("cleanup".to_string()),
                criteria: "move weak duplicate to Trash".to_string(),
                visual_level: "tiny".to_string(),
                selection_preset_id: None,
                estimated_input_tokens: Some(100),
                estimated_output_tokens: Some(20),
                estimated_cost_eur: Some(0.001),
                source_context_json: "{}".to_string(),
                items_json: serde_json::json!([
                    {"image_id":"img_trash","reason":"duplicate"}
                ])
                .to_string(),
                guard_results_json: "{}".to_string(),
            },
        )
        .unwrap();
        let approved = vec!["img_trash".to_string()];

        let trash_result =
            crate::commands::library::trash_images_detailed_inner(&state, &approved).unwrap();
        let apply_result = apply_action_proposal_db(
            &state.db,
            &state.action_manager,
            &proposal.id,
            &approved,
            &serde_json::to_string(&trash_result).unwrap(),
        )
        .unwrap();

        assert_eq!(trash_result.succeeded, 1);
        assert!(!file_path.exists(), "proposal trash should move the file");
        let image_file = state
            .db
            .get_image_file_by_path(&file_path.to_string_lossy())
            .unwrap()
            .unwrap();
        assert!(image_file.missing_at.is_some());
        assert_eq!(apply_result.status, "applied");

        let applied = state.db.get_action_proposal(&proposal.id).unwrap().unwrap();
        assert_eq!(applied.status, "applied");
        let apply_json: serde_json::Value =
            serde_json::from_str(applied.apply_result_json.as_deref().unwrap()).unwrap();
        assert_eq!(apply_json["succeeded"], 1);
        let undo: serde_json::Value =
            serde_json::from_str(applied.undo_journal_json.as_deref().unwrap()).unwrap();
        assert_eq!(undo["kind"], "trash_images");
        assert_eq!(undo["approved_image_ids"], serde_json::json!(approved));
    }

    // ----- Shortlist proposal integration -----

    fn seed_shortlist_fixture(db: &Database) -> String {
        // Images live in /library/shoot so a Folder scope resolves them.
        let now = "2026-09-01T00:00:00Z".to_string();
        for id in ["img_a", "img_b", "img_c"] {
            db.insert_image(&Image {
                id: id.to_string(),
                sha256_hash: format!("hash-{id}"),
                width: 800,
                height: 600,
                format: "png".to_string(),
                file_size: 1024,
                created_at: now.clone(),
                imported_at: now.clone(),
                ai_prompt: None,
                raw_metadata: None,
            })
            .unwrap();
            db.insert_image_file(&ImageFile {
                id: format!("file-{id}"),
                image_id: id.to_string(),
                path: format!("/library/shoot/{id}.png"),
                last_seen_at: now.clone(),
                missing_at: None,
                last_seen_size: None,
                last_seen_mtime: None,
            })
            .unwrap();
        }
        let scope = crate::db_core::models::SelectionSourceScope::Folder {
            path: "/library/shoot".to_string(),
            min_size: 0,
            include_rejected: false,
        };
        db.create_selection_run("Client final", &scope, None)
            .unwrap()
    }

    fn shortlist_proposal_request(
        kind: &str,
        selection_id: &str,
        items: &[&str],
    ) -> CreateActionProposalRequest {
        CreateActionProposalRequest {
            kind: kind.to_string(),
            persona: "copilot".to_string(),
            lens: Some("portfolio".to_string()),
            criteria: "final picks".to_string(),
            visual_level: "text".to_string(),
            selection_preset_id: None,
            estimated_input_tokens: Some(100),
            estimated_output_tokens: Some(20),
            estimated_cost_eur: Some(0.001),
            source_context_json: serde_json::json!({
                "scope": "selection_mode",
                "selection_id": selection_id
            })
            .to_string(),
            items_json: serde_json::json!(items
                .iter()
                .map(|id| serde_json::json!({"image_id": id, "reason": "strong"}))
                .collect::<Vec<_>>())
            .to_string(),
            guard_results_json: "{}".to_string(),
        }
    }

    fn shortlist_membership(db: &Database, selection_id: &str) -> Vec<String> {
        db.selection_shortlist_ids(selection_id).unwrap()
    }

    fn undo_record_count(db: &Database) -> i64 {
        let conn = db.conn.lock();
        conn.query_row("SELECT COUNT(*) FROM undo_records", [], |row| row.get(0))
            .unwrap()
    }

    #[test]
    fn shortlist_proposal_applies_membership_and_consumes_atomically() {
        let db = db();
        let selection_id = seed_shortlist_fixture(&db);
        let proposal = create_action_proposal_db(
            &db,
            shortlist_proposal_request("shortlist_add", &selection_id, &["img_a", "img_b"]),
        )
        .unwrap();

        let result = apply_action_proposal_db(
            &db,
            &crate::services::undo::ActionManager::new(),
            &proposal.id,
            &["img_a".to_string()],
            &serde_json::json!({"applied": 1}).to_string(),
        )
        .unwrap();

        assert_eq!(result.status, "applied");
        assert_eq!(result.applied_count, 1);
        assert_eq!(shortlist_membership(&db, &selection_id), vec!["img_a"]);
        let applied = db.get_action_proposal(&proposal.id).unwrap().unwrap();
        assert_eq!(applied.status, "applied");
        let undo: serde_json::Value =
            serde_json::from_str(applied.undo_journal_json.as_deref().unwrap()).unwrap();
        assert_eq!(undo["kind"], "shortlist_add");
        assert_eq!(undo["approved_image_ids"], serde_json::json!(["img_a"]));
        // The undo manager holds the matching record for one-step undo.
        assert_eq!(undo_record_count(&db), 1);
        // Agent provenance is recorded.
        let conn = db.conn.lock();
        let (event_type, actor_type): (String, String) = conn
            .query_row(
                "SELECT event_type, actor_type FROM session_events
                 WHERE subject_id = ?1 ORDER BY created_at DESC LIMIT 1",
                rusqlite::params![selection_id],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .unwrap();
        drop(conn);
        assert_eq!(event_type, "shortlist_proposal_applied");
        assert_eq!(actor_type, "agent");
    }

    #[test]
    fn shortlist_proposal_update_failure_rolls_back_membership_and_undo() {
        let db = db();
        let selection_id = seed_shortlist_fixture(&db);
        let proposal = create_action_proposal_db(
            &db,
            shortlist_proposal_request("shortlist_add", &selection_id, &["img_a", "img_b"]),
        )
        .unwrap();

        // Injected failure: the proposal consumption UPDATE aborts after the
        // membership mutation and undo record were staged in the same
        // transaction. Everything must roll back together.
        {
            let conn = db.conn.lock();
            conn.execute(
                "CREATE TRIGGER fail_proposal_update
                 BEFORE UPDATE ON agent_action_proposals
                 BEGIN
                     SELECT RAISE(ABORT, 'injected proposal update failure');
                 END",
                [],
            )
            .unwrap();
        }
        let error = apply_action_proposal_db(
            &db,
            &crate::services::undo::ActionManager::new(),
            &proposal.id,
            &["img_a".to_string()],
            "{}",
        )
        .unwrap_err();
        assert!(error
            .to_string()
            .contains("injected proposal update failure"));
        assert!(
            shortlist_membership(&db, &selection_id).is_empty(),
            "no membership may survive a failed proposal consumption"
        );
        assert_eq!(
            undo_record_count(&db),
            0,
            "no undo record may survive a failed proposal consumption"
        );
        assert_eq!(
            db.get_action_proposal(&proposal.id)
                .unwrap()
                .unwrap()
                .status,
            "pending",
            "the proposal must remain pending and retryable"
        );

        // Removing the trigger lets a retry succeed fully.
        {
            let conn = db.conn.lock();
            conn.execute("DROP TRIGGER fail_proposal_update", [])
                .unwrap();
        }
        apply_action_proposal_db(
            &db,
            &crate::services::undo::ActionManager::new(),
            &proposal.id,
            &["img_a".to_string()],
            "{}",
        )
        .unwrap();
        assert_eq!(shortlist_membership(&db, &selection_id), vec!["img_a"]);
        assert_eq!(undo_record_count(&db), 1);

        // Duplicate consumption is rejected inside the transaction.
        let error = apply_action_proposal_db(
            &db,
            &crate::services::undo::ActionManager::new(),
            &proposal.id,
            &["img_b".to_string()],
            "{}",
        )
        .unwrap_err();
        assert!(error.to_string().contains("not pending"));
        assert_eq!(shortlist_membership(&db, &selection_id), vec!["img_a"]);
    }

    #[test]
    fn shortlist_proposal_rejects_approved_ids_outside_reviewed_items() {
        let db = db();
        let selection_id = seed_shortlist_fixture(&db);
        let proposal = create_action_proposal_db(
            &db,
            shortlist_proposal_request("shortlist_add", &selection_id, &["img_a"]),
        )
        .unwrap();

        // img_c is inside the captured source but was never part of the
        // reviewed proposal.
        let error = apply_action_proposal_db(
            &db,
            &crate::services::undo::ActionManager::new(),
            &proposal.id,
            &["img_c".to_string()],
            "{}",
        )
        .unwrap_err();
        assert!(error
            .to_string()
            .contains("not part of the reviewed proposal"));
        assert!(shortlist_membership(&db, &selection_id).is_empty());
        assert_eq!(
            db.get_action_proposal(&proposal.id)
                .unwrap()
                .unwrap()
                .status,
            "pending"
        );
    }

    #[test]
    fn shortlist_proposal_rejects_wrong_target_at_apply_time() {
        let db = db();
        seed_shortlist_fixture(&db);
        // Created directly through the DB layer to bypass creation-time
        // validation, simulating a stale or forged proposal row.
        let proposal = db
            .create_action_proposal(CreateActionProposalDb {
                kind: "shortlist_add".to_string(),
                persona: "copilot".to_string(),
                lens: None,
                criteria: "final picks".to_string(),
                visual_level: "text".to_string(),
                selection_preset_id: None,
                estimated_input_tokens: None,
                estimated_output_tokens: None,
                estimated_cost_eur: None,
                source_context_json: serde_json::json!({"selection_id": "missing-run"}).to_string(),
                items_json: serde_json::json!([{"image_id": "img_a", "reason": "x"}]).to_string(),
                guard_results_json: "{}".to_string(),
            })
            .unwrap();

        let error = apply_action_proposal_db(
            &db,
            &crate::services::undo::ActionManager::new(),
            &proposal.id,
            &["img_a".to_string()],
            "{}",
        )
        .unwrap_err();
        assert!(error.to_string().contains("missing-run"));
        assert_eq!(undo_record_count(&db), 0);
    }

    #[test]
    fn idempotent_shortlist_proposal_consumes_without_undo_record() {
        let db = db();
        let selection_id = seed_shortlist_fixture(&db);
        // img_a is already shortlisted by the user.
        let manager = crate::services::undo::ActionManager::new();
        crate::services::selection_run::apply_shortlist_change(
            &db,
            &manager,
            &selection_id,
            &["img_a".to_string()],
            crate::services::selection_run::ShortlistDirection::Add,
            crate::services::selection_run::ShortlistActor::User,
        )
        .unwrap();
        assert_eq!(undo_record_count(&db), 1);

        let proposal = create_action_proposal_db(
            &db,
            shortlist_proposal_request("shortlist_add", &selection_id, &["img_a"]),
        )
        .unwrap();
        let result =
            apply_action_proposal_db(&db, &manager, &proposal.id, &["img_a".to_string()], "{}")
                .unwrap();

        assert_eq!(result.status, "applied");
        assert_eq!(result.applied_count, 0, "no-op consumption applies nothing");
        assert_eq!(shortlist_membership(&db, &selection_id), vec!["img_a"]);
        assert_eq!(
            undo_record_count(&db),
            1,
            "an idempotent no-op must not add an undo record"
        );
        assert_eq!(
            db.get_action_proposal(&proposal.id)
                .unwrap()
                .unwrap()
                .status,
            "applied"
        );
    }

    #[test]
    fn shortlist_proposal_reports_actual_changes_for_partial_duplicate_batch() {
        let db = db();
        let selection_id = seed_shortlist_fixture(&db);
        let manager = crate::services::undo::ActionManager::new();
        for (kind, approved, expected) in [
            ("shortlist_add", vec!["img_a"], 1),
            ("shortlist_add", vec!["img_a", "img_b", "img_b"], 1),
            ("shortlist_remove", vec!["img_b", "img_b", "img_c"], 1),
        ] {
            let proposal = create_action_proposal_db(
                &db,
                shortlist_proposal_request(kind, &selection_id, &["img_a", "img_b", "img_c"]),
            )
            .unwrap();
            let approved: Vec<String> = approved.into_iter().map(str::to_string).collect();
            let result =
                apply_action_proposal_db(&db, &manager, &proposal.id, &approved, "{}").unwrap();
            assert_eq!(result.applied_count, expected);
        }
        assert_eq!(shortlist_membership(&db, &selection_id), vec!["img_a"]);
    }

    #[test]
    fn create_shortlist_proposal_validates_target_and_context() {
        let db = db();
        let selection_id = seed_shortlist_fixture(&db);

        // Missing top-level selection_id.
        let missing = create_action_proposal_db(
            &db,
            CreateActionProposalRequest {
                source_context_json: "{}".to_string(),
                ..shortlist_proposal_request("shortlist_add", "ignored", &["img_a"])
            },
        )
        .unwrap_err();
        assert!(missing.to_string().contains("selection_id"));

        // Unknown run target.
        let unknown = create_action_proposal_db(
            &db,
            shortlist_proposal_request("shortlist_add", "no-such-run", &["img_a"]),
        )
        .unwrap_err();
        assert!(unknown.to_string().contains("no-such-run"));

        // Finished run target.
        {
            let conn = db.conn.lock();
            conn.execute(
                "UPDATE selection_runs SET status = 'finished' WHERE id = ?1",
                rusqlite::params![selection_id],
            )
            .unwrap();
        }
        let finished = create_action_proposal_db(
            &db,
            shortlist_proposal_request("shortlist_add", &selection_id, &["img_a"]),
        )
        .unwrap_err();
        assert!(finished.to_string().contains("require an active run"));
    }

    #[test]
    fn upsert_selection_preset_validates_json() {
        let db = db();
        let err = upsert_agent_selection_preset_db(
            &db,
            UpsertAgentSelectionPresetRequest {
                id: None,
                name: "Bad".to_string(),
                purpose: "test".to_string(),
                prompt: "Select".to_string(),
                criteria_json: "{bad".to_string(),
                sort_order: None,
            },
        )
        .unwrap_err();
        assert!(err.to_string().contains("Invalid criteria_json"));
    }
}
