use super::db::Database;
use super::models::{
    AgentActionProposal, AgentSelectionPreset, CreateActionProposalDb, UpsertAgentSelectionPresetDb,
};
use rusqlite::{params, OptionalExtension, Result};

fn map_agent_action_proposal(row: &rusqlite::Row<'_>) -> Result<AgentActionProposal> {
    Ok(AgentActionProposal {
        id: row.get(0)?,
        kind: row.get(1)?,
        status: row.get(2)?,
        persona: row.get(3)?,
        lens: row.get(4)?,
        criteria: row.get(5)?,
        visual_level: row.get(6)?,
        selection_preset_id: row.get(7)?,
        estimated_input_tokens: row.get(8)?,
        estimated_output_tokens: row.get(9)?,
        estimated_cost_eur: row.get(10)?,
        source_context_json: row.get(11)?,
        items_json: row.get(12)?,
        guard_results_json: row.get(13)?,
        apply_result_json: row.get(14)?,
        undo_journal_json: row.get(15)?,
        created_at: row.get(16)?,
        updated_at: row.get(17)?,
        applied_at: row.get(18)?,
    })
}

fn map_agent_selection_preset(row: &rusqlite::Row<'_>) -> Result<AgentSelectionPreset> {
    Ok(AgentSelectionPreset {
        id: row.get(0)?,
        name: row.get(1)?,
        purpose: row.get(2)?,
        prompt: row.get(3)?,
        criteria_json: row.get(4)?,
        sort_order: row.get(5)?,
        created_at: row.get(6)?,
        updated_at: row.get(7)?,
    })
}

impl Database {
    pub fn create_action_proposal(
        &self,
        request: CreateActionProposalDb,
    ) -> Result<AgentActionProposal> {
        let id = format!("proposal_{}", uuid::Uuid::new_v4().simple());
        let conn = self.conn.lock();
        conn.execute(
            "INSERT INTO agent_action_proposals (
                id, kind, status, persona, lens, criteria, visual_level, selection_preset_id,
                estimated_input_tokens, estimated_output_tokens, estimated_cost_eur,
                source_context_json, items_json, guard_results_json,
                created_at, updated_at
             ) VALUES (?1, ?2, 'pending', ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, datetime('now'), datetime('now'))",
            params![
                id,
                request.kind,
                request.persona,
                request.lens,
                request.criteria,
                request.visual_level,
                request.selection_preset_id,
                request.estimated_input_tokens,
                request.estimated_output_tokens,
                request.estimated_cost_eur,
                request.source_context_json,
                request.items_json,
                request.guard_results_json,
            ],
        )?;
        drop(conn);
        self.get_action_proposal(&id)
            .map(|proposal| proposal.expect("created action proposal should be readable"))
    }

    pub fn get_action_proposal(&self, id: &str) -> Result<Option<AgentActionProposal>> {
        let conn = self.conn.lock();
        conn.query_row(
            "SELECT id, kind, status, persona, lens, criteria, visual_level, selection_preset_id,
                    estimated_input_tokens, estimated_output_tokens, estimated_cost_eur,
                    source_context_json, items_json, guard_results_json,
                    apply_result_json, undo_journal_json, created_at, updated_at, applied_at
             FROM agent_action_proposals WHERE id = ?1",
            params![id],
            map_agent_action_proposal,
        )
        .optional()
    }

    pub fn list_action_proposals(
        &self,
        status: Option<&str>,
        limit: u32,
    ) -> Result<Vec<AgentActionProposal>> {
        let limit = limit.clamp(1, 100);
        let conn = self.conn.lock();
        if let Some(status) = status {
            let mut stmt = conn.prepare(
                "SELECT id, kind, status, persona, lens, criteria, visual_level, selection_preset_id,
                        estimated_input_tokens, estimated_output_tokens, estimated_cost_eur,
                        source_context_json, items_json, guard_results_json,
                        apply_result_json, undo_journal_json, created_at, updated_at, applied_at
                 FROM agent_action_proposals
                 WHERE status = ?1
                 ORDER BY datetime(created_at) DESC
                 LIMIT ?2",
            )?;
            let rows = stmt.query_map(params![status, limit], map_agent_action_proposal)?;
            rows.collect()
        } else {
            let mut stmt = conn.prepare(
                "SELECT id, kind, status, persona, lens, criteria, visual_level, selection_preset_id,
                        estimated_input_tokens, estimated_output_tokens, estimated_cost_eur,
                        source_context_json, items_json, guard_results_json,
                        apply_result_json, undo_journal_json, created_at, updated_at, applied_at
                 FROM agent_action_proposals
                 ORDER BY datetime(created_at) DESC
                 LIMIT ?1",
            )?;
            let rows = stmt.query_map(params![limit], map_agent_action_proposal)?;
            rows.collect()
        }
    }

    pub fn mark_action_proposal_applied(
        &self,
        id: &str,
        apply_result_json: &str,
        undo_journal_json: &str,
    ) -> Result<bool> {
        let conn = self.conn.lock();
        let changed = conn.execute(
            "UPDATE agent_action_proposals
             SET status = 'applied',
                 apply_result_json = ?2,
                 undo_journal_json = ?3,
                 applied_at = datetime('now'),
                 updated_at = datetime('now')
             WHERE id = ?1 AND status = 'pending'",
            params![id, apply_result_json, undo_journal_json],
        )?;
        Ok(changed > 0)
    }

    pub fn dismiss_action_proposal(&self, id: &str) -> Result<bool> {
        let conn = self.conn.lock();
        let changed = conn.execute(
            "UPDATE agent_action_proposals
             SET status = 'dismissed', updated_at = datetime('now')
             WHERE id = ?1 AND status = 'pending'",
            params![id],
        )?;
        Ok(changed > 0)
    }

    pub fn upsert_agent_selection_preset(
        &self,
        request: UpsertAgentSelectionPresetDb,
    ) -> Result<AgentSelectionPreset> {
        let id = request
            .id
            .filter(|value| !value.trim().is_empty())
            .unwrap_or_else(|| format!("selpreset_{}", uuid::Uuid::new_v4().simple()));
        let sort_order = request.sort_order.unwrap_or(100);
        let conn = self.conn.lock();
        conn.execute(
            "INSERT INTO agent_selection_presets (
                id, name, purpose, prompt, criteria_json, sort_order, created_at, updated_at
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, datetime('now'), datetime('now'))
             ON CONFLICT(id) DO UPDATE SET
                name = excluded.name,
                purpose = excluded.purpose,
                prompt = excluded.prompt,
                criteria_json = excluded.criteria_json,
                sort_order = excluded.sort_order,
                updated_at = datetime('now')",
            params![
                id,
                request.name,
                request.purpose,
                request.prompt,
                request.criteria_json,
                sort_order,
            ],
        )?;
        drop(conn);
        self.get_agent_selection_preset(&id)
            .map(|preset| preset.expect("upserted selection preset should be readable"))
    }

    pub fn get_agent_selection_preset(&self, id: &str) -> Result<Option<AgentSelectionPreset>> {
        let conn = self.conn.lock();
        conn.query_row(
            "SELECT id, name, purpose, prompt, criteria_json, sort_order, created_at, updated_at
             FROM agent_selection_presets WHERE id = ?1",
            params![id],
            map_agent_selection_preset,
        )
        .optional()
    }

    pub fn list_agent_selection_presets(&self) -> Result<Vec<AgentSelectionPreset>> {
        let conn = self.conn.lock();
        let mut stmt = conn.prepare(
            "SELECT id, name, purpose, prompt, criteria_json, sort_order, created_at, updated_at
             FROM agent_selection_presets
             ORDER BY sort_order ASC, name COLLATE NOCASE ASC",
        )?;
        let rows = stmt.query_map([], map_agent_selection_preset)?;
        rows.collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_db() -> Database {
        let dir = tempfile::tempdir().unwrap();
        Database::open(&dir.path().join("test.db")).unwrap()
    }

    fn sample_request() -> CreateActionProposalDb {
        CreateActionProposalDb {
            kind: "trash_images".to_string(),
            persona: "copilot".to_string(),
            lens: Some("near_duplicates".to_string()),
            criteria: "near duplicate, lower focus, unrated".to_string(),
            visual_level: "tiny".to_string(),
            selection_preset_id: Some("selpreset_cleanup".to_string()),
            estimated_input_tokens: Some(2100),
            estimated_output_tokens: Some(420),
            estimated_cost_eur: Some(0.014),
            source_context_json: serde_json::json!({"scope":"grid","image_count":24}).to_string(),
            items_json: serde_json::json!([
                {"image_id":"img_a","reason":"lower focus","confidence":0.91}
            ])
            .to_string(),
            guard_results_json: serde_json::json!({"blocked":[]}).to_string(),
        }
    }

    #[test]
    fn create_get_and_list_action_proposal_round_trips() {
        let db = test_db();
        let created = db.create_action_proposal(sample_request()).unwrap();
        assert_eq!(created.kind, "trash_images");
        assert_eq!(created.status, "pending");
        assert_eq!(created.visual_level, "tiny");
        assert_eq!(
            created.selection_preset_id.as_deref(),
            Some("selpreset_cleanup")
        );
        assert_eq!(created.estimated_input_tokens, Some(2100));

        let loaded = db.get_action_proposal(&created.id).unwrap().unwrap();
        assert_eq!(loaded.id, created.id);
        assert_eq!(loaded.items_json, created.items_json);

        let pending = db.list_action_proposals(Some("pending"), 10).unwrap();
        assert_eq!(pending.len(), 1);
        assert_eq!(pending[0].id, created.id);
    }

    #[test]
    fn dismiss_and_mark_applied_update_statuses() {
        let db = test_db();
        let first = db.create_action_proposal(sample_request()).unwrap();
        assert!(db.dismiss_action_proposal(&first.id).unwrap());
        assert_eq!(
            db.get_action_proposal(&first.id).unwrap().unwrap().status,
            "dismissed"
        );

        let second = db.create_action_proposal(sample_request()).unwrap();
        assert!(db
            .mark_action_proposal_applied(
                &second.id,
                r#"{"applied":1,"failed":0}"#,
                r#"{"steps":[{"kind":"trash","image_id":"img_a"}]}"#,
            )
            .unwrap());
        let applied = db.get_action_proposal(&second.id).unwrap().unwrap();
        assert_eq!(applied.status, "applied");
        assert_eq!(
            applied.apply_result_json.as_deref(),
            Some(r#"{"applied":1,"failed":0}"#)
        );
        assert_eq!(
            applied.undo_journal_json.as_deref(),
            Some(r#"{"steps":[{"kind":"trash","image_id":"img_a"}]}"#)
        );
        assert!(applied.applied_at.is_some());
    }

    #[test]
    fn upsert_and_list_selection_presets_round_trips() {
        let db = test_db();
        let preset = db
            .upsert_agent_selection_preset(UpsertAgentSelectionPresetDb {
                id: Some("selpreset_portfolio".to_string()),
                name: "Portfolio picks".to_string(),
                purpose: "portfolio".to_string(),
                prompt: "Select images suitable for a portfolio edit.".to_string(),
                criteria_json: serde_json::json!({"prefer":["coherence","print_quality"]})
                    .to_string(),
                sort_order: Some(10),
            })
            .unwrap();
        assert_eq!(preset.id, "selpreset_portfolio");
        assert_eq!(preset.purpose, "portfolio");

        let updated = db
            .upsert_agent_selection_preset(UpsertAgentSelectionPresetDb {
                id: Some("selpreset_portfolio".to_string()),
                name: "Portfolio final".to_string(),
                purpose: "portfolio".to_string(),
                prompt: "Select only final portfolio candidates.".to_string(),
                criteria_json: "{}".to_string(),
                sort_order: Some(5),
            })
            .unwrap();
        assert_eq!(updated.name, "Portfolio final");

        let presets = db.list_agent_selection_presets().unwrap();
        let updated = presets
            .iter()
            .find(|preset| preset.id == "selpreset_portfolio")
            .unwrap();
        assert_eq!(updated.name, "Portfolio final");
    }
}
