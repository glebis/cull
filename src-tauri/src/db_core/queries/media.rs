// Copyright (c) 2026-present Gleb Kalinin. Architecture and design by author.
// Implementation assisted by Claude (Anthropic). See AUTHORSHIP.md.

use crate::db_core::db::Database;
use crate::db_core::db::{map_media_asset_row, map_media_file_row, map_pdf_page_row};

use crate::db_core::models::*;
use rusqlite::{params, OptionalExtension, Result};

impl Database {
    pub fn insert_media_asset(&self, asset: &MediaAsset) -> Result<()> {
        let conn = self.conn.lock();
        conn.execute(
            "INSERT OR IGNORE INTO media_assets (
                id, media_type, primary_image_id, sha256_hash, format, file_size,
                page_count, title, created_at, imported_at
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
            params![
                asset.id,
                asset.media_type,
                asset.primary_image_id,
                asset.sha256_hash,
                asset.format,
                asset.file_size,
                asset.page_count,
                asset.title,
                asset.created_at,
                asset.imported_at,
            ],
        )?;
        Ok(())
    }

    pub fn set_media_asset_page_count_by_image_id(
        &self,
        image_id: &str,
        page_count: u32,
    ) -> Result<()> {
        let conn = self.conn.lock();
        conn.execute(
            "UPDATE media_assets SET page_count = ?1 WHERE primary_image_id = ?2",
            params![page_count as i64, image_id],
        )?;
        Ok(())
    }

    pub fn set_media_asset_title_by_image_id(&self, image_id: &str, title: &str) -> Result<()> {
        let conn = self.conn.lock();
        conn.execute(
            "UPDATE media_assets SET title = ?1 WHERE primary_image_id = ?2",
            params![title, image_id],
        )?;
        Ok(())
    }

    pub fn media_asset_id_for_image_id(&self, image_id: &str) -> Result<Option<String>> {
        let conn = self.conn.lock();
        conn.query_row(
            "SELECT id FROM media_assets WHERE primary_image_id = ?1",
            params![image_id],
            |row| row.get(0),
        )
        .optional()
    }

    pub fn insert_media_file(&self, file: &MediaFile) -> Result<()> {
        let conn = self.conn.lock();
        conn.execute(
            "INSERT OR REPLACE INTO media_files (
                id, media_asset_id, path, last_seen_at, missing_at, last_seen_size, last_seen_mtime
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            params![
                file.id,
                file.media_asset_id,
                file.path,
                file.last_seen_at,
                file.missing_at,
                file.last_seen_size,
                file.last_seen_mtime,
            ],
        )?;
        Ok(())
    }

    pub fn media_asset(&self, media_asset_id: &str) -> Result<Option<MediaAsset>> {
        let conn = self.conn.lock();
        conn.query_row(
            "SELECT id, media_type, primary_image_id, sha256_hash, format, file_size,
                    page_count, title, created_at, imported_at
             FROM media_assets WHERE id = ?1",
            params![media_asset_id],
            map_media_asset_row,
        )
        .optional()
    }

    pub fn media_asset_for_image(&self, image_id: &str) -> Result<Option<MediaAsset>> {
        let conn = self.conn.lock();
        conn.query_row(
            "SELECT id, media_type, primary_image_id, sha256_hash, format, file_size,
                    page_count, title, created_at, imported_at
             FROM media_assets WHERE primary_image_id = ?1",
            params![image_id],
            map_media_asset_row,
        )
        .optional()
    }

    pub fn list_media_assets(
        &self,
        media_type: Option<&str>,
        limit: u32,
        offset: u32,
    ) -> Result<Vec<MediaAsset>> {
        let conn = self.conn.lock();
        if media_type.is_some() {
            let mut stmt = conn.prepare(
                "SELECT id, media_type, primary_image_id, sha256_hash, format, file_size,
                        page_count, title, created_at, imported_at
                 FROM media_assets WHERE media_type = ?1 ORDER BY imported_at DESC LIMIT ?2 OFFSET ?3",
            )?;
            let rows = stmt.query_map(
                params![media_type.unwrap_or_default(), limit, offset],
                map_media_asset_row,
            )?;
            return rows.collect();
        }

        let mut stmt = conn.prepare(
            "SELECT id, media_type, primary_image_id, sha256_hash, format, file_size,
                    page_count, title, created_at, imported_at
             FROM media_assets ORDER BY imported_at DESC LIMIT ?1 OFFSET ?2",
        )?;
        let rows = stmt.query_map(params![limit, offset], map_media_asset_row)?;
        rows.collect()
    }

    pub fn list_media_assets_by_image_ids(&self, image_ids: &[&str]) -> Result<Vec<MediaAsset>> {
        if image_ids.is_empty() {
            return Ok(vec![]);
        }

        let conn = self.conn.lock();
        let placeholders = image_ids.iter().map(|_| "?").collect::<Vec<_>>().join(",");
        let query = format!(
            "SELECT id, media_type, primary_image_id, sha256_hash, format, file_size,
                    page_count, title, created_at, imported_at
             FROM media_assets WHERE primary_image_id IN ({}) ORDER BY imported_at DESC",
            placeholders
        );

        let mut stmt = conn.prepare(&query)?;
        let mut query_params: Vec<&dyn rusqlite::ToSql> = Vec::with_capacity(image_ids.len());
        for id in image_ids {
            query_params.push(id as &dyn rusqlite::ToSql);
        }
        let rows = stmt.query_map(
            rusqlite::params_from_iter(query_params),
            map_media_asset_row,
        )?;

        rows.collect()
    }

    pub fn list_media_files(&self, media_asset_id: &str) -> Result<Vec<MediaFile>> {
        let conn = self.conn.lock();
        let mut stmt = conn.prepare(
            "SELECT id, media_asset_id, path, last_seen_at, missing_at, last_seen_size, last_seen_mtime
             FROM media_files WHERE media_asset_id = ?1 ORDER BY id",
        )?;
        let rows = stmt.query_map(params![media_asset_id], map_media_file_row)?;
        rows.collect()
    }

    pub fn upsert_pdf_page(&self, page: &PdfPage) -> Result<()> {
        let conn = self.conn.lock();
        conn.execute(
            "INSERT OR REPLACE INTO pdf_pages (
                id, media_asset_id, page_index, width_points, height_points,
                thumbnail_path, preview_path, extracted_text, text_extracted_at
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
            params![
                page.id,
                page.media_asset_id,
                page.page_index,
                page.width_points,
                page.height_points,
                page.thumbnail_path,
                page.preview_path,
                page.extracted_text,
                page.text_extracted_at,
            ],
        )?;
        Ok(())
    }

    pub fn clear_pdf_pages(&self, media_asset_id: &str) -> Result<u32> {
        let conn = self.conn.lock();
        let deleted = conn.execute(
            "DELETE FROM pdf_pages WHERE media_asset_id = ?1",
            params![media_asset_id],
        )?;
        Ok(deleted as u32)
    }

    pub fn list_pdf_pages(&self, media_asset_id: &str) -> Result<Vec<PdfPage>> {
        let conn = self.conn.lock();
        let mut stmt = conn.prepare(
            "SELECT id, media_asset_id, page_index, width_points, height_points,
                    thumbnail_path, preview_path, extracted_text, text_extracted_at
             FROM pdf_pages WHERE media_asset_id = ?1 ORDER BY page_index ASC",
        )?;
        let rows = stmt.query_map(params![media_asset_id], map_pdf_page_row)?;
        rows.collect()
    }
}
