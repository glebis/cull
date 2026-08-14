use crate::db_core::db::Database;
use rusqlite::{params, OptionalExtension, Result};
use uuid::Uuid;

#[derive(Debug, Clone)]
pub struct ExternalImportPreparation {
    pub resource_id: String,
    pub item_id: String,
    pub managed_path: String,
    pub existing_image_id: Option<String>,
}

#[derive(Debug, Clone)]
pub struct MaterializingExternalImport {
    pub item_id: String,
    pub resource_id: String,
    pub managed_path: String,
}

#[derive(Debug, Clone)]
pub struct PendingExternalImport {
    pub asset_id: String,
    pub resource_id: String,
    pub item_id: String,
}

#[derive(Debug, Clone)]
pub struct ExternalContentConsolidation {
    pub resource_id: String,
    pub managed_path: String,
    pub existing_image_id: Option<String>,
    pub discarded_path: Option<String>,
    pub discarded_content_type: Option<String>,
}

impl Database {
    pub fn consolidate_external_resource_by_content(
        &self,
        item_id: &str,
        resource_id: &str,
        content_sha256: &str,
    ) -> Result<ExternalContentConsolidation> {
        let now = chrono::Utc::now().to_rfc3339();
        let stable_fingerprint = format!("content-sha256:{content_sha256}");
        let mut conn = self.conn.lock();
        let tx = conn.transaction()?;
        let (version_id, asset_id, current_path, current_content_type): (
            String,
            String,
            String,
            Option<String>,
        ) = tx.query_row(
            "SELECT v.id, v.external_asset_id, r.managed_path, r.content_type
             FROM external_asset_resources r
             JOIN external_asset_versions v ON v.id = r.version_id
             WHERE r.id = ?1",
            [resource_id],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
        )?;
        let canonical_version = tx
            .query_row(
                "SELECT id FROM external_asset_versions
                 WHERE external_asset_id = ?1 AND representation = 'current'
                   AND version_fingerprint = ?2",
                params![asset_id, stable_fingerprint],
                |row| row.get::<_, String>(0),
            )
            .optional()?;

        if let Some(canonical_version) = canonical_version {
            let (canonical_resource, canonical_path, canonical_image, canonical_hash, state): (
                String,
                String,
                Option<String>,
                Option<String>,
                String,
            ) = tx.query_row(
                "SELECT id, managed_path, image_id, content_sha256, state
                 FROM external_asset_resources
                 WHERE version_id = ?1 AND resource_key = 'current'",
                [&canonical_version],
                |row| {
                    Ok((
                        row.get(0)?,
                        row.get(1)?,
                        row.get(2)?,
                        row.get(3)?,
                        row.get(4)?,
                    ))
                },
            )?;
            if canonical_hash.as_deref() != Some(content_sha256) {
                return Err(rusqlite::Error::InvalidQuery);
            }
            let item_state = if state == "imported" && canonical_image.is_some() {
                "imported"
            } else {
                "materialized"
            };
            tx.execute(
                "UPDATE external_import_items
                 SET resource_id = ?2, state = ?3, updated_at = ?4 WHERE id = ?1",
                params![item_id, canonical_resource, item_state, now],
            )?;
            tx.execute(
                "DELETE FROM external_asset_resources WHERE id = ?1",
                [resource_id],
            )?;
            tx.execute(
                "DELETE FROM external_asset_versions WHERE id = ?1",
                [&version_id],
            )?;
            tx.commit()?;
            Ok(ExternalContentConsolidation {
                resource_id: canonical_resource,
                managed_path: canonical_path.clone(),
                existing_image_id: canonical_image,
                discarded_path: (current_path != canonical_path).then_some(current_path),
                discarded_content_type: current_content_type,
            })
        } else {
            tx.execute(
                "UPDATE external_asset_versions
                 SET version_fingerprint = ?2 WHERE id = ?1",
                params![version_id, stable_fingerprint],
            )?;
            let existing_image_id = tx.query_row(
                "SELECT image_id FROM external_asset_resources WHERE id = ?1",
                [resource_id],
                |row| row.get(0),
            )?;
            tx.commit()?;
            Ok(ExternalContentConsolidation {
                resource_id: resource_id.to_string(),
                managed_path: current_path,
                existing_image_id,
                discarded_path: None,
                discarded_content_type: None,
            })
        }
    }

    pub fn journal_external_import_selection(
        &self,
        job_id: &str,
        batch_id: &str,
        source_album_id: Option<&str>,
        asset_ids: &[String],
    ) -> Result<Vec<PendingExternalImport>> {
        let now = chrono::Utc::now().to_rfc3339();
        let mut conn = self.conn.lock();
        let tx = conn.transaction()?;
        let mut pending = Vec::with_capacity(asset_ids.len());
        for (ordinal, provider_asset_id) in asset_ids.iter().enumerate() {
            let proposed_asset_id = Uuid::new_v4().to_string();
            tx.execute(
                "INSERT OR IGNORE INTO external_assets
                 (id, provider, provider_asset_id, created_at, updated_at)
                 VALUES (?1, 'apple_photos', ?2, ?3, ?3)",
                params![proposed_asset_id, provider_asset_id, now],
            )?;
            let asset_id: String = tx.query_row(
                "SELECT id FROM external_assets
                 WHERE provider = 'apple_photos' AND provider_asset_id = ?1",
                [provider_asset_id],
                |row| row.get(0),
            )?;
            let version_id = Uuid::new_v4().to_string();
            let fingerprint = format!("pending:{job_id}:{ordinal}");
            tx.execute(
                "INSERT INTO external_asset_versions
                 (id, external_asset_id, representation, version_fingerprint, created_at)
                 VALUES (?1, ?2, 'current', ?3, ?4)",
                params![version_id, asset_id, fingerprint, now],
            )?;
            let resource_id = Uuid::new_v4().to_string();
            let pending_path = format!("pending://apple-photos/{job_id}/{ordinal}");
            tx.execute(
                "INSERT INTO external_asset_resources
                 (id, version_id, resource_key, original_filename, managed_path, state, created_at, updated_at)
                 VALUES (?1, ?2, 'current', 'pending', ?3, 'requested', ?4, ?4)",
                params![resource_id, version_id, pending_path, now],
            )?;
            let item_id = Uuid::new_v4().to_string();
            tx.execute(
                "INSERT INTO external_import_items
                 (id, job_id, batch_id, resource_id, source_album_id, ordinal, state, created_at, updated_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, 'requested', ?7, ?7)",
                params![
                    item_id,
                    job_id,
                    batch_id,
                    resource_id,
                    source_album_id,
                    ordinal as i64,
                    now
                ],
            )?;
            pending.push(PendingExternalImport {
                asset_id: provider_asset_id.clone(),
                resource_id,
                item_id,
            });
        }
        tx.commit()?;
        Ok(pending)
    }

    #[allow(clippy::too_many_arguments)]
    pub fn bind_external_import_descriptor(
        &self,
        pending: &PendingExternalImport,
        version_fingerprint: &str,
        provider_modified_at: Option<&str>,
        filename: &str,
        managed_path: &str,
    ) -> Result<ExternalImportPreparation> {
        let now = chrono::Utc::now().to_rfc3339();
        let mut conn = self.conn.lock();
        let tx = conn.transaction()?;
        let (asset_id, pending_version_id): (String, String) = tx.query_row(
            "SELECT v.external_asset_id, v.id
             FROM external_asset_resources r
             JOIN external_asset_versions v ON v.id = r.version_id
             WHERE r.id = ?1",
            [&pending.resource_id],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )?;
        let proposed_version_id = Uuid::new_v4().to_string();
        tx.execute(
            "INSERT OR IGNORE INTO external_asset_versions
             (id, external_asset_id, representation, version_fingerprint, provider_modified_at, created_at)
             VALUES (?1, ?2, 'current', ?3, ?4, ?5)",
            params![
                proposed_version_id,
                asset_id,
                version_fingerprint,
                provider_modified_at,
                now
            ],
        )?;
        let version_id: String = tx.query_row(
            "SELECT id FROM external_asset_versions
             WHERE external_asset_id = ?1 AND representation = 'current' AND version_fingerprint = ?2",
            params![asset_id, version_fingerprint],
            |row| row.get(0),
        )?;
        let proposed_resource_id = Uuid::new_v4().to_string();
        tx.execute(
            "INSERT OR IGNORE INTO external_asset_resources
             (id, version_id, resource_key, original_filename, managed_path, state, created_at, updated_at)
             VALUES (?1, ?2, 'current', ?3, ?4, 'requested', ?5, ?5)",
            params![proposed_resource_id, version_id, filename, managed_path, now],
        )?;
        let (resource_id, stored_path, existing_image_id, resource_state): (
            String,
            String,
            Option<String>,
            String,
        ) = tx.query_row(
            "SELECT id, managed_path, image_id, state FROM external_asset_resources
             WHERE version_id = ?1 AND resource_key = 'current'",
            [&version_id],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
        )?;
        let item_state = if resource_state == "imported" && existing_image_id.is_some() {
            "imported"
        } else {
            "requested"
        };
        tx.execute(
            "UPDATE external_import_items
             SET resource_id = ?2, state = ?3, updated_at = ?4 WHERE id = ?1",
            params![pending.item_id, resource_id, item_state, now],
        )?;
        tx.execute(
            "DELETE FROM external_asset_resources WHERE id = ?1",
            [&pending.resource_id],
        )?;
        tx.execute(
            "DELETE FROM external_asset_versions WHERE id = ?1",
            [pending_version_id],
        )?;
        tx.commit()?;
        Ok(ExternalImportPreparation {
            resource_id,
            item_id: pending.item_id.clone(),
            managed_path: stored_path,
            existing_image_id,
        })
    }

    pub fn list_materializing_external_imports(&self) -> Result<Vec<MaterializingExternalImport>> {
        let conn = self.conn.lock();
        let mut statement = conn.prepare(
            "SELECT i.id, r.id, r.managed_path
             FROM external_import_items i
             JOIN external_asset_resources r ON r.id = i.resource_id
             JOIN external_asset_versions v ON v.id = r.version_id
             JOIN external_assets a ON a.id = v.external_asset_id
             WHERE a.provider = 'apple_photos'
               AND i.state = 'materializing'
             ORDER BY i.created_at, i.id",
        )?;
        let rows = statement
            .query_map([], |row| {
                Ok(MaterializingExternalImport {
                    item_id: row.get(0)?,
                    resource_id: row.get(1)?,
                    managed_path: row.get(2)?,
                })
            })?
            .collect();
        rows
    }

    #[allow(clippy::too_many_arguments)]
    pub fn prepare_external_import_item(
        &self,
        job_id: &str,
        batch_id: &str,
        ordinal: u32,
        source_album_id: Option<&str>,
        provider_asset_id: &str,
        version_fingerprint: &str,
        provider_modified_at: Option<&str>,
        filename: &str,
        managed_path: &str,
    ) -> Result<ExternalImportPreparation> {
        let now = chrono::Utc::now().to_rfc3339();
        let mut conn = self.conn.lock();
        let tx = conn.transaction()?;

        let proposed_asset_id = Uuid::new_v4().to_string();
        tx.execute(
            "INSERT OR IGNORE INTO external_assets
             (id, provider, provider_asset_id, created_at, updated_at)
             VALUES (?1, 'apple_photos', ?2, ?3, ?3)",
            params![proposed_asset_id, provider_asset_id, now],
        )?;
        let asset_id: String = tx.query_row(
            "SELECT id FROM external_assets WHERE provider = 'apple_photos' AND provider_asset_id = ?1",
            params![provider_asset_id],
            |row| row.get(0),
        )?;

        let proposed_version_id = Uuid::new_v4().to_string();
        tx.execute(
            "INSERT OR IGNORE INTO external_asset_versions
             (id, external_asset_id, representation, version_fingerprint, provider_modified_at, created_at)
             VALUES (?1, ?2, 'current', ?3, ?4, ?5)",
            params![proposed_version_id, asset_id, version_fingerprint, provider_modified_at, now],
        )?;
        let version_id: String = tx.query_row(
            "SELECT id FROM external_asset_versions
             WHERE external_asset_id = ?1 AND representation = 'current' AND version_fingerprint = ?2",
            params![asset_id, version_fingerprint],
            |row| row.get(0),
        )?;

        let proposed_resource_id = Uuid::new_v4().to_string();
        tx.execute(
            "INSERT OR IGNORE INTO external_asset_resources
             (id, version_id, resource_key, original_filename, managed_path, state, created_at, updated_at)
             VALUES (?1, ?2, 'current', ?3, ?4, 'requested', ?5, ?5)",
            params![proposed_resource_id, version_id, filename, managed_path, now],
        )?;
        let (resource_id, stored_path, existing_image_id, resource_state): (
            String,
            String,
            Option<String>,
            String,
        ) = tx.query_row(
            "SELECT id, managed_path, image_id, state FROM external_asset_resources
             WHERE version_id = ?1 AND resource_key = 'current'",
            params![version_id],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
        )?;

        let item_id = Uuid::new_v4().to_string();
        let item_state = if resource_state == "imported" && existing_image_id.is_some() {
            "imported"
        } else {
            "requested"
        };
        tx.execute(
            "INSERT INTO external_import_items
             (id, job_id, batch_id, resource_id, source_album_id, ordinal, state, created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?8)",
            params![item_id, job_id, batch_id, resource_id, source_album_id, ordinal, item_state, now],
        )?;
        tx.commit()?;

        Ok(ExternalImportPreparation {
            resource_id,
            item_id,
            managed_path: stored_path,
            existing_image_id,
        })
    }

    pub fn mark_external_import_item(
        &self,
        item_id: &str,
        resource_id: &str,
        state: &str,
        content_sha256: Option<&str>,
        byte_count: Option<u64>,
        error_code: Option<&str>,
        error_message: Option<&str>,
    ) -> Result<()> {
        let now = chrono::Utc::now().to_rfc3339();
        let mut conn = self.conn.lock();
        let tx = conn.transaction()?;
        let resource_state = if state == "inaccessible" {
            "failed"
        } else {
            state
        };
        tx.execute(
            "UPDATE external_asset_resources SET state = ?2, content_sha256 = COALESCE(?3, content_sha256),
             byte_count = COALESCE(?4, byte_count), error_code = ?5, error_message = ?6, updated_at = ?7 WHERE id = ?1",
            params![resource_id, resource_state, content_sha256, byte_count.and_then(|v| i64::try_from(v).ok()), error_code, error_message, now],
        )?;
        let item_state = if state == "failed"
            || state == "cancelled"
            || state == "skipped"
            || state == "inaccessible"
        {
            state
        } else if state == "materialized" || state == "materializing" {
            state
        } else {
            "requested"
        };
        tx.execute(
            "UPDATE external_import_items SET state = ?2, error_code = ?3, error_message = ?4, updated_at = ?5 WHERE id = ?1",
            params![item_id, item_state, error_code, error_message, now],
        )?;
        tx.commit()
    }

    pub fn finalize_external_import_item(
        &self,
        item_id: &str,
        resource_id: &str,
        image_id: &str,
        batch_id: &str,
    ) -> Result<()> {
        let now = chrono::Utc::now().to_rfc3339();
        let mut conn = self.conn.lock();
        let tx = conn.transaction()?;
        tx.execute(
            "UPDATE external_asset_resources SET image_id = ?2, state = 'imported', error_code = NULL,
             error_message = NULL, updated_at = ?3 WHERE id = ?1",
            params![resource_id, image_id, now],
        )?;
        tx.execute(
            "UPDATE external_import_items SET state = 'imported', error_code = NULL,
             error_message = NULL, updated_at = ?2 WHERE id = ?1",
            params![item_id, now],
        )?;
        tx.execute(
            "UPDATE images SET import_batch_id = COALESCE(import_batch_id, ?2) WHERE id = ?1",
            params![image_id, batch_id],
        )?;
        tx.commit()
    }

    pub fn update_external_resource_location(
        &self,
        resource_id: &str,
        managed_path: &str,
        content_type: &str,
    ) -> Result<()> {
        let conn = self.conn.lock();
        conn.execute(
            "UPDATE external_asset_resources
             SET managed_path = ?2, content_type = ?3, updated_at = ?4 WHERE id = ?1",
            params![
                resource_id,
                managed_path,
                content_type,
                chrono::Utc::now().to_rfc3339()
            ],
        )?;
        Ok(())
    }

    pub fn update_import_batch_count(&self, batch_id: &str, count: u32) -> Result<()> {
        let conn = self.conn.lock();
        conn.execute(
            "UPDATE import_batches SET image_count = ?2 WHERE id = ?1",
            params![batch_id, count],
        )?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use crate::db_core::db::Database;
    use crate::db_core::models::{Image, ImageFile};

    #[test]
    fn retry_of_imported_current_resource_reuses_existing_image() {
        let db = Database::open(std::path::Path::new(":memory:")).unwrap();
        let batch = db.create_import_batch("apple_photos", 0, None).unwrap();
        let first = db
            .prepare_external_import_item(
                "job-one",
                &batch,
                0,
                None,
                "opaque-provider-id",
                "modified:1",
                Some("2026-08-14T10:00:00Z"),
                "photo-current.jpg",
                "/managed/photo-current.jpg",
            )
            .unwrap();
        assert_eq!(first.existing_image_id, None);

        let image_id = "image-one";
        db.insert_image(&Image {
            id: image_id.into(),
            sha256_hash: "content-hash".into(),
            width: 10,
            height: 10,
            format: "jpg".into(),
            file_size: 10,
            created_at: "2026-08-14T10:00:00Z".into(),
            imported_at: "2026-08-14T10:00:00Z".into(),
            ai_prompt: None,
            raw_metadata: None,
        })
        .unwrap();
        db.insert_image_file(&ImageFile {
            id: "file-one".into(),
            image_id: image_id.into(),
            path: "/managed/photo-current.jpg".into(),
            last_seen_at: "2026-08-14T10:00:00Z".into(),
            missing_at: None,
            last_seen_size: Some(10),
            last_seen_mtime: None,
        })
        .unwrap();
        let original_batch = db.create_import_batch("folder", 1, None).unwrap();
        db.conn
            .lock()
            .execute(
                "UPDATE images SET import_batch_id = ?2 WHERE id = ?1",
                rusqlite::params![image_id, original_batch],
            )
            .unwrap();
        db.finalize_external_import_item(&first.item_id, &first.resource_id, image_id, &batch)
            .unwrap();

        let preserved_batch: Option<String> = db
            .conn
            .lock()
            .query_row(
                "SELECT import_batch_id FROM images WHERE id = ?1",
                [image_id],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(preserved_batch.as_deref(), Some(original_batch.as_str()));

        let retry_batch = db.create_import_batch("apple_photos", 0, None).unwrap();
        let retry = db
            .prepare_external_import_item(
                "job-two",
                &retry_batch,
                0,
                None,
                "opaque-provider-id",
                "modified:1",
                Some("2026-08-14T10:00:00Z"),
                "photo-current.jpg",
                "/managed/photo-current.jpg",
            )
            .unwrap();

        assert_eq!(retry.resource_id, first.resource_id);
        assert_eq!(retry.existing_image_id.as_deref(), Some(image_id));
    }
}
