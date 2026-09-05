// Copyright (c) 2026-present Gleb Kalinin. Architecture and design by author.
// Implementation assisted by Claude (Anthropic). See AUTHORSHIP.md.

use crate::db_core::db::Database;
use crate::db_core::models::*;
use rusqlite::{params, OptionalExtension, Result};

impl Database {
    /// Persist a job snapshot. On conflict, a stored terminal row
    /// ('completed'/'failed'/'cancelled') is never overwritten by a
    /// non-terminal snapshot: lifecycle writes are cloned outside the registry
    /// lock, so concurrent transitions can reach the database out of order and
    /// a stale 'running' write must not resurrect a finished job. Terminal
    /// snapshots may overwrite other terminal snapshots. The JobRegistry
    /// serializes writes and rereads its current state to preserve ordering;
    /// this guard additionally prevents resurrecting terminal rows.
    /// Timestamps are deliberately not compared: RFC3339 strings vary in
    /// fractional-digit precision and are not reliably ordered, while the
    /// state-based guard is deterministic and treats equal timestamps safely.
    /// A rejected stale write is a no-op, not an error.
    pub fn save_job(&self, snapshot: &crate::services::jobs::JobSnapshot) -> Result<()> {
        let conn = self.conn.lock();
        conn.execute(
            "INSERT INTO mcp_jobs (job_id, kind, status, current, total, message, error, created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)
             ON CONFLICT(job_id) DO UPDATE SET
                kind = excluded.kind,
                status = excluded.status,
                current = excluded.current,
                total = excluded.total,
                message = excluded.message,
                error = excluded.error,
                updated_at = excluded.updated_at
             WHERE mcp_jobs.status NOT IN ('completed', 'failed', 'cancelled')
                OR excluded.status IN ('completed', 'failed', 'cancelled')",
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
        // A paused or cancelling job cannot outlive the app either: it is
        // interrupted just like a running job and must be reported as failed
        // on the next start rather than lingering in a non-terminal state.
        let updated = conn.execute(
            "UPDATE mcp_jobs SET status = 'failed', error = 'App stopped before job completed', updated_at = ?1
             WHERE status IN ('running', 'cancelling', 'paused')",
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

    pub fn fail_running_model_runs_for_job(&self, job_id: &str, error: &str) -> Result<u32> {
        let now = chrono::Utc::now().to_rfc3339();
        let conn = self.conn.lock();
        let updated = conn.execute(
            "UPDATE model_runs
             SET status = 'failed', error = ?2, completed_at = ?3
             WHERE job_id = ?1 AND status = 'running'",
            params![job_id, error, now],
        )?;
        Ok(updated as u32)
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    fn model_run(id: &str, status: &str) -> NewModelRun {
        let now = "2026-08-08T00:00:00Z".to_string();
        NewModelRun {
            id: id.to_string(),
            job_id: Some("job-embedding".to_string()),
            parent_run_id: None,
            profile_id: None,
            task: "embedding".to_string(),
            provider: "local".to_string(),
            model_id: "clip-vit-b32".to_string(),
            model_revision: None,
            status: status.to_string(),
            input_scope_json: "{}".to_string(),
            params_json: "{}".to_string(),
            output_summary_json: if status == "completed" {
                "{\"generated\":1}".to_string()
            } else {
                "{}".to_string()
            },
            cost_estimate_usd: None,
            cost_actual_usd: None,
            error: None,
            created_at: now.clone(),
            started_at: Some(now.clone()),
            completed_at: (status == "completed").then_some(now),
        }
    }

    #[test]
    fn failing_running_model_runs_for_job_preserves_terminal_runs() {
        let db = Database::open(Path::new(":memory:")).unwrap();
        db.insert_model_run(&model_run("run-running", "running"))
            .unwrap();
        db.insert_model_run(&model_run("run-completed", "completed"))
            .unwrap();

        let updated = db
            .fail_running_model_runs_for_job("job-embedding", "Embedding generation panicked")
            .unwrap();

        assert_eq!(updated, 1);
        let failed = db.get_model_run("run-running").unwrap().unwrap();
        assert_eq!(failed.status, "failed");
        assert_eq!(
            failed.error.as_deref(),
            Some("Embedding generation panicked")
        );
        assert!(failed.completed_at.is_some());

        let completed = db.get_model_run("run-completed").unwrap().unwrap();
        assert_eq!(completed.status, "completed");
        assert_eq!(completed.output_summary_json, "{\"generated\":1}");
        assert_eq!(completed.error, None);
        assert_eq!(
            completed.completed_at.as_deref(),
            Some("2026-08-08T00:00:00Z")
        );
    }

    #[test]
    fn mark_stale_running_jobs_failed_covers_paused_jobs() {
        let db = Database::open(Path::new(":memory:")).unwrap();
        {
            let conn = db.conn.lock();
            conn.execute(
                "INSERT INTO mcp_jobs (job_id, kind, status, current, total, created_at, updated_at)
                 VALUES ('job_paused_stale', 'import', 'paused', 2, 10, '2026-01-01T00:00:00Z', '2026-01-01T00:00:00Z')",
                [],
            )
            .unwrap();
        }

        let updated = db.mark_stale_running_jobs_failed().unwrap();

        assert_eq!(updated, 1);
        let snapshot = db.load_terminal_jobs().unwrap();
        assert_eq!(snapshot.len(), 1);
        assert_eq!(snapshot[0].job_id, "job_paused_stale");
        assert_eq!(snapshot[0].status, "failed");
        assert_eq!(
            snapshot[0].error.as_deref(),
            Some("App stopped before job completed")
        );
    }

    fn job_snapshot(status: &str, updated_at: &str) -> crate::services::jobs::JobSnapshot {
        crate::services::jobs::JobSnapshot {
            job_id: "job_ordering".to_string(),
            kind: "import".to_string(),
            status: status.to_string(),
            current: 3,
            total: 10,
            message: None,
            error: None,
            created_at: "2026-01-01T00:00:00Z".to_string(),
            updated_at: updated_at.to_string(),
        }
    }

    #[test]
    fn save_job_keeps_terminal_row_when_a_stale_running_snapshot_arrives() {
        let db = Database::open(Path::new(":memory:")).unwrap();

        // The terminal snapshot is written first (the worker finished), then a
        // stale 'running' transition from a racing cancel/pause write arrives
        // late — with the exact same timestamp, so the guard cannot depend on
        // timestamp ordering.
        db.save_job(&job_snapshot("completed", "2026-01-01T00:00:05Z"))
            .unwrap();
        db.save_job(&job_snapshot("running", "2026-01-01T00:00:05Z"))
            .unwrap();

        let loaded = db.load_terminal_jobs().unwrap();
        assert_eq!(loaded.len(), 1);
        assert_eq!(loaded[0].job_id, "job_ordering");
        assert_eq!(loaded[0].status, "completed");

        // A newer terminal snapshot still lands (the registry's final state).
        db.save_job(&job_snapshot("failed", "2026-01-01T00:00:06Z"))
            .unwrap();
        let loaded = db.load_terminal_jobs().unwrap();
        assert_eq!(loaded[0].status, "failed");
    }

    #[test]
    fn save_job_keeps_terminal_row_over_a_stale_cancelling_snapshot() {
        let db = Database::open(Path::new(":memory:")).unwrap();

        // mark_cancelled persisted 'cancelled', then the racing cancel
        // transition's 'cancelling' write arrives late.
        db.save_job(&job_snapshot("cancelled", "2026-01-01T00:00:05Z"))
            .unwrap();
        db.save_job(&job_snapshot("cancelling", "2026-01-01T00:00:05Z"))
            .unwrap();

        let loaded = db.load_terminal_jobs().unwrap();
        assert_eq!(loaded.len(), 1);
        assert_eq!(loaded[0].status, "cancelled");
    }

    #[test]
    fn save_job_lets_non_terminal_lifecycle_transitions_land_in_order() {
        let db = Database::open(Path::new(":memory:")).unwrap();

        let statuses = [
            ("running", "2026-01-01T00:00:01Z"),
            ("paused", "2026-01-01T00:00:02Z"),
            ("running", "2026-01-01T00:00:03Z"),
            ("cancelling", "2026-01-01T00:00:04Z"),
        ];
        for (status, updated_at) in statuses {
            db.save_job(&job_snapshot(status, updated_at)).unwrap();
            let stored: String = {
                let conn = db.conn.lock();
                conn.query_row(
                    "SELECT status FROM mcp_jobs WHERE job_id = 'job_ordering'",
                    [],
                    |row| row.get(0),
                )
                .unwrap()
            };
            assert_eq!(stored, status);
        }
    }
}
