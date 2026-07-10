// Copyright (c) 2026-present Gleb Kalinin. Architecture and design by author.
// Implementation assisted by Claude (Anthropic). See AUTHORSHIP.md.

use crate::db_core::db::Database;
use crate::db_core::models::*;
use rusqlite::{params, OptionalExtension, Result};

impl Database {
    pub fn save_job(&self, snapshot: &crate::services::jobs::JobSnapshot) -> Result<()> {
        let conn = self.conn.lock();
        conn.execute(
            "INSERT OR REPLACE INTO mcp_jobs (job_id, kind, status, current, total, message, error, created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
            params![
                snapshot.job_id, snapshot.kind, snapshot.status,
                snapshot.current, snapshot.total, snapshot.message, snapshot.error,
                snapshot.created_at, snapshot.updated_at
            ],
        )?;
        Ok(())
    }

    pub fn load_terminal_jobs(&self) -> Result<Vec<crate::services::jobs::JobSnapshot>> {
        let conn = self.conn.lock();
        let mut stmt = conn.prepare(
            "SELECT job_id, kind, status, current, total, message, error, created_at, updated_at
             FROM mcp_jobs WHERE status IN ('completed', 'failed', 'cancelled')
             ORDER BY updated_at DESC LIMIT 100",
        )?;
        let jobs = stmt
            .query_map([], |row| {
                Ok(crate::services::jobs::JobSnapshot {
                    job_id: row.get(0)?,
                    kind: row.get(1)?,
                    status: row.get(2)?,
                    current: row.get(3)?,
                    total: row.get(4)?,
                    message: row.get(5)?,
                    error: row.get(6)?,
                    created_at: row.get(7)?,
                    updated_at: row.get(8)?,
                })
            })?
            .collect::<Result<Vec<_>>>()?;
        Ok(jobs)
    }

    pub fn prune_old_jobs(&self, max_age_hours: i64) -> Result<u32> {
        let cutoff = (chrono::Utc::now() - chrono::Duration::hours(max_age_hours)).to_rfc3339();
        let conn = self.conn.lock();
        let deleted = conn.execute(
            "DELETE FROM mcp_jobs WHERE updated_at < ?1",
            params![cutoff],
        )?;
        Ok(deleted as u32)
    }

    pub fn mark_stale_running_jobs_failed(&self) -> Result<u32> {
        let now = chrono::Utc::now().to_rfc3339();
        let conn = self.conn.lock();
        let updated = conn.execute(
            "UPDATE mcp_jobs SET status = 'failed', error = 'App stopped before job completed', updated_at = ?1
             WHERE status IN ('running', 'cancelling')",
            params![now],
        )?;
        Ok(updated as u32)
    }

    pub fn insert_model_run(&self, run: &NewModelRun) -> Result<()> {
        let conn = self.conn.lock();
        conn.execute(
            "INSERT INTO model_runs (
                id, job_id, parent_run_id, profile_id, task, provider, model_id,
                model_revision, status, input_scope_json, params_json, output_summary_json,
                cost_estimate_usd, cost_actual_usd, error, created_at, started_at, completed_at
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17, ?18)",
            params![
                run.id,
                run.job_id,
                run.parent_run_id,
                run.profile_id,
                run.task,
                run.provider,
                run.model_id,
                run.model_revision,
                run.status,
                run.input_scope_json,
                run.params_json,
                run.output_summary_json,
                run.cost_estimate_usd,
                run.cost_actual_usd,
                run.error,
                run.created_at,
                run.started_at,
                run.completed_at,
            ],
        )?;
        Ok(())
    }

    pub fn update_model_run_terminal(
        &self,
        run_id: &str,
        status: &str,
        output_summary_json: &str,
        error: Option<&str>,
    ) -> Result<()> {
        let now = chrono::Utc::now().to_rfc3339();
        let conn = self.conn.lock();
        conn.execute(
            "UPDATE model_runs
             SET status = ?2, output_summary_json = ?3, error = ?4, completed_at = ?5
             WHERE id = ?1",
            params![run_id, status, output_summary_json, error, now],
        )?;
        Ok(())
    }

    pub fn insert_model_run_item(&self, item: &NewModelRunItem) -> Result<()> {
        let conn = self.conn.lock();
        conn.execute(
            "INSERT INTO model_run_items (
                id, run_id, image_id, input_asset_uri, input_hash, status,
                output_ref_kind, output_ref_id, audit_payload_json, cost_usd,
                attempt_count, error, started_at, completed_at
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14)",
            params![
                item.id,
                item.run_id,
                item.image_id,
                item.input_asset_uri,
                item.input_hash,
                item.status,
                item.output_ref_kind,
                item.output_ref_id,
                item.audit_payload_json,
                item.cost_usd,
                item.attempt_count,
                item.error,
                item.started_at,
                item.completed_at,
            ],
        )?;
        Ok(())
    }

    pub fn get_model_run(&self, run_id: &str) -> Result<Option<ModelRun>> {
        let conn = self.conn.lock();
        conn.query_row(
            "SELECT id, job_id, parent_run_id, profile_id, task, provider, model_id,
                    model_revision, status, input_scope_json, params_json, output_summary_json,
                    cost_estimate_usd, cost_actual_usd, error, created_at, started_at, completed_at
             FROM model_runs WHERE id = ?1",
            params![run_id],
            |row| {
                Ok(ModelRun {
                    id: row.get(0)?,
                    job_id: row.get(1)?,
                    parent_run_id: row.get(2)?,
                    profile_id: row.get(3)?,
                    task: row.get(4)?,
                    provider: row.get(5)?,
                    model_id: row.get(6)?,
                    model_revision: row.get(7)?,
                    status: row.get(8)?,
                    input_scope_json: row.get(9)?,
                    params_json: row.get(10)?,
                    output_summary_json: row.get(11)?,
                    cost_estimate_usd: row.get(12)?,
                    cost_actual_usd: row.get(13)?,
                    error: row.get(14)?,
                    created_at: row.get(15)?,
                    started_at: row.get(16)?,
                    completed_at: row.get(17)?,
                })
            },
        )
        .optional()
    }
}
