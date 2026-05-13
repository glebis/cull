// Copyright (c) 2025-present Gleb Kalinin. Architecture and design by author.
// Implementation assisted by Claude (Anthropic). See AUTHORSHIP.md.

use super::models::*;
use super::smart_collections::{FilterNode, SmartCollection};
use parking_lot::Mutex;
use rusqlite::{params, Connection, OptionalExtension, Result};
use std::path::Path;
use std::sync::Arc;

#[derive(Clone)]
pub struct Database {
    pub(crate) conn: Arc<Mutex<Connection>>,
}

impl Database {
    pub fn open(db_path: &Path) -> Result<Self> {
        let conn = Connection::open(db_path)?;
        let db = Database {
            conn: Arc::new(Mutex::new(conn)),
        };
        db.run_migrations()?;
        Ok(db)
    }

    fn run_migrations(&self) -> Result<()> {
        let conn = self.conn.lock();
        let schema = include_str!("schema.sql");
        conn.execute_batch(schema)?;
        drop(conn);
        self.migrate_smart_collections()?;
        self.seed_preset_collections()?;
        self.migrate_lineage_tables()?;
        self.migrate_mcp_tables()?;
        self.migrate_model_processing()?;
        self.migrate_generation_runs()?;
        self.migrate_undo_tables()?;
        self.migrate_sessions()?;
        self.migrate_session_events()?;
        self.migrate_library_roots()?;
        self.migrate_image_file_stat_columns()?;
        self.migrate_raw_metadata()?;
        self.migrate_audit_log()?;
        self.migrate_asset_load_events()?;
        Ok(())
    }

    fn migrate_raw_metadata(&self) -> Result<()> {
        let conn = self.conn.lock();
        let sql = "ALTER TABLE images ADD COLUMN raw_metadata TEXT";
        match conn.execute(sql, []) {
            Ok(_) => {}
            Err(e) if e.to_string().contains("duplicate column") => {}
            Err(e) => return Err(e),
        }
        Ok(())
    }

    fn migrate_audit_log(&self) -> Result<()> {
        let conn = self.conn.lock();
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS api_audit_log (
                id TEXT PRIMARY KEY,
                timestamp TEXT NOT NULL,
                provider TEXT NOT NULL,
                endpoint TEXT NOT NULL,
                data_type TEXT NOT NULL,
                data_size_bytes INTEGER,
                prompt_preview TEXT,
                image_dimensions TEXT,
                model TEXT,
                response_status INTEGER,
                jurisdiction TEXT
            );",
        )?;
        Ok(())
    }

    fn migrate_asset_load_events(&self) -> Result<()> {
        let conn = self.conn.lock();
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS asset_load_events (
                seq INTEGER PRIMARY KEY AUTOINCREMENT,
                id TEXT NOT NULL UNIQUE,
                created_at TEXT NOT NULL,
                view TEXT NOT NULL,
                image_id TEXT,
                asset_kind TEXT NOT NULL,
                image_format TEXT,
                fallback_used INTEGER NOT NULL DEFAULT 0,
                fallback_succeeded INTEGER,
                path_basename TEXT,
                path_hash TEXT,
                error_kind TEXT NOT NULL,
                details_json TEXT
            );
            CREATE INDEX IF NOT EXISTS asset_load_events_created_idx ON asset_load_events(created_at);
            CREATE INDEX IF NOT EXISTS asset_load_events_image_created_idx ON asset_load_events(image_id, created_at);
            CREATE INDEX IF NOT EXISTS asset_load_events_error_idx ON asset_load_events(error_kind, created_at);",
        )?;
        Ok(())
    }

    fn migrate_library_roots(&self) -> Result<()> {
        let conn = self.conn.lock();
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS library_roots (
                id TEXT PRIMARY KEY,
                path TEXT NOT NULL UNIQUE,
                added_at TEXT NOT NULL
            );",
        )?;
        Ok(())
    }

    fn migrate_image_file_stat_columns(&self) -> Result<()> {
        let conn = self.conn.lock();
        for (name, typ) in &[("last_seen_size", "INTEGER"), ("last_seen_mtime", "TEXT")] {
            let sql = format!("ALTER TABLE image_files ADD COLUMN {} {}", name, typ);
            match conn.execute(&sql, []) {
                Ok(_) => {}
                Err(e) if e.to_string().contains("duplicate column") => {}
                Err(e) => return Err(e),
            }
        }
        Ok(())
    }

    fn migrate_smart_collections(&self) -> Result<()> {
        let conn = self.conn.lock();

        let columns = vec![
            ("source_label", "TEXT"),
            ("source_confidence", "REAL"),
            ("source_evidence_json", "TEXT"),
            ("source_detected_at", "TEXT"),
            ("source_detector_version", "TEXT"),
            ("is_ai_generated", "INTEGER"),
            ("ai_prompt", "TEXT"),
            ("aspect_ratio", "REAL"),
            ("orientation", "TEXT"),
            ("original_date", "TEXT"),
            ("megapixels", "REAL"),
        ];

        for (name, typ) in &columns {
            let sql = format!("ALTER TABLE images ADD COLUMN {} {}", name, typ);
            match conn.execute(&sql, []) {
                Ok(_) => {}
                Err(e) if e.to_string().contains("duplicate column") => {}
                Err(e) => return Err(e),
            }
        }

        let project_columns = vec![
            ("collection_type", "TEXT DEFAULT 'manual'"),
            ("filter_json", "TEXT"),
            ("nl_query", "TEXT"),
            ("is_preset", "INTEGER DEFAULT 0"),
            ("sort_order", "INTEGER DEFAULT 0"),
        ];

        for (name, typ) in &project_columns {
            let sql = format!("ALTER TABLE projects ADD COLUMN {} {}", name, typ);
            match conn.execute(&sql, []) {
                Ok(_) => {}
                Err(e) if e.to_string().contains("duplicate column") => {}
                Err(e) => return Err(e),
            }
        }

        Ok(())
    }

    fn seed_preset_collections(&self) -> Result<()> {
        let conn = self.conn.lock();

        let existing: i64 = conn.query_row(
            "SELECT COUNT(*) FROM projects WHERE is_preset = 1",
            [],
            |row| row.get(0),
        )?;

        if existing > 0 {
            return Ok(());
        }

        let presets: Vec<(&str, &str, i32)> = vec![
            (
                "5 Stars",
                r#"{"type":"rule","field":"rating","op":"eq","value":5.0}"#,
                1,
            ),
            (
                "4 Stars+",
                r#"{"type":"rule","field":"rating","op":"gte","value":4.0}"#,
                2,
            ),
            (
                "Picks",
                r#"{"type":"rule","field":"decision","op":"eq","value":"accept"}"#,
                3,
            ),
            (
                "Rejects",
                r#"{"type":"rule","field":"decision","op":"eq","value":"reject"}"#,
                4,
            ),
            (
                "Unrated",
                r#"{"type":"group","op":"and","children":[{"type":"rule","field":"rating","op":"eq","value":0.0},{"type":"rule","field":"decision","op":"eq","value":"undecided"}]}"#,
                5,
            ),
            (
                "Recent Imports",
                r#"{"type":"rule","field":"imported_at","op":"last_n_days","value":7.0}"#,
                6,
            ),
            (
                "Imported Today",
                r#"{"type":"rule","field":"imported_at","op":"last_n_days","value":1.0}"#,
                7,
            ),
            (
                "This Week",
                r#"{"type":"rule","field":"imported_at","op":"this_week","value":true}"#,
                8,
            ),
            (
                "This Month",
                r#"{"type":"rule","field":"imported_at","op":"this_month","value":true}"#,
                9,
            ),
            (
                "Landscape",
                r#"{"type":"rule","field":"orientation","op":"eq","value":"landscape"}"#,
                10,
            ),
            (
                "Portrait",
                r#"{"type":"rule","field":"orientation","op":"eq","value":"portrait"}"#,
                11,
            ),
            (
                "Square",
                r#"{"type":"rule","field":"orientation","op":"eq","value":"square"}"#,
                12,
            ),
            (
                "Panoramic",
                r#"{"type":"rule","field":"aspect_ratio","op":"gt","value":2.0}"#,
                13,
            ),
            (
                "PNG",
                r#"{"type":"rule","field":"format","op":"eq","value":"png"}"#,
                14,
            ),
            (
                "WebP",
                r#"{"type":"rule","field":"format","op":"eq","value":"webp"}"#,
                15,
            ),
            (
                "Large (>4K)",
                r#"{"type":"rule","field":"width","op":"gte","value":3840.0}"#,
                16,
            ),
            (
                "Small (<1024px)",
                r#"{"type":"rule","field":"width","op":"lt","value":1024.0}"#,
                17,
            ),
            (
                "AI Generated",
                r#"{"type":"rule","field":"is_ai_generated","op":"eq","value":true}"#,
                18,
            ),
            (
                "Red Label",
                r#"{"type":"rule","field":"color_label","op":"eq","value":"red"}"#,
                19,
            ),
            (
                "Green Label",
                r#"{"type":"rule","field":"color_label","op":"eq","value":"green"}"#,
                20,
            ),
            (
                "Blue Label",
                r#"{"type":"rule","field":"color_label","op":"eq","value":"blue"}"#,
                21,
            ),
            (
                "Yellow Label",
                r#"{"type":"rule","field":"color_label","op":"eq","value":"yellow"}"#,
                22,
            ),
        ];

        for (name, filter, order) in presets {
            let id = uuid::Uuid::new_v4().to_string();
            conn.execute(
                "INSERT INTO projects (id, name, collection_type, filter_json, is_preset, sort_order, created_at)
                 VALUES (?1, ?2, 'smart', ?3, 1, ?4, datetime('now'))",
                params![id, name, filter, order],
            )?;
        }

        Ok(())
    }

    fn migrate_lineage_tables(&self) -> Result<()> {
        let conn = self.conn.lock();

        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS lineage_groups (
                id TEXT PRIMARY KEY,
                name TEXT,
                created_at TEXT NOT NULL,
                detection_method TEXT,
                detection_score REAL
            );

            CREATE TABLE IF NOT EXISTS import_batches (
                id TEXT PRIMARY KEY,
                created_at TEXT NOT NULL,
                source TEXT,
                image_count INTEGER,
                collection_id TEXT
            );",
        )?;

        let image_columns = vec![
            ("lineage_group_id", "TEXT REFERENCES lineage_groups(id)"),
            ("lineage_order", "INTEGER DEFAULT 0"),
            ("import_batch_id", "TEXT"),
        ];
        for (name, typ) in &image_columns {
            let sql = format!("ALTER TABLE images ADD COLUMN {} {}", name, typ);
            let _ = conn.execute(&sql, []);
        }

        Ok(())
    }

    fn migrate_mcp_tables(&self) -> Result<()> {
        let conn = self.conn.lock();
        conn.execute_batch(
            "
            CREATE TABLE IF NOT EXISTS mcp_tokens (
                id TEXT PRIMARY KEY,
                name TEXT NOT NULL,
                secret_hash TEXT NOT NULL,
                role TEXT NOT NULL,
                scope_json TEXT,
                created_at TEXT NOT NULL,
                expires_at TEXT,
                last_used_at TEXT,
                revoked INTEGER DEFAULT 0
            );
            CREATE TABLE IF NOT EXISTS mcp_audit_log (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                token_id TEXT,
                tool_name TEXT NOT NULL,
                params_json TEXT,
                result_status TEXT NOT NULL,
                timestamp TEXT NOT NULL
            );
            CREATE TABLE IF NOT EXISTS mcp_jobs (
                job_id TEXT PRIMARY KEY,
                kind TEXT NOT NULL,
                status TEXT NOT NULL,
                current INTEGER NOT NULL DEFAULT 0,
                total INTEGER NOT NULL DEFAULT 0,
                message TEXT,
                error TEXT,
                created_at TEXT NOT NULL,
                updated_at TEXT NOT NULL
            );
        ",
        )?;
        Ok(())
    }

    fn migrate_undo_tables(&self) -> Result<()> {
        let conn = self.conn.lock();
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS undo_records (
                seq INTEGER PRIMARY KEY AUTOINCREMENT,
                id TEXT NOT NULL UNIQUE,
                action_type TEXT NOT NULL,
                label TEXT NOT NULL,
                before_json TEXT NOT NULL,
                after_json TEXT NOT NULL,
                affected_image_ids TEXT,
                group_id TEXT,
                has_file_backup INTEGER NOT NULL DEFAULT 0,
                created_at TEXT NOT NULL
            );

            CREATE TABLE IF NOT EXISTS undo_file_backups (
                id TEXT PRIMARY KEY,
                undo_record_id TEXT NOT NULL REFERENCES undo_records(id) ON DELETE CASCADE,
                original_path TEXT NOT NULL,
                backup_path TEXT NOT NULL,
                file_hash TEXT,
                created_at TEXT NOT NULL
            );",
        )?;
        Ok(())
    }

    fn migrate_generation_runs(&self) -> Result<()> {
        let conn = self.conn.lock();
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS generation_runs (
                id TEXT PRIMARY KEY,
                prompt TEXT,
                negative_prompt TEXT,
                provider TEXT,
                model TEXT,
                settings_json TEXT NOT NULL DEFAULT '{}',
                seed TEXT,
                parent_run_id TEXT REFERENCES generation_runs(id),
                source_type TEXT NOT NULL,
                source_path TEXT,
                raw_metadata_json TEXT,
                created_at TEXT,
                imported_at TEXT NOT NULL
            );",
        )?;
        let sql =
            "ALTER TABLE images ADD COLUMN generation_run_id TEXT REFERENCES generation_runs(id)";
        let _ = conn.execute(sql, []);
        Ok(())
    }

    fn migrate_session_events(&self) -> Result<()> {
        let conn = self.conn.lock();
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS session_events (
                id TEXT PRIMARY KEY,
                session_id TEXT REFERENCES projects(id) ON DELETE SET NULL,
                event_type TEXT NOT NULL,
                actor_type TEXT NOT NULL CHECK (actor_type IN ('user', 'agent', 'system')),
                actor_id TEXT,
                subject_type TEXT,
                subject_id TEXT,
                payload_json TEXT NOT NULL DEFAULT '{}',
                created_at TEXT NOT NULL
            );
            CREATE INDEX IF NOT EXISTS session_events_session_created_idx ON session_events(session_id, created_at);
            CREATE INDEX IF NOT EXISTS session_events_type_created_idx ON session_events(event_type, created_at);
            CREATE INDEX IF NOT EXISTS session_events_subject_idx ON session_events(subject_type, subject_id);",
        )?;
        Ok(())
    }

    fn migrate_model_processing(&self) -> Result<()> {
        let conn = self.conn.lock();
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS model_profiles (
                id TEXT PRIMARY KEY,
                slug TEXT NOT NULL UNIQUE,
                display_name TEXT NOT NULL,
                provider TEXT NOT NULL,
                task TEXT NOT NULL,
                model_id TEXT NOT NULL,
                runtime TEXT NOT NULL,
                source TEXT NOT NULL,
                privacy_class TEXT NOT NULL DEFAULT 'local',
                config_json TEXT NOT NULL DEFAULT '{}',
                license_class TEXT NOT NULL DEFAULT 'unknown',
                license_acknowledged_at TEXT,
                enabled INTEGER NOT NULL DEFAULT 1,
                created_at TEXT NOT NULL,
                updated_at TEXT NOT NULL
            );

            CREATE TABLE IF NOT EXISTS model_runs (
                id TEXT PRIMARY KEY,
                job_id TEXT,
                parent_run_id TEXT REFERENCES model_runs(id),
                profile_id TEXT REFERENCES model_profiles(id),
                task TEXT NOT NULL,
                provider TEXT NOT NULL,
                model_id TEXT NOT NULL,
                model_revision TEXT,
                status TEXT NOT NULL,
                input_scope_json TEXT NOT NULL,
                params_json TEXT NOT NULL DEFAULT '{}',
                output_summary_json TEXT NOT NULL DEFAULT '{}',
                cost_estimate_usd REAL,
                cost_actual_usd REAL,
                error TEXT,
                created_at TEXT NOT NULL,
                started_at TEXT,
                completed_at TEXT
            );

            CREATE TABLE IF NOT EXISTS model_run_items (
                id TEXT PRIMARY KEY,
                run_id TEXT NOT NULL REFERENCES model_runs(id) ON DELETE CASCADE,
                image_id TEXT REFERENCES images(id),
                input_asset_uri TEXT NOT NULL,
                input_hash TEXT,
                status TEXT NOT NULL,
                output_ref_kind TEXT,
                output_ref_id TEXT,
                audit_payload_json TEXT,
                cost_usd REAL,
                attempt_count INTEGER NOT NULL DEFAULT 1,
                error TEXT,
                started_at TEXT,
                completed_at TEXT
            );

            CREATE INDEX IF NOT EXISTS model_runs_job_idx ON model_runs(job_id);
            CREATE INDEX IF NOT EXISTS model_runs_status_idx ON model_runs(status);
            CREATE INDEX IF NOT EXISTS model_runs_parent_idx ON model_runs(parent_run_id);
            CREATE INDEX IF NOT EXISTS model_run_items_run_status_idx ON model_run_items(run_id, status);
            CREATE INDEX IF NOT EXISTS model_run_items_image_run_idx ON model_run_items(image_id, run_id);
            CREATE INDEX IF NOT EXISTS model_run_items_input_hash_idx ON model_run_items(input_hash);",
        )?;
        let sql = "ALTER TABLE embeddings ADD COLUMN model_run_id TEXT";
        match conn.execute(sql, []) {
            Ok(_) => {}
            Err(e) if e.to_string().contains("duplicate column") => {}
            Err(e) => return Err(e),
        }
        conn.execute(
            "CREATE INDEX IF NOT EXISTS embeddings_model_run_idx ON embeddings(model_run_id)",
            [],
        )?;
        Ok(())
    }

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

    pub fn insert_image(&self, image: &Image) -> Result<()> {
        let conn = self.conn.lock();
        conn.execute(
            "INSERT OR IGNORE INTO images (id, sha256_hash, width, height, format, file_size, created_at, imported_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
            params![image.id, image.sha256_hash, image.width, image.height,
                    image.format, image.file_size, image.created_at, image.imported_at],
        )?;
        Ok(())
    }

    pub fn insert_image_file(&self, file: &ImageFile) -> Result<()> {
        let conn = self.conn.lock();
        conn.execute(
            "INSERT OR REPLACE INTO image_files (id, image_id, path, last_seen_at, missing_at, last_seen_size, last_seen_mtime)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            params![file.id, file.image_id, file.path, file.last_seen_at, file.missing_at, file.last_seen_size, file.last_seen_mtime],
        )?;
        Ok(())
    }

    pub fn find_by_hash(&self, hash: &str) -> Result<Option<Image>> {
        let conn = self.conn.lock();
        let mut stmt = conn.prepare(
            "SELECT id, sha256_hash, width, height, format, file_size, created_at, imported_at, ai_prompt, raw_metadata
             FROM images WHERE sha256_hash = ?1"
        )?;
        let mut rows = stmt.query_map(params![hash], |row| {
            Ok(Image {
                id: row.get(0)?,
                sha256_hash: row.get(1)?,
                width: row.get(2)?,
                height: row.get(3)?,
                format: row.get(4)?,
                file_size: row.get(5)?,
                created_at: row.get(6)?,
                imported_at: row.get(7)?,
                ai_prompt: row.get(8)?,
                raw_metadata: row.get(9)?,
            })
        })?;
        match rows.next() {
            Some(Ok(img)) => Ok(Some(img)),
            _ => Ok(None),
        }
    }

    pub fn list_images(&self, limit: u32, offset: u32) -> Result<Vec<ImageWithFile>> {
        let conn = self.conn.lock();
        let mut stmt = conn.prepare(
            "SELECT i.id, i.sha256_hash, i.width, i.height, i.format, i.file_size,
                    i.created_at, i.imported_at, f.path,
                    s.star_rating, s.color_label, s.decision, i.source_label, i.ai_prompt,
                    i.raw_metadata, f.missing_at
             FROM images i
             JOIN image_files f ON f.image_id = i.id AND f.missing_at IS NULL
             LEFT JOIN selections s ON s.image_id = i.id AND s.project_id = '__global__'
             GROUP BY i.id
             ORDER BY i.imported_at DESC
             LIMIT ?1 OFFSET ?2",
        )?;
        let rows = stmt.query_map(params![limit, offset], |row| {
            let star: Option<u8> = row.get(9)?;
            let color: Option<String> = row.get(10)?;
            let decision: Option<String> = row.get(11)?;
            let selection = decision.map(|d| Selection {
                image_id: row.get(0).unwrap(),
                project_id: None,
                star_rating: star,
                color_label: color,
                decision: d,
            });
            Ok(ImageWithFile {
                image: Image {
                    id: row.get(0)?,
                    sha256_hash: row.get(1)?,
                    width: row.get(2)?,
                    height: row.get(3)?,
                    format: row.get(4)?,
                    file_size: row.get(5)?,
                    created_at: row.get(6)?,
                    imported_at: row.get(7)?,
                    ai_prompt: row.get(13)?,
                    raw_metadata: row.get(14)?,
                },
                path: row.get(8)?,
                thumbnail_path: None,
                selection,
                source_label: row.get(12)?,
                missing_at: row.get(15)?,
            })
        })?;
        rows.collect::<Result<Vec<_>>>()
    }

    pub fn set_rating(&self, image_id: &str, rating: u8) -> Result<()> {
        let conn = self.conn.lock();
        conn.execute(
            "INSERT INTO selections (image_id, project_id, star_rating, decision)
             VALUES (?1, '__global__', ?2, 'undecided')
             ON CONFLICT(image_id, project_id)
             DO UPDATE SET star_rating = ?2",
            params![image_id, rating],
        )?;
        Ok(())
    }

    pub fn set_decision(&self, image_id: &str, decision: &str) -> Result<()> {
        let conn = self.conn.lock();
        conn.execute(
            "INSERT INTO selections (image_id, project_id, decision)
             VALUES (?1, '__global__', ?2)
             ON CONFLICT(image_id, project_id)
             DO UPDATE SET decision = ?2",
            params![image_id, decision],
        )?;
        Ok(())
    }

    pub fn list_folders(&self) -> Result<Vec<(String, u32)>> {
        let conn = self.conn.lock();
        let mut stmt = conn
            .prepare("SELECT f.path, f.image_id FROM image_files f WHERE f.missing_at IS NULL")?;
        let rows = stmt.query_map([], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
        })?;

        let mut folder_counts: std::collections::HashMap<String, u32> =
            std::collections::HashMap::new();
        for row in rows {
            let (path, _) = row?;
            if let Some(parent) = std::path::Path::new(&path).parent() {
                let folder = parent.to_string_lossy().to_string();
                *folder_counts.entry(folder).or_insert(0) += 1;
            }
        }

        let mut result: Vec<(String, u32)> = folder_counts.into_iter().collect();
        result.sort_by(|a, b| a.0.cmp(&b.0));
        Ok(result)
    }

    pub fn list_images_by_folder(
        &self,
        folder: &str,
        limit: u32,
        offset: u32,
    ) -> Result<Vec<ImageWithFile>> {
        let conn = self.conn.lock();
        let pattern = format!("{}/%", folder);
        let mut stmt = conn.prepare(
            "SELECT i.id, i.sha256_hash, i.width, i.height, i.format, i.file_size,
                    i.created_at, i.imported_at, f.path,
                    s.star_rating, s.color_label, s.decision, i.source_label, i.ai_prompt,
                    i.raw_metadata, f.missing_at
             FROM images i
             JOIN image_files f ON f.image_id = i.id AND f.missing_at IS NULL
             LEFT JOIN selections s ON s.image_id = i.id AND s.project_id = '__global__'
             WHERE f.path LIKE ?1
             GROUP BY i.id
             ORDER BY i.imported_at DESC
             LIMIT ?2 OFFSET ?3",
        )?;
        let rows = stmt.query_map(params![pattern, limit, offset], |row| {
            let star: Option<u8> = row.get(9)?;
            let color: Option<String> = row.get(10)?;
            let decision: Option<String> = row.get(11)?;
            let selection = decision.map(|d| Selection {
                image_id: row.get(0).unwrap(),
                project_id: None,
                star_rating: star,
                color_label: color,
                decision: d,
            });
            Ok(ImageWithFile {
                image: Image {
                    id: row.get(0)?,
                    sha256_hash: row.get(1)?,
                    width: row.get(2)?,
                    height: row.get(3)?,
                    format: row.get(4)?,
                    file_size: row.get(5)?,
                    created_at: row.get(6)?,
                    imported_at: row.get(7)?,
                    ai_prompt: row.get(13)?,
                    raw_metadata: row.get(14)?,
                },
                path: row.get(8)?,
                thumbnail_path: None,
                selection,
                source_label: row.get(12)?,
                missing_at: row.get(15)?,
            })
        })?;
        rows.collect::<Result<Vec<_>>>()
    }

    pub fn list_images_filtered(
        &self,
        min_width: Option<u32>,
        min_height: Option<u32>,
        limit: u32,
        offset: u32,
    ) -> Result<Vec<ImageWithFile>> {
        let conn = self.conn.lock();
        let mut sql = String::from(
            "SELECT i.id, i.sha256_hash, i.width, i.height, i.format, i.file_size,
                    i.created_at, i.imported_at, f.path,
                    s.star_rating, s.color_label, s.decision, i.source_label, i.ai_prompt,
                    i.raw_metadata, f.missing_at
             FROM images i
             JOIN image_files f ON f.image_id = i.id AND f.missing_at IS NULL
             LEFT JOIN selections s ON s.image_id = i.id AND s.project_id = '__global__'
             WHERE 1=1",
        );
        if let Some(w) = min_width {
            sql.push_str(&format!(" AND i.width >= {}", w));
        }
        if let Some(h) = min_height {
            sql.push_str(&format!(" AND i.height >= {}", h));
        }
        sql.push_str(" GROUP BY i.id ORDER BY i.imported_at DESC LIMIT ?1 OFFSET ?2");

        let mut stmt = conn.prepare(&sql)?;
        let rows = stmt.query_map(params![limit, offset], |row| {
            let star: Option<u8> = row.get(9)?;
            let color: Option<String> = row.get(10)?;
            let decision: Option<String> = row.get(11)?;
            let selection = decision.map(|d| Selection {
                image_id: row.get(0).unwrap(),
                project_id: None,
                star_rating: star,
                color_label: color,
                decision: d,
            });
            Ok(ImageWithFile {
                image: Image {
                    id: row.get(0)?,
                    sha256_hash: row.get(1)?,
                    width: row.get(2)?,
                    height: row.get(3)?,
                    format: row.get(4)?,
                    file_size: row.get(5)?,
                    created_at: row.get(6)?,
                    imported_at: row.get(7)?,
                    ai_prompt: row.get(13)?,
                    raw_metadata: row.get(14)?,
                },
                path: row.get(8)?,
                thumbnail_path: None,
                selection,
                source_label: row.get(12)?,
                missing_at: row.get(15)?,
            })
        })?;
        rows.collect::<Result<Vec<_>>>()
    }

    // ---- Collection methods ----

    pub fn create_collection(&self, name: &str) -> Result<String> {
        let id = uuid::Uuid::new_v4().to_string();
        let now = chrono::Utc::now().to_rfc3339();
        let conn = self.conn.lock();
        conn.execute(
            "INSERT INTO projects (id, name, description, created_at) VALUES (?1, ?2, NULL, ?3)",
            params![id, name, now],
        )?;
        Ok(id)
    }

    pub fn list_collections(&self) -> Result<Vec<(String, String, u32)>> {
        let conn = self.conn.lock();
        let mut stmt = conn.prepare(
            "SELECT p.id, p.name, COUNT(ci.image_id) as cnt
             FROM projects p
             LEFT JOIN collection_items ci ON ci.collection_id = p.id
             WHERE (p.collection_type IS NULL OR p.collection_type = 'manual')
             GROUP BY p.id
             ORDER BY p.created_at DESC",
        )?;
        let rows = stmt.query_map([], |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)))?;
        rows.collect::<Result<Vec<_>>>()
    }

    pub fn add_to_collection(&self, collection_id: &str, image_ids: &[&str]) -> Result<()> {
        let conn = self.conn.lock();
        let max_pos: i64 = conn.query_row(
            "SELECT COALESCE(MAX(position), -1) FROM collection_items WHERE collection_id = ?1",
            params![collection_id],
            |row| row.get(0),
        )?;
        for (i, id) in image_ids.iter().enumerate() {
            conn.execute(
                "INSERT OR IGNORE INTO collection_items (collection_id, image_id, position) VALUES (?1, ?2, ?3)",
                params![collection_id, id, max_pos + 1 + i as i64],
            )?;
        }
        Ok(())
    }

    pub fn list_collection_images(&self, collection_id: &str) -> Result<Vec<ImageWithFile>> {
        let conn = self.conn.lock();
        let mut stmt = conn.prepare(
            "SELECT i.id, i.sha256_hash, i.width, i.height, i.format, i.file_size,
                    i.created_at, i.imported_at, f.path,
                    s.star_rating, s.color_label, s.decision, i.source_label, i.ai_prompt,
                    i.raw_metadata, f.missing_at
             FROM collection_items ci
             JOIN images i ON i.id = ci.image_id
             JOIN image_files f ON f.image_id = i.id AND f.missing_at IS NULL
             LEFT JOIN selections s ON s.image_id = i.id AND s.project_id = '__global__'
             WHERE ci.collection_id = ?1
             GROUP BY i.id
             ORDER BY ci.position ASC",
        )?;
        let rows = stmt.query_map(params![collection_id], |row| {
            let star: Option<u8> = row.get(9)?;
            let color: Option<String> = row.get(10)?;
            let decision: Option<String> = row.get(11)?;
            let selection = decision.map(|d| Selection {
                image_id: row.get(0).unwrap(),
                project_id: None,
                star_rating: star,
                color_label: color,
                decision: d,
            });
            Ok(ImageWithFile {
                image: Image {
                    id: row.get(0)?,
                    sha256_hash: row.get(1)?,
                    width: row.get(2)?,
                    height: row.get(3)?,
                    format: row.get(4)?,
                    file_size: row.get(5)?,
                    created_at: row.get(6)?,
                    imported_at: row.get(7)?,
                    ai_prompt: row.get(13)?,
                    raw_metadata: row.get(14)?,
                },
                path: row.get(8)?,
                thumbnail_path: None,
                selection,
                source_label: row.get(12)?,
                missing_at: row.get(15)?,
            })
        })?;
        rows.collect::<Result<Vec<_>>>()
    }

    pub fn delete_collection(&self, collection_id: &str) -> Result<()> {
        let conn = self.conn.lock();
        conn.execute(
            "DELETE FROM collection_items WHERE collection_id = ?1",
            params![collection_id],
        )?;
        conn.execute("DELETE FROM projects WHERE id = ?1", params![collection_id])?;
        Ok(())
    }

    // ---- Settings methods ----

    pub fn get_setting(&self, key: &str) -> Result<Option<String>> {
        let conn = self.conn.lock();
        let mut stmt = conn.prepare("SELECT value FROM app_settings WHERE key = ?1")?;
        let mut rows = stmt.query_map(params![key], |row| row.get(0))?;
        match rows.next() {
            Some(Ok(val)) => Ok(Some(val)),
            _ => Ok(None),
        }
    }

    pub fn set_setting(&self, key: &str, value: &str) -> Result<()> {
        let conn = self.conn.lock();
        conn.execute(
            "INSERT OR REPLACE INTO app_settings (key, value) VALUES (?1, ?2)",
            params![key, value],
        )?;
        Ok(())
    }

    // ---- Embedding methods ----

    pub fn store_embedding(&self, image_id: &str, model_name: &str, vector: &[f32]) -> Result<()> {
        self.store_embedding_with_model_run(image_id, model_name, vector, None)
            .map(|_| ())
    }

    pub fn store_embedding_with_model_run(
        &self,
        image_id: &str,
        model_name: &str,
        vector: &[f32],
        model_run_id: Option<&str>,
    ) -> Result<String> {
        let conn = self.conn.lock();
        let bytes: Vec<u8> = vector.iter().flat_map(|f| f.to_le_bytes()).collect();
        let embedding_id = uuid::Uuid::new_v4().to_string();
        conn.execute(
            "INSERT OR REPLACE INTO embeddings (id, image_id, model_name, model_run_id, vector, dims, dtype, normalized, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, 'float32', 1, ?7)",
            params![
                embedding_id,
                image_id,
                model_name,
                model_run_id,
                bytes,
                vector.len() as u32,
                chrono::Utc::now().to_rfc3339(),
            ],
        )?;
        Ok(embedding_id)
    }

    pub fn get_all_embeddings(&self, model_name: &str) -> Result<Vec<(String, Vec<f32>)>> {
        let conn = self.conn.lock();
        let mut stmt =
            conn.prepare("SELECT image_id, vector, dims FROM embeddings WHERE model_name = ?1")?;
        let rows = stmt.query_map(params![model_name], |row| {
            let image_id: String = row.get(0)?;
            let bytes: Vec<u8> = row.get(1)?;
            let _dims: u32 = row.get(2)?;
            let vector: Vec<f32> = bytes
                .chunks(4)
                .map(|chunk| f32::from_le_bytes([chunk[0], chunk[1], chunk[2], chunk[3]]))
                .collect();
            Ok((image_id, vector))
        })?;
        rows.collect::<Result<Vec<_>>>()
    }

    pub fn find_similar(
        &self,
        vector: &[f32],
        model_name: &str,
        top_k: usize,
    ) -> Result<Vec<(String, f32)>> {
        let all = self.get_all_embeddings(model_name)?;
        let mut scores: Vec<(String, f32)> = all
            .iter()
            .map(|(id, emb)| {
                let score = cosine_similarity(vector, emb);
                (id.clone(), score)
            })
            .collect();
        scores.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
        scores.truncate(top_k);
        Ok(scores)
    }

    pub fn embedding_count(&self, model_name: &str) -> Result<u32> {
        let conn = self.conn.lock();
        conn.query_row(
            "SELECT COUNT(*) FROM embeddings WHERE model_name = ?1",
            params![model_name],
            |row| row.get(0),
        )
    }

    pub fn remove_from_collection(&self, collection_id: &str, image_id: &str) -> Result<()> {
        let conn = self.conn.lock();
        conn.execute(
            "DELETE FROM collection_items WHERE collection_id = ?1 AND image_id = ?2",
            params![collection_id, image_id],
        )?;
        Ok(())
    }

    pub fn delete_images_by_folder(&self, folder: &str) -> Result<u32> {
        let conn = self.conn.lock();
        let pattern = format!("{}/%", folder);

        // Get image IDs that ONLY exist in this folder (no other paths)
        let mut stmt = conn.prepare(
            "SELECT DISTINCT f.image_id FROM image_files f
             WHERE f.path LIKE ?1 AND f.missing_at IS NULL
             AND f.image_id NOT IN (
                 SELECT image_id FROM image_files
                 WHERE path NOT LIKE ?1 AND missing_at IS NULL
             )",
        )?;
        let image_ids: Vec<String> = stmt
            .query_map(params![pattern], |row| row.get(0))?
            .filter_map(|r| r.ok())
            .collect();

        let count = image_ids.len() as u32;

        // Delete the images (CASCADE will handle image_files, selections, etc.)
        for id in &image_ids {
            conn.execute("DELETE FROM images WHERE id = ?1", params![id])?;
        }

        // Also delete file records from this folder for images that still exist elsewhere
        conn.execute(
            "DELETE FROM image_files WHERE path LIKE ?1",
            params![pattern],
        )?;

        Ok(count)
    }

    // ---- Vision metadata methods ----

    pub fn store_vision_metadata(
        &self,
        image_id: &str,
        source: &str,
        fields: &std::collections::HashMap<String, String>,
    ) -> Result<()> {
        let conn = self.conn.lock();
        for (key, value) in fields {
            conn.execute(
                "INSERT OR REPLACE INTO image_metadata (image_id, key, value, source) VALUES (?1, ?2, ?3, ?4)",
                params![image_id, key, value, source],
            )?;
        }
        Ok(())
    }

    pub fn get_vision_metadata(&self, image_id: &str) -> Result<Vec<(String, String, String)>> {
        let conn = self.conn.lock();
        let mut stmt = conn.prepare(
            "SELECT key, value, source FROM image_metadata WHERE image_id = ?1 ORDER BY key",
        )?;
        let rows = stmt.query_map(params![image_id], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
            ))
        })?;
        rows.collect::<Result<Vec<_>>>()
    }

    pub fn count_vision_processed(&self, source: &str) -> Result<u32> {
        let conn = self.conn.lock();
        conn.query_row(
            "SELECT COUNT(DISTINCT image_id) FROM image_metadata WHERE source = ?1",
            params![source],
            |row| row.get::<_, u32>(0),
        )
    }

    // ---- File watcher helpers ----

    pub fn mark_file_missing(&self, path: &str) -> Result<bool> {
        let conn = self.conn.lock();
        let updated = conn.execute(
            "UPDATE image_files SET missing_at = datetime('now') WHERE path = ?1 AND missing_at IS NULL",
            params![path],
        )?;
        Ok(updated > 0)
    }

    pub fn restore_file(&self, path: &str) -> Result<bool> {
        let conn = self.conn.lock();
        let updated = conn.execute(
            "UPDATE image_files SET missing_at = NULL, last_seen_at = datetime('now') WHERE path = ?1",
            params![path],
        )?;
        Ok(updated > 0)
    }

    pub fn update_image_file_path(&self, file_id: &str, new_path: &str) -> Result<()> {
        let conn = self.conn.lock();
        conn.execute(
            "UPDATE image_files SET path = ?2, last_seen_at = datetime('now'), missing_at = NULL WHERE id = ?1",
            params![file_id, new_path],
        )?;
        Ok(())
    }

    pub fn restore_or_move_file_by_hash(&self, sha256: &str, new_path: &str) -> Result<bool> {
        let conn = self.conn.lock();
        let file_id: Option<String> = conn
            .query_row(
                "SELECT f.id FROM image_files f
             JOIN images i ON i.id = f.image_id
             WHERE i.sha256_hash = ?1 AND f.missing_at IS NOT NULL
             ORDER BY f.missing_at DESC LIMIT 1",
                params![sha256],
                |row| row.get(0),
            )
            .optional()?;

        if let Some(fid) = file_id {
            conn.execute(
                "UPDATE image_files SET path = ?2, missing_at = NULL, last_seen_at = datetime('now') WHERE id = ?1",
                params![fid, new_path],
            )?;
            Ok(true)
        } else {
            Ok(false)
        }
    }

    pub fn get_image_file_by_path(&self, path: &str) -> Result<Option<ImageFile>> {
        let conn = self.conn.lock();
        let mut stmt = conn.prepare(
            "SELECT id, image_id, path, last_seen_at, missing_at, last_seen_size, last_seen_mtime FROM image_files WHERE path = ?1"
        )?;
        let mut rows = stmt.query_map(params![path], |row| {
            Ok(ImageFile {
                id: row.get(0)?,
                image_id: row.get(1)?,
                path: row.get(2)?,
                last_seen_at: row.get(3)?,
                missing_at: row.get(4)?,
                last_seen_size: row.get(5)?,
                last_seen_mtime: row.get(6)?,
            })
        })?;
        match rows.next() {
            Some(Ok(f)) => Ok(Some(f)),
            _ => Ok(None),
        }
    }

    pub fn touch_image_file(&self, file_id: &str, size: u64, mtime: &str) -> Result<()> {
        let conn = self.conn.lock();
        conn.execute(
            "UPDATE image_files SET last_seen_at = datetime('now'), missing_at = NULL,
             last_seen_size = ?2, last_seen_mtime = ?3 WHERE id = ?1",
            params![file_id, size as i64, mtime],
        )?;
        Ok(())
    }

    pub fn repoint_image_file(
        &self,
        file_id: &str,
        new_image_id: &str,
        size: u64,
        mtime: &str,
    ) -> Result<()> {
        let conn = self.conn.lock();
        conn.execute(
            "UPDATE image_files SET image_id = ?2, last_seen_at = datetime('now'), missing_at = NULL,
             last_seen_size = ?3, last_seen_mtime = ?4 WHERE id = ?1",
            params![file_id, new_image_id, size as i64, mtime],
        )?;
        Ok(())
    }

    // ---- Library roots ----

    pub fn add_library_root(&self, path: &str) -> Result<String> {
        let id = uuid::Uuid::new_v4().to_string();
        let conn = self.conn.lock();
        conn.execute(
            "INSERT OR IGNORE INTO library_roots (id, path, added_at) VALUES (?1, ?2, datetime('now'))",
            params![id, path],
        )?;
        Ok(id)
    }

    pub fn list_library_roots(&self) -> Result<Vec<String>> {
        let conn = self.conn.lock();
        let mut stmt = conn.prepare("SELECT path FROM library_roots ORDER BY added_at")?;
        let rows = stmt.query_map([], |row| row.get(0))?;
        rows.collect()
    }

    pub fn remove_library_root(&self, path: &str) -> Result<bool> {
        let conn = self.conn.lock();
        let deleted = conn.execute("DELETE FROM library_roots WHERE path = ?1", params![path])?;
        Ok(deleted > 0)
    }

    pub fn image_count(&self) -> Result<u32> {
        let conn = self.conn.lock();
        conn.query_row(
            "SELECT COUNT(DISTINCT i.id) FROM images i
             JOIN image_files f ON f.image_id = i.id AND f.missing_at IS NULL",
            [],
            |row| row.get(0),
        )
    }

    pub fn get_images_by_ids(&self, ids: &[&str]) -> Result<Vec<ImageWithFile>> {
        if ids.is_empty() {
            return Ok(vec![]);
        }
        let conn = self.conn.lock();
        let placeholders: Vec<String> = ids
            .iter()
            .enumerate()
            .map(|(i, _)| format!("?{}", i + 1))
            .collect();
        let sql = format!(
            "SELECT i.id, i.sha256_hash, i.width, i.height, i.format, i.file_size,
                    i.created_at, i.imported_at, f.path,
                    s.star_rating, s.color_label, s.decision, i.source_label, i.ai_prompt,
                    i.raw_metadata, f.missing_at
             FROM images i
             JOIN image_files f ON f.image_id = i.id AND f.missing_at IS NULL
             LEFT JOIN selections s ON s.image_id = i.id AND s.project_id = '__global__'
             WHERE i.id IN ({})
             GROUP BY i.id",
            placeholders.join(", ")
        );
        let params: Vec<&dyn rusqlite::types::ToSql> = ids
            .iter()
            .map(|id| id as &dyn rusqlite::types::ToSql)
            .collect();
        let mut stmt = conn.prepare(&sql)?;
        let rows = stmt.query_map(params.as_slice(), |row| {
            let star: Option<u8> = row.get(9)?;
            let color: Option<String> = row.get(10)?;
            let decision: Option<String> = row.get(11)?;
            let selection = decision.map(|d| Selection {
                image_id: row.get(0).unwrap(),
                project_id: None,
                star_rating: star,
                color_label: color,
                decision: d,
            });
            Ok(ImageWithFile {
                image: Image {
                    id: row.get(0)?,
                    sha256_hash: row.get(1)?,
                    width: row.get(2)?,
                    height: row.get(3)?,
                    format: row.get(4)?,
                    file_size: row.get(5)?,
                    created_at: row.get(6)?,
                    imported_at: row.get(7)?,
                    ai_prompt: row.get(13)?,
                    raw_metadata: row.get(14)?,
                },
                path: row.get(8)?,
                thumbnail_path: None,
                selection,
                source_label: row.get(12)?,
                missing_at: row.get(15)?,
            })
        })?;
        rows.collect::<Result<Vec<_>>>()
    }

    pub fn get_iteration_siblings(&self, parent_id: &str) -> Result<Vec<ImageWithFile>> {
        let conn = self.conn.lock();
        let mut stmt = conn.prepare(
            "SELECT i.id, i.sha256_hash, i.width, i.height, i.format, i.file_size,
                    i.created_at, i.imported_at, f.path,
                    s.star_rating, s.color_label, s.decision, i.source_label, i.ai_prompt,
                    i.raw_metadata, f.missing_at
             FROM iterations it
             JOIN images i ON i.id = it.child_id
             JOIN image_files f ON f.image_id = i.id AND f.missing_at IS NULL
             LEFT JOIN selections s ON s.image_id = i.id AND s.project_id = '__global__'
             WHERE it.parent_id = ?1
             GROUP BY i.id
             ORDER BY it.created_at ASC",
        )?;
        let rows = stmt.query_map(params![parent_id], |row| {
            let star: Option<u8> = row.get(9)?;
            let color: Option<String> = row.get(10)?;
            let decision: Option<String> = row.get(11)?;
            let selection = decision.map(|d| Selection {
                image_id: row.get(0).unwrap(),
                project_id: None,
                star_rating: star,
                color_label: color,
                decision: d,
            });
            Ok(ImageWithFile {
                image: Image {
                    id: row.get(0)?,
                    sha256_hash: row.get(1)?,
                    width: row.get(2)?,
                    height: row.get(3)?,
                    format: row.get(4)?,
                    file_size: row.get(5)?,
                    created_at: row.get(6)?,
                    imported_at: row.get(7)?,
                    ai_prompt: row.get(13)?,
                    raw_metadata: row.get(14)?,
                },
                path: row.get(8)?,
                thumbnail_path: None,
                selection,
                source_label: row.get(12)?,
                missing_at: row.get(15)?,
            })
        })?;
        rows.collect::<Result<Vec<_>>>()
    }

    // ---- Detection methods ----

    pub fn store_detections(
        &self,
        image_id: &str,
        model_name: &str,
        detections: &[super::detection::Detection],
    ) -> Result<()> {
        let conn = self.conn.lock();
        // Clear previous detections for this image+model
        conn.execute(
            "DELETE FROM detections WHERE image_id = ?1 AND model_name = ?2",
            params![image_id, model_name],
        )?;
        for det in detections {
            conn.execute(
                "INSERT INTO detections (id, image_id, model_name, class_name, confidence, x, y, width, height, created_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
                params![
                    uuid::Uuid::new_v4().to_string(),
                    image_id,
                    model_name,
                    det.class_name,
                    det.confidence,
                    det.x,
                    det.y,
                    det.width,
                    det.height,
                    chrono::Utc::now().to_rfc3339(),
                ],
            )?;
        }
        Ok(())
    }

    pub fn get_detections(
        &self,
        image_id: &str,
        model_name: Option<&str>,
    ) -> Result<Vec<super::detection::Detection>> {
        let conn = self.conn.lock();
        let (sql, params_vec): (String, Vec<Box<dyn rusqlite::types::ToSql>>) = if let Some(mn) =
            model_name
        {
            (
                "SELECT class_name, confidence, x, y, width, height FROM detections WHERE image_id = ?1 AND model_name = ?2 ORDER BY confidence DESC".to_string(),
                vec![Box::new(image_id.to_string()), Box::new(mn.to_string())],
            )
        } else {
            (
                "SELECT class_name, confidence, x, y, width, height FROM detections WHERE image_id = ?1 ORDER BY confidence DESC".to_string(),
                vec![Box::new(image_id.to_string())],
            )
        };
        let mut stmt = conn.prepare(&sql)?;
        let params_refs: Vec<&dyn rusqlite::types::ToSql> =
            params_vec.iter().map(|p| p.as_ref()).collect();
        let rows = stmt.query_map(params_refs.as_slice(), |row| {
            Ok(super::detection::Detection {
                class_name: row.get(0)?,
                confidence: row.get(1)?,
                x: row.get(2)?,
                y: row.get(3)?,
                width: row.get(4)?,
                height: row.get(5)?,
            })
        })?;
        rows.collect::<Result<Vec<_>>>()
    }

    pub fn search_by_class(&self, class_name: &str, limit: u32) -> Result<Vec<(String, f32)>> {
        let conn = self.conn.lock();
        let mut stmt = conn.prepare(
            "SELECT DISTINCT image_id, MAX(confidence) as max_conf
             FROM detections WHERE class_name = ?1
             GROUP BY image_id ORDER BY max_conf DESC LIMIT ?2",
        )?;
        let rows = stmt.query_map(params![class_name, limit], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, f32>(1)?))
        })?;
        rows.collect::<Result<Vec<_>>>()
    }

    pub fn detection_count(&self, model_name: &str) -> Result<u32> {
        let conn = self.conn.lock();
        conn.query_row(
            "SELECT COUNT(DISTINCT image_id) FROM detections WHERE model_name = ?1",
            params![model_name],
            |row| row.get::<_, u32>(0),
        )
    }

    pub fn create_smart_collection(
        &self,
        name: &str,
        filter_json: &str,
        nl_query: Option<&str>,
        is_preset: bool,
    ) -> Result<String> {
        let conn = self.conn.lock();
        let id = uuid::Uuid::new_v4().to_string();
        conn.execute(
            "INSERT INTO projects (id, name, collection_type, filter_json, nl_query, is_preset, created_at)
             VALUES (?1, ?2, 'smart', ?3, ?4, ?5, datetime('now'))",
            rusqlite::params![id, name, filter_json, nl_query, is_preset as i32],
        )?;
        Ok(id)
    }

    pub fn list_smart_collections(&self) -> Result<Vec<SmartCollection>> {
        let conn = self.conn.lock();
        let mut stmt = conn.prepare(
            "SELECT id, name, description, collection_type, filter_json, nl_query,
                    is_preset, sort_order, created_at
             FROM projects
             WHERE collection_type = 'smart'
             ORDER BY sort_order ASC, created_at DESC",
        )?;
        let mut collections: Vec<SmartCollection> = stmt
            .query_map([], |row| {
                Ok(SmartCollection {
                    id: row.get(0)?,
                    name: row.get(1)?,
                    description: row.get(2)?,
                    collection_type: row.get(3)?,
                    filter_json: row.get(4)?,
                    nl_query: row.get(5)?,
                    is_preset: row.get::<_, i32>(6)? != 0,
                    sort_order: row.get(7)?,
                    created_at: row.get(8)?,
                    image_count: None,
                })
            })?
            .collect::<Result<Vec<_>>>()?;

        for sc in &mut collections {
            if let Some(ref filter_json) = sc.filter_json {
                if let Ok(filter) = serde_json::from_str::<FilterNode>(filter_json) {
                    if let Ok((where_clause, params)) = filter.to_sql_clause() {
                        let sql = format!(
                            "SELECT COUNT(DISTINCT i.id)
                             FROM images i
                             JOIN image_files f ON f.image_id = i.id AND f.missing_at IS NULL
                             LEFT JOIN selections s ON s.image_id = i.id AND s.project_id = '__global__'
                             WHERE ({})",
                            where_clause
                        );
                        let param_refs: Vec<&dyn rusqlite::types::ToSql> = params
                            .iter()
                            .map(|p| p as &dyn rusqlite::types::ToSql)
                            .collect();
                        if let Ok(count) =
                            conn.query_row(&sql, param_refs.as_slice(), |row| row.get::<_, i64>(0))
                        {
                            sc.image_count = Some(count);
                        }
                    }
                }
            }
        }

        Ok(collections)
    }

    pub fn delete_smart_collection(&self, id: &str) -> Result<()> {
        let conn = self.conn.lock();
        conn.execute(
            "DELETE FROM projects WHERE id = ?1 AND collection_type = 'smart' AND is_preset = 0",
            [id],
        )?;
        Ok(())
    }

    pub fn update_smart_collection(
        &self,
        id: &str,
        name: &str,
        filter_json: &str,
        nl_query: Option<&str>,
    ) -> Result<()> {
        let conn = self.conn.lock();
        conn.execute(
            "UPDATE projects SET name = ?2, filter_json = ?3, nl_query = ?4
             WHERE id = ?1 AND collection_type = 'smart'",
            rusqlite::params![id, name, filter_json, nl_query],
        )?;
        Ok(())
    }

    pub fn evaluate_smart_collection(&self, filter_json: &str) -> Result<Vec<ImageWithFile>> {
        let filter: FilterNode = serde_json::from_str(filter_json)
            .map_err(|e| rusqlite::Error::InvalidParameterName(e.to_string()))?;

        let (where_clause, params) = filter
            .to_sql_clause()
            .map_err(|e| rusqlite::Error::InvalidParameterName(e))?;

        let conn = self.conn.lock();
        let sql = format!(
            "SELECT i.id, i.sha256_hash, i.width, i.height, i.format, i.file_size,
                    i.created_at, i.imported_at, f.path,
                    s.star_rating, s.color_label, s.decision, i.source_label, i.ai_prompt,
                    i.raw_metadata, f.missing_at
             FROM images i
             JOIN image_files f ON f.image_id = i.id AND f.missing_at IS NULL
             LEFT JOIN selections s ON s.image_id = i.id AND s.project_id = '__global__'
             WHERE ({})
             GROUP BY i.id
             ORDER BY i.imported_at DESC",
            where_clause
        );

        let param_refs: Vec<&dyn rusqlite::types::ToSql> = params
            .iter()
            .map(|p| p as &dyn rusqlite::types::ToSql)
            .collect();

        let mut stmt = conn.prepare(&sql)?;
        let rows = stmt.query_map(param_refs.as_slice(), |row| {
            let star: Option<u8> = row.get(9)?;
            let color: Option<String> = row.get(10)?;
            let decision: Option<String> = row.get(11)?;
            let selection = decision.map(|d| Selection {
                image_id: row.get(0).unwrap(),
                project_id: None,
                star_rating: star,
                color_label: color,
                decision: d,
            });
            Ok(ImageWithFile {
                image: Image {
                    id: row.get(0)?,
                    sha256_hash: row.get(1)?,
                    width: row.get(2)?,
                    height: row.get(3)?,
                    format: row.get(4)?,
                    file_size: row.get(5)?,
                    created_at: row.get(6)?,
                    imported_at: row.get(7)?,
                    ai_prompt: row.get(13)?,
                    raw_metadata: row.get(14)?,
                },
                path: row.get(8)?,
                thumbnail_path: None,
                selection,
                source_label: row.get(12)?,
                missing_at: row.get(15)?,
            })
        })?;
        rows.collect::<Result<Vec<_>>>()
    }

    pub fn update_source_detection(
        &self,
        image_id: &str,
        source_label: Option<&str>,
        source_confidence: f64,
        source_evidence_json: &str,
        is_ai_generated: Option<bool>,
        ai_prompt: Option<&str>,
        aspect_ratio: f64,
        orientation: &str,
        megapixels: f64,
    ) -> Result<()> {
        let conn = self.conn.lock();
        conn.execute(
            "UPDATE images SET source_label = ?2, source_confidence = ?3,
             source_evidence_json = ?4, is_ai_generated = ?5, ai_prompt = ?6,
             aspect_ratio = ?7, orientation = ?8, megapixels = ?9,
             source_detected_at = datetime('now'), source_detector_version = '1.0'
             WHERE id = ?1",
            params![
                image_id,
                source_label,
                source_confidence,
                source_evidence_json,
                is_ai_generated.map(|b| b as i32),
                ai_prompt,
                aspect_ratio,
                orientation,
                megapixels,
            ],
        )?;
        Ok(())
    }

    pub fn update_raw_metadata(&self, image_id: &str, metadata_json: &str) -> Result<()> {
        let conn = self.conn.lock();
        conn.execute(
            "UPDATE images SET raw_metadata = ?1 WHERE id = ?2",
            params![metadata_json, image_id],
        )?;
        Ok(())
    }

    pub fn backfill_image_metadata(&self) -> Result<u32> {
        let conn = self.conn.lock();
        let mut stmt =
            conn.prepare("SELECT id, width, height FROM images WHERE orientation IS NULL")?;
        let rows: Vec<(String, u32, u32)> = stmt
            .query_map([], |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)))?
            .filter_map(|r| r.ok())
            .collect();
        drop(stmt);

        let mut count = 0u32;
        for (id, width, height) in &rows {
            let aspect_ratio = *width as f64 / (*height).max(1) as f64;
            let orientation = if (aspect_ratio - 1.0).abs() < 0.05 {
                "square"
            } else if aspect_ratio > 1.0 {
                "landscape"
            } else {
                "portrait"
            };
            let megapixels = (*width as f64 * *height as f64) / 1_000_000.0;
            conn.execute(
                "UPDATE images SET aspect_ratio = ?2, orientation = ?3, megapixels = ?4 WHERE id = ?1",
                params![id, aspect_ratio, orientation, megapixels],
            )?;
            count += 1;
        }
        Ok(count)
    }

    pub fn update_image_dimensions(&self, image_id: &str, width: u32, height: u32) -> Result<()> {
        let conn = self.conn.lock();
        let aspect = width as f64 / height as f64;
        let orientation = if width > height {
            "landscape"
        } else if height > width {
            "portrait"
        } else {
            "square"
        };
        let megapixels = (width as f64 * height as f64) / 1_000_000.0;
        conn.execute(
            "UPDATE images SET width = ?2, height = ?3, aspect_ratio = ?4, orientation = ?5, megapixels = ?6 WHERE id = ?1",
            rusqlite::params![image_id, width, height, aspect, orientation, megapixels],
        )?;
        Ok(())
    }

    pub fn insert_generation_run(&self, run: &GenerationRun) -> Result<()> {
        let conn = self.conn.lock();
        conn.execute(
            "INSERT OR IGNORE INTO generation_runs (id, prompt, negative_prompt, provider, model, settings_json, seed, parent_run_id, source_type, source_path, raw_metadata_json, created_at, imported_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13)",
            rusqlite::params![run.id, run.prompt, run.negative_prompt, run.provider, run.model, run.settings_json, run.seed, run.parent_run_id, run.source_type, run.source_path, run.raw_metadata_json, run.created_at, run.imported_at],
        )?;
        Ok(())
    }

    pub fn link_image_to_run(&self, image_id: &str, run_id: &str) -> Result<()> {
        let conn = self.conn.lock();
        conn.execute(
            "UPDATE images SET generation_run_id = ?1 WHERE id = ?2",
            rusqlite::params![run_id, image_id],
        )?;
        Ok(())
    }

    pub fn get_generation_run_for_image(&self, image_id: &str) -> Result<Option<GenerationRun>> {
        let conn = self.conn.lock();
        let mut stmt = conn.prepare(
            "SELECT g.id, g.prompt, g.negative_prompt, g.provider, g.model, g.settings_json, g.seed, g.parent_run_id, g.source_type, g.source_path, g.raw_metadata_json, g.created_at, g.imported_at
             FROM generation_runs g
             JOIN images i ON i.generation_run_id = g.id
             WHERE i.id = ?1"
        )?;
        let run = stmt
            .query_row(rusqlite::params![image_id], |row| {
                Ok(GenerationRun {
                    id: row.get(0)?,
                    prompt: row.get(1)?,
                    negative_prompt: row.get(2)?,
                    provider: row.get(3)?,
                    model: row.get(4)?,
                    settings_json: row.get(5)?,
                    seed: row.get(6)?,
                    parent_run_id: row.get(7)?,
                    source_type: row.get(8)?,
                    source_path: row.get(9)?,
                    raw_metadata_json: row.get(10)?,
                    created_at: row.get(11)?,
                    imported_at: row.get(12)?,
                })
            })
            .optional()?;
        Ok(run)
    }

    pub fn get_images_without_generation_run(&self) -> Result<Vec<(String, String)>> {
        let conn = self.conn.lock();
        let mut stmt = conn.prepare(
            "SELECT i.id, f.path
             FROM images i
             JOIN image_files f ON f.image_id = i.id AND f.missing_at IS NULL
             WHERE i.generation_run_id IS NULL",
        )?;
        let rows = stmt.query_map([], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
        })?;
        let mut results = Vec::new();
        for row in rows {
            results.push(row?);
        }
        Ok(results)
    }

    // ---- Undo/Redo helpers ----

    pub fn get_selection_for_image(&self, image_id: &str) -> Result<Option<Selection>> {
        let conn = self.conn.lock();
        let mut stmt = conn.prepare(
            "SELECT image_id, project_id, star_rating, color_label, decision
             FROM selections WHERE image_id = ?1 AND project_id = '__global__'",
        )?;
        stmt.query_row(params![image_id], |row| {
            Ok(Selection {
                image_id: row.get(0)?,
                project_id: row.get(1)?,
                star_rating: row.get(2)?,
                color_label: row.get(3)?,
                decision: row.get(4).unwrap_or_else(|_| "undecided".to_string()),
            })
        })
        .optional()
    }

    pub fn get_undo_record_by_seq(&self, seq: i64) -> Result<Option<UndoRecord>> {
        let conn = self.conn.lock();
        let mut stmt = conn.prepare(
            "SELECT seq, id, action_type, label, before_json, after_json, affected_image_ids, group_id, has_file_backup, created_at
             FROM undo_records WHERE seq = ?1"
        )?;
        stmt.query_row(params![seq], |row| {
            Ok(UndoRecord {
                seq: row.get(0)?,
                id: row.get(1)?,
                action_type: row.get(2)?,
                label: row.get(3)?,
                before_json: row.get(4)?,
                after_json: row.get(5)?,
                affected_image_ids: row.get(6)?,
                group_id: row.get(7)?,
                has_file_backup: row.get::<_, i32>(8)? != 0,
                created_at: row.get(9)?,
            })
        })
        .optional()
    }

    pub fn get_max_undo_seq(&self) -> Result<Option<i64>> {
        let conn = self.conn.lock();
        conn.query_row("SELECT MAX(seq) FROM undo_records", [], |row| row.get(0))
    }

    pub fn count_undo_records(&self) -> Result<i64> {
        let conn = self.conn.lock();
        conn.query_row("SELECT COUNT(*) FROM undo_records", [], |row| row.get(0))
    }

    pub fn list_undo_records(&self, limit: u32) -> Result<Vec<UndoRecord>> {
        let conn = self.conn.lock();
        let mut stmt = conn.prepare(
            "SELECT seq, id, action_type, label, before_json, after_json, affected_image_ids, group_id, has_file_backup, created_at
             FROM undo_records ORDER BY seq DESC LIMIT ?1"
        )?;
        let rows = stmt.query_map(params![limit], |row| {
            Ok(UndoRecord {
                seq: row.get(0)?,
                id: row.get(1)?,
                action_type: row.get(2)?,
                label: row.get(3)?,
                before_json: row.get(4)?,
                after_json: row.get(5)?,
                affected_image_ids: row.get(6)?,
                group_id: row.get(7)?,
                has_file_backup: row.get::<_, i32>(8)? != 0,
                created_at: row.get(9)?,
            })
        })?;
        rows.collect::<Result<Vec<_>>>()
    }

    pub fn prune_oldest_undo_records(&self, keep_count: usize) -> Result<()> {
        let conn = self.conn.lock();
        conn.execute(
            "DELETE FROM undo_records WHERE seq NOT IN (
                SELECT seq FROM undo_records ORDER BY seq DESC LIMIT ?1
            )",
            params![keep_count],
        )?;
        Ok(())
    }

    fn migrate_sessions(&self) -> Result<()> {
        let conn = self.conn.lock();

        let project_columns = vec![
            ("folder_path", "TEXT"),
            ("owning_session_id", "TEXT REFERENCES projects(id)"),
            ("settings_json", "TEXT"),
        ];
        for (name, typ) in &project_columns {
            let sql = format!("ALTER TABLE projects ADD COLUMN {} {}", name, typ);
            match conn.execute(&sql, []) {
                Ok(_) => {}
                Err(e) if e.to_string().contains("duplicate column") => {}
                Err(e) => return Err(e),
            }
        }

        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS canvases (
                id TEXT PRIMARY KEY,
                session_id TEXT NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
                name TEXT NOT NULL,
                canvas_type TEXT NOT NULL DEFAULT 'manual'
                    CHECK (canvas_type IN ('manual', 'query')),
                layout_json TEXT NOT NULL DEFAULT '{}',
                filter_json TEXT,
                grid_config_json TEXT,
                sort_order INTEGER DEFAULT 0,
                created_at TEXT NOT NULL,
                updated_at TEXT NOT NULL
            );
            CREATE INDEX IF NOT EXISTS idx_canvases_session ON canvases(session_id);",
        )?;

        conn.execute_batch(
            "CREATE INDEX IF NOT EXISTS idx_collection_items_image ON collection_items(image_id);
             CREATE INDEX IF NOT EXISTS idx_selections_project ON selections(project_id);
             CREATE INDEX IF NOT EXISTS idx_embeddings_image ON embeddings(image_id);
             CREATE INDEX IF NOT EXISTS idx_images_import_batch ON images(import_batch_id);",
        )?;

        Ok(())
    }
}

fn cosine_similarity(a: &[f32], b: &[f32]) -> f32 {
    if a.len() != b.len() {
        return 0.0;
    }
    let dot: f32 = a.iter().zip(b.iter()).map(|(x, y)| x * y).sum();
    let norm_a: f32 = a.iter().map(|x| x * x).sum::<f32>().sqrt();
    let norm_b: f32 = b.iter().map(|x| x * x).sum::<f32>().sqrt();
    if norm_a == 0.0 || norm_b == 0.0 {
        0.0
    } else {
        dot / (norm_a * norm_b)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_db() -> Database {
        let db = Database::open(Path::new(":memory:")).unwrap();
        db
    }

    fn insert_test_image(db: &Database, id: &str, hash: &str) {
        let img = Image {
            id: id.to_string(),
            sha256_hash: hash.to_string(),
            width: 100,
            height: 100,
            format: "png".to_string(),
            file_size: 1024,
            created_at: "2026-05-07T00:00:00Z".to_string(),
            imported_at: "2026-05-07T00:00:00Z".to_string(),
            ai_prompt: None,
            raw_metadata: None,
        };
        db.insert_image(&img).unwrap();
        let file = ImageFile {
            id: format!("f-{}", id),
            image_id: id.to_string(),
            path: format!("/tmp/{}.png", id),
            last_seen_at: "2026-05-07T00:00:00Z".to_string(),
            missing_at: None,
            last_seen_size: None,
            last_seen_mtime: None,
        };
        db.insert_image_file(&file).unwrap();
    }

    #[test]
    fn test_get_images_by_ids_returns_matching_images() {
        let db = test_db();
        insert_test_image(&db, "img-1", "hash-1");
        insert_test_image(&db, "img-2", "hash-2");
        insert_test_image(&db, "img-3", "hash-3");

        let results = db.get_images_by_ids(&["img-1", "img-3"]).unwrap();
        assert_eq!(results.len(), 2);
        let ids: Vec<&str> = results.iter().map(|r| r.image.id.as_str()).collect();
        assert!(ids.contains(&"img-1"));
        assert!(ids.contains(&"img-3"));
    }

    #[test]
    fn test_get_images_by_ids_returns_empty_for_no_match() {
        let db = test_db();
        insert_test_image(&db, "img-1", "hash-1");

        let results = db.get_images_by_ids(&["nonexistent"]).unwrap();
        assert_eq!(results.len(), 0);
    }

    #[test]
    fn test_get_images_by_ids_includes_selection_data() {
        let db = test_db();
        insert_test_image(&db, "img-1", "hash-1");
        db.set_rating("img-1", 4).unwrap();

        let results = db.get_images_by_ids(&["img-1"]).unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].selection.as_ref().unwrap().star_rating, Some(4));
    }

    #[test]
    fn test_get_iteration_siblings_returns_children() {
        let db = test_db();
        insert_test_image(&db, "parent", "hash-p");
        insert_test_image(&db, "child-1", "hash-c1");
        insert_test_image(&db, "child-2", "hash-c2");

        // Insert iteration records
        let conn = db.conn.lock();
        conn.execute(
            "INSERT INTO iterations (id, parent_id, child_id, prompt, model_used, created_at)
             VALUES ('it-1', 'parent', 'child-1', 'make it blue', 'flux', '2026-05-07T00:00:00Z')",
            [],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO iterations (id, parent_id, child_id, prompt, model_used, created_at)
             VALUES ('it-2', 'parent', 'child-2', 'make it red', 'flux', '2026-05-07T00:00:00Z')",
            [],
        )
        .unwrap();
        drop(conn);

        let results = db.get_iteration_siblings("parent").unwrap();
        assert_eq!(results.len(), 2);
    }

    #[test]
    fn test_update_image_dimensions() {
        let db = test_db();
        insert_test_image(&db, "img-1", "hash-1");

        db.update_image_dimensions("img-1", 500, 300).unwrap();

        let images = db.get_images_by_ids(&["img-1"]).unwrap();
        assert_eq!(images[0].image.width, 500);
        assert_eq!(images[0].image.height, 300);
    }

    #[test]
    fn test_update_image_dimensions_nonexistent() {
        let db = test_db();
        // Should not error, just affect 0 rows
        let result = db.update_image_dimensions("nonexistent", 100, 100);
        assert!(result.is_ok());
    }

    #[test]
    fn test_model_processing_migration_creates_tables_and_indexes() {
        let db = test_db();
        let conn = db.conn.lock();
        let tables: Vec<String> = conn
            .prepare("SELECT name FROM sqlite_master WHERE type='table'")
            .unwrap()
            .query_map([], |row| row.get(0))
            .unwrap()
            .filter_map(|r| r.ok())
            .collect();
        assert!(tables.contains(&"model_profiles".to_string()));
        assert!(tables.contains(&"model_runs".to_string()));
        assert!(tables.contains(&"model_run_items".to_string()));

        let stmt = conn
            .prepare("SELECT model_run_id FROM embeddings LIMIT 0")
            .unwrap();
        drop(stmt);

        let indexes: Vec<String> = conn
            .prepare("SELECT name FROM sqlite_master WHERE type='index'")
            .unwrap()
            .query_map([], |row| row.get(0))
            .unwrap()
            .filter_map(|r| r.ok())
            .collect();
        assert!(indexes.contains(&"embeddings_model_run_idx".to_string()));
        assert!(indexes.contains(&"model_run_items_run_status_idx".to_string()));
    }

    #[test]
    fn test_model_run_item_and_embedding_link_round_trip() {
        let db = test_db();
        insert_test_image(&db, "img-1", "hash-1");
        let now = "2026-05-13T00:00:00Z".to_string();
        let run = NewModelRun {
            id: "mr-test".to_string(),
            job_id: Some("job-test".to_string()),
            parent_run_id: None,
            profile_id: None,
            task: "embedding".to_string(),
            provider: "local".to_string(),
            model_id: "clip-vit-b32".to_string(),
            model_revision: None,
            status: "running".to_string(),
            input_scope_json: "{\"type\":\"image_ids\",\"image_ids\":[\"img-1\"]}".to_string(),
            params_json: "{\"runtime\":\"onnx\"}".to_string(),
            output_summary_json: "{}".to_string(),
            cost_estimate_usd: None,
            cost_actual_usd: None,
            error: None,
            created_at: now.clone(),
            started_at: Some(now.clone()),
            completed_at: None,
        };
        db.insert_model_run(&run).unwrap();

        let embedding_id = db
            .store_embedding_with_model_run(
                "img-1",
                "clip-vit-b32",
                &[0.1, 0.2, 0.3],
                Some("mr-test"),
            )
            .unwrap();
        db.insert_model_run_item(&NewModelRunItem {
            id: "mri-test".to_string(),
            run_id: "mr-test".to_string(),
            image_id: Some("img-1".to_string()),
            input_asset_uri: "cull://images/img-1/ml-input".to_string(),
            input_hash: Some("hash-1".to_string()),
            status: "completed".to_string(),
            output_ref_kind: Some("embedding".to_string()),
            output_ref_id: Some(embedding_id.clone()),
            audit_payload_json: None,
            cost_usd: None,
            attempt_count: 1,
            error: None,
            started_at: Some(now.clone()),
            completed_at: Some(now),
        })
        .unwrap();
        db.update_model_run_terminal(
            "mr-test",
            "completed",
            "{\"generated\":1,\"failed\":0,\"total\":1}",
            None,
        )
        .unwrap();

        let loaded = db.get_model_run("mr-test").unwrap().unwrap();
        assert_eq!(loaded.status, "completed");
        assert!(loaded.output_summary_json.contains("\"generated\":1"));

        let conn = db.conn.lock();
        let linked_run: String = conn
            .query_row(
                "SELECT model_run_id FROM embeddings WHERE id = ?1",
                params![embedding_id],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(linked_run, "mr-test");
        let item_count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM model_run_items WHERE run_id = 'mr-test'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(item_count, 1);
    }
}

#[cfg(test)]
mod session_tests {
    use super::*;

    #[test]
    fn test_session_migration_creates_canvases_table() {
        let db = Database::open(std::path::Path::new(":memory:")).unwrap();
        let conn = db.conn.lock();
        let count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name='canvases'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(count, 1, "canvases table should exist after migration");
    }

    #[test]
    fn test_session_migration_adds_project_columns() {
        let db = Database::open(std::path::Path::new(":memory:")).unwrap();
        let conn = db.conn.lock();
        let mut stmt = conn
            .prepare("SELECT folder_path FROM projects LIMIT 0")
            .unwrap();
        drop(stmt);
        stmt = conn
            .prepare("SELECT owning_session_id FROM projects LIMIT 0")
            .unwrap();
        drop(stmt);
        stmt = conn
            .prepare("SELECT settings_json FROM projects LIMIT 0")
            .unwrap();
        drop(stmt);
    }

    #[test]
    fn test_session_indexes_exist() {
        let db = Database::open(std::path::Path::new(":memory:")).unwrap();
        let conn = db.conn.lock();
        let indexes: Vec<String> = conn
            .prepare("SELECT name FROM sqlite_master WHERE type='index'")
            .unwrap()
            .query_map([], |row| row.get(0))
            .unwrap()
            .filter_map(|r| r.ok())
            .collect();
        assert!(indexes.contains(&"idx_canvases_session".to_string()));
        assert!(indexes.contains(&"idx_collection_items_image".to_string()));
        assert!(indexes.contains(&"idx_selections_project".to_string()));
        assert!(indexes.contains(&"idx_embeddings_image".to_string()));
        assert!(indexes.contains(&"idx_images_import_batch".to_string()));
    }
}

#[cfg(test)]
mod file_watcher_tests {
    use super::*;

    fn test_db() -> Database {
        Database::open(std::path::Path::new(":memory:")).unwrap()
    }

    fn insert_test_image(db: &Database, id: &str, hash: &str) {
        let img = Image {
            id: id.to_string(),
            sha256_hash: hash.to_string(),
            width: 100,
            height: 100,
            format: "png".to_string(),
            file_size: 1000,
            created_at: "2026-05-07T00:00:00Z".to_string(),
            imported_at: "2026-05-07T00:00:00Z".to_string(),
            ai_prompt: None,
            raw_metadata: None,
        };
        db.insert_image(&img).unwrap();
        let file = ImageFile {
            id: format!("f-{}", id),
            image_id: id.to_string(),
            path: format!("/tmp/{}.png", id),
            last_seen_at: "2026-05-07T00:00:00Z".to_string(),
            missing_at: None,
            last_seen_size: None,
            last_seen_mtime: None,
        };
        db.insert_image_file(&file).unwrap();
    }

    // -- mark_file_missing --

    #[test]
    fn test_mark_file_missing_sets_timestamp() {
        let db = test_db();
        insert_test_image(&db, "img-1", "hash-1");

        let result = db.mark_file_missing("/tmp/img-1.png").unwrap();
        assert!(result);

        let images = db.list_images(100, 0).unwrap();
        assert_eq!(
            images.len(),
            0,
            "missing image should be excluded from list_images"
        );
    }

    #[test]
    fn test_mark_file_missing_returns_false_for_unknown_path() {
        let db = test_db();
        let result = db.mark_file_missing("/nonexistent/path.png").unwrap();
        assert!(!result);
    }

    #[test]
    fn test_mark_file_missing_idempotent() {
        let db = test_db();
        insert_test_image(&db, "img-1", "hash-1");

        assert!(db.mark_file_missing("/tmp/img-1.png").unwrap());
        assert!(
            !db.mark_file_missing("/tmp/img-1.png").unwrap(),
            "second call should return false"
        );
    }

    // -- restore_file --

    #[test]
    fn test_restore_file_clears_missing() {
        let db = test_db();
        insert_test_image(&db, "img-1", "hash-1");
        db.mark_file_missing("/tmp/img-1.png").unwrap();

        assert_eq!(db.list_images(100, 0).unwrap().len(), 0);

        let restored = db.restore_file("/tmp/img-1.png").unwrap();
        assert!(restored);

        let images = db.list_images(100, 0).unwrap();
        assert_eq!(images.len(), 1, "restored image should reappear");
    }

    #[test]
    fn test_restore_file_unknown_path() {
        let db = test_db();
        let result = db.restore_file("/nonexistent/path.png").unwrap();
        assert!(!result);
    }

    // -- update_image_file_path --

    #[test]
    fn test_update_image_file_path() {
        let db = test_db();
        insert_test_image(&db, "img-1", "hash-1");

        db.update_image_file_path("f-img-1", "/new/location/img-1.png")
            .unwrap();

        let images = db.list_images(100, 0).unwrap();
        assert_eq!(images.len(), 1);
        assert_eq!(images[0].path, "/new/location/img-1.png");
    }

    #[test]
    fn test_update_image_file_path_clears_missing() {
        let db = test_db();
        insert_test_image(&db, "img-1", "hash-1");
        db.mark_file_missing("/tmp/img-1.png").unwrap();
        assert_eq!(db.list_images(100, 0).unwrap().len(), 0);

        db.update_image_file_path("f-img-1", "/new/img-1.png")
            .unwrap();

        let images = db.list_images(100, 0).unwrap();
        assert_eq!(images.len(), 1, "path update should clear missing_at");
        assert_eq!(images[0].path, "/new/img-1.png");
    }

    // -- restore_or_move_file_by_hash --

    #[test]
    fn test_restore_by_hash_moves_missing_file() {
        let db = test_db();
        insert_test_image(&db, "img-1", "hash-abc");
        db.mark_file_missing("/tmp/img-1.png").unwrap();

        let moved = db
            .restore_or_move_file_by_hash("hash-abc", "/new/path.png")
            .unwrap();
        assert!(moved);

        let images = db.list_images(100, 0).unwrap();
        assert_eq!(images.len(), 1);
        assert_eq!(images[0].path, "/new/path.png");
        assert!(images[0].missing_at.is_none());
    }

    #[test]
    fn test_restore_by_hash_returns_false_no_match() {
        let db = test_db();
        let result = db
            .restore_or_move_file_by_hash("unknown-hash", "/some/path.png")
            .unwrap();
        assert!(!result);
    }

    #[test]
    fn test_restore_by_hash_ignores_non_missing() {
        let db = test_db();
        insert_test_image(&db, "img-1", "hash-abc");
        // Not marked missing
        let result = db
            .restore_or_move_file_by_hash("hash-abc", "/new/path.png")
            .unwrap();
        assert!(!result, "should not operate on non-missing files");
    }

    // -- image_count with missing --

    #[test]
    fn test_image_count_excludes_missing() {
        let db = test_db();
        insert_test_image(&db, "img-1", "hash-1");
        insert_test_image(&db, "img-2", "hash-2");
        insert_test_image(&db, "img-3", "hash-3");

        assert_eq!(db.image_count().unwrap(), 3);

        db.mark_file_missing("/tmp/img-2.png").unwrap();

        assert_eq!(db.image_count().unwrap(), 2);
    }

    // -- library_roots --

    #[test]
    fn test_add_and_list_library_roots() {
        let db = test_db();
        db.add_library_root("/photos/vacation").unwrap();
        db.add_library_root("/photos/work").unwrap();

        let roots = db.list_library_roots().unwrap();
        assert_eq!(roots.len(), 2);
        assert!(roots.contains(&"/photos/vacation".to_string()));
        assert!(roots.contains(&"/photos/work".to_string()));
    }

    #[test]
    fn test_add_library_root_idempotent() {
        let db = test_db();
        db.add_library_root("/photos/vacation").unwrap();
        db.add_library_root("/photos/vacation").unwrap();

        let roots = db.list_library_roots().unwrap();
        assert_eq!(roots.len(), 1);
    }

    #[test]
    fn test_remove_library_root() {
        let db = test_db();
        db.add_library_root("/photos/vacation").unwrap();
        assert_eq!(db.list_library_roots().unwrap().len(), 1);

        let removed = db.remove_library_root("/photos/vacation").unwrap();
        assert!(removed);

        let roots = db.list_library_roots().unwrap();
        assert!(roots.is_empty());
    }

    // -- get_image_file_by_path --

    #[test]
    fn test_get_image_file_by_path_found() {
        let db = test_db();
        insert_test_image(&db, "img-1", "hash-1");

        let file = db.get_image_file_by_path("/tmp/img-1.png").unwrap();
        assert!(file.is_some());
        let f = file.unwrap();
        assert_eq!(f.image_id, "img-1");
        assert_eq!(f.path, "/tmp/img-1.png");
        assert!(f.missing_at.is_none());
    }

    #[test]
    fn test_get_image_file_by_path_not_found() {
        let db = test_db();
        let file = db.get_image_file_by_path("/nonexistent/path.png").unwrap();
        assert!(file.is_none());
    }
}
