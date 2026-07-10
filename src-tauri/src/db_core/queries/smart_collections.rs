// Copyright (c) 2026-present Gleb Kalinin. Architecture and design by author.
// Implementation assisted by Claude (Anthropic). See AUTHORSHIP.md.

use crate::db_core::db::{row_u64, Database};
use crate::db_core::models::*;
use crate::db_core::smart_collections::{FilterNode, SmartCollection};
use rusqlite::Result;

impl Database {
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
                             LEFT JOIN image_quality_metrics qm ON qm.image_id = i.id
                             LEFT JOIN image_color_metrics cm ON cm.image_id = i.id
                             LEFT JOIN image_similarity_group_items sgi ON sgi.image_id = i.id
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
        self.evaluate_smart_collection_page(filter_json, None, None)
    }

    pub fn count_smart_collection(&self, filter_json: &str) -> Result<i64> {
        let filter: FilterNode = serde_json::from_str(filter_json)
            .map_err(|e| rusqlite::Error::InvalidParameterName(e.to_string()))?;

        let (where_clause, params) = filter
            .to_sql_clause()
            .map_err(|e| rusqlite::Error::InvalidParameterName(e))?;

        let conn = self.conn.lock();
        let sql = format!(
            "SELECT COUNT(DISTINCT i.id)
             FROM images i
             JOIN image_files f ON f.image_id = i.id AND f.missing_at IS NULL
             LEFT JOIN selections s ON s.image_id = i.id AND s.project_id = '__global__'
             LEFT JOIN image_quality_metrics qm ON qm.image_id = i.id
             LEFT JOIN image_color_metrics cm ON cm.image_id = i.id
             LEFT JOIN image_similarity_group_items sgi ON sgi.image_id = i.id
             WHERE ({})",
            where_clause
        );
        let param_refs: Vec<&dyn rusqlite::types::ToSql> = params
            .iter()
            .map(|p| p as &dyn rusqlite::types::ToSql)
            .collect();

        conn.query_row(&sql, param_refs.as_slice(), |row| row.get::<_, i64>(0))
    }

    pub fn evaluate_smart_collection_page(
        &self,
        filter_json: &str,
        limit: Option<u32>,
        offset: Option<u32>,
    ) -> Result<Vec<ImageWithFile>> {
        let filter: FilterNode = serde_json::from_str(filter_json)
            .map_err(|e| rusqlite::Error::InvalidParameterName(e.to_string()))?;

        let (where_clause, mut params) = filter
            .to_sql_clause()
            .map_err(|e| rusqlite::Error::InvalidParameterName(e))?;

        let conn = self.conn.lock();
        let mut sql = format!(
            "SELECT i.id, i.sha256_hash, i.width, i.height, i.format, i.file_size,
                    i.created_at, i.imported_at, f.path,
                    s.star_rating, s.color_label, s.decision, i.source_label, i.ai_prompt,
                    i.raw_metadata, f.missing_at
             FROM images i
             JOIN image_files f ON f.image_id = i.id AND f.missing_at IS NULL
             LEFT JOIN selections s ON s.image_id = i.id AND s.project_id = '__global__'
             LEFT JOIN image_quality_metrics qm ON qm.image_id = i.id
             LEFT JOIN image_color_metrics cm ON cm.image_id = i.id
             LEFT JOIN image_similarity_group_items sgi ON sgi.image_id = i.id
             WHERE ({})
             GROUP BY i.id
             ORDER BY i.imported_at DESC",
            where_clause
        );

        if let Some(limit) = limit {
            sql.push_str(" LIMIT ? OFFSET ?");
            params.push(rusqlite::types::Value::Integer(limit as i64));
            params.push(rusqlite::types::Value::Integer(offset.unwrap_or(0) as i64));
        }

        let param_refs: Vec<&dyn rusqlite::types::ToSql> = params
            .iter()
            .map(|p| p as &dyn rusqlite::types::ToSql)
            .collect();

        let mut stmt = conn.prepare(&sql)?;
        let rows = stmt.query_map(param_refs.as_slice(), |row| {
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
                    file_size: row_u64(row, 5)?,
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
}
