use crate::db_core::db::Database;
use crate::db_core::models::{
    AgentActionProposal, AgentSelectionPreset, CreateActionProposalDb, UpsertAgentSelectionPresetDb,
};
use crate::services::ServiceError;
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
