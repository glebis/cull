// Copyright (c) 2026-present Gleb Kalinin. Architecture and design by author.
// Implementation assisted by Claude (Anthropic). See AUTHORSHIP.md.

use super::models::*;
use parking_lot::Mutex;
use rusqlite::{ffi, params, Connection, Error as SqlError, Result};
use std::path::{Path, PathBuf};
use std::sync::Arc;

const CURRENT_SCHEMA_VERSION: i64 = 25;

const MIGRATIONS: &[(i64, &str)] = &[
    (1, "initial_schema"),
    (2, "reserved_schema_history_2"),
    (3, "reserved_schema_history_3"),
    (4, "reserved_schema_history_4"),
    (5, "reserved_schema_history_5"),
    (6, "reserved_schema_history_6"),
    (7, "reserved_schema_history_7"),
    (8, "reserved_schema_history_8"),
    (9, "reserved_schema_history_9"),
    (10, "reserved_schema_history_10"),
    (11, "reserved_schema_history_11"),
    (12, "reserved_schema_history_12"),
    (13, "reserved_schema_history_13"),
    (14, "reserved_schema_history_14"),
    (15, "reserved_schema_history_15"),
    (16, "reserved_schema_history_16"),
    (17, "reserved_schema_history_17"),
    (18, "reserved_schema_history_18"),
    (19, "reserved_schema_history_19"),
    (20, "reserved_schema_history_20"),
    (21, "reserved_schema_history_21"),
    (22, "reserved_schema_history_22"),
    (23, "reserved_schema_history_23"),
    (24, "reserved_schema_history_24"),
    (25, "agent_action_proposals"),
];

#[derive(Clone)]
pub struct Database {
    pub(crate) conn: Arc<Mutex<Connection>>,
}

/// Map a row from the standard image+file+selection SELECT (the 16-column
/// projection used by `list_images` and `list_images_in_scope`) into an
/// `ImageWithFile`. Shared so the two list queries cannot drift apart.
pub(crate) fn map_image_with_file_row(row: &rusqlite::Row) -> rusqlite::Result<ImageWithFile> {
    let star: Option<u8> = row.get(9)?;
    let color: Option<String> = row.get(10)?;
    let decision: Option<String> = row.get(11)?;
    let selection = Selection::from_nullable_parts(row.get(0)?, None, star, color, decision);
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
}

pub(crate) fn map_media_asset_row(row: &rusqlite::Row) -> rusqlite::Result<MediaAsset> {
    Ok(MediaAsset {
        id: row.get(0)?,
        media_type: row.get(1)?,
        primary_image_id: row.get(2)?,
        sha256_hash: row.get(3)?,
        format: row.get(4)?,
        file_size: row.get(5)?,
        page_count: row.get(6)?,
        title: row.get(7)?,
        created_at: row.get(8)?,
        imported_at: row.get(9)?,
    })
}

pub(crate) fn map_media_file_row(row: &rusqlite::Row) -> rusqlite::Result<MediaFile> {
    Ok(MediaFile {
        id: row.get(0)?,
        media_asset_id: row.get(1)?,
        path: row.get(2)?,
        last_seen_at: row.get(3)?,
        missing_at: row.get(4)?,
        last_seen_size: row.get(5)?,
        last_seen_mtime: row.get(6)?,
    })
}

pub(crate) fn map_pdf_page_row(row: &rusqlite::Row) -> rusqlite::Result<PdfPage> {
    Ok(PdfPage {
        id: row.get(0)?,
        media_asset_id: row.get(1)?,
        page_index: row.get(2)?,
        width_points: row.get(3)?,
        height_points: row.get(4)?,
        thumbnail_path: row.get(5)?,
        preview_path: row.get(6)?,
        extracted_text: row.get(7)?,
        text_extracted_at: row.get(8)?,
    })
}

pub(crate) fn map_catalog_work_row(row: &rusqlite::Row) -> rusqlite::Result<CatalogWork> {
    Ok(CatalogWork {
        id: row.get(0)?,
        primary_image_id: row.get(1)?,
        created_at: row.get(2)?,
        updated_at: row.get(3)?,
        deleted_at: row.get(4)?,
    })
}

pub(crate) fn map_catalog_work_image_row(
    row: &rusqlite::Row,
) -> rusqlite::Result<CatalogWorkImage> {
    Ok(CatalogWorkImage {
        id: row.get(0)?,
        work_id: row.get(1)?,
        image_id: row.get(2)?,
        role: row.get(3)?,
        ordinal: row.get(4)?,
        edition_label: row.get(5)?,
        created_at: row.get(6)?,
    })
}

pub(crate) fn map_catalog_field_def_row(row: &rusqlite::Row) -> rusqlite::Result<CatalogFieldDef> {
    Ok(CatalogFieldDef {
        id: row.get(0)?,
        stable_key: row.get(1)?,
        label: row.get(2)?,
        description: row.get(3)?,
        subject_scope: row.get(4)?,
        value_type: row.get(5)?,
        cardinality: row.get(6)?,
        unit_kind: row.get(7)?,
        validation_json: row.get(8)?,
        sensitivity: row.get(9)?,
        derived_source: row.get(10)?,
        crosswalk_json: row.get(11)?,
        version: row.get(12)?,
        supersedes_field_def_id: row.get(13)?,
        created_at: row.get(14)?,
        deprecated_at: row.get(15)?,
    })
}

pub(crate) fn map_catalog_preset_row(row: &rusqlite::Row) -> rusqlite::Result<CatalogPreset> {
    Ok(CatalogPreset {
        id: row.get(0)?,
        name: row.get(1)?,
        description: row.get(2)?,
        preset_kind: row.get(3)?,
        field_def_ids_json: row.get(4)?,
        layout_json: row.get(5)?,
        version: row.get(6)?,
        created_at: row.get(7)?,
        updated_at: row.get(8)?,
    })
}

pub(crate) fn map_catalog_field_value_row(
    row: &rusqlite::Row,
) -> rusqlite::Result<CatalogFieldValue> {
    Ok(CatalogFieldValue {
        id: row.get(0)?,
        subject_type: row.get(1)?,
        subject_id: row.get(2)?,
        field_def_id: row.get(3)?,
        value_json: row.get(4)?,
        display_value: row.get(5)?,
        source_type: row.get(6)?,
        source_id: row.get(7)?,
        confidence: row.get(8)?,
        status: row.get(9)?,
        approved_by: row.get(10)?,
        approved_at: row.get(11)?,
        created_at: row.get(12)?,
        updated_at: row.get(13)?,
    })
}

pub(crate) fn map_catalog_value_event_row(
    row: &rusqlite::Row,
) -> rusqlite::Result<CatalogValueEvent> {
    Ok(CatalogValueEvent {
        id: row.get(0)?,
        value_id: row.get(1)?,
        event_type: row.get(2)?,
        actor_type: row.get(3)?,
        actor_id: row.get(4)?,
        before_json: row.get(5)?,
        after_json: row.get(6)?,
        created_at: row.get(7)?,
    })
}

fn catalog_field_def_id(stable_key: &str) -> String {
    let sanitized = stable_key
        .chars()
        .map(|ch| if ch.is_ascii_alphanumeric() { ch } else { '_' })
        .collect::<String>();
    format!("cfd_{}", sanitized)
}

pub(crate) fn catalog_work_image_id(work_id: &str, image_id: &str, role: &str) -> String {
    format!(
        "cwi_{}",
        format!("{work_id}_{image_id}_{role}").replace('-', "")
    )
}

impl Database {
    pub fn open(db_path: &Path) -> Result<Self> {
        let should_consider_backup = should_consider_migration_backup(db_path);
        let conn = Connection::open(db_path)?;
        Self::configure_connection(&conn)?;
        let db = Database {
            conn: Arc::new(Mutex::new(conn)),
        };
        db.preflight_migrations(db_path, should_consider_backup)?;
        db.run_migrations()?;
        Ok(db)
    }

    fn configure_connection(conn: &Connection) -> Result<()> {
        conn.pragma_update(None, "foreign_keys", true)?;
        conn.pragma_update(None, "journal_mode", "WAL")?;
        conn.pragma_update(None, "synchronous", "NORMAL")?;
        conn.pragma_update(None, "busy_timeout", 5000)?;
        Ok(())
    }

    fn preflight_migrations(&self, db_path: &Path, should_consider_backup: bool) -> Result<()> {
        let (user_version, needs_backup) = {
            let conn = self.conn.lock();
            let user_version = user_version(&conn)?;

            if should_consider_backup {
                integrity_check(&conn)?;
            }

            let has_migration_history = table_exists(&conn, "schema_migrations")?;
            let needs_backup = should_consider_backup
                && (user_version < CURRENT_SCHEMA_VERSION || !has_migration_history);
            (user_version, needs_backup)
        };

        if needs_backup {
            self.create_pre_migration_backup(db_path, user_version)?;
        }

        Ok(())
    }

    fn create_pre_migration_backup(&self, db_path: &Path, from_version: i64) -> Result<()> {
        let backup_path = next_migration_backup_path(db_path, from_version)?;
        if let Some(parent) = backup_path.parent() {
            std::fs::create_dir_all(parent).map_err(|e| {
                migration_error(format!(
                    "failed to create migration backup directory {}: {}",
                    parent.display(),
                    e
                ))
            })?;
        }

        let backup_path = backup_path.to_string_lossy().to_string();
        let conn = self.conn.lock();
        conn.execute("VACUUM main INTO ?1", params![backup_path])?;
        Ok(())
    }

    fn run_migrations(&self) -> Result<()> {
        debug_assert_eq!(
            Some(CURRENT_SCHEMA_VERSION),
            MIGRATIONS.last().map(|(version, _)| *version)
        );

        self.ensure_migration_state_tables()?;
        let schema = include_str!("schema.sql");
        self.run_migration_step(1, "initial_schema", || {
            let conn = self.conn.lock();
            conn.execute_batch(schema)?;
            drop(conn);
            self.seed_preset_collections()?;
            {
                let mut conn = self.conn.lock();
                self.seed_catalog_defaults(&mut conn)?;
            }
            Ok(())
        })?;
        for (version, name) in MIGRATIONS
            .iter()
            .copied()
            .filter(|(version, _)| (2..25).contains(version))
        {
            self.run_migration_step(version, name, || Ok(()))?;
        }
        self.run_migration_step(25, "agent_action_proposals", || {
            let conn = self.conn.lock();
            conn.execute_batch(agent_action_proposals_schema())?;
            drop(conn);
            self.seed_agent_selection_presets()?;
            Ok(())
        })?;

        self.verify_schema_invariants()?;
        Ok(())
    }

    /// Test-only hook: lets integration tests (which compile against the lib
    /// without `cfg(test)`) exercise the private schema-invariant check. Enabled
    /// via the `test-support` Cargo feature. See `tests/compat_golden.rs`.
    #[cfg(feature = "test-support")]
    pub fn verify_schema_invariants_for_test(&self) -> Result<()> {
        self.verify_schema_invariants()
    }

    /// Required tables that must exist after migrations complete. Missing any of
    /// these indicates a partially-migrated/corrupt database despite a high
    /// user_version.
    fn verify_schema_invariants(&self) -> Result<()> {
        const REQUIRED_TABLES: &[&str] = &[
            "images",
            "image_files",
            "media_assets",
            "media_files",
            "pdf_pages",
            "catalog_works",
            "catalog_work_images",
            "catalog_field_defs",
            "catalog_presets",
            "catalog_field_values",
            "catalog_value_events",
            "selections",
            "projects",
            "collection_items",
            "tags",
            "image_tags",
            "generation_runs",
            "agent_action_proposals",
            "agent_selection_presets",
            "schema_migrations",
        ];
        let conn = self.conn.lock();
        let mut missing = Vec::new();
        for table in REQUIRED_TABLES {
            if !table_exists(&conn, table)? {
                missing.push(*table);
            }
        }
        if !missing.is_empty() {
            return Err(migration_error(format!(
                "database failed schema invariant check (user_version={} claims migrations applied) \
                 but required tables are missing: {}. The database may be partially migrated or corrupt.",
                user_version(&conn)?,
                missing.join(", ")
            )));
        }
        Ok(())
    }

    fn run_migration_step<F>(&self, version: i64, name: &str, apply: F) -> Result<()>
    where
        F: FnOnce() -> Result<()>,
    {
        self.ensure_migration_state_tables()?;
        if self.migration_already_applied(version)? {
            self.record_migration_step_succeeded(version, name)?;
            return Ok(());
        }

        self.record_migration_step_started(version, name)?;
        {
            let conn = self.conn.lock();
            conn.execute_batch("BEGIN IMMEDIATE")?;
        }

        let result = apply()
            .and_then(|_| self.record_migration(version, name))
            .and_then(|_| self.set_schema_version(version));

        match result {
            Ok(()) => {
                {
                    let conn = self.conn.lock();
                    conn.execute_batch("COMMIT")?;
                }
                self.record_migration_step_succeeded(version, name)?;
                Ok(())
            }
            Err(err) => {
                {
                    let conn = self.conn.lock();
                    let _ = conn.execute_batch("ROLLBACK");
                }
                self.record_migration_step_failed(version, name, &err.to_string())?;
                Err(err)
            }
        }
    }

    fn migration_already_applied(&self, version: i64) -> Result<bool> {
        let conn = self.conn.lock();
        Ok(user_version(&conn)? >= version)
    }

    fn ensure_migration_state_tables(&self) -> Result<()> {
        let conn = self.conn.lock();
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS schema_migrations (
                version INTEGER PRIMARY KEY,
                name TEXT NOT NULL,
                applied_at TEXT NOT NULL,
                checksum TEXT NOT NULL
            );
            CREATE TABLE IF NOT EXISTS schema_migration_steps (
                version INTEGER PRIMARY KEY,
                name TEXT NOT NULL,
                status TEXT NOT NULL CHECK (status IN ('started', 'succeeded', 'failed')),
                started_at TEXT NOT NULL,
                finished_at TEXT,
                error TEXT
            );",
        )?;
        Ok(())
    }

    fn record_migration_step_started(&self, version: i64, name: &str) -> Result<()> {
        let now = chrono::Utc::now().to_rfc3339();
        let conn = self.conn.lock();
        conn.execute(
            "INSERT INTO schema_migration_steps (version, name, status, started_at, finished_at, error)
             VALUES (?1, ?2, 'started', ?3, NULL, NULL)
             ON CONFLICT(version) DO UPDATE SET
                name = excluded.name,
                status = 'started',
                started_at = excluded.started_at,
                finished_at = NULL,
                error = NULL",
            params![version, name, now],
        )?;
        Ok(())
    }

    fn record_migration_step_succeeded(&self, version: i64, name: &str) -> Result<()> {
        let now = chrono::Utc::now().to_rfc3339();
        let conn = self.conn.lock();
        conn.execute(
            "INSERT INTO schema_migration_steps (version, name, status, started_at, finished_at, error)
             VALUES (?1, ?2, 'succeeded', ?3, ?3, NULL)
             ON CONFLICT(version) DO UPDATE SET
                name = excluded.name,
                status = 'succeeded',
                finished_at = excluded.finished_at,
                error = NULL",
            params![version, name, now],
        )?;
        Ok(())
    }

    fn record_migration_step_failed(&self, version: i64, name: &str, error: &str) -> Result<()> {
        let now = chrono::Utc::now().to_rfc3339();
        let conn = self.conn.lock();
        conn.execute(
            "INSERT INTO schema_migration_steps (version, name, status, started_at, finished_at, error)
             VALUES (?1, ?2, 'failed', ?3, ?3, ?4)
             ON CONFLICT(version) DO UPDATE SET
                name = excluded.name,
                status = 'failed',
                finished_at = excluded.finished_at,
                error = excluded.error",
            params![version, name, now, error],
        )?;
        Ok(())
    }

    fn record_migration(&self, version: i64, name: &str) -> Result<()> {
        let now = chrono::Utc::now().to_rfc3339();
        let checksum = migration_checksum(version, name);
        let conn = self.conn.lock();
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS schema_migrations (
                version INTEGER PRIMARY KEY,
                name TEXT NOT NULL,
                applied_at TEXT NOT NULL,
                checksum TEXT NOT NULL
            );",
        )?;
        conn.execute(
            "INSERT OR IGNORE INTO schema_migrations (version, name, applied_at, checksum)
             VALUES (?1, ?2, ?3, ?4)",
            params![version, name, now, checksum],
        )?;
        Ok(())
    }

    fn set_schema_version(&self, version: i64) -> Result<()> {
        let conn = self.conn.lock();
        conn.pragma_update(None, "user_version", version)?;
        Ok(())
    }

    // Client feedback is intentionally separate from `selections` so client
    // favorites/comments never overwrite curator ratings or decisions.

    // Per-plugin capability grants for the plugin runtime. A plugin is a
    // locally-installed actor with a capability set — exactly what an MCP
    // token is — so grants store the same capability vocabulary.

    fn seed_catalog_defaults(&self, conn: &mut Connection) -> Result<()> {
        let catalog_fields: Vec<(
            &str,
            &str,
            Option<&str>,
            &str,
            &str,
            &str,
            Option<&str>,
            &str,
            Option<&str>,
        )> = vec![
            (
                "inventory.height",
                "Height",
                Some("Artwork height"),
                "work",
                "dimension",
                "single",
                Some("cm"),
                "normal",
                None,
            ),
            (
                "inventory.depth",
                "Depth",
                Some("Artwork depth"),
                "work",
                "dimension",
                "single",
                Some("cm"),
                "normal",
                None,
            ),
            (
                "inventory.materials",
                "Materials",
                Some("Primary materials"),
                "work",
                "text",
                "multi",
                None,
                "normal",
                None,
            ),
            (
                "inventory.year",
                "Year",
                Some("Creation year"),
                "work",
                "integer",
                "single",
                None,
                "normal",
                None,
            ),
            (
                "inventory.name",
                "Name",
                Some("Work title"),
                "both",
                "text",
                "single",
                None,
                "normal",
                None,
            ),
            (
                "inventory.series",
                "Series",
                Some("Series name"),
                "both",
                "text",
                "single",
                None,
                "normal",
                None,
            ),
            (
                "inventory.description",
                "Description",
                Some("Artist notes and public description"),
                "both",
                "long_text",
                "single",
                None,
                "normal",
                None,
            ),
            (
                "inventory.weight",
                "Weight",
                Some("Weight"),
                "work",
                "number",
                "single",
                Some("kg"),
                "normal",
                None,
            ),
            (
                "inventory.price",
                "Price",
                Some("Price"),
                "work",
                "money",
                "single",
                Some("USD"),
                "normal",
                None,
            ),
        ];

        let now = chrono::Utc::now().to_rfc3339();
        for (
            stable_key,
            label,
            description,
            subject_scope,
            value_type,
            cardinality,
            unit_kind,
            sensitivity,
            supersedes,
        ) in catalog_fields
        {
            let existing: i64 = conn.query_row(
                "SELECT COUNT(*) FROM catalog_field_defs WHERE stable_key = ?1",
                params![stable_key],
                |row| row.get(0),
            )?;
            if existing == 1 {
                continue;
            }

            let id = format!("cfd_{}", uuid::Uuid::new_v4().to_string().replace('-', ""));
            conn.execute(
                "INSERT INTO catalog_field_defs (
                    id, stable_key, label, description, subject_scope, value_type, cardinality,
                    unit_kind, validation_json, sensitivity, derived_source, crosswalk_json,
                    version, supersedes_field_def_id, created_at, deprecated_at
                ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, NULL, ?9, NULL, NULL, 1, ?10, ?11, NULL)",
                params![
                    id,
                    stable_key,
                    label,
                    description,
                    subject_scope,
                    value_type,
                    cardinality,
                    unit_kind,
                    sensitivity,
                    supersedes,
                    now
                ],
            )?;
        }

        let existing_presets: i64 =
            conn.query_row("SELECT COUNT(*) FROM catalog_presets", [], |row| row.get(0))?;
        if existing_presets == 0 {
            let all_field_ids: Vec<String> = conn
                .prepare("SELECT id FROM catalog_field_defs WHERE stable_key LIKE 'inventory.%' ORDER BY stable_key")?
                .query_map([], |row| row.get::<_, String>(0))?
                .collect::<Result<Vec<_>>>()?;
            let field_ids_json =
                serde_json::to_string(&all_field_ids).unwrap_or_else(|_| "[]".to_string());
            conn.execute(
                "INSERT INTO catalog_presets (
                    id, name, description, preset_kind, field_def_ids_json, layout_json, version, created_at, updated_at
                ) VALUES (
                    ?1, 'Artist Inventory', 'Default artist-inventory profile', 'artist_inventory',
                    ?2, NULL, 1, ?3, ?3
                )",
                params![uuid::Uuid::new_v4().to_string().replace('-', ""), field_ids_json, now],
            )?;
        }

        Ok(())
    }

    /// Replace the full grant set for a plugin (install-time consent writes
    /// the manifest permissions; re-install replaces them).
    /// Upsert client feedback for an image. Passing `favorite=false` and an
    /// empty/None comment leaves a cleared-but-present row, which is harmless.
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

    fn seed_agent_selection_presets(&self) -> Result<()> {
        let conn = self.conn.lock();
        let existing: i64 =
            conn.query_row("SELECT COUNT(*) FROM agent_selection_presets", [], |row| {
                row.get(0)
            })?;
        if existing > 0 {
            return Ok(());
        }

        let now = chrono::Utc::now().to_rfc3339();
        let presets = [
            (
                "selpreset_portfolio",
                "Portfolio edit",
                "portfolio",
                "Select the strongest coherent set for a portfolio page. Prefer finished work, visual range, and images that can stand alone.",
                r#"{"prefer":["finished_work","visual_range","standalone_strength"],"avoid":["near_duplicate","weak_focus","test_render"]}"#,
                10_i64,
            ),
            (
                "selpreset_client_review",
                "Client review",
                "client_review",
                "Select a concise review set for a client. Prefer variety and clear choices, avoid confusing near duplicates.",
                r#"{"prefer":["variety","clear_choice","representative_options"],"avoid":["confusing_duplicates","technical_failures"]}"#,
                20_i64,
            ),
            (
                "selpreset_print_shortlist",
                "Print shortlist",
                "print",
                "Select images likely to print well. Prefer sharpness, clean edges, balanced exposure, and high resolution.",
                r#"{"prefer":["sharpness","balanced_exposure","high_resolution"],"avoid":["blur","compression_artifacts","low_resolution"]}"#,
                30_i64,
            ),
            (
                "selpreset_cleanup",
                "Cleanup rejects",
                "cleanup",
                "Select weak alternates and failed variants for review before moving to Trash. Be conservative and explain each candidate.",
                r#"{"prefer":["weak_alternate","failed_variant","lower_focus"],"avoid":["unique_strong_image"],"requires_review":true}"#,
                40_i64,
            ),
        ];

        for (id, name, purpose, prompt, criteria_json, sort_order) in presets {
            conn.execute(
                "INSERT INTO agent_selection_presets (
                    id, name, purpose, prompt, criteria_json, sort_order, created_at, updated_at
                ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?7)",
                params![id, name, purpose, prompt, criteria_json, sort_order, now],
            )?;
        }
        Ok(())
    }
}

fn should_consider_migration_backup(db_path: &Path) -> bool {
    if db_path == Path::new(":memory:") {
        return false;
    }

    std::fs::metadata(db_path)
        .map(|metadata| metadata.is_file() && metadata.len() > 0)
        .unwrap_or(false)
}

fn user_version(conn: &Connection) -> Result<i64> {
    conn.pragma_query_value(None, "user_version", |row| row.get(0))
}

fn table_exists(conn: &Connection, table_name: &str) -> Result<bool> {
    let count: i64 = conn.query_row(
        "SELECT COUNT(*) FROM sqlite_master WHERE type = 'table' AND name = ?1",
        params![table_name],
        |row| row.get(0),
    )?;
    Ok(count > 0)
}

fn integrity_check(conn: &Connection) -> Result<()> {
    let mut stmt = conn.prepare("PRAGMA integrity_check")?;
    let results = stmt
        .query_map([], |row| row.get::<_, String>(0))?
        .collect::<Result<Vec<_>>>()?;

    if results.len() == 1 && results[0] == "ok" {
        Ok(())
    } else {
        Err(migration_error(format!(
            "database integrity check failed before migration: {}",
            results.join("; ")
        )))
    }
}

fn next_migration_backup_path(db_path: &Path, from_version: i64) -> Result<PathBuf> {
    let backup_dir = migration_backup_dir(db_path)?;
    let file_name = db_path
        .file_name()
        .ok_or_else(|| migration_error(format!("invalid database path {}", db_path.display())))?
        .to_string_lossy();
    let timestamp = chrono::Utc::now().format("%Y%m%dT%H%M%S%.9fZ");
    Ok(backup_dir.join(format!(
        "{}-before-v{}-to-v{}-{}.sqlite",
        file_name, from_version, CURRENT_SCHEMA_VERSION, timestamp
    )))
}

fn migration_backup_dir(db_path: &Path) -> Result<PathBuf> {
    let file_name = db_path
        .file_name()
        .ok_or_else(|| migration_error(format!("invalid database path {}", db_path.display())))?
        .to_string_lossy();
    Ok(db_path.with_file_name(format!("{}.backups", file_name)))
}

fn agent_action_proposals_schema() -> &'static str {
    r#"
    CREATE TABLE IF NOT EXISTS agent_selection_presets (
        id TEXT PRIMARY KEY,
        name TEXT NOT NULL,
        purpose TEXT NOT NULL,
        prompt TEXT NOT NULL,
        criteria_json TEXT NOT NULL DEFAULT '{}',
        sort_order INTEGER NOT NULL DEFAULT 100,
        created_at TEXT NOT NULL,
        updated_at TEXT NOT NULL
    );

    CREATE TABLE IF NOT EXISTS agent_action_proposals (
        id TEXT PRIMARY KEY,
        kind TEXT NOT NULL CHECK (
            kind IN (
                'select_images',
                'set_decisions',
                'create_collection',
                'add_to_collection',
                'remove_from_collection',
                'reorder_canvas',
                'remove_from_canvas',
                'trash_images'
            )
        ),
        status TEXT NOT NULL DEFAULT 'pending'
            CHECK (status IN ('pending', 'applied', 'dismissed')),
        persona TEXT NOT NULL
            CHECK (persona IN ('curator', 'copilot', 'operator')),
        lens TEXT,
        criteria TEXT NOT NULL,
        visual_level TEXT NOT NULL
            CHECK (visual_level IN ('text', 'tiny', 'preview', 'full')),
        selection_preset_id TEXT REFERENCES agent_selection_presets(id) ON DELETE SET NULL,
        estimated_input_tokens INTEGER,
        estimated_output_tokens INTEGER,
        estimated_cost_eur REAL,
        source_context_json TEXT NOT NULL DEFAULT '{}',
        items_json TEXT NOT NULL DEFAULT '[]',
        guard_results_json TEXT NOT NULL DEFAULT '{}',
        apply_result_json TEXT,
        undo_journal_json TEXT,
        created_at TEXT NOT NULL DEFAULT (datetime('now')),
        updated_at TEXT NOT NULL DEFAULT (datetime('now')),
        applied_at TEXT
    );

    CREATE INDEX IF NOT EXISTS idx_agent_action_proposals_status_created
        ON agent_action_proposals(status, created_at DESC);
    CREATE INDEX IF NOT EXISTS idx_agent_action_proposals_preset
        ON agent_action_proposals(selection_preset_id);
    CREATE INDEX IF NOT EXISTS idx_agent_selection_presets_purpose
        ON agent_selection_presets(purpose, sort_order);
    "#
}

fn migration_checksum(version: i64, name: &str) -> String {
    let mut hash = 0xcbf29ce484222325u64;
    for byte in version
        .to_le_bytes()
        .iter()
        .copied()
        .chain(name.as_bytes().iter().copied())
        .chain(include_str!("schema.sql").as_bytes().iter().copied())
    {
        hash ^= byte as u64;
        hash = hash.wrapping_mul(0x100000001b3);
    }
    format!("{:016x}", hash)
}

fn migration_error(message: String) -> SqlError {
    SqlError::SqliteFailure(ffi::Error::new(ffi::SQLITE_ERROR), Some(message))
}

pub(crate) fn validate_delete_folder_path(folder: &str) -> Result<()> {
    if folder.is_empty() {
        return Err(SqlError::InvalidParameterName(
            "folder path must not be empty".to_string(),
        ));
    }

    let path = Path::new(folder);
    if !path.is_absolute() {
        return Err(SqlError::InvalidParameterName(
            "folder path must be absolute".to_string(),
        ));
    }

    let has_non_root_component = path.components().any(|component| {
        !matches!(
            component,
            std::path::Component::RootDir | std::path::Component::Prefix(_)
        )
    });
    if !has_non_root_component {
        return Err(SqlError::InvalidParameterName(
            "folder path must not be filesystem root".to_string(),
        ));
    }

    Ok(())
}

pub(crate) fn decode_embedding_bytes(bytes: &[u8]) -> Vec<f32> {
    bytes
        .chunks_exact(4)
        .map(|chunk| f32::from_le_bytes([chunk[0], chunk[1], chunk[2], chunk[3]]))
        .collect()
}

pub(crate) fn cosine_similarity(a: &[f32], b: &[f32]) -> f32 {
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
mod migration_safety_tests {
    use super::*;
    use std::fs;

    fn create_legacy_db(path: &Path) {
        let conn = Connection::open(path).unwrap();
        conn.execute_batch(
            "
            PRAGMA user_version = 0;
            CREATE TABLE app_settings (
                key TEXT PRIMARY KEY,
                value TEXT NOT NULL
            );
            INSERT INTO app_settings (key, value) VALUES ('legacy_marker', 'preserved');
            ",
        )
        .unwrap();
    }

    #[test]
    fn test_open_records_schema_version_and_migration_history() {
        let tmp = tempfile::tempdir().unwrap();
        let db_path = tmp.path().join("cull.db");

        let db = Database::open(&db_path).unwrap();
        let conn = db.conn.lock();
        let user_version: i64 = conn
            .pragma_query_value(None, "user_version", |row| row.get(0))
            .unwrap();
        assert!(user_version > 0);

        let migration_count: i64 = conn
            .query_row("SELECT COUNT(*) FROM schema_migrations", [], |row| {
                row.get(0)
            })
            .unwrap();
        assert!(migration_count > 0);

        let latest_migration: i64 = conn
            .query_row("SELECT MAX(version) FROM schema_migrations", [], |row| {
                row.get(0)
            })
            .unwrap();
        assert_eq!(latest_migration, user_version);
    }

    #[test]
    fn test_open_records_successful_migration_step_state() {
        let tmp = tempfile::tempdir().unwrap();
        let db_path = tmp.path().join("cull.db");

        let db = Database::open(&db_path).unwrap();
        let conn = db.conn.lock();
        let successful_steps: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM schema_migration_steps WHERE status = 'succeeded'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        let failed_steps: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM schema_migration_steps WHERE status = 'failed'",
                [],
                |row| row.get(0),
            )
            .unwrap();

        assert_eq!(successful_steps, CURRENT_SCHEMA_VERSION);
        assert_eq!(failed_steps, 0);
    }

    #[test]
    fn test_failed_migration_step_rolls_back_and_records_recovery_state() {
        let tmp = tempfile::tempdir().unwrap();
        let db_path = tmp.path().join("cull.db");
        let db = Database::open(&db_path).unwrap();

        // Use a version far beyond any real migration so this synthetic failure
        // never collides with an actually-applied step.
        let err = db
            .run_migration_step(9999, "failing_test_step", || {
                let conn = db.conn.lock();
                conn.execute_batch("CREATE TABLE migration_failure_probe (id INTEGER);")?;
                Err(migration_error("synthetic migration failure".to_string()))
            })
            .unwrap_err();
        assert!(err.to_string().contains("synthetic migration failure"));

        let conn = db.conn.lock();
        assert!(!table_exists(&conn, "migration_failure_probe").unwrap());

        let current_user_version = user_version(&conn).unwrap();
        let latest_migration: i64 = conn
            .query_row("SELECT MAX(version) FROM schema_migrations", [], |row| {
                row.get(0)
            })
            .unwrap();
        assert_eq!(latest_migration, current_user_version);

        let (status, error): (String, String) = conn
            .query_row(
                "SELECT status, error FROM schema_migration_steps WHERE version = 9999",
                [],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .unwrap();
        assert_eq!(status, "failed");
        assert!(error.contains("synthetic migration failure"));
    }

    #[test]
    fn test_plugin_grants_table_created_on_fresh_and_legacy_databases() {
        // Fresh database: the plugin runtime's grants table must exist.
        let tmp = tempfile::tempdir().unwrap();
        let fresh_path = tmp.path().join("fresh.db");
        {
            let db = Database::open(&fresh_path).unwrap();
            let conn = db.conn.lock();
            assert!(table_exists(&conn, "plugin_grants").unwrap());
        }

        // Legacy database migrated forward gets the table too.
        let legacy_path = tmp.path().join("legacy.db");
        create_legacy_db(&legacy_path);
        let db = Database::open(&legacy_path).unwrap();
        let conn = db.conn.lock();
        assert!(table_exists(&conn, "plugin_grants").unwrap());
    }

    #[test]
    fn test_plugin_grants_round_trip_and_replace() {
        let tmp = tempfile::tempdir().unwrap();
        let db = Database::open(&tmp.path().join("cull.db")).unwrap();

        // No grants recorded -> empty capability set.
        assert!(db
            .granted_plugin_capabilities("cull-publish")
            .unwrap()
            .is_empty());

        db.set_plugin_grants(
            "cull-publish",
            &["library:read".to_string(), "export:read".to_string()],
        )
        .unwrap();
        let mut caps = db.granted_plugin_capabilities("cull-publish").unwrap();
        caps.sort();
        assert_eq!(caps, vec!["export:read", "library:read"]);

        // Grants are per plugin id.
        assert!(db
            .granted_plugin_capabilities("other-plugin")
            .unwrap()
            .is_empty());

        // Re-granting replaces the previous set (no stale rows).
        db.set_plugin_grants("cull-publish", &["library:read".to_string()])
            .unwrap();
        assert_eq!(
            db.granted_plugin_capabilities("cull-publish").unwrap(),
            vec!["library:read"]
        );
    }

    fn mcp_tokens_has_expires_at(conn: &Connection) -> bool {
        let mut stmt = conn.prepare("PRAGMA table_info(mcp_tokens)").unwrap();
        let columns: Vec<String> = stmt
            .query_map([], |row| row.get::<_, String>(1))
            .unwrap()
            .collect::<Result<Vec<_>>>()
            .unwrap();
        columns.iter().any(|c| c == "expires_at")
    }

    #[test]
    fn test_mcp_tokens_table_supports_expiry_on_fresh_and_legacy_databases() {
        // Fresh database: mcp_tokens must carry the expires_at column used for
        // token expiry enforcement.
        let tmp = tempfile::tempdir().unwrap();
        let fresh_path = tmp.path().join("fresh.db");
        {
            let db = Database::open(&fresh_path).unwrap();
            let conn = db.conn.lock();
            assert!(mcp_tokens_has_expires_at(&conn));
        }

        // Legacy database migrated forward: same invariant after run_migrations,
        // and pre-existing rows keep NULL expiry (no expiry) untouched.
        let legacy_path = tmp.path().join("legacy.db");
        create_legacy_db(&legacy_path);
        let db = Database::open(&legacy_path).unwrap();
        let conn = db.conn.lock();
        assert!(mcp_tokens_has_expires_at(&conn));
        conn.execute(
            "INSERT INTO mcp_tokens (id, name, secret_hash, role, created_at)
             VALUES ('tok_legacy', 'Legacy', 'hash', 'viewer', '2026-01-01T00:00:00Z')",
            [],
        )
        .unwrap();
        let expires_at: Option<String> = conn
            .query_row(
                "SELECT expires_at FROM mcp_tokens WHERE id = 'tok_legacy'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert!(expires_at.is_none());
    }

    #[test]
    fn test_open_creates_backup_before_migrating_existing_database() {
        let tmp = tempfile::tempdir().unwrap();
        let db_path = tmp.path().join("legacy.db");
        create_legacy_db(&db_path);

        Database::open(&db_path).unwrap();

        let backup_dir = tmp.path().join("legacy.db.backups");
        let backups: Vec<_> = fs::read_dir(&backup_dir)
            .unwrap()
            .map(|entry| entry.unwrap().path())
            .collect();
        assert_eq!(backups.len(), 1);

        let backup = Connection::open(&backups[0]).unwrap();
        let marker: String = backup
            .query_row(
                "SELECT value FROM app_settings WHERE key = 'legacy_marker'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(marker, "preserved");

        let backup_version: i64 = backup
            .pragma_query_value(None, "user_version", |row| row.get(0))
            .unwrap();
        assert_eq!(backup_version, 0);
    }

    #[test]
    fn test_open_accepts_legacy_high_version_database() {
        // A database created by a previous build with user_version=24.
        // Open must accept it — the consolidated schema uses CREATE TABLE IF NOT
        // EXISTS everywhere, so re-applying is a no-op on existing tables.
        let tmp = tempfile::tempdir().unwrap();
        let db_path = tmp.path().join("legacy_v24.db");
        {
            let _ = Database::open(&db_path).unwrap();
        }
        {
            let conn = Connection::open(&db_path).unwrap();
            conn.execute_batch("PRAGMA user_version = 24;").unwrap();
        }

        let db = Database::open(&db_path).unwrap();
        let conn = db.conn.lock();
        assert!(table_exists(&conn, "images").unwrap());
        assert!(table_exists(&conn, "catalog_works").unwrap());
    }

    #[test]
    fn test_open_idempotent_reopen() {
        // Re-opening an already-migrated database must succeed without changes.
        let tmp = tempfile::tempdir().unwrap();
        let db_path = tmp.path().join("reopen.db");
        {
            let _ = Database::open(&db_path).unwrap();
        }

        let db = Database::open(&db_path).unwrap();
        let conn = db.conn.lock();
        assert!(table_exists(&conn, "client_feedback").unwrap());
        assert!(table_exists(&conn, "images").unwrap());
        let user_version = user_version(&conn).unwrap();
        assert_eq!(user_version, CURRENT_SCHEMA_VERSION);
    }

    #[test]
    fn test_open_user_version_24_runs_agent_proposal_migration() {
        let tmp = tempfile::tempdir().unwrap();
        let db_path = tmp.path().join("existing-v24.db");
        {
            let _ = Database::open(&db_path).unwrap();
        }
        {
            let conn = Connection::open(&db_path).unwrap();
            conn.execute_batch(
                "
                DROP TABLE agent_action_proposals;
                DROP TABLE agent_selection_presets;
                DELETE FROM schema_migrations WHERE version >= 25;
                DELETE FROM schema_migration_steps WHERE version >= 25;
                PRAGMA user_version = 24;
                ",
            )
            .unwrap();
        }

        let db = Database::open(&db_path).unwrap();
        let conn = db.conn.lock();
        assert!(table_exists(&conn, "agent_action_proposals").unwrap());
        assert!(table_exists(&conn, "agent_selection_presets").unwrap());
        assert_eq!(user_version(&conn).unwrap(), CURRENT_SCHEMA_VERSION);
        let preset_count: i64 = conn
            .query_row("SELECT COUNT(*) FROM agent_selection_presets", [], |row| {
                row.get(0)
            })
            .unwrap();
        assert!(preset_count >= 4);
    }
}

#[cfg(test)]
mod client_feedback_tests {
    use super::*;

    fn seed_image(db: &Database, id: &str) {
        db.insert_image(&Image {
            id: id.to_string(),
            sha256_hash: format!("hash-{}", id),
            width: 100,
            height: 100,
            format: "png".to_string(),
            file_size: 1024,
            created_at: "2026-05-07T00:00:00Z".to_string(),
            imported_at: "2026-05-07T00:00:00Z".to_string(),
            ai_prompt: None,
            raw_metadata: None,
        })
        .unwrap();
    }

    #[test]
    fn set_get_and_list_client_feedback_roundtrip() {
        let db = Database::open(Path::new(":memory:")).unwrap();
        seed_image(&db, "img-1");
        seed_image(&db, "img-2");

        assert!(db.get_client_feedback("img-1").unwrap().is_none());

        db.set_client_feedback("img-1", true, Some("Love this one"))
            .unwrap();
        let feedback = db.get_client_feedback("img-1").unwrap().unwrap();
        assert!(feedback.favorite);
        assert_eq!(feedback.comment.as_deref(), Some("Love this one"));

        // Upsert updates in place rather than duplicating.
        db.set_client_feedback("img-1", false, None).unwrap();
        let feedback = db.get_client_feedback("img-1").unwrap().unwrap();
        assert!(!feedback.favorite);
        assert!(feedback.comment.is_none());

        db.set_client_feedback("img-2", true, Some("maybe"))
            .unwrap();
        assert_eq!(db.list_client_feedback().unwrap().len(), 2);
    }

    #[test]
    fn client_feedback_does_not_touch_selections() {
        let db = Database::open(Path::new(":memory:")).unwrap();
        seed_image(&db, "img-1");
        db.set_rating("img-1", 4).unwrap();
        db.set_client_feedback("img-1", true, Some("client pick"))
            .unwrap();

        let stored_rating: Option<i64> = {
            let conn = db.conn.lock();
            conn.query_row(
                "SELECT star_rating FROM selections WHERE image_id = ?1 AND project_id = '__global__'",
                params!["img-1"],
                |row| row.get(0),
            )
            .unwrap()
        };
        assert_eq!(stored_rating, Some(4));
        assert!(db.get_client_feedback("img-1").unwrap().unwrap().favorite);
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
        insert_test_image_at_path(db, id, hash, &format!("/tmp/{}.png", id));
    }

    fn insert_test_image_at_path(db: &Database, id: &str, hash: &str, path: &str) {
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
            path: path.to_string(),
            last_seen_at: "2026-05-07T00:00:00Z".to_string(),
            missing_at: None,
            last_seen_size: None,
            last_seen_mtime: None,
        };
        db.insert_image_file(&file).unwrap();
    }

    fn insert_test_pdf(db: &Database, id: &str, hash: &str) {
        let img = Image {
            id: id.to_string(),
            sha256_hash: hash.to_string(),
            width: 100,
            height: 100,
            format: "pdf".to_string(),
            file_size: 3072,
            created_at: "2026-05-07T00:00:00Z".to_string(),
            imported_at: "2026-05-07T00:00:00Z".to_string(),
            ai_prompt: None,
            raw_metadata: None,
        };
        db.insert_image(&img).unwrap();
        let file = ImageFile {
            id: format!("f-{}", id),
            image_id: id.to_string(),
            path: format!("/tmp/{}.pdf", id),
            last_seen_at: "2026-05-07T00:00:00Z".to_string(),
            missing_at: None,
            last_seen_size: None,
            last_seen_mtime: None,
        };
        db.insert_image_file(&file).unwrap();
    }

    #[test]
    fn verify_schema_invariants_passes_for_fresh_db() {
        let db = test_db();
        assert!(db.verify_schema_invariants().is_ok());
    }

    #[test]
    fn insert_image_creates_media_asset_row() {
        let db = test_db();
        let img = Image {
            id: "image-1".to_string(),
            sha256_hash: "hash-image-1".to_string(),
            width: 16,
            height: 16,
            format: "png".to_string(),
            file_size: 128,
            created_at: "2026-01-01T00:00:00Z".to_string(),
            imported_at: "2026-01-01T00:00:00Z".to_string(),
            ai_prompt: None,
            raw_metadata: None,
        };

        db.insert_image(&img).unwrap();

        let media: (String, String, String) = {
            let conn = db.conn.lock();
            conn.query_row(
                "SELECT id, media_type, format FROM media_assets WHERE primary_image_id = ?1",
                params![img.id],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
            )
            .unwrap()
        };
        assert_eq!(media.0, "ma_image-1");
        assert_eq!(media.1, "image");
        assert_eq!(media.2, "png");
    }

    #[test]
    fn insert_pdf_image_creates_pdf_media_asset_row() {
        let db = test_db();
        let img = Image {
            id: "document-1".to_string(),
            sha256_hash: "hash-document-1".to_string(),
            width: 16,
            height: 16,
            format: "pdf".to_string(),
            file_size: 2048,
            created_at: "2026-01-01T00:00:00Z".to_string(),
            imported_at: "2026-01-01T00:00:00Z".to_string(),
            ai_prompt: None,
            raw_metadata: None,
        };

        db.insert_image(&img).unwrap();

        let media_type: String = {
            let conn = db.conn.lock();
            conn.query_row(
                "SELECT media_type FROM media_assets WHERE primary_image_id = ?1",
                params![img.id],
                |row| row.get(0),
            )
            .unwrap()
        };
        assert_eq!(media_type, "pdf");
    }

    #[test]
    fn set_media_asset_page_count_by_image_id_updates_record() {
        let db = test_db();
        let img = Image {
            id: "document-pages-1".to_string(),
            sha256_hash: "hash-document-pages-1".to_string(),
            width: 16,
            height: 16,
            format: "pdf".to_string(),
            file_size: 2048,
            created_at: "2026-01-01T00:00:00Z".to_string(),
            imported_at: "2026-01-01T00:00:00Z".to_string(),
            ai_prompt: None,
            raw_metadata: None,
        };

        db.insert_image(&img).unwrap();

        db.set_media_asset_page_count_by_image_id(&img.id, 13)
            .unwrap();

        let count: Option<i64> = {
            let conn = db.conn.lock();
            conn.query_row(
                "SELECT page_count FROM media_assets WHERE primary_image_id = ?1",
                rusqlite::params![&img.id],
                |row| row.get(0),
            )
            .unwrap()
        };
        assert_eq!(count, Some(13));
    }

    #[test]
    fn set_media_asset_title_by_image_id_updates_record() {
        let db = test_db();
        insert_test_pdf(&db, "document-title-1", "hash-document-title-1");

        db.set_media_asset_title_by_image_id(&"document-title-1", "Quarterly Report")
            .unwrap();

        let title: Option<String> = {
            let conn = db.conn.lock();
            conn.query_row(
                "SELECT title FROM media_assets WHERE primary_image_id = ?1",
                rusqlite::params!["document-title-1"],
                |row| row.get(0),
            )
            .unwrap()
        };

        assert_eq!(title.as_deref(), Some("Quarterly Report"));
    }

    #[test]
    fn verify_schema_invariants_detects_missing_required_table() {
        let db = test_db();
        // Simulate a partially-migrated DB whose user_version claims completion
        // but is missing a required table. user_version is unchanged (still high).
        db.conn
            .lock()
            .execute_batch("DROP TABLE image_tags")
            .unwrap();
        let err = db.verify_schema_invariants().unwrap_err();
        assert!(
            err.to_string().contains("image_tags"),
            "error should name the missing table: {err}"
        );
    }

    #[test]
    fn list_folders_groups_by_parent_directory() {
        let db = test_db();
        insert_test_image_at_path(&db, "a", "h-a", "/lib/art/a.png");
        insert_test_image_at_path(&db, "b", "h-b", "/lib/art/b.png");
        insert_test_image_at_path(&db, "c", "h-c", "/lib/photos/c.png");
        insert_test_image_at_path(&db, "d", "h-d", "/lib/photos/sub/d.png");

        let folders = db.list_folders().unwrap();
        // Counts are per immediate parent directory, sorted by path.
        assert_eq!(
            folders,
            vec![
                ("/lib/art".to_string(), 2),
                ("/lib/photos".to_string(), 1),
                ("/lib/photos/sub".to_string(), 1),
            ]
        );
    }

    #[test]
    fn list_folders_root_level_file_matches_path_parent() {
        let db = test_db();
        insert_test_image_at_path(&db, "r", "h-r", "/root.png");
        insert_test_image_at_path(&db, "n", "h-n", "/lib/n.png");
        // Path::parent("/root.png") == "/", Path::parent("/lib/n.png") == "/lib".
        assert_eq!(
            db.list_folders().unwrap(),
            vec![("/".to_string(), 1), ("/lib".to_string(), 1)]
        );
    }

    #[test]
    fn list_folders_excludes_missing_files() {
        let db = test_db();
        insert_test_image_at_path(&db, "a", "h-a", "/lib/art/a.png");
        insert_test_image_at_path(&db, "b", "h-b", "/lib/art/b.png");
        // Mark one file missing; it must not count toward its folder.
        db.conn
            .lock()
            .execute(
                "UPDATE image_files SET missing_at = '2026-01-01T00:00:00Z' WHERE image_id = 'b'",
                [],
            )
            .unwrap();

        assert_eq!(
            db.list_folders().unwrap(),
            vec![("/lib/art".to_string(), 1)]
        );
    }

    #[test]
    fn list_images_in_scope_collection_paginates_completely() {
        let db = test_db();
        let col = db.create_collection("C1").unwrap();
        // 5 in the scoped collection, 3 outside it.
        for i in 0..5 {
            insert_test_image(&db, &format!("in{i}"), &format!("h-in{i}"));
        }
        for i in 0..3 {
            insert_test_image(&db, &format!("out{i}"), &format!("h-out{i}"));
        }
        let in_ids: Vec<&str> = ["in0", "in1", "in2", "in3", "in4"].to_vec();
        db.add_to_collection(&col, &in_ids).unwrap();

        let cols = vec![col.clone()];
        // Page through with a small limit; the union of all pages must be exactly
        // the 5 scoped images, with no truncation or out-of-scope leakage.
        let mut seen = std::collections::BTreeSet::new();
        for offset in (0..6).step_by(2) {
            let page = db.list_images_in_scope(&[], &cols, &[], 2, offset).unwrap();
            assert!(page.len() <= 2);
            for img in page {
                assert!(in_ids.contains(&img.image.id.as_str()));
                seen.insert(img.image.id);
            }
        }
        assert_eq!(seen.len(), 5, "all scoped images returned across pages");
    }

    #[test]
    fn list_images_in_scope_folder_prefix_is_exact() {
        let db = test_db();
        insert_test_image_at_path(&db, "a", "h-a", "/art/a.png");
        insert_test_image_at_path(&db, "b", "h-b", "/artisan/b.png");
        insert_test_image_at_path(&db, "c", "h-c", "/art/sub/c.png");

        let folders = vec!["/art".to_string()];
        let ids: std::collections::BTreeSet<String> = db
            .list_images_in_scope(&folders, &[], &[], 100, 0)
            .unwrap()
            .into_iter()
            .map(|i| i.image.id)
            .collect();
        // /art and /art/sub match; the sibling prefix /artisan must NOT.
        assert!(ids.contains("a"));
        assert!(ids.contains("c"));
        assert!(!ids.contains("b"));
    }

    #[test]
    fn list_images_in_scope_union_of_dimensions() {
        let db = test_db();
        insert_test_image_at_path(&db, "byFolder", "h1", "/art/x.png");
        insert_test_image_at_path(&db, "byColl", "h2", "/other/y.png");
        insert_test_image_at_path(&db, "byTag", "h3", "/other/z.png");
        insert_test_image_at_path(&db, "none", "h4", "/other/n.png");
        let col = db.create_collection("C1").unwrap();
        db.add_to_collection(&col, &["byColl"]).unwrap();
        db.add_image_tag("byTag", "public", "user", "manual", None)
            .unwrap();

        let ids: std::collections::BTreeSet<String> = db
            .list_images_in_scope(
                &["/art".to_string()],
                &[col],
                &["public".to_string()],
                100,
                0,
            )
            .unwrap()
            .into_iter()
            .map(|i| i.image.id)
            .collect();
        assert!(ids.contains("byFolder"));
        assert!(ids.contains("byColl"));
        assert!(ids.contains("byTag"));
        assert!(!ids.contains("none"));
        assert_eq!(ids.len(), 3);
    }

    #[test]
    fn list_images_in_scope_empty_dimensions_returns_nothing() {
        let db = test_db();
        insert_test_image(&db, "a", "h-a");
        // No folders/collections/tags -> matches nothing (must NOT return all rows).
        assert!(db
            .list_images_in_scope(&[], &[], &[], 100, 0)
            .unwrap()
            .is_empty());
    }

    #[test]
    fn test_delete_images_by_folder_rejects_empty_folder() {
        let db = test_db();
        insert_test_image_at_path(&db, "img-1", "hash-delete-empty", "/tmp/a/img-1.png");

        let result = db.delete_images_by_folder("");

        assert!(result.is_err(), "empty folder path should be rejected");
        assert_eq!(db.list_images(100, 0).unwrap().len(), 1);
    }

    #[test]
    fn test_delete_images_by_folder_rejects_root_folder() {
        let db = test_db();
        insert_test_image_at_path(&db, "img-1", "hash-delete-root", "/tmp/a/img-1.png");

        let result = db.delete_images_by_folder("/");

        assert!(result.is_err(), "root folder path should be rejected");
        assert_eq!(db.list_images(100, 0).unwrap().len(), 1);
    }

    #[test]
    fn test_delete_images_by_folder_rejects_relative_folder() {
        let db = test_db();
        insert_test_image_at_path(&db, "img-1", "hash-delete-relative", "/tmp/a/img-1.png");

        let result = db.delete_images_by_folder("tmp/a");

        assert!(result.is_err(), "relative folder path should be rejected");
        assert_eq!(db.list_images(100, 0).unwrap().len(), 1);
    }

    #[test]
    fn test_delete_images_by_folder_treats_percent_as_literal_path_character() {
        let db = test_db();
        insert_test_image_at_path(&db, "inside", "hash-delete-percent", "/tmp/a%b/inside.png");
        insert_test_image_at_path(&db, "outside", "hash-keep-percent", "/tmp/axb/outside.png");

        let deleted = db.delete_images_by_folder("/tmp/a%b").unwrap();

        assert_eq!(deleted, 1);
        let remaining = db.list_images(100, 0).unwrap();
        assert_eq!(remaining.len(), 1);
        assert_eq!(remaining[0].image.id, "outside");
    }

    #[test]
    fn test_delete_images_by_folder_treats_underscore_as_literal_path_character() {
        let db = test_db();
        insert_test_image_at_path(
            &db,
            "inside",
            "hash-delete-underscore",
            "/tmp/a_b/inside.png",
        );
        insert_test_image_at_path(
            &db,
            "outside",
            "hash-keep-underscore",
            "/tmp/axb/outside.png",
        );

        let deleted = db.delete_images_by_folder("/tmp/a_b").unwrap();

        assert_eq!(deleted, 1);
        let remaining = db.list_images(100, 0).unwrap();
        assert_eq!(remaining.len(), 1);
        assert_eq!(remaining[0].image.id, "outside");
    }

    #[test]
    fn test_delete_images_by_folder_keeps_adjacent_prefix_folder() {
        let db = test_db();
        insert_test_image_at_path(&db, "inside", "hash-delete-inside", "/tmp/a/inside.png");
        insert_test_image_at_path(
            &db,
            "adjacent",
            "hash-delete-adjacent",
            "/tmp/abc/adjacent.png",
        );

        let deleted = db.delete_images_by_folder("/tmp/a").unwrap();

        assert_eq!(deleted, 1);
        let remaining = db.list_images(100, 0).unwrap();
        assert_eq!(remaining.len(), 1);
        assert_eq!(remaining[0].image.id, "adjacent");
    }

    #[test]
    fn test_delete_images_by_folder_keeps_case_distinct_folder() {
        let db = test_db();
        insert_test_image_at_path(&db, "lower", "hash-delete-lower", "/tmp/a/lower.png");
        insert_test_image_at_path(&db, "upper", "hash-delete-upper", "/tmp/A/upper.png");

        let deleted = db.delete_images_by_folder("/tmp/a").unwrap();

        assert_eq!(deleted, 1);
        let remaining = db.list_images(100, 0).unwrap();
        assert_eq!(remaining.len(), 1);
        assert_eq!(remaining[0].image.id, "upper");
    }

    #[test]
    fn test_delete_images_by_folder_handles_non_ascii_folder_names() {
        let db = test_db();
        insert_test_image_at_path(&db, "inside", "hash-delete-nonascii", "/tmp/ä/inside.png");
        insert_test_image_at_path(
            &db,
            "adjacent",
            "hash-keep-nonascii",
            "/tmp/äx/adjacent.png",
        );

        let deleted = db.delete_images_by_folder("/tmp/ä").unwrap();

        assert_eq!(deleted, 1);
        let remaining = db.list_images(100, 0).unwrap();
        assert_eq!(remaining.len(), 1);
        assert_eq!(remaining[0].image.id, "adjacent");
    }

    #[test]
    fn test_configure_connection_enables_foreign_keys() {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch("PRAGMA foreign_keys=OFF").unwrap();

        Database::configure_connection(&conn).unwrap();

        let enabled: i64 = conn
            .pragma_query_value(None, "foreign_keys", |row| row.get(0))
            .unwrap();
        assert_eq!(enabled, 1);
    }

    #[test]
    fn test_reopen_enables_foreign_key_cascades() {
        let tmp = tempfile::tempdir().unwrap();
        let db_path = tmp.path().join("runtime.db");

        {
            let db = Database::open(&db_path).unwrap();
            insert_test_image(&db, "img-1", "hash-1");
            db.set_rating("img-1", 4).unwrap();
            let collection_id = db.create_collection("Cascade Check").unwrap();
            db.add_to_collection(&collection_id, &["img-1"]).unwrap();
        }

        let db = Database::open(&db_path).unwrap();
        let conn = db.conn.lock();
        conn.execute("DELETE FROM images WHERE id = ?1", params!["img-1"])
            .unwrap();

        let image_files: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM image_files WHERE image_id = ?1",
                params!["img-1"],
                |row| row.get(0),
            )
            .unwrap();
        let selections: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM selections WHERE image_id = ?1",
                params!["img-1"],
                |row| row.get(0),
            )
            .unwrap();
        let collection_items: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM collection_items WHERE image_id = ?1",
                params!["img-1"],
                |row| row.get(0),
            )
            .unwrap();

        assert_eq!(image_files, 0);
        assert_eq!(selections, 0);
        assert_eq!(collection_items, 0);
    }

    #[test]
    fn test_collection_settings_json_round_trips() {
        let db = Database::open(std::path::Path::new(":memory:")).unwrap();
        let collection_id = db.create_collection("Clipboard 2026.05.30 14:35").unwrap();

        db.set_collection_settings_json(
            &collection_id,
            r#"{"source":"clipboard_monitor","capture_dir":"/tmp/cull"}"#,
        )
        .unwrap();

        let stored = db.get_collection_settings_json(&collection_id).unwrap();
        assert_eq!(
            stored.as_deref(),
            Some(r#"{"source":"clipboard_monitor","capture_dir":"/tmp/cull"}"#)
        );
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
    fn test_get_images_by_ids_includes_rating_when_decision_is_null() {
        let db = test_db();
        insert_test_image(&db, "img-1", "hash-1");
        {
            let conn = db.conn.lock();
            conn.execute(
                "INSERT INTO selections (image_id, project_id, star_rating, decision)
                 VALUES (?1, '__global__', 5, NULL)",
                params!["img-1"],
            )
            .unwrap();
        }

        let results = db.get_images_by_ids(&["img-1"]).unwrap();
        assert_eq!(results.len(), 1);
        let selection = results[0].selection.as_ref().unwrap();
        assert_eq!(selection.star_rating, Some(5));
        assert_eq!(selection.decision, "undecided");
    }

    #[test]
    fn test_smart_collection_text_search_matches_filename_and_metadata_text() {
        let db = test_db();
        insert_test_image(&db, "astra-filename", "hash-astra-filename");
        insert_test_image(&db, "ocr-match", "hash-ocr-match");
        insert_test_image(&db, "plain-image", "hash-plain-image");
        insert_test_pdf(&db, "pdf-text-match", "hash-pdf-text-match");

        db.set_media_asset_title_by_image_id(&"pdf-text-match", "Astra Appendix")
            .unwrap();

        let pdf_media_asset_id = db
            .media_asset_for_image("pdf-text-match")
            .unwrap()
            .unwrap()
            .id;
        db.upsert_pdf_page(&PdfPage {
            id: "pp-pdf-text-match-0".to_string(),
            media_asset_id: pdf_media_asset_id,
            page_index: 0,
            width_points: Some(612.0),
            height_points: Some(792.0),
            thumbnail_path: None,
            preview_path: None,
            extracted_text: Some("contains term astra in body".to_string()),
            text_extracted_at: Some("2026-05-07T00:00:00Z".to_string()),
        })
        .unwrap();

        let mut fields = std::collections::HashMap::new();
        fields.insert(
            "ocr_text".to_string(),
            "the ASTRA word is visible".to_string(),
        );
        db.store_vision_metadata("ocr-match", "ocr", &fields)
            .unwrap();

        let filter = r#"{"type":"rule","field":"search_text","op":"contains","value":"astra"}"#;
        assert_eq!(db.count_smart_collection(filter).unwrap(), 3);

        let results = db.evaluate_smart_collection(filter).unwrap();
        let ids: Vec<&str> = results.iter().map(|r| r.image.id.as_str()).collect();
        assert_eq!(ids.len(), 3);
        assert!(ids.contains(&"astra-filename"));
        assert!(ids.contains(&"ocr-match"));
        assert!(!ids.contains(&"plain-image"));
        assert!(ids.contains(&"pdf-text-match"));
    }

    #[test]
    fn test_image_tags_are_stored_searchable_and_filterable() {
        let db = test_db();
        insert_test_image(&db, "tagged", "hash-tagged");
        insert_test_image(&db, "plain", "hash-plain");

        assert!(db
            .add_image_tag("tagged", "Golden Hour", "vision", "manual", Some(0.9))
            .unwrap());
        assert!(!db
            .add_image_tag("tagged", "golden hour", "vision", "manual", Some(0.9))
            .unwrap());

        let image_tags = db.list_image_tags("tagged").unwrap();
        assert_eq!(image_tags.len(), 1);
        assert_eq!(image_tags[0].normalized_name, "golden-hour");

        let tags = db.list_tags(10, 0).unwrap();
        assert_eq!(tags.len(), 1);
        assert_eq!(tags[0].image_count, 1);

        let tag_filter = r#"{"type":"rule","field":"tag","op":"eq","value":"golden hour"}"#;
        assert_eq!(db.count_smart_collection(tag_filter).unwrap(), 1);
        let tag_results = db.evaluate_smart_collection(tag_filter).unwrap();
        assert_eq!(tag_results[0].image.id, "tagged");

        let search_filter =
            r#"{"type":"rule","field":"search_text","op":"contains","value":"golden"}"#;
        let ids: Vec<String> = db
            .evaluate_smart_collection(search_filter)
            .unwrap()
            .into_iter()
            .map(|img| img.image.id)
            .collect();
        assert_eq!(ids, vec!["tagged".to_string()]);
    }

    #[test]
    fn test_backfill_image_tags_promotes_existing_enrichment() {
        let db = test_db();
        insert_test_image(&db, "enriched", "hash-enriched");
        db.update_source_detection(
            "enriched",
            Some("midjourney"),
            0.95,
            "{}",
            Some(true),
            Some("a luminous portrait"),
            1.0,
            "portrait",
            0.01,
        )
        .unwrap();

        let run = GenerationRun {
            id: "run-1".to_string(),
            prompt: Some("a luminous portrait".to_string()),
            negative_prompt: None,
            provider: Some("openai".to_string()),
            model: Some("gpt-image-1".to_string()),
            settings_json: "{}".to_string(),
            seed: None,
            parent_run_id: None,
            source_type: "sidecar".to_string(),
            source_path: None,
            raw_metadata_json: None,
            created_at: Some("2026-05-07T00:00:00Z".to_string()),
            imported_at: "2026-05-07T00:00:00Z".to_string(),
        };
        db.insert_generation_run(&run).unwrap();
        db.link_image_to_run("enriched", "run-1").unwrap();

        let mut fields = std::collections::HashMap::new();
        fields.insert("tags".to_string(), "golden hour, editorial".to_string());
        fields.insert("scene_type".to_string(), "studio portrait".to_string());
        db.store_vision_metadata("enriched", "minicpm-v", &fields)
            .unwrap();
        db.store_detections(
            "enriched",
            "yolo11m",
            &[
                crate::db_core::detection::Detection {
                    class_name: "person".to_string(),
                    confidence: 0.91,
                    x: 0.1,
                    y: 0.1,
                    width: 0.8,
                    height: 0.8,
                },
                crate::db_core::detection::Detection {
                    class_name: "chair".to_string(),
                    confidence: 0.2,
                    x: 0.0,
                    y: 0.0,
                    width: 0.2,
                    height: 0.2,
                },
            ],
        )
        .unwrap();

        let result = db.backfill_image_tags().unwrap();
        assert_eq!(result.images_processed, 1);
        assert!(result.tags_created >= 8);
        assert!(result.image_tags_created >= 8);

        let tags = db.list_image_tags("enriched").unwrap();
        let normalized: Vec<&str> = tags
            .iter()
            .map(|tag| tag.normalized_name.as_str())
            .collect();
        assert!(normalized.contains(&"png"));
        assert!(normalized.contains(&"portrait"));
        assert!(normalized.contains(&"midjourney"));
        assert!(normalized.contains(&"openai"));
        assert!(normalized.contains(&"gpt-image-1"));
        assert!(normalized.contains(&"golden-hour"));
        assert!(normalized.contains(&"editorial"));
        assert!(normalized.contains(&"studio-portrait"));
        assert!(normalized.contains(&"person"));
        assert!(!normalized.contains(&"chair"));
    }

    #[test]
    fn test_perceptual_hash_identifies_near_duplicates() {
        let db = test_db();
        insert_test_image(&db, "base", "hash-phash-base");
        insert_test_image(&db, "near", "hash-phash-near");
        insert_test_image(&db, "far", "hash-phash-far");

        let base_hash =
            ImagePerceptualHash::from_hash_lo("base", "phash-dct-64-v1", 0xFFFF_0000_FFFF_0000u64);
        let near_hash =
            ImagePerceptualHash::from_hash_lo("near", "phash-dct-64-v1", 0xFFFF_0000_FFFF_0001u64);
        let far_hash =
            ImagePerceptualHash::from_hash_lo("far", "phash-dct-64-v1", 0x0000_FFFF_0000_FFFFu64);

        db.store_image_perceptual_hash(&base_hash).unwrap();
        db.store_image_perceptual_hash(&near_hash).unwrap();
        db.store_image_perceptual_hash(&far_hash).unwrap();

        let stored = db
            .get_image_perceptual_hash("base", "phash-dct-64-v1")
            .unwrap()
            .unwrap();
        assert_eq!(stored.hash_lo as u64, 0xFFFF_0000_FFFF_0000u64);
        assert_eq!(db.perceptual_hash_count("phash-dct-64-v1").unwrap(), 3);

        let duplicates = db
            .find_near_duplicates_by_phash("base", "phash-dct-64-v1", 4, 10)
            .unwrap();
        assert_eq!(duplicates.len(), 1);
        assert_eq!(duplicates[0].image.image.id, "near");
        assert_eq!(duplicates[0].distance, 1);
    }

    #[test]
    fn test_phash_is_stable_under_small_brightness_changes() {
        use crate::db_core::perceptual_hash::{
            analyze_image_perceptual_hash, hamming_distance, PHASH_ALGORITHM,
        };
        use image::{ImageBuffer, Rgb};

        let tmp = tempfile::tempdir().unwrap();
        let base_path = tmp.path().join("base.png");
        let bright_path = tmp.path().join("bright.png");
        let checker_path = tmp.path().join("checker.png");

        let base = ImageBuffer::from_fn(96, 96, |x, y| {
            let v = ((x * 3 + y * 2) % 256) as u8;
            Rgb([v, v.saturating_add(8), v.saturating_sub(8)])
        });
        let bright = ImageBuffer::from_fn(96, 96, |x, y| {
            let v = ((x * 3 + y * 2) % 256) as u8;
            let b = v.saturating_add(12);
            Rgb([b, b.saturating_add(8), b.saturating_sub(8)])
        });
        let checker = ImageBuffer::from_fn(96, 96, |x, y| {
            let v = if ((x / 8) + (y / 8)) % 2 == 0 {
                0u8
            } else {
                255u8
            };
            Rgb([v, v, v])
        });
        base.save(&base_path).unwrap();
        bright.save(&bright_path).unwrap();
        checker.save(&checker_path).unwrap();

        let base_hash = analyze_image_perceptual_hash("base", &base_path).unwrap();
        let bright_hash = analyze_image_perceptual_hash("bright", &bright_path).unwrap();
        let checker_hash = analyze_image_perceptual_hash("checker", &checker_path).unwrap();

        assert_eq!(base_hash.algorithm, PHASH_ALGORITHM);
        let near_distance = hamming_distance(&base_hash, &bright_hash);
        let far_distance = hamming_distance(&base_hash, &checker_hash);
        assert!(near_distance <= 8, "near distance: {}", near_distance);
        assert!(
            far_distance > near_distance + 8,
            "near: {}, far: {}",
            near_distance,
            far_distance
        );
    }

    #[test]
    fn test_color_metrics_extract_dominant_red_palette() {
        use crate::db_core::color::{analyze_image_color_metrics, COLOR_ANALYZER_VERSION};
        use image::{ImageBuffer, Rgb};

        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join("red.png");
        let img = ImageBuffer::from_pixel(32, 32, Rgb([240u8, 12, 20]));
        img.save(&path).unwrap();

        let metrics = analyze_image_color_metrics("red", &path).unwrap();
        assert_eq!(metrics.analyzer_version, COLOR_ANALYZER_VERSION);
        assert_eq!(metrics.dominant_hue_bucket, "red");
        assert_eq!(metrics.palette.len(), 1);
        assert!(metrics.palette[0].percentage > 0.99, "{:?}", metrics);
        assert!(metrics.mean_saturation > 0.8, "{:?}", metrics);
    }

    #[test]
    fn test_color_metrics_are_stored_and_filterable() {
        let db = test_db();
        insert_test_image(&db, "red", "hash-red-color");
        insert_test_image(&db, "blue", "hash-blue-color");

        db.store_image_color_metrics(&ImageColorMetrics {
            image_id: "red".to_string(),
            analyzer_version: "color-v1".to_string(),
            dominant_hex: "#f20c14".to_string(),
            palette: vec![ImagePaletteColor {
                hex: "#f20c14".to_string(),
                red: 242,
                green: 12,
                blue: 20,
                percentage: 1.0,
            }],
            dominant_hue_bucket: "red".to_string(),
            mean_luma: 0.27,
            mean_saturation: 0.9,
            colorfulness: 0.6,
            contrast: 0.02,
            analyzed_at: "2026-05-17T00:00:00Z".to_string(),
        })
        .unwrap();
        db.store_image_color_metrics(&ImageColorMetrics {
            image_id: "blue".to_string(),
            analyzer_version: "color-v1".to_string(),
            dominant_hex: "#1848f0".to_string(),
            palette: vec![ImagePaletteColor {
                hex: "#1848f0".to_string(),
                red: 24,
                green: 72,
                blue: 240,
                percentage: 1.0,
            }],
            dominant_hue_bucket: "blue".to_string(),
            mean_luma: 0.33,
            mean_saturation: 0.85,
            colorfulness: 0.62,
            contrast: 0.02,
            analyzed_at: "2026-05-17T00:00:00Z".to_string(),
        })
        .unwrap();

        let stored = db.get_image_color_metrics("red").unwrap().unwrap();
        assert_eq!(stored.dominant_hex, "#f20c14");
        assert_eq!(stored.palette[0].red, 242);
        assert_eq!(db.color_metrics_count().unwrap(), 2);

        let images = db.list_images_by_color_bucket("red", 10, 0).unwrap();
        assert_eq!(images.len(), 1);
        assert_eq!(images[0].image.id, "red");

        let filter = r#"{"type":"rule","field":"dominant_hue_bucket","op":"eq","value":"red"}"#;
        assert_eq!(db.count_smart_collection(filter).unwrap(), 1);
        let results = db.evaluate_smart_collection(filter).unwrap();
        assert_eq!(results[0].image.id, "red");
    }

    #[test]
    fn test_quality_metrics_are_stored_and_filterable() {
        let db = test_db();
        insert_test_image(&db, "sharp", "hash-sharp");
        insert_test_image(&db, "soft", "hash-soft");

        db.store_image_quality_metrics(&ImageQualityMetrics {
            image_id: "sharp".to_string(),
            analyzer_version: "quality-v1".to_string(),
            focus_score: 250.0,
            blur_score: 0.2,
            exposure_score: 0.9,
            clipped_shadow_pct: 0.01,
            clipped_highlight_pct: 0.01,
            mean_luma: 0.5,
            contrast: 0.4,
            analyzed_at: "2026-05-07T00:00:00Z".to_string(),
        })
        .unwrap();
        db.store_image_quality_metrics(&ImageQualityMetrics {
            image_id: "soft".to_string(),
            analyzer_version: "quality-v1".to_string(),
            focus_score: 12.0,
            blur_score: 0.9,
            exposure_score: 0.7,
            clipped_shadow_pct: 0.0,
            clipped_highlight_pct: 0.0,
            mean_luma: 0.45,
            contrast: 0.1,
            analyzed_at: "2026-05-07T00:00:00Z".to_string(),
        })
        .unwrap();

        let metrics = db.get_image_quality_metrics("sharp").unwrap().unwrap();
        assert_eq!(metrics.analyzer_version, "quality-v1");
        assert_eq!(db.quality_metrics_count().unwrap(), 2);

        let filter = r#"{"type":"rule","field":"focus_score","op":"gte","value":100.0}"#;
        assert_eq!(db.count_smart_collection(filter).unwrap(), 1);
        let results = db.evaluate_smart_collection(filter).unwrap();
        assert_eq!(results[0].image.id, "sharp");
    }

    #[test]
    fn test_similarity_groups_are_replaced_and_listed() {
        let db = test_db();
        insert_test_image(&db, "img-1", "hash-sg-1");
        insert_test_image(&db, "img-2", "hash-sg-2");
        insert_test_image(&db, "img-3", "hash-sg-3");

        let groups = vec![vec![
            ("img-1".to_string(), 1.0),
            ("img-2".to_string(), 0.91),
            ("img-3".to_string(), 0.89),
        ]];
        let result = db
            .replace_similarity_groups("clip-vit-b32", 0.88, "test_method", &groups, 0)
            .unwrap();
        assert_eq!(result.groups_created, 1);
        assert_eq!(result.images_grouped, 3);

        let summaries = db.list_similarity_groups(10, 0).unwrap();
        assert_eq!(summaries.len(), 1);
        assert_eq!(summaries[0].image_count, 3);

        let images = db.list_similarity_group_images(&summaries[0].id).unwrap();
        let ids: Vec<&str> = images.iter().map(|img| img.image.id.as_str()).collect();
        assert_eq!(ids, vec!["img-1", "img-2", "img-3"]);

        let replacement = vec![vec![
            ("img-1".to_string(), 1.0),
            ("img-2".to_string(), 0.92),
        ]];
        db.replace_similarity_groups("clip-vit-b32", 0.9, "test_method", &replacement, 1)
            .unwrap();
        let summaries = db.list_similarity_groups(10, 0).unwrap();
        assert_eq!(summaries.len(), 1);
        assert_eq!(summaries[0].image_count, 2);
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

    #[test]
    fn test_wal_mode_enabled() {
        let tmp = tempfile::tempdir().unwrap();
        let db = Database::open(&tmp.path().join("wal_test.db")).unwrap();
        let conn = db.conn.lock();
        let mode: String = conn
            .pragma_query_value(None, "journal_mode", |row| row.get(0))
            .unwrap();
        assert_eq!(mode.to_lowercase(), "wal");
    }

    #[test]
    fn test_busy_timeout_configured() {
        let db = test_db();
        let conn = db.conn.lock();
        let timeout: i64 = conn
            .pragma_query_value(None, "busy_timeout", |row| row.get(0))
            .unwrap();
        assert_eq!(timeout, 5000);
    }

    #[test]
    fn test_concurrent_read_write_lock_hold_times() {
        use std::sync::Arc;
        use std::thread;
        use std::time::{Duration, Instant};

        let tmp = tempfile::tempdir().unwrap();
        let db_path = tmp.path().join("bench.db");
        let db = Arc::new(Database::open(&db_path).unwrap());

        // Seed with images
        for i in 0..200 {
            insert_test_image(&db, &format!("img-{}", i), &format!("hash-{}", i));
        }

        let db_clone = Arc::clone(&db);
        let writer = thread::spawn(move || {
            let mut max_hold = Duration::ZERO;
            for i in 0..100 {
                let t0 = Instant::now();
                {
                    let conn = db_clone.conn.lock();
                    let _ = conn.execute(
                        "INSERT OR REPLACE INTO selections (image_id, project_id, decision, rating, color_label) VALUES (?1, '__global__', 'accept', 3, '')",
                        rusqlite::params![format!("img-{}", i % 200)],
                    );
                }
                let hold = t0.elapsed();
                if hold > max_hold {
                    max_hold = hold;
                }
            }
            max_hold
        });

        let db_clone2 = Arc::clone(&db);
        let reader = thread::spawn(move || {
            let mut max_hold = Duration::ZERO;
            for _ in 0..100 {
                let t0 = Instant::now();
                {
                    let conn = db_clone2.conn.lock();
                    let _ = conn.query_row("SELECT COUNT(*) FROM images", [], |row| {
                        row.get::<_, i64>(0)
                    });
                }
                let hold = t0.elapsed();
                if hold > max_hold {
                    max_hold = hold;
                }
            }
            max_hold
        });

        let writer_max = writer.join().unwrap();
        let reader_max = reader.join().unwrap();

        // Document findings: with a single Mutex<Connection>, all access
        // serializes. WAL mode helps when multiple connections exist, but the
        // current single-connection architecture means lock contention is the
        // bottleneck. Report the measured hold times.
        eprintln!(
            "Lock hold times — writer max: {:.2}ms, reader max: {:.2}ms",
            writer_max.as_secs_f64() * 1000.0,
            reader_max.as_secs_f64() * 1000.0,
        );

        // Under normal desktop load, holds should be well under 50ms even
        // with contention from a competing thread.
        let threshold = Duration::from_millis(50);
        assert!(
            writer_max < threshold,
            "Writer lock hold exceeded threshold: {:.2}ms",
            writer_max.as_secs_f64() * 1000.0
        );
        assert!(
            reader_max < threshold,
            "Reader lock hold exceeded threshold: {:.2}ms",
            reader_max.as_secs_f64() * 1000.0
        );
    }
}
