// Copyright (c) 2026-present Gleb Kalinin. Architecture and design by author.
use crate::db_core::db::{map_image_with_file_row, validate_delete_folder_path};

// Implementation assisted by Claude (Anthropic). See AUTHORSHIP.md.

use crate::db_core::db::Database;
use crate::db_core::models::*;
use rusqlite::{params, OptionalExtension, Result};

impl Database {
    pub fn insert_image(&self, image: &Image) -> Result<()> {
        let conn = self.conn.lock();
        conn.execute(
            "INSERT OR IGNORE INTO images (id, sha256_hash, width, height, format, file_size, created_at, imported_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
            params![image.id, image.sha256_hash, image.width, image.height,
                    image.format, image.file_size, image.created_at, image.imported_at],
        )?;
        let media_type = if image.format.eq_ignore_ascii_case("pdf") {
            "pdf"
        } else {
            "image"
        };
        let media_asset = MediaAsset {
            id: format!("ma_{}", image.id),
            media_type: media_type.to_string(),
            primary_image_id: image.id.clone(),
            sha256_hash: image.sha256_hash.clone(),
            format: image.format.clone(),
            file_size: image.file_size,
            page_count: None,
            title: None,
            created_at: image.created_at.clone(),
            imported_at: image.imported_at.clone(),
        };
        conn.execute(
            "INSERT OR IGNORE INTO media_assets (id, media_type, primary_image_id, sha256_hash, format, file_size, page_count, title, created_at, imported_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
            params![
                media_asset.id,
                media_asset.media_type,
                media_asset.primary_image_id,
                media_asset.sha256_hash,
                media_asset.format,
                media_asset.file_size,
                media_asset.page_count,
                media_asset.title,
                media_asset.created_at,
                media_asset.imported_at
            ],
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
             ORDER BY i.imported_at DESC, i.id ASC
             LIMIT ?1 OFFSET ?2",
        )?;
        let rows = stmt.query_map(params![limit, offset], map_image_with_file_row)?;
        rows.collect::<Result<Vec<_>>>()
    }

    pub fn list_images_in_scope(
        &self,
        folders: &[String],
        collections: &[String],
        tag_norms: &[String],
        limit: u32,
        offset: u32,
    ) -> Result<Vec<ImageWithFile>> {
        use rusqlite::types::Value;

        if folders.is_empty() && collections.is_empty() && tag_norms.is_empty() {
            return Ok(Vec::new());
        }

        let mut clauses: Vec<String> = Vec::new();
        let mut args: Vec<Value> = Vec::new();

        for folder in folders {
            let folder = folder.trim_end_matches('/');
            // Exact folder OR any descendant. The trailing "/%" keeps sibling
            // prefixes like /artisan from matching a /art scope.
            clauses.push("(f.path = ? OR f.path LIKE ? || '/%')".to_string());
            args.push(Value::Text(folder.to_string()));
            args.push(Value::Text(folder.to_string()));
        }
        if !collections.is_empty() {
            let placeholders = vec!["?"; collections.len()].join(",");
            clauses.push(format!(
                "i.id IN (SELECT image_id FROM collection_items WHERE collection_id IN ({}))",
                placeholders
            ));
            for c in collections {
                args.push(Value::Text(c.clone()));
            }
        }
        if !tag_norms.is_empty() {
            let placeholders = vec!["?"; tag_norms.len()].join(",");
            clauses.push(format!(
                "i.id IN (SELECT it.image_id FROM image_tags it JOIN tags t ON t.id = it.tag_id \
                 WHERE t.normalized_name IN ({}))",
                placeholders
            ));
            for t in tag_norms {
                args.push(Value::Text(t.clone()));
            }
        }

        let sql = format!(
            "SELECT i.id, i.sha256_hash, i.width, i.height, i.format, i.file_size,
                    i.created_at, i.imported_at, f.path,
                    s.star_rating, s.color_label, s.decision, i.source_label, i.ai_prompt,
                    i.raw_metadata, f.missing_at
             FROM images i
             JOIN image_files f ON f.image_id = i.id AND f.missing_at IS NULL
             LEFT JOIN selections s ON s.image_id = i.id AND s.project_id = '__global__'
             WHERE {}
             GROUP BY i.id
             ORDER BY i.imported_at DESC, i.id ASC
             LIMIT ? OFFSET ?",
            clauses.join(" OR ")
        );
        args.push(Value::Integer(limit as i64));
        args.push(Value::Integer(offset as i64));

        let conn = self.conn.lock();
        let mut stmt = conn.prepare(&sql)?;
        let rows = stmt.query_map(rusqlite::params_from_iter(args), map_image_with_file_row)?;
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
            let selection =
                Selection::from_nullable_parts(row.get(0)?, None, star, color, decision);
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

    pub fn list_folders(&self) -> Result<Vec<(String, u32)>> {
        // Group/count/sort in SQLite instead of streaming every image path into
        // a Rust HashMap. The parent directory is derived with the standard
        // rtrim dirname trick: rtrim(path, <all non-'/' chars of path>) strips
        // the trailing basename up to the last '/', and the outer rtrim drops
        // that trailing '/'. This matches std::path::Path::parent for the
        // absolute file paths stored at import time.
        // The CASE matches std::path::Path::parent exactly: a path with no '/'
        // has no parent (excluded, like `None`); a root-level file ("/x.png")
        // whose stripped prefix is empty maps to "/".
        let conn = self.conn.lock();
        let mut stmt = conn.prepare(
            "SELECT folder, COUNT(*) AS cnt
             FROM (
                 SELECT CASE
                     WHEN instr(f.path, '/') = 0 THEN NULL
                     WHEN rtrim(rtrim(f.path, replace(f.path, '/', '')), '/') = '' THEN '/'
                     ELSE rtrim(rtrim(f.path, replace(f.path, '/', '')), '/')
                 END AS folder
                 FROM image_files f
                 WHERE f.missing_at IS NULL
             )
             WHERE folder IS NOT NULL
             GROUP BY folder
             ORDER BY folder",
        )?;
        let rows = stmt.query_map([], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, u32>(1)?))
        })?;
        rows.collect::<Result<Vec<_>>>()
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
            let selection =
                Selection::from_nullable_parts(row.get(0)?, None, star, color, decision);
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

    pub fn image_count(&self) -> Result<u32> {
        let conn = self.conn.lock();
        conn.query_row(
            "SELECT COUNT(DISTINCT i.id) FROM images i
             JOIN image_files f ON f.image_id = i.id AND f.missing_at IS NULL",
            [],
            |row| row.get(0),
        )
    }

    pub fn list_image_ids(&self) -> Result<Vec<String>> {
        let conn = self.conn.lock();
        let mut stmt = conn.prepare(
            "SELECT DISTINCT i.id
             FROM images i
             JOIN image_files f ON f.image_id = i.id AND f.missing_at IS NULL
             ORDER BY i.imported_at DESC",
        )?;
        let rows = stmt.query_map([], |row| row.get(0))?;
        rows.collect::<Result<Vec<_>>>()
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
            let selection =
                Selection::from_nullable_parts(row.get(0)?, None, star, color, decision);
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
            let selection =
                Selection::from_nullable_parts(row.get(0)?, None, star, color, decision);
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

    pub fn delete_images_by_folder(&self, folder: &str) -> Result<u32> {
        validate_delete_folder_path(folder)?;

        let conn = self.conn.lock();
        let prefix = format!("{}/", folder.trim_end_matches('/'));
        let prefix_len = prefix.chars().count() as i64;

        // Get image IDs that ONLY exist in this folder (no other paths)
        let mut stmt = conn.prepare(
            "SELECT DISTINCT f.image_id FROM image_files f
             WHERE substr(f.path, 1, ?2) COLLATE BINARY = ?1 COLLATE BINARY
             AND f.missing_at IS NULL
             AND f.image_id NOT IN (
                 SELECT image_id FROM image_files
                 WHERE substr(path, 1, ?2) COLLATE BINARY != ?1 COLLATE BINARY
                 AND missing_at IS NULL
             )",
        )?;
        let image_ids: Vec<String> = stmt
            .query_map(params![&prefix, prefix_len], |row| row.get(0))?
            .filter_map(|r| r.ok())
            .collect();

        let count = image_ids.len() as u32;

        // Delete the images (CASCADE will handle image_files, selections, etc.)
        for id in &image_ids {
            conn.execute("DELETE FROM images WHERE id = ?1", params![id])?;
        }

        // Also delete file records from this folder for images that still exist elsewhere
        conn.execute(
            "DELETE FROM image_files
             WHERE substr(path, 1, ?2) COLLATE BINARY = ?1 COLLATE BINARY",
            params![&prefix, prefix_len],
        )?;

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
        let media_file_id = format!("mf_{}", file_id);
        conn.execute(
            "UPDATE media_files
             SET path = ?2, last_seen_at = datetime('now'), missing_at = NULL
             WHERE id = ?1",
            params![media_file_id, new_path],
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
            let media_file_id = format!("mf_{}", fid);
            let _ = conn.execute(
                "UPDATE media_files SET path = ?2, missing_at = NULL, last_seen_at = datetime('now') WHERE id = ?1",
                params![media_file_id, new_path],
            );
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

    pub fn list_image_files_under_path(&self, folder_path: &str) -> Result<Vec<(String, String)>> {
        let prefix = format!("{}/", folder_path.trim_end_matches('/'));
        let conn = self.conn.lock();
        let mut stmt =
            conn.prepare("SELECT id, path FROM image_files WHERE path = ?1 OR path LIKE ?2")?;
        let rows = stmt.query_map(params![folder_path, prefix + "%"], |row| {
            Ok((row.get(0)?, row.get(1)?))
        })?;
        rows.collect::<Result<Vec<_>>>()
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
        let media_asset_id: Option<String> = conn
            .query_row(
                "SELECT id FROM media_assets WHERE primary_image_id = ?1",
                params![new_image_id],
                |row| row.get(0),
            )
            .optional()?;
        if let Some(media_asset_id) = media_asset_id {
            let media_file_id = format!("mf_{}", file_id);
            conn.execute(
                "UPDATE media_files
                 SET media_asset_id = ?2, last_seen_at = datetime('now'), missing_at = NULL,
                     last_seen_size = ?3, last_seen_mtime = ?4
                 WHERE id = ?1",
                params![media_file_id, media_asset_id, size as i64, mtime,],
            )?;
        }
        Ok(())
    }

    pub fn insert_image_file(&self, file: &ImageFile) -> Result<()> {
        let conn = self.conn.lock();
        conn.execute(
            "INSERT OR REPLACE INTO image_files (id, image_id, path, last_seen_at, missing_at, last_seen_size, last_seen_mtime)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            params![file.id, file.image_id, file.path, file.last_seen_at, file.missing_at, file.last_seen_size, file.last_seen_mtime],
        )?;

        let media_asset_id = conn
            .query_row(
                "SELECT id FROM media_assets WHERE primary_image_id = ?1",
                params![&file.image_id],
                |row| row.get::<_, String>(0),
            )
            .optional()?;

        if let Some(media_asset_id) = media_asset_id {
            conn.execute(
                "INSERT OR REPLACE INTO media_files (
                    id, media_asset_id, path, last_seen_at, missing_at, last_seen_size, last_seen_mtime
                ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
                params![
                    format!("mf_{}", file.id),
                    media_asset_id,
                    file.path,
                    file.last_seen_at,
                    file.missing_at,
                    file.last_seen_size,
                    file.last_seen_mtime,
                ],
            )?;
        }

        Ok(())
    }

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
}
