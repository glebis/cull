// Copyright (c) 2026-present Gleb Kalinin. Architecture and design by author.
// Implementation assisted by Claude (Anthropic). See AUTHORSHIP.md.

use crate::db_core::db::Database;
use crate::db_core::db::{
    catalog_work_image_id, map_catalog_field_def_row, map_catalog_field_value_row,
    map_catalog_preset_row, map_catalog_value_event_row, map_catalog_work_image_row,
    map_catalog_work_row,
};
use rusqlite::ToSql;

use crate::db_core::models::*;
use rusqlite::{params, OptionalExtension, Result};

impl Database {
    pub fn list_catalog_presets(&self) -> Result<Vec<CatalogPreset>> {
        let conn = self.conn.lock();
        let mut stmt = conn.prepare(
            "SELECT id, name, description, preset_kind, field_def_ids_json, layout_json,
                    version, created_at, updated_at
             FROM catalog_presets
             ORDER BY updated_at DESC",
        )?;
        let rows = stmt.query_map([], map_catalog_preset_row)?;
        rows.collect()
    }

    pub fn get_catalog_preset(&self, preset_id: &str) -> Result<Option<CatalogPreset>> {
        let conn = self.conn.lock();
        conn.query_row(
            "SELECT id, name, description, preset_kind, field_def_ids_json, layout_json,
                    version, created_at, updated_at
             FROM catalog_presets WHERE id = ?1",
            params![preset_id],
            map_catalog_preset_row,
        )
        .optional()
    }

    pub fn list_catalog_fields(
        &self,
        subject_scope: Option<&str>,
        include_deprecated: bool,
    ) -> Result<Vec<CatalogFieldDef>> {
        let conn = self.conn.lock();
        let mut query =
            "SELECT id, stable_key, label, description, subject_scope, value_type, cardinality,
                    unit_kind, validation_json, sensitivity, derived_source, crosswalk_json,
                    version, supersedes_field_def_id, created_at, deprecated_at
             FROM catalog_field_defs"
                .to_string();
        let mut filters = Vec::new();
        let mut query_args: Vec<String> = Vec::new();

        if let Some(scope) = subject_scope {
            filters.push("subject_scope = ?");
            query_args.push(scope.to_string());
        }
        if !include_deprecated {
            filters.push("deprecated_at IS NULL");
        }

        if !filters.is_empty() {
            query.push_str(" WHERE ");
            query.push(' ');
            query.push_str(&filters.join(" AND "));
        }

        query.push_str(" ORDER BY stable_key");

        let mut stmt = conn.prepare(&query)?;
        let query_refs: Vec<&dyn ToSql> =
            query_args.iter().map(|value| value as &dyn ToSql).collect();
        let rows = stmt.query_map(
            rusqlite::params_from_iter(query_refs),
            map_catalog_field_def_row,
        )?;
        rows.collect()
    }

    pub fn get_catalog_field_def_by_stable_key(
        &self,
        stable_key: &str,
    ) -> Result<Option<CatalogFieldDef>> {
        let conn = self.conn.lock();
        conn.query_row(
            "SELECT id, stable_key, label, description, subject_scope, value_type, cardinality,
                    unit_kind, validation_json, sensitivity, derived_source, crosswalk_json,
                    version, supersedes_field_def_id, created_at, deprecated_at
             FROM catalog_field_defs WHERE stable_key = ?1",
            params![stable_key],
            map_catalog_field_def_row,
        )
        .optional()
    }

    pub fn get_catalog_field_def(&self, field_def_id: &str) -> Result<Option<CatalogFieldDef>> {
        let conn = self.conn.lock();
        conn.query_row(
            "SELECT id, stable_key, label, description, subject_scope, value_type, cardinality,
                    unit_kind, validation_json, sensitivity, derived_source, crosswalk_json,
                    version, supersedes_field_def_id, created_at, deprecated_at
             FROM catalog_field_defs WHERE id = ?1",
            params![field_def_id],
            map_catalog_field_def_row,
        )
        .optional()
    }

    pub fn create_catalog_field_def(
        &self,
        stable_key: &str,
        label: &str,
        description: Option<&str>,
        subject_scope: &str,
        value_type: &str,
        cardinality: &str,
        unit_kind: Option<&str>,
        validation_json: Option<&str>,
        sensitivity: &str,
        derived_source: Option<&str>,
        crosswalk_json: Option<&str>,
    ) -> Result<String> {
        let id = format!("cfd_{}", uuid::Uuid::new_v4().to_string().replace('-', ""));
        let now = chrono::Utc::now().to_rfc3339();
        let conn = self.conn.lock();
        conn.execute(
            "INSERT INTO catalog_field_defs (
                id, stable_key, label, description, subject_scope, value_type, cardinality,
                unit_kind, validation_json, sensitivity, derived_source, crosswalk_json,
                version, supersedes_field_def_id, created_at, deprecated_at
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, 1, NULL, ?13, NULL)",
            params![
                id,
                stable_key,
                label,
                description,
                subject_scope,
                value_type,
                cardinality,
                unit_kind,
                validation_json,
                sensitivity,
                derived_source,
                crosswalk_json,
                now
            ],
        )?;
        Ok(id)
    }

    pub fn deprecate_catalog_field_def(&self, field_def_id: &str) -> Result<()> {
        let now = chrono::Utc::now().to_rfc3339();
        let conn = self.conn.lock();
        conn.execute(
            "UPDATE catalog_field_defs SET deprecated_at = COALESCE(deprecated_at, ?1) WHERE id = ?2",
            params![now, field_def_id],
        )?;
        Ok(())
    }

    pub fn create_catalog_preset(
        &self,
        name: &str,
        description: Option<&str>,
        preset_kind: &str,
        field_def_ids: &[String],
        layout_json: Option<&str>,
    ) -> Result<String> {
        if field_def_ids.is_empty() {
            return Err(rusqlite::Error::InvalidParameterName(
                "catalog preset requires at least one field".to_string(),
            )
            .into());
        }
        let id = format!("cp_{}", uuid::Uuid::new_v4().to_string().replace('-', ""));
        let now = chrono::Utc::now().to_rfc3339();
        let conn = self.conn.lock();
        let field_def_ids_json = serde_json::to_string(field_def_ids)
            .map_err(|error| rusqlite::Error::ToSqlConversionFailure(error.into()))?;
        conn.execute(
            "INSERT INTO catalog_presets (
                id, name, description, preset_kind, field_def_ids_json, layout_json,
                version, created_at, updated_at
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, 1, ?7, ?8)",
            params![
                id,
                name,
                description,
                preset_kind,
                field_def_ids_json,
                layout_json,
                now,
                now
            ],
        )?;
        Ok(id)
    }

    pub fn update_catalog_preset(
        &self,
        preset_id: &str,
        name: Option<&str>,
        description: Option<&str>,
        field_def_ids: Option<&[String]>,
        layout_json: Option<&str>,
    ) -> Result<()> {
        let now = chrono::Utc::now().to_rfc3339();
        let conn = self.conn.lock();
        let mut fields: Vec<String> = Vec::new();
        let mut args: Vec<String> = Vec::new();

        if let Some(name) = name {
            fields.push("name = ?".to_string());
            args.push(name.to_string());
        }
        if let Some(description) = description {
            fields.push("description = ?".to_string());
            args.push(description.to_string());
        }
        if let Some(field_def_ids) = field_def_ids {
            let field_def_ids_json = serde_json::to_string(field_def_ids)
                .map_err(|error| rusqlite::Error::ToSqlConversionFailure(error.into()))?;
            fields.push("field_def_ids_json = ?".to_string());
            args.push(field_def_ids_json);
        }
        if let Some(layout_json) = layout_json {
            fields.push("layout_json = ?".to_string());
            args.push(layout_json.to_string());
        }

        if fields.is_empty() {
            return Ok(());
        }

        fields.push("updated_at = ?".to_string());
        fields.push("version = version + 1".to_string());
        args.push(now.clone());

        let mut sql = String::from("UPDATE catalog_presets SET ");
        sql.push_str(&fields.join(", "));
        sql.push_str(" WHERE id = ?");
        args.push(preset_id.to_string());

        let param_refs: Vec<&dyn ToSql> = args.iter().map(|value| value as &dyn ToSql).collect();
        conn.execute(&sql, rusqlite::params_from_iter(param_refs))?;
        Ok(())
    }

    pub fn get_catalog_work(&self, work_id: &str) -> Result<Option<CatalogWork>> {
        let conn = self.conn.lock();
        conn.query_row(
            "SELECT id, primary_image_id, created_at, updated_at, deleted_at
             FROM catalog_works WHERE id = ?1 AND deleted_at IS NULL",
            params![work_id],
            map_catalog_work_row,
        )
        .optional()
    }

    pub fn list_catalog_works(&self) -> Result<Vec<CatalogWork>> {
        let conn = self.conn.lock();
        let mut stmt = conn.prepare(
            "SELECT id, primary_image_id, created_at, updated_at, deleted_at FROM catalog_works
             WHERE deleted_at IS NULL ORDER BY updated_at DESC",
        )?;
        let rows = stmt.query_map([], map_catalog_work_row)?;
        rows.collect()
    }

    pub fn create_catalog_work(&self, primary_image_id: &str) -> Result<String> {
        let conn = self.conn.lock();
        let image_exists: i64 = conn.query_row(
            "SELECT COUNT(*) FROM images WHERE id = ?1",
            params![primary_image_id],
            |row| row.get(0),
        )?;
        if image_exists == 0 {
            return Err(rusqlite::Error::InvalidParameterName(format!(
                "Image '{}' not found",
                primary_image_id
            ))
            .into());
        }

        let conn = self.conn.lock();
        let id = format!("cw_{}", uuid::Uuid::new_v4().to_string().replace('-', ""));
        let now = chrono::Utc::now().to_rfc3339();
        conn.execute(
            "INSERT INTO catalog_works (id, primary_image_id, created_at, updated_at, deleted_at)
             VALUES (?1, ?2, ?3, ?4, NULL)",
            params![id, primary_image_id, now.clone(), now.clone()],
        )?;
        let image_link_id = catalog_work_image_id(&id, primary_image_id, "primary");
        conn.execute(
            "INSERT INTO catalog_work_images (
                id, work_id, image_id, role, ordinal, edition_label, created_at
             ) VALUES (?1, ?2, ?3, 'primary', 0, NULL, ?4)",
            params![image_link_id, id, primary_image_id, now],
        )?;
        Ok(id)
    }

    pub fn list_catalog_work_images(&self, work_id: &str) -> Result<Vec<CatalogWorkImage>> {
        let conn = self.conn.lock();
        let mut stmt = conn.prepare(
            "SELECT id, work_id, image_id, role, ordinal, edition_label, created_at
             FROM catalog_work_images WHERE work_id = ?1 ORDER BY ordinal, id",
        )?;
        let rows = stmt.query_map(params![work_id], map_catalog_work_image_row)?;
        rows.collect()
    }

    pub fn attach_images_to_catalog_work(
        &self,
        work_id: &str,
        images: &[(String, String, i64, Option<String>)],
    ) -> Result<u32> {
        let conn = self.conn.lock();
        let work_exists: i64 = conn.query_row(
            "SELECT COUNT(*) FROM catalog_works WHERE id = ?1 AND deleted_at IS NULL",
            params![work_id],
            |row| row.get(0),
        )?;
        if work_exists == 0 {
            return Err(rusqlite::Error::InvalidParameterName(format!(
                "Work '{}' not found",
                work_id
            ))
            .into());
        }

        let mut attached = 0u32;
        for (image_id, role, ordinal, edition_label) in images {
            let image_exists: i64 = conn.query_row(
                "SELECT COUNT(*) FROM images WHERE id = ?1",
                params![image_id],
                |row| row.get(0),
            )?;
            if image_exists == 0 {
                continue;
            }

            let id = catalog_work_image_id(work_id, image_id, role);
            conn.execute(
                "INSERT INTO catalog_work_images (id, work_id, image_id, role, ordinal, edition_label, created_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, datetime('now'))
                 ON CONFLICT(work_id, image_id, role) DO UPDATE SET
                    ordinal = excluded.ordinal,
                    edition_label = excluded.edition_label",
                params![id, work_id, image_id, role, ordinal, edition_label],
            )?;
            attached += 1;
        }
        Ok(attached)
    }

    pub fn list_catalog_values(
        &self,
        subject_type: Option<&str>,
        subject_id: Option<&str>,
        status: Option<&str>,
        source_type: Option<&str>,
        field_def_id: Option<&str>,
    ) -> Result<Vec<CatalogFieldValue>> {
        let conn = self.conn.lock();
        let mut query = String::from(
            "SELECT id, subject_type, subject_id, field_def_id, value_json, display_value,
                    source_type, source_id, confidence, status, approved_by, approved_at,
                    created_at, updated_at
             FROM catalog_field_values",
        );
        let mut clauses = Vec::new();
        if subject_type.is_some() {
            clauses.push("subject_type = ?");
        }
        if subject_id.is_some() {
            clauses.push("subject_id = ?");
        }
        if status.is_some() {
            clauses.push("status = ?");
        }
        if source_type.is_some() {
            clauses.push("source_type = ?");
        }
        if field_def_id.is_some() {
            clauses.push("field_def_id = ?");
        }
        if !clauses.is_empty() {
            query.push_str(" WHERE ");
            query.push_str(&clauses.join(" AND "));
        }
        query.push_str(" ORDER BY updated_at DESC");

        let mut params_vec: Vec<String> = Vec::new();
        if let Some(subject_type) = subject_type {
            params_vec.push(subject_type.to_string());
        }
        if let Some(subject_id) = subject_id {
            params_vec.push(subject_id.to_string());
        }
        if let Some(status) = status {
            params_vec.push(status.to_string());
        }
        if let Some(source_type) = source_type {
            params_vec.push(source_type.to_string());
        }
        if let Some(field_def_id) = field_def_id {
            params_vec.push(field_def_id.to_string());
        }

        let mut stmt = conn.prepare(&query)?;
        let params_refs: Vec<&dyn ToSql> =
            params_vec.iter().map(|value| value as &dyn ToSql).collect();
        let rows = stmt.query_map(
            rusqlite::params_from_iter(params_refs),
            map_catalog_field_value_row,
        )?;
        rows.collect()
    }

    pub fn list_catalog_drafts(
        &self,
        subject_type: Option<&str>,
        subject_id: Option<&str>,
        source_type: Option<&str>,
    ) -> Result<Vec<CatalogFieldValue>> {
        self.list_catalog_values(subject_type, subject_id, Some("draft"), source_type, None)
    }

    pub fn get_catalog_record(
        &self,
        subject_type: &str,
        subject_id: &str,
    ) -> Result<Option<CatalogRecord>> {
        let normalized_subject = match subject_type {
            "image" => "image",
            "work" => "work",
            other => {
                return Err(rusqlite::Error::InvalidParameterName(format!(
                    "Unsupported subject_type '{}'",
                    other
                ))
                .into());
            }
        };

        let exists: i64 = match normalized_subject {
            "image" => {
                let conn = self.conn.lock();
                conn.query_row(
                    "SELECT COUNT(*) FROM images WHERE id = ?1",
                    params![subject_id],
                    |row| row.get(0),
                )?
            }
            "work" => {
                let conn = self.conn.lock();
                conn.query_row(
                    "SELECT COUNT(*) FROM catalog_works WHERE id = ?1 AND deleted_at IS NULL",
                    params![subject_id],
                    |row| row.get(0),
                )?
            }
            _ => 0,
        };
        if exists == 0 {
            return Ok(None);
        }

        let values =
            self.list_catalog_values(Some(normalized_subject), Some(subject_id), None, None, None)?;
        if normalized_subject == "work" {
            let work = self.get_catalog_work(subject_id)?.unwrap_or(CatalogWork {
                id: subject_id.to_string(),
                primary_image_id: String::new(),
                created_at: String::new(),
                updated_at: String::new(),
                deleted_at: None,
            });
            let images = self.list_catalog_work_images(subject_id)?;
            Ok(Some(CatalogRecord {
                subject_type: normalized_subject.to_string(),
                subject_id: subject_id.to_string(),
                work: Some(work),
                work_images: images,
                values,
            }))
        } else {
            Ok(Some(CatalogRecord {
                subject_type: normalized_subject.to_string(),
                subject_id: subject_id.to_string(),
                work: None,
                work_images: Vec::new(),
                values,
            }))
        }
    }

    pub fn upsert_catalog_draft_value(
        &self,
        subject_type: &str,
        subject_id: &str,
        field_def_id: &str,
        value_json: &str,
        display_value: &str,
        source_type: &str,
        source_id: Option<&str>,
        confidence: Option<f64>,
        actor_type: &str,
        actor_id: Option<&str>,
        status: &str,
    ) -> Result<String> {
        let mut conn = self.conn.lock();
        let tx = conn.transaction()?;

        if status != "draft"
            && status != "approved"
            && status != "rejected"
            && status != "superseded"
        {
            return Err(rusqlite::Error::InvalidParameterName(format!(
                "Unsupported catalog value status '{}'",
                status
            ))
            .into());
        }
        let source_type = if source_type == "agent" {
            "agent"
        } else {
            source_type
        };
        let effective_status = if source_type == "agent" {
            "draft"
        } else {
            status
        };
        let now = chrono::Utc::now().to_rfc3339();
        let source_id = source_id.unwrap_or("");

        let subject_exists: i64 = match subject_type {
            "image" => tx.query_row(
                "SELECT COUNT(*) FROM images WHERE id = ?1",
                params![subject_id],
                |row| row.get(0),
            )?,
            "work" => tx.query_row(
                "SELECT COUNT(*) FROM catalog_works WHERE id = ?1 AND deleted_at IS NULL",
                params![subject_id],
                |row| row.get(0),
            )?,
            _ => {
                return Err(rusqlite::Error::InvalidParameterName(format!(
                    "Unsupported subject_type '{}'",
                    subject_type
                ))
                .into());
            }
        };
        if subject_exists == 0 {
            return Err(rusqlite::Error::InvalidParameterName(format!(
                "Subject '{}' of type '{}' not found",
                subject_id, subject_type
            ))
            .into());
        }

        let field_def_exists: i64 = tx.query_row(
            "SELECT COUNT(*) FROM catalog_field_defs WHERE id = ?1",
            params![field_def_id],
            |row| row.get(0),
        )?;
        if field_def_exists == 0 {
            return Err(rusqlite::Error::InvalidParameterName(format!(
                "Field definition '{}' not found",
                field_def_id
            ))
            .into());
        }

        let maybe_value: Option<CatalogFieldValue> = tx
            .query_row(
                "SELECT id, subject_type, subject_id, field_def_id, value_json, display_value,
                        source_type, source_id, confidence, status, approved_by, approved_at,
                        created_at, updated_at
                 FROM catalog_field_values
                 WHERE subject_type = ?1 AND subject_id = ?2 AND field_def_id = ?3 AND source_type = ?4 AND COALESCE(source_id, '') = ?5",
                params![subject_type, subject_id, field_def_id, source_type, source_id],
                map_catalog_field_value_row,
            )
            .optional()?;

        if source_type != "user" {
            let user_approved: i64 = tx.query_row(
                "SELECT COUNT(*) FROM catalog_field_values
                 WHERE subject_type = ?1 AND subject_id = ?2 AND field_def_id = ?3
                   AND status = 'approved' AND source_type = 'user'",
                params![subject_type, subject_id, field_def_id],
                |row| row.get(0),
            )?;
            if user_approved > 0 && source_type == "agent" {
                return Err(rusqlite::Error::InvalidParameterName(
                    "agent writes cannot overwrite approved user values".to_string(),
                )
                .into());
            }
        }

        let before_json = maybe_value
            .as_ref()
            .and_then(|value| serde_json::to_string(value).ok());
        let value_id = match maybe_value {
            Some(existing) => {
                let (_new_status, approved_by, approved_at) = if effective_status == "approved" {
                    (
                        Some(actor_id.unwrap_or("approved")),
                        Some(actor_type.to_string()),
                        Some(now.clone()),
                    )
                } else {
                    (None, None, None)
                };
                tx.execute(
                    "UPDATE catalog_field_values
                        SET value_json = ?1, display_value = ?2, confidence = ?3,
                            status = ?4, approved_by = ?5, approved_at = ?6,
                            updated_at = ?7
                     WHERE id = ?8",
                    params![
                        value_json,
                        display_value,
                        confidence,
                        effective_status,
                        approved_by,
                        approved_at,
                        now,
                        existing.id
                    ],
                )?;
                let after_json = serde_json::json!({
                    "id": existing.id,
                    "subject_type": subject_type,
                    "subject_id": subject_id,
                    "field_def_id": field_def_id,
                    "value_json": value_json,
                    "display_value": display_value,
                    "source_type": source_type,
                    "status": effective_status,
                })
                .to_string();
                let event_type = if existing.status == "approved" || existing.status == "draft" {
                    "updated"
                } else {
                    "updated"
                };
                tx.execute(
                    "INSERT INTO catalog_value_events (id, value_id, event_type, actor_type, actor_id, before_json, after_json, created_at)
                     VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
                    params![
                        format!("cve_{}", uuid::Uuid::new_v4().to_string().replace('-', "")),
                        existing.id,
                        event_type,
                        actor_type,
                        actor_id,
                        before_json,
                        after_json,
                        now,
                    ],
                )?;
                existing.id
            }
            None => {
                let id = format!("cv_{}", uuid::Uuid::new_v4().to_string().replace('-', ""));
                let (approved_by, approved_at) = if effective_status == "approved" {
                    (Some(actor_type.to_string()), Some(now.clone()))
                } else {
                    (None, None)
                };
                tx.execute(
                    "INSERT INTO catalog_field_values (
                        id, subject_type, subject_id, field_def_id, value_json, display_value,
                        source_type, source_id, confidence, status, approved_by, approved_at,
                        created_at, updated_at
                     ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14)",
                    params![
                        id,
                        subject_type,
                        subject_id,
                        field_def_id,
                        value_json,
                        display_value,
                        source_type,
                        Some(&source_id),
                        confidence,
                        effective_status,
                        approved_by,
                        approved_at,
                        now,
                        now,
                    ],
                )?;
                let after_json = serde_json::json!({
                    "id": id,
                    "subject_type": subject_type,
                    "subject_id": subject_id,
                    "field_def_id": field_def_id,
                    "value_json": value_json,
                    "display_value": display_value,
                    "source_type": source_type,
                    "status": effective_status,
                })
                .to_string();
                tx.execute(
                    "INSERT INTO catalog_value_events (id, value_id, event_type, actor_type, actor_id, before_json, after_json, created_at)
                     VALUES (?1, ?2, 'created', ?3, ?4, ?5, ?6, ?7)",
                    params![
                        format!("cve_{}", uuid::Uuid::new_v4().to_string().replace('-', "")),
                        id,
                        actor_type,
                        actor_id,
                        Option::<String>::None,
                        after_json,
                        now,
                    ],
                )?;
                id
            }
        };

        tx.commit()?;
        Ok(value_id)
    }

    pub fn set_catalog_draft_values(
        &self,
        values: &[(
            String,
            String,
            String,
            String,
            String,
            Option<String>,
            Option<f64>,
            Option<String>,
        )],
        actor_type: &str,
        actor_id: Option<&str>,
    ) -> Result<Vec<String>> {
        let mut ids = Vec::new();
        for (
            subject_type,
            subject_id,
            field_def_id,
            value_json,
            display_value,
            source_type,
            confidence,
            source_id,
        ) in values
        {
            let source_type = source_type.clone().unwrap_or_else(|| "user".to_string());
            let id = self.upsert_catalog_draft_value(
                subject_type,
                subject_id,
                field_def_id,
                value_json,
                display_value,
                &source_type,
                source_id.as_deref(),
                *confidence,
                actor_type,
                actor_id,
                "draft",
            )?;
            ids.push(id);
        }
        Ok(ids)
    }

    pub fn list_catalog_value_events(&self, value_id: &str) -> Result<Vec<CatalogValueEvent>> {
        let conn = self.conn.lock();
        let mut stmt = conn.prepare(
            "SELECT id, value_id, event_type, actor_type, actor_id, before_json, after_json, created_at
             FROM catalog_value_events WHERE value_id = ?1 ORDER BY created_at ASC",
        )?;
        let rows = stmt.query_map(params![value_id], map_catalog_value_event_row)?;
        rows.collect()
    }

    pub fn approve_catalog_values(
        &self,
        value_ids: &[String],
        approved_by: Option<&str>,
    ) -> Result<u32> {
        let mut conn = self.conn.lock();
        let tx = conn.transaction()?;
        let now = chrono::Utc::now().to_rfc3339();
        let mut count = 0u32;
        for value_id in value_ids {
            let existing: Option<CatalogFieldValue> = tx
                .query_row(
                    "SELECT id, subject_type, subject_id, field_def_id, value_json, display_value,
                            source_type, source_id, confidence, status, approved_by, approved_at,
                            created_at, updated_at
                     FROM catalog_field_values WHERE id = ?1",
                    params![value_id],
                    map_catalog_field_value_row,
                )
                .optional()?;
            let Some(value) = existing else {
                continue;
            };
            if value.status == "approved" {
                continue;
            }
            tx.execute(
                "UPDATE catalog_field_values
                    SET status = 'approved', approved_by = ?1, approved_at = ?2, updated_at = ?3
                    WHERE id = ?4",
                params![approved_by.unwrap_or("admin"), now, now, value_id],
            )?;
            let after_json = serde_json::json!({
                "id": value.id,
                "status": "approved"
            })
            .to_string();
            tx.execute(
                "INSERT INTO catalog_value_events (id, value_id, event_type, actor_type, actor_id, before_json, after_json, created_at)
                 VALUES (?1, ?2, 'approved', ?3, ?4, ?5, ?6, ?7)",
                params![
                    format!("cve_{}", uuid::Uuid::new_v4().to_string().replace('-', "")),
                    value_id,
                    "user",
                    approved_by,
                    serde_json::to_string(&value).ok(),
                    after_json,
                    now
                ],
            )?;
            count += 1;
        }
        tx.commit()?;
        Ok(count)
    }

    pub fn reject_catalog_values(
        &self,
        value_ids: &[String],
        approved_by: Option<&str>,
    ) -> Result<u32> {
        let mut conn = self.conn.lock();
        let tx = conn.transaction()?;
        let now = chrono::Utc::now().to_rfc3339();
        let mut count = 0u32;
        for value_id in value_ids {
            let existing: Option<CatalogFieldValue> = tx
                .query_row(
                    "SELECT id, subject_type, subject_id, field_def_id, value_json, display_value,
                            source_type, source_id, confidence, status, approved_by, approved_at,
                            created_at, updated_at
                     FROM catalog_field_values WHERE id = ?1",
                    params![value_id],
                    map_catalog_field_value_row,
                )
                .optional()?;
            let Some(value) = existing else {
                continue;
            };
            if value.status == "rejected" {
                continue;
            }
            tx.execute(
                "UPDATE catalog_field_values
                    SET status = 'rejected', approved_by = ?1, approved_at = ?2, updated_at = ?3
                    WHERE id = ?4",
                params![approved_by.unwrap_or("admin"), now, now, value_id],
            )?;
            let after_json = serde_json::json!({
                "id": value.id,
                "status": "rejected"
            })
            .to_string();
            tx.execute(
                "INSERT INTO catalog_value_events (id, value_id, event_type, actor_type, actor_id, before_json, after_json, created_at)
                 VALUES (?1, ?2, 'rejected', ?3, ?4, ?5, ?6, ?7)",
                params![
                    format!("cve_{}", uuid::Uuid::new_v4().to_string().replace('-', "")),
                    value_id,
                    "user",
                    approved_by,
                    serde_json::to_string(&value).ok(),
                    after_json,
                    now
                ],
            )?;
            count += 1;
        }
        tx.commit()?;
        Ok(count)
    }
}
