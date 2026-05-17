use serde::Serialize;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use tokio_util::sync::CancellationToken;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize)]
pub struct JobSnapshot {
    pub job_id: String,
    pub kind: String,
    pub status: String,
    pub current: u32,
    pub total: u32,
    pub message: Option<String>,
    pub error: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

pub struct JobState {
    pub snapshot: JobSnapshot,
    pub cancel: CancellationToken,
}

#[derive(Clone)]
pub struct JobRegistry {
    jobs: Arc<Mutex<HashMap<String, JobState>>>,
}

impl Default for JobRegistry {
    fn default() -> Self {
        Self {
            jobs: Arc::new(Mutex::new(HashMap::new())),
        }
    }
}

impl JobRegistry {
    pub fn create_job(&self, kind: &str, total: u32) -> (String, CancellationToken) {
        let raw = Uuid::new_v4().to_string().replace('-', "");
        let job_id = format!("job_{}", &raw[..12]);
        let cancel = CancellationToken::new();
        let now = chrono::Utc::now().to_rfc3339();
        let snapshot = JobSnapshot {
            job_id: job_id.clone(),
            kind: kind.to_string(),
            status: "running".to_string(),
            current: 0,
            total,
            message: None,
            error: None,
            created_at: now.clone(),
            updated_at: now,
        };
        let state = JobState {
            snapshot,
            cancel: cancel.clone(),
        };
        let mut jobs = self.jobs.lock().unwrap();
        self.prune_locked(&mut jobs);
        jobs.insert(job_id.clone(), state);
        (job_id, cancel)
    }

    pub fn update_progress(&self, job_id: &str, current: u32, message: Option<&str>) {
        let mut jobs = self.jobs.lock().unwrap();
        if let Some(state) = jobs.get_mut(job_id) {
            state.snapshot.current = current;
            if let Some(msg) = message {
                state.snapshot.message = Some(msg.to_string());
            }
            state.snapshot.updated_at = chrono::Utc::now().to_rfc3339();
        }
    }

    pub fn complete(&self, job_id: &str) {
        let mut jobs = self.jobs.lock().unwrap();
        if let Some(state) = jobs.get_mut(job_id) {
            if state.cancel.is_cancelled() {
                state.snapshot.status = "cancelled".to_string();
            } else {
                state.snapshot.status = "completed".to_string();
                state.snapshot.current = state.snapshot.total;
            }
            state.snapshot.updated_at = chrono::Utc::now().to_rfc3339();
        }
    }

    pub fn fail(&self, job_id: &str, error: &str) {
        let mut jobs = self.jobs.lock().unwrap();
        if let Some(state) = jobs.get_mut(job_id) {
            if state.cancel.is_cancelled() {
                state.snapshot.status = "cancelled".to_string();
            } else {
                state.snapshot.status = "failed".to_string();
                state.snapshot.error = Some(error.to_string());
            }
            state.snapshot.updated_at = chrono::Utc::now().to_rfc3339();
        }
    }

    pub fn cancel(&self, job_id: &str) -> Result<(), String> {
        let mut jobs = self.jobs.lock().unwrap();
        let state = jobs
            .get_mut(job_id)
            .ok_or_else(|| format!("Job '{}' not found", job_id))?;
        if state.snapshot.status != "running" {
            return Err(format!(
                "Job '{}' is not running (status: {})",
                job_id, state.snapshot.status
            ));
        }
        state.snapshot.status = "cancelling".to_string();
        state.snapshot.updated_at = chrono::Utc::now().to_rfc3339();
        state.cancel.cancel();
        Ok(())
    }

    pub fn mark_cancelled(&self, job_id: &str) {
        let mut jobs = self.jobs.lock().unwrap();
        if let Some(state) = jobs.get_mut(job_id) {
            state.snapshot.status = "cancelled".to_string();
            state.snapshot.updated_at = chrono::Utc::now().to_rfc3339();
        }
    }

    pub fn get(&self, job_id: &str) -> Option<JobSnapshot> {
        let jobs = self.jobs.lock().unwrap();
        jobs.get(job_id).map(|s| s.snapshot.clone())
    }

    pub fn list(&self) -> Vec<JobSnapshot> {
        let mut jobs = self.jobs.lock().unwrap();
        self.prune_locked(&mut jobs);
        jobs.values().map(|s| s.snapshot.clone()).collect()
    }

    pub fn is_cancelled(&self, job_id: &str) -> bool {
        let jobs = self.jobs.lock().unwrap();
        jobs.get(job_id)
            .map(|s| s.cancel.is_cancelled())
            .unwrap_or(true)
    }

    pub fn persist_terminal(&self, job_id: &str, db: &crate::db_core::db::Database) {
        let jobs = self.jobs.lock().unwrap();
        if let Some(state) = jobs.get(job_id) {
            let status = &state.snapshot.status;
            if matches!(status.as_str(), "completed" | "failed" | "cancelled") {
                let _ = db.save_job(&state.snapshot);
            }
        }
    }

    pub fn load_from_db(&self, db: &crate::db_core::db::Database) {
        if let Ok(stale) = db.mark_stale_running_jobs_failed() {
            if stale > 0 {
                crate::safe_eprintln!("Marked {} stale jobs as failed", stale);
            }
        }
        let _ = db.prune_old_jobs(1);
        if let Ok(snapshots) = db.load_terminal_jobs() {
            let mut jobs = self.jobs.lock().unwrap();
            for snapshot in snapshots {
                let cancel = CancellationToken::new();
                cancel.cancel();
                jobs.entry(snapshot.job_id.clone())
                    .or_insert(JobState { snapshot, cancel });
            }
        }
    }

    fn prune_locked(&self, jobs: &mut HashMap<String, JobState>) {
        let now = chrono::Utc::now();
        let is_terminal = |s: &str| matches!(s, "completed" | "failed" | "cancelled");

        jobs.retain(|_, state| {
            if !is_terminal(&state.snapshot.status) {
                return true;
            }
            if let Ok(t) = chrono::DateTime::parse_from_rfc3339(&state.snapshot.updated_at) {
                let t_utc: chrono::DateTime<chrono::Utc> = t.into();
                (now - t_utc).num_hours() < 1
            } else {
                true
            }
        });

        let terminal_count = jobs
            .values()
            .filter(|s| is_terminal(&s.snapshot.status))
            .count();
        if terminal_count > 100 {
            let mut terminal_ids: Vec<(String, String)> = jobs
                .iter()
                .filter(|(_, s)| is_terminal(&s.snapshot.status))
                .map(|(id, s)| (id.clone(), s.snapshot.updated_at.clone()))
                .collect();
            terminal_ids.sort_by(|a, b| b.1.cmp(&a.1));
            let to_remove: Vec<String> = terminal_ids
                .into_iter()
                .skip(100)
                .map(|(id, _)| id)
                .collect();
            for id in to_remove {
                jobs.remove(&id);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_create_job() {
        let registry = JobRegistry::default();
        let (job_id, _token) = registry.create_job("import", 50);
        assert!(job_id.starts_with("job_"));
        assert_eq!(job_id.len(), 16);

        let snapshot = registry.get(&job_id).unwrap();
        assert_eq!(snapshot.kind, "import");
        assert_eq!(snapshot.status, "running");
        assert_eq!(snapshot.current, 0);
        assert_eq!(snapshot.total, 50);
        assert!(snapshot.error.is_none());
        assert!(snapshot.message.is_none());
    }

    #[test]
    fn test_update_progress() {
        let registry = JobRegistry::default();
        let (job_id, _token) = registry.create_job("embeddings", 100);

        registry.update_progress(&job_id, 42, Some("Processing image_abc"));
        let snapshot = registry.get(&job_id).unwrap();
        assert_eq!(snapshot.current, 42);
        assert_eq!(snapshot.message.as_deref(), Some("Processing image_abc"));
    }

    #[test]
    fn test_complete_job() {
        let registry = JobRegistry::default();
        let (job_id, _token) = registry.create_job("detection", 10);

        registry.update_progress(&job_id, 7, None);
        registry.complete(&job_id);

        let snapshot = registry.get(&job_id).unwrap();
        assert_eq!(snapshot.status, "completed");
        assert_eq!(snapshot.current, 10);
    }

    #[test]
    fn test_fail_job() {
        let registry = JobRegistry::default();
        let (job_id, _token) = registry.create_job("vision", 5);

        registry.fail(&job_id, "Ollama connection refused");

        let snapshot = registry.get(&job_id).unwrap();
        assert_eq!(snapshot.status, "failed");
        assert_eq!(snapshot.error.as_deref(), Some("Ollama connection refused"));
    }

    #[test]
    fn test_cancel_job() {
        let registry = JobRegistry::default();
        let (job_id, token) = registry.create_job("import", 200);

        assert!(!token.is_cancelled());
        registry.cancel(&job_id).unwrap();

        let snapshot = registry.get(&job_id).unwrap();
        assert_eq!(snapshot.status, "cancelling");
        assert!(token.is_cancelled());
    }

    #[test]
    fn test_mark_cancelled() {
        let registry = JobRegistry::default();
        let (job_id, _token) = registry.create_job("import", 200);

        registry.cancel(&job_id).unwrap();
        registry.mark_cancelled(&job_id);

        let snapshot = registry.get(&job_id).unwrap();
        assert_eq!(snapshot.status, "cancelled");
    }

    #[test]
    fn test_cancel_non_running() {
        let registry = JobRegistry::default();
        let (job_id, _token) = registry.create_job("import", 10);

        registry.complete(&job_id);
        let result = registry.cancel(&job_id);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("is not running"));
    }

    #[test]
    fn test_list_jobs() {
        let registry = JobRegistry::default();
        let (id1, _) = registry.create_job("import", 10);
        let (id2, _) = registry.create_job("embeddings", 20);
        registry.complete(&id1);

        let jobs = registry.list();
        assert_eq!(jobs.len(), 2);

        let ids: Vec<&str> = jobs.iter().map(|j| j.job_id.as_str()).collect();
        assert!(ids.contains(&id1.as_str()));
        assert!(ids.contains(&id2.as_str()));
    }

    #[test]
    fn test_get_unknown_job() {
        let registry = JobRegistry::default();
        assert!(registry.get("job_nonexistent").is_none());
    }

    #[test]
    fn test_is_cancelled() {
        let registry = JobRegistry::default();
        let (job_id, _token) = registry.create_job("import", 10);

        assert!(!registry.is_cancelled(&job_id));
        registry.cancel(&job_id).unwrap();
        assert!(registry.is_cancelled(&job_id));
    }

    #[test]
    fn test_is_cancelled_unknown_returns_true() {
        let registry = JobRegistry::default();
        assert!(registry.is_cancelled("job_nonexistent"));
    }

    #[test]
    fn test_prune_does_not_remove_running() {
        let registry = JobRegistry::default();
        let (job_id, _token) = registry.create_job("import", 10);

        let jobs = registry.list();
        assert_eq!(jobs.len(), 1);
        assert_eq!(jobs[0].job_id, job_id);
    }

    #[test]
    fn test_complete_after_cancel_yields_cancelled() {
        let registry = JobRegistry::default();
        let (job_id, _token) = registry.create_job("import", 10);
        registry.cancel(&job_id).unwrap();
        registry.complete(&job_id);
        let snapshot = registry.get(&job_id).unwrap();
        assert_eq!(snapshot.status, "cancelled");
    }

    #[test]
    fn test_fail_after_cancel_yields_cancelled() {
        let registry = JobRegistry::default();
        let (job_id, _token) = registry.create_job("import", 10);
        registry.cancel(&job_id).unwrap();
        registry.fail(&job_id, "some error");
        let snapshot = registry.get(&job_id).unwrap();
        assert_eq!(snapshot.status, "cancelled");
        assert!(snapshot.error.is_none());
    }

    // --- Persistence tests ---

    fn test_db() -> crate::db_core::db::Database {
        crate::db_core::db::Database::open(std::path::Path::new(":memory:")).unwrap()
    }

    #[test]
    fn test_persist_terminal_saves_completed() {
        let db = test_db();
        let registry = JobRegistry::default();
        let (job_id, _) = registry.create_job("import", 10);
        registry.complete(&job_id);
        registry.persist_terminal(&job_id, &db);

        let loaded = db.load_terminal_jobs().unwrap();
        assert_eq!(loaded.len(), 1);
        assert_eq!(loaded[0].job_id, job_id);
        assert_eq!(loaded[0].status, "completed");
        assert_eq!(loaded[0].kind, "import");
    }

    #[test]
    fn test_persist_terminal_saves_failed() {
        let db = test_db();
        let registry = JobRegistry::default();
        let (job_id, _) = registry.create_job("embeddings", 5);
        registry.fail(&job_id, "Model not found");
        registry.persist_terminal(&job_id, &db);

        let loaded = db.load_terminal_jobs().unwrap();
        assert_eq!(loaded.len(), 1);
        assert_eq!(loaded[0].status, "failed");
        assert_eq!(loaded[0].error.as_deref(), Some("Model not found"));
    }

    #[test]
    fn test_persist_terminal_saves_cancelled() {
        let db = test_db();
        let registry = JobRegistry::default();
        let (job_id, _) = registry.create_job("detection", 20);
        registry.cancel(&job_id).unwrap();
        registry.mark_cancelled(&job_id);
        registry.persist_terminal(&job_id, &db);

        let loaded = db.load_terminal_jobs().unwrap();
        assert_eq!(loaded.len(), 1);
        assert_eq!(loaded[0].status, "cancelled");
    }

    #[test]
    fn test_persist_terminal_ignores_running() {
        let db = test_db();
        let registry = JobRegistry::default();
        let (job_id, _) = registry.create_job("import", 10);
        registry.persist_terminal(&job_id, &db);

        let loaded = db.load_terminal_jobs().unwrap();
        assert_eq!(loaded.len(), 0);
    }

    #[test]
    fn test_load_from_db_restores_terminal_jobs() {
        let db = test_db();

        // Create and persist two jobs
        let registry1 = JobRegistry::default();
        let (id1, _) = registry1.create_job("import", 10);
        registry1.complete(&id1);
        registry1.persist_terminal(&id1, &db);

        let (id2, _) = registry1.create_job("vision", 5);
        registry1.fail(&id2, "timeout");
        registry1.persist_terminal(&id2, &db);

        // Load into a fresh registry (simulates restart)
        let registry2 = JobRegistry::default();
        registry2.load_from_db(&db);

        assert!(registry2.get(&id1).is_some());
        assert_eq!(registry2.get(&id1).unwrap().status, "completed");
        assert!(registry2.get(&id2).is_some());
        assert_eq!(registry2.get(&id2).unwrap().status, "failed");
    }

    #[test]
    fn test_load_from_db_marks_stale_running_as_failed() {
        let db = test_db();

        // Manually insert a "running" job into DB (simulates crash)
        {
            let conn = db.conn.lock();
            conn.execute(
                "INSERT INTO mcp_jobs (job_id, kind, status, current, total, created_at, updated_at)
                 VALUES ('job_crashed123', 'import', 'running', 5, 10, '2026-01-01T00:00:00Z', '2026-01-01T00:00:00Z')",
                [],
            ).unwrap();
        }

        let registry = JobRegistry::default();
        registry.load_from_db(&db);

        let snapshot = registry.get("job_crashed123").unwrap();
        assert_eq!(snapshot.status, "failed");
    }

    #[test]
    fn test_load_from_db_marks_stale_cancelling_as_failed() {
        let db = test_db();

        {
            let conn = db.conn.lock();
            conn.execute(
                "INSERT INTO mcp_jobs (job_id, kind, status, current, total, created_at, updated_at)
                 VALUES ('job_cancel_stale', 'detection', 'cancelling', 3, 10, '2026-01-01T00:00:00Z', '2026-01-01T00:00:00Z')",
                [],
            ).unwrap();
        }

        let registry = JobRegistry::default();
        registry.load_from_db(&db);

        let snapshot = registry.get("job_cancel_stale").unwrap();
        assert_eq!(snapshot.status, "failed");
    }

    #[test]
    fn test_persist_then_load_preserves_progress() {
        let db = test_db();
        let registry = JobRegistry::default();
        let (job_id, _) = registry.create_job("embeddings", 100);
        registry.update_progress(&job_id, 73, Some("image_xyz"));
        registry.complete(&job_id);
        registry.persist_terminal(&job_id, &db);

        let registry2 = JobRegistry::default();
        registry2.load_from_db(&db);

        let snapshot = registry2.get(&job_id).unwrap();
        assert_eq!(snapshot.current, 100); // complete sets current = total
        assert_eq!(snapshot.total, 100);
        assert_eq!(snapshot.kind, "embeddings");
    }

    #[test]
    fn test_db_prune_old_jobs() {
        let db = test_db();

        // Insert a job with old timestamp
        {
            let conn = db.conn.lock();
            conn.execute(
                "INSERT INTO mcp_jobs (job_id, kind, status, current, total, created_at, updated_at)
                 VALUES ('job_old_one', 'import', 'completed', 10, 10, '2020-01-01T00:00:00Z', '2020-01-01T00:00:00Z')",
                [],
            ).unwrap();
        }

        let pruned = db.prune_old_jobs(1).unwrap();
        assert_eq!(pruned, 1);

        let loaded = db.load_terminal_jobs().unwrap();
        assert_eq!(loaded.len(), 0);
    }

    #[test]
    fn test_multiple_persist_updates_not_duplicates() {
        let db = test_db();
        let registry = JobRegistry::default();
        let (job_id, _) = registry.create_job("import", 10);
        registry.complete(&job_id);
        registry.persist_terminal(&job_id, &db);
        registry.persist_terminal(&job_id, &db); // second persist should update, not error

        let loaded = db.load_terminal_jobs().unwrap();
        assert_eq!(loaded.len(), 1);
    }
}
