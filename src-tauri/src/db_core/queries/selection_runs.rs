// Copyright (c) 2026-present Gleb Kalinin. Architecture and design by author.
// Implementation assisted by AI tools. See AUTHORSHIP.md.

//! Selection Mode persistence: source-scope resolution, source snapshots,
//! shortlist membership over `collection_items`, and run lifecycle.
//!
//! Invariants enforced here:
//! - shortlist membership lives in `collection_items` of the run's backing
//!   project and preserves addition order by `position`;
//! - every member must exist in the captured `selection_run_source_items`
//!   snapshot (add-time validation, replay-time filtering);
//! - membership may only change while the run is `active`;
//! - generic collection helpers refuse to touch `selection` projects so they
//!   cannot bypass the invariants above.

use crate::db_core::db::{map_image_with_file_row, Database};
use crate::db_core::models::*;
use crate::db_core::referenced_sources::NORMAL_LIBRARY_FILE_PREDICATE;
use crate::db_core::smart_collections::FilterNode;
use crate::db_core::visibility::RejectedVisibility;
use rusqlite::types::Value as SqlValue;
use rusqlite::{params, Connection, OptionalExtension, Result};
use std::collections::HashSet;

const ACTIVE: &str = "active";
const FINISHED: &str = "finished";
const ARCHIVED: &str = "archived";

#[derive(Debug, Clone, Default)]
pub struct PageFilters {
    pub query: Option<String>,
    pub min_size: Option<u32>,
    pub include_rejected: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PageSource {
    SourceSnapshot,
    Shortlist,
}

fn invalid_request(message: impl Into<String>) -> rusqlite::Error {
    rusqlite::Error::InvalidParameterName(message.into())
}

/// Escapes a user search term for a literal `LIKE` match and wraps it in `%`.
fn like_pattern(query: &str) -> String {
    let escaped = query
        .trim()
        .replace('\\', "\\\\")
        .replace('%', "\\%")
        .replace('_', "\\_");
    format!("%{escaped}%")
}

/// Returns the FROM/WHERE relation for a scope: `(sql, params)` where `sql`
/// ends with the scope predicate and uses only anonymous `?` placeholders.
/// Visibility and search predicates are appended by the caller, with their
/// parameters appended after the scope parameters in appearance order.
fn scope_relation(scope: &SelectionSourceScope) -> Result<(String, Vec<SqlValue>)> {
    let normal_from = "FROM images i
        JOIN image_files f ON f.image_id = i.id AND f.missing_at IS NULL
        LEFT JOIN selections s ON s.image_id = i.id AND s.project_id = '__global__'";
    match scope {
        SelectionSourceScope::All { .. } => Ok((
            format!("{normal_from} WHERE {NORMAL_LIBRARY_FILE_PREDICATE}"),
            vec![],
        )),
        SelectionSourceScope::Folder {
            path,
            min_size,
            include_rejected: _,
        } => {
            let folder = path.trim_end_matches('/');
            let prefix = if folder.is_empty() {
                "/".to_string()
            } else {
                format!("{folder}/")
            };
            Ok((
                format!(
                    "{normal_from}
                     WHERE (f.path = ? OR substr(f.path, 1, ?) COLLATE BINARY = ? COLLATE BINARY)
                       AND i.width >= ? AND i.height >= ?
                       AND {NORMAL_LIBRARY_FILE_PREDICATE}"
                ),
                vec![
                    SqlValue::Text(path.clone()),
                    SqlValue::Integer(prefix.chars().count() as i64),
                    SqlValue::Text(prefix),
                    SqlValue::Integer(*min_size as i64),
                    SqlValue::Integer(*min_size as i64),
                ],
            ))
        }
        SelectionSourceScope::Filtered {
            min_size,
            include_rejected: _,
        } => Ok((
            format!(
                "{normal_from}
                 WHERE i.width >= ? AND i.height >= ?
                 AND {NORMAL_LIBRARY_FILE_PREDICATE}"
            ),
            vec![
                SqlValue::Integer(*min_size as i64),
                SqlValue::Integer(*min_size as i64),
            ],
        )),
        SelectionSourceScope::Collection {
            id,
            include_rejected: _,
        } => Ok((
            format!(
                "{normal_from}
                 WHERE EXISTS (
                     SELECT 1 FROM collection_items ci
                     WHERE ci.image_id = i.id AND ci.collection_id = ?
                 )
                 AND {NORMAL_LIBRARY_FILE_PREDICATE}"
            ),
            vec![SqlValue::Text(id.clone())],
        )),
        SelectionSourceScope::Smart {
            id: _,
            filter_json,
            include_rejected: _,
        } => {
            let filter: FilterNode = serde_json::from_str(filter_json)
                .map_err(|e| invalid_request(format!("Invalid smart collection filter: {e}")))?;
            let (where_clause, params) = filter
                .to_sql_clause()
                .map_err(|e| invalid_request(format!("Invalid smart collection filter: {e}")))?;
            Ok((
                format!(
                    "FROM images i
                     JOIN image_files f ON f.image_id = i.id AND f.missing_at IS NULL
                     LEFT JOIN selections s ON s.image_id = i.id AND s.project_id = '__global__'
                     LEFT JOIN image_quality_metrics qm ON qm.image_id = i.id
                     LEFT JOIN image_color_metrics cm ON cm.image_id = i.id
                     LEFT JOIN image_similarity_group_items sgi ON sgi.image_id = i.id
                     WHERE ({where_clause}) AND {NORMAL_LIBRARY_FILE_PREDICATE}"
                ),
                params,
            ))
        }
        SelectionSourceScope::DetectedClass {
            class_name,
            include_rejected: _,
        } => Ok((
            format!(
                "FROM detections d
                 JOIN images i ON i.id = d.image_id
                 JOIN image_files f ON f.image_id = i.id AND f.missing_at IS NULL
                 LEFT JOIN selections s ON s.image_id = i.id AND s.project_id = '__global__'
                 WHERE d.class_name = ? AND {NORMAL_LIBRARY_FILE_PREDICATE}"
            ),
            vec![SqlValue::Text(class_name.clone())],
        )),
        SelectionSourceScope::ImportBatch {
            batch_id,
            include_rejected: _,
        } => Ok((
            format!(
                "{normal_from}
                 WHERE i.import_batch_id = ? AND {NORMAL_LIBRARY_FILE_PREDICATE}"
            ),
            vec![SqlValue::Text(batch_id.clone())],
        )),
        SelectionSourceScope::ReferencedFolder {
            source_id,
            relative_path,
            recursive,
            include_rejected: _,
        } => {
            let normalized = relative_path.trim_matches('/');
            let prefix = if normalized.is_empty() {
                String::new()
            } else {
                format!("{normalized}/")
            };
            Ok((
                "FROM referenced_files rf
                     JOIN image_files f ON f.id = rf.image_file_id
                     JOIN images i ON i.id = f.image_id
                     LEFT JOIN selections s ON s.image_id = i.id AND s.project_id = '__global__'
                     WHERE rf.source_id = ?
                       AND substr(rf.relative_path, 1, length(?)) = ?
                       AND (? OR instr(substr(rf.relative_path, length(?) + 1), '/') = 0)"
                    .to_string(),
                vec![
                    SqlValue::Text(source_id.clone()),
                    SqlValue::Text(prefix.clone()),
                    SqlValue::Text(prefix.clone()),
                    SqlValue::Integer(*recursive as i64),
                    SqlValue::Text(prefix),
                ],
            ))
        }
        SelectionSourceScope::Search { base, query } => {
            let (from_where, mut params) = scope_relation(base)?;
            let pattern = like_pattern(query);
            params.extend(std::iter::repeat_n(SqlValue::Text(pattern.clone()), 3));
            Ok((
                format!(
                    "{from_where}
                     AND (EXISTS (
                         SELECT 1 FROM image_files sf
                         WHERE sf.image_id = i.id AND sf.missing_at IS NULL
                           AND sf.path LIKE ? ESCAPE '\\'
                     ) OR i.source_label LIKE ? ESCAPE '\\'
                       OR i.ai_prompt LIKE ? ESCAPE '\\')"
                ),
                params,
            ))
        }
    }
}

/// Deterministic snapshot ordering that mirrors each scope's canonical
/// listing. Returns `(order_by_sql, extra_params_in_order)`.
fn scope_order(scope: &SelectionSourceScope) -> (String, Vec<SqlValue>) {
    match scope {
        // A collection's own ordering is part of its identity.
        SelectionSourceScope::Collection { id, .. } => (
            "(SELECT sci.position FROM collection_items sci
              WHERE sci.collection_id = ? AND sci.image_id = i.id) ASC, i.id ASC"
                .to_string(),
            vec![SqlValue::Text(id.clone())],
        ),
        SelectionSourceScope::DetectedClass { .. } => (
            "MAX(d.confidence) DESC, i.imported_at DESC, i.id ASC".to_string(),
            vec![],
        ),
        SelectionSourceScope::ReferencedFolder { .. } => (
            "rf.relative_path COLLATE NOCASE, i.id ASC".to_string(),
            vec![],
        ),
        _ => ("i.imported_at DESC, i.id ASC".to_string(), vec![]),
    }
}

pub fn resolve_scope_ids_conn(
    conn: &Connection,
    scope: &SelectionSourceScope,
) -> Result<Vec<String>> {
    let (from_where, mut params) = scope_relation(scope)?;
    let visibility =
        RejectedVisibility::from_include_rejected(scope.include_rejected()).sql_predicate();
    let (order_by, mut order_params) = scope_order(scope);
    params.append(&mut order_params);
    // Appearance order: scope predicate params, then ordering params, then
    // visibility (static).
    let sql = format!(
        "SELECT i.id {from_where} AND {visibility}
         GROUP BY i.id
         ORDER BY {order_by}"
    );
    let mut stmt = conn.prepare(&sql)?;
    let rows = stmt.query_map(rusqlite::params_from_iter(params), |row| {
        row.get::<_, String>(0)
    })?;
    rows.collect()
}

// ---------------------------------------------------------------------------
// Connection-scoped helpers (usable inside an open transaction/savepoint)
// ---------------------------------------------------------------------------

pub(crate) fn selection_run_status_conn(
    conn: &Connection,
    selection_id: &str,
) -> Result<Option<String>> {
    conn.query_row(
        "SELECT status FROM selection_runs WHERE id = ?1",
        params![selection_id],
        |row| row.get(0),
    )
    .optional()
}

pub(crate) fn filter_ids_in_source_conn(
    conn: &Connection,
    selection_id: &str,
    image_ids: &[String],
) -> Result<Vec<String>> {
    let mut present = Vec::new();
    let mut stmt = conn.prepare(
        "SELECT 1 FROM selection_run_source_items
         WHERE selection_id = ?1 AND image_id = ?2",
    )?;
    for id in image_ids {
        let found = stmt
            .query_row(params![selection_id, id], |row| row.get::<_, i64>(0))
            .optional()?;
        if found.is_some() {
            present.push(id.clone());
        }
    }
    Ok(present)
}

pub(crate) fn filter_ids_in_membership_conn(
    conn: &Connection,
    selection_id: &str,
    image_ids: &[String],
) -> Result<Vec<String>> {
    let mut present = Vec::new();
    let mut stmt = conn.prepare(
        "SELECT 1 FROM collection_items
         WHERE collection_id = ?1 AND image_id = ?2",
    )?;
    for id in image_ids {
        let found = stmt
            .query_row(params![selection_id, id], |row| row.get::<_, i64>(0))
            .optional()?;
        if found.is_some() {
            present.push(id.clone());
        }
    }
    Ok(present)
}

/// Appends image IDs to the shortlist at MAX(position)+1, preserving the given
/// order. Already-member IDs are silently skipped (idempotent). Returns the
/// IDs that were actually inserted.
pub(crate) fn shortlist_append_conn(
    conn: &Connection,
    selection_id: &str,
    image_ids: &[String],
) -> Result<Vec<String>> {
    let max_pos: i64 = conn.query_row(
        "SELECT COALESCE(MAX(position), -1) FROM collection_items WHERE collection_id = ?1",
        params![selection_id],
        |row| row.get(0),
    )?;
    let mut inserted = Vec::new();
    for (offset, id) in image_ids.iter().enumerate() {
        let result = conn.execute(
            "INSERT OR IGNORE INTO collection_items (collection_id, image_id, position)
             VALUES (?1, ?2, ?3)",
            params![selection_id, id, max_pos + 1 + offset as i64],
        )?;
        if result == 1 {
            inserted.push(id.clone());
        }
    }
    Ok(inserted)
}

/// Removes image IDs from the shortlist. Non-members are skipped. Returns the
/// IDs that were actually removed.
pub(crate) fn shortlist_delete_conn(
    conn: &Connection,
    selection_id: &str,
    image_ids: &[String],
) -> Result<Vec<String>> {
    let mut removed = Vec::new();
    for id in image_ids {
        let result = conn.execute(
            "DELETE FROM collection_items WHERE collection_id = ?1 AND image_id = ?2",
            params![selection_id, id],
        )?;
        if result == 1 {
            removed.push(id.clone());
        }
    }
    Ok(removed)
}

/// Reads the shortlist membership in addition order.
pub(crate) fn shortlist_ordered_ids_conn(
    conn: &Connection,
    selection_id: &str,
) -> Result<Vec<String>> {
    let mut stmt = conn.prepare(
        "SELECT image_id FROM collection_items
         WHERE collection_id = ?1
         ORDER BY position ASC, image_id ASC",
    )?;
    let ids = stmt
        .query_map(params![selection_id], |row| row.get::<_, String>(0))?
        .collect::<Result<Vec<_>>>()?;
    Ok(ids)
}

/// Replays only the membership delta between the expected and target states.
/// IDs that no longer exist in the surviving source snapshot are dropped, so
/// replay never resurrects deleted images or outside-source members. The run
/// must exist and be `active`.
pub(crate) fn shortlist_restore_conn(
    conn: &Connection,
    selection_id: &str,
    expected_ids: &[String],
    ordered_ids: &[String],
) -> Result<()> {
    let status = selection_run_status_conn(conn, selection_id)?.ok_or_else(|| {
        invalid_request(format!("Selection run '{selection_id}' no longer exists"))
    })?;
    if status != ACTIVE {
        return Err(invalid_request(format!(
            "Selection run is {status}; reopen or restore it before replaying history"
        )));
    }
    let expected = filter_ids_in_source_conn(conn, selection_id, expected_ids)?;
    let target = filter_ids_in_source_conn(conn, selection_id, ordered_ids)?;
    let expected_set: HashSet<_> = expected.iter().collect();
    let target_set: HashSet<_> = target.iter().collect();
    let removed: Vec<String> = expected
        .iter()
        .filter(|id| !target_set.contains(id))
        .cloned()
        .collect();
    let added: Vec<String> = target
        .iter()
        .filter(|id| !expected_set.contains(id))
        .cloned()
        .collect();
    let mut current = shortlist_ordered_ids_conn(conn, selection_id)?;
    // An ordinary collection edit may have changed the same member while the
    // run was finished. Refuse that conflict without advancing history.
    if removed.iter().any(|id| !current.contains(id)) || added.iter().any(|id| current.contains(id))
    {
        return Err(invalid_request(
            "Shortlist history conflicts with later collection edits",
        ));
    }
    shortlist_delete_conn(conn, selection_id, &removed)?;
    current.retain(|id| !removed.contains(id));
    shortlist_append_conn(conn, selection_id, &added)?;
    for id in &added {
        let index = target.iter().position(|item| item == id).unwrap();
        // Restore relative to surviving neighbors, preserving unrelated members
        // and their order, including members added after the action was recorded.
        let position = target[index + 1..]
            .iter()
            .find_map(|next| current.iter().position(|item| item == next))
            .or_else(|| {
                target[..index].iter().rev().find_map(|prev| {
                    current
                        .iter()
                        .position(|item| item == prev)
                        .map(|pos| pos + 1)
                })
            })
            .unwrap_or(0);
        current.insert(position, id.clone());
    }
    for (position, id) in current.iter().enumerate() {
        conn.execute(
            "UPDATE collection_items SET position = ?3 WHERE collection_id = ?1 AND image_id = ?2",
            params![selection_id, id, position as i64],
        )?;
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// Database API: creation, reads, lifecycle
// ---------------------------------------------------------------------------

fn count_as_u32(value: i64) -> Result<u32> {
    u32::try_from(value)
        .map_err(|_| invalid_request("Selection run counts exceed the supported maximum"))
}

impl Database {
    /// Fully resolves a scope to an ordered, deduplicated list of image IDs.
    /// Reads never consult original files.
    pub fn resolve_selection_scope_ids(&self, scope: &SelectionSourceScope) -> Result<Vec<String>> {
        let conn = self.read_connection();
        let mut ids = resolve_scope_ids_conn(&conn, scope)?;
        drop(conn);
        let mut seen = HashSet::new();
        ids.retain(|id| seen.insert(id.clone()));
        Ok(ids)
    }

    /// Creates the backing `selection` project, the run row and the complete
    /// source snapshot in one transaction. The scope is resolved inside the
    /// transaction so the snapshot is exactly what was resolved.
    pub fn create_selection_run(
        &self,
        name: &str,
        scope: &SelectionSourceScope,
        target_count: Option<u32>,
    ) -> Result<String> {
        let scope_json = serde_json::to_string(scope)
            .map_err(|e| invalid_request(format!("Could not serialize source scope: {e}")))?;
        let now = chrono::Utc::now().to_rfc3339();
        let id = uuid::Uuid::new_v4().to_string();

        let mut conn = self.conn.lock();
        let tx = conn.transaction()?;
        let mut resolved = resolve_scope_ids_conn(&tx, scope)?;
        // Deduplicate defensively; scope queries already GROUP BY i.id.
        let mut seen = HashSet::new();
        resolved.retain(|id| seen.insert(id.clone()));
        if resolved.is_empty() {
            return Err(invalid_request(
                "Source resolved to no images; Selection Mode needs a non-empty source",
            ));
        }

        tx.execute(
            "INSERT INTO projects (id, name, description, collection_type, created_at)
             VALUES (?1, ?2, NULL, 'selection', ?3)",
            params![id, name, now],
        )?;
        tx.execute(
            "INSERT INTO selection_runs
                (id, status, archived_from, source_scope_json, source_count,
                 target_count, created_at, updated_at, finished_at)
             VALUES (?1, 'active', NULL, ?2, ?3, ?4, ?5, ?5, NULL)",
            params![
                id,
                scope_json,
                resolved.len() as i64,
                target_count.map(|t| t as i64),
                now
            ],
        )?;
        for (position, image_id) in resolved.iter().enumerate() {
            let inserted = tx.execute(
                "INSERT INTO selection_run_source_items (selection_id, image_id, position)
                 SELECT ?1, id, ?3 FROM images WHERE id = ?2",
                params![id, image_id, position as i64],
            )?;
            if inserted != 1 {
                return Err(invalid_request(format!(
                    "Image '{image_id}' disappeared while capturing the source snapshot"
                )));
            }
        }
        tx.commit()?;
        Ok(id)
    }

    /// Recomputes stored source counts that drifted because authorized image
    /// deletion cascaded out of the snapshot. Spec: "counts are then recomputed
    /// and the run records that the source changed".
    fn reconcile_source_counts(&self) -> Result<()> {
        let conn = self.conn.lock();
        conn.execute(
            "UPDATE selection_runs
             SET source_count = (
                     SELECT COUNT(*) FROM selection_run_source_items
                     WHERE selection_id = selection_runs.id
                 ),
                 updated_at = ?1
             WHERE source_count != (
                     SELECT COUNT(*) FROM selection_run_source_items
                     WHERE selection_id = selection_runs.id
                 )",
            params![chrono::Utc::now().to_rfc3339()],
        )?;
        Ok(())
    }

    fn selection_run_row(conn: &Connection, id: &str) -> Result<Option<SelectionRun>> {
        let mut stmt = conn.prepare(
            "SELECT r.source_scope_json, r.status, r.target_count, r.created_at,
                    r.updated_at, r.finished_at, p.name,
                    (SELECT COUNT(*) FROM selection_run_source_items si
                     WHERE si.selection_id = r.id),
                    (SELECT COUNT(*) FROM collection_items ci
                     WHERE ci.collection_id = r.id),
                    (SELECT COUNT(*)
                     FROM collection_items ci
                     JOIN selections s ON s.image_id = ci.image_id
                      AND s.project_id = '__global__'
                     WHERE ci.collection_id = r.id AND s.decision = 'reject')
             FROM selection_runs r
             JOIN projects p ON p.id = r.id
             WHERE r.id = ?1",
        )?;
        let run = stmt
            .query_row(params![id], |row| {
                let scope_json: String = row.get(0)?;
                let scope: SelectionSourceScope =
                    serde_json::from_str(&scope_json).map_err(|e| {
                        invalid_request(format!("Stored selection scope is not valid JSON: {e}"))
                    })?;
                Ok(SelectionRun {
                    id: id.to_string(),
                    name: row.get(6)?,
                    status: row.get(1)?,
                    source_count: count_as_u32(row.get::<_, i64>(7)?)?,
                    shortlist_count: count_as_u32(row.get::<_, i64>(8)?)?,
                    target_count: row.get::<_, Option<i64>>(2)?.map(|v| v as u32),
                    source_scope: scope,
                    created_at: row.get(3)?,
                    updated_at: row.get(4)?,
                    finished_at: row.get(5)?,
                    rejected_shortlist_count: count_as_u32(row.get::<_, i64>(9)?)?,
                })
            })
            .optional()?;
        Ok(run)
    }

    pub fn get_selection_run(&self, selection_id: &str) -> Result<Option<SelectionRun>> {
        self.reconcile_source_counts()?;
        let conn = self.conn.lock();
        Self::selection_run_row(&conn, selection_id)
    }

    pub fn list_selection_runs(&self, status: Option<&str>) -> Result<Vec<SelectionRun>> {
        self.reconcile_source_counts()?;
        let conn = self.conn.lock();
        let mut stmt = conn.prepare(
            "SELECT r.id
             FROM selection_runs r
             WHERE (?1 IS NULL OR r.status = ?1)
             ORDER BY r.created_at DESC, r.id ASC",
        )?;
        let ids = stmt
            .query_map(params![status], |row| row.get::<_, String>(0))?
            .collect::<Result<Vec<_>>>()?;
        let mut runs = Vec::with_capacity(ids.len());
        for id in ids {
            if let Some(run) = Self::selection_run_row(&conn, &id)? {
                runs.push(run);
            }
        }
        Ok(runs)
    }

    /// Returns `(items, total)` for one page of the captured source. Missing
    /// original files (offline devices) remain listed so members and cached
    /// thumbnails survive disconnects; `missing_at` is surfaced per item.
    pub fn list_selection_source_page(
        &self,
        selection_id: &str,
        limit: u32,
        offset: u32,
        filters: PageFilters,
    ) -> Result<(Vec<ImageWithFile>, u32)> {
        self.list_selection_page(
            selection_id,
            limit,
            offset,
            filters,
            PageSource::SourceSnapshot,
        )
    }

    pub fn list_selection_shortlist_page(
        &self,
        selection_id: &str,
        limit: u32,
        offset: u32,
        filters: PageFilters,
    ) -> Result<(Vec<ImageWithFile>, u32)> {
        self.list_selection_page(selection_id, limit, offset, filters, PageSource::Shortlist)
    }

    fn list_selection_page(
        &self,
        selection_id: &str,
        limit: u32,
        offset: u32,
        filters: PageFilters,
        source: PageSource,
    ) -> Result<(Vec<ImageWithFile>, u32)> {
        let PageFilters {
            query,
            min_size,
            include_rejected,
        } = filters;
        let mut clauses: Vec<String> = vec![];
        let mut bind: Vec<SqlValue> = vec![];

        let from = match source {
            PageSource::SourceSnapshot => {
                clauses.push("srs.selection_id = ?".to_string());
                bind.push(SqlValue::Text(selection_id.to_string()));
                "FROM selection_run_source_items srs
                 JOIN images i ON i.id = srs.image_id"
                    .to_string()
            }
            PageSource::Shortlist => {
                clauses.push("ci.collection_id = ?".to_string());
                bind.push(SqlValue::Text(selection_id.to_string()));
                "FROM collection_items ci
                 JOIN images i ON i.id = ci.image_id"
                    .to_string()
            }
        };

        // Prefer a live file for display but keep missing files visible with
        // their missing_at timestamp (disconnect-safe reads).
        let file_join = "JOIN image_files f ON f.id = (
            SELECT sf.id FROM image_files sf
            WHERE sf.image_id = i.id
            ORDER BY sf.missing_at IS NOT NULL, sf.id
            LIMIT 1)";

        if let Some(min_size) = min_size {
            clauses.push("i.width >= ? AND i.height >= ?".to_string());
            bind.push(SqlValue::Integer(min_size as i64));
            bind.push(SqlValue::Integer(min_size as i64));
        }
        if let Some(query) = query.as_deref().map(str::trim).filter(|q| !q.is_empty()) {
            let pattern = like_pattern(query);
            clauses.push(
                "(f.path LIKE ? ESCAPE '\\'
                  OR i.source_label LIKE ? ESCAPE '\\'
                  OR i.ai_prompt LIKE ? ESCAPE '\\')"
                    .to_string(),
            );
            bind.extend(std::iter::repeat_n(SqlValue::Text(pattern), 3));
        }
        let visibility =
            RejectedVisibility::from_include_rejected(include_rejected).sql_predicate();
        clauses.push(visibility.to_string());

        let order_by = match source {
            PageSource::SourceSnapshot => "srs.position ASC, i.id ASC",
            PageSource::Shortlist => "ci.position ASC, i.id ASC",
        };
        let where_sql = clauses.join(" AND ");

        let count_sql = format!(
            "SELECT COUNT(DISTINCT i.id) {from}
             {file_join}
             LEFT JOIN selections s ON s.image_id = i.id AND s.project_id = '__global__'
             WHERE {where_sql}"
        );
        let page_sql = format!(
            "SELECT i.id, i.sha256_hash, i.width, i.height, i.format, i.file_size,
                    i.created_at, i.imported_at, f.path,
                    s.star_rating, s.color_label, s.decision, i.source_label, i.ai_prompt,
                    i.raw_metadata, f.missing_at
             {from}
             {file_join}
             LEFT JOIN selections s ON s.image_id = i.id AND s.project_id = '__global__'
             WHERE {where_sql}
             GROUP BY i.id
             ORDER BY {order_by}
             LIMIT ? OFFSET ?"
        );
        let mut page_bind = bind.clone();
        page_bind.push(SqlValue::Integer(limit as i64));
        page_bind.push(SqlValue::Integer(offset as i64));

        let conn = self.read_connection();
        let total: i64 = {
            let mut stmt = conn.prepare(&count_sql)?;
            stmt.query_row(rusqlite::params_from_iter(bind.iter()), |row| row.get(0))?
        };
        let mut stmt = conn.prepare(&page_sql)?;
        let items = stmt
            .query_map(
                rusqlite::params_from_iter(page_bind.iter()),
                map_image_with_file_row,
            )?
            .collect::<Result<Vec<_>>>()?;
        Ok((items, count_as_u32(total)?))
    }

    /// Finishes the run: marks it finished and exposes the shortlist as a
    /// normal manual collection, atomically. Membership, decisions and file
    /// records are untouched.
    pub fn finish_selection_run(&self, selection_id: &str) -> Result<()> {
        let mut conn = self.conn.lock();
        let tx = conn.transaction()?;
        let status: Option<String> = tx
            .query_row(
                "SELECT status FROM selection_runs WHERE id = ?1",
                params![selection_id],
                |row| row.get(0),
            )
            .optional()?;
        match status.as_deref() {
            None => {
                return Err(invalid_request(format!(
                    "Selection run '{selection_id}' was not found"
                )))
            }
            Some(FINISHED) => {
                return Err(invalid_request(format!(
                    "Selection run '{selection_id}' is already finished"
                )))
            }
            Some(ARCHIVED) => {
                return Err(invalid_request(format!(
                    "Selection run '{selection_id}' is archived; restore it first"
                )))
            }
            _ => {}
        }
        let shortlist: i64 = tx.query_row(
            "SELECT COUNT(*) FROM collection_items WHERE collection_id = ?1",
            params![selection_id],
            |row| row.get(0),
        )?;
        if shortlist == 0 {
            return Err(invalid_request(
                "Cannot finish an empty selection; archive it instead",
            ));
        }
        let now = chrono::Utc::now().to_rfc3339();
        let updated = tx.execute(
            "UPDATE selection_runs SET status = 'finished', finished_at = ?2, updated_at = ?2
             WHERE id = ?1 AND status = 'active'",
            params![selection_id, now],
        )?;
        if updated != 1 {
            return Err(invalid_request(
                "Selection run changed concurrently; retry the finish",
            ));
        }
        let updated = tx.execute(
            "UPDATE projects SET collection_type = 'manual'
             WHERE id = ?1 AND collection_type = 'selection'",
            params![selection_id],
        )?;
        if updated != 1 {
            return Err(invalid_request(
                "Selection run backing collection is missing or was converted; retry the finish",
            ));
        }
        tx.commit()?;
        Ok(())
    }

    /// Reopens a finished run: lifecycle flips back to active without altering
    /// membership. Does not duplicate membership or reset the shortlist.
    pub fn reopen_selection_run(&self, selection_id: &str) -> Result<()> {
        let mut conn = self.conn.lock();
        let tx = conn.transaction()?;
        let status: Option<String> = tx
            .query_row(
                "SELECT status FROM selection_runs WHERE id = ?1",
                params![selection_id],
                |row| row.get(0),
            )
            .optional()?;
        match status.as_deref() {
            None => {
                return Err(invalid_request(format!(
                    "Selection run '{selection_id}' was not found"
                )))
            }
            Some(ACTIVE) => {
                return Err(invalid_request(format!(
                    "Selection run '{selection_id}' is already active"
                )))
            }
            Some(ARCHIVED) => {
                return Err(invalid_request(format!(
                    "Selection run '{selection_id}' is archived; restore it first"
                )))
            }
            _ => {}
        }
        // A finished run's collection is a normal manual collection and may
        // have gained members through ordinary collection edits. Continuing
        // as Selection must not silently admit them into the captured source
        // or alter the snapshot: refuse until they are removed (data is
        // retained either way).
        let outside: i64 = tx.query_row(
            "SELECT COUNT(*) FROM collection_items ci
             WHERE ci.collection_id = ?1
               AND NOT EXISTS (
                   SELECT 1 FROM selection_run_source_items si
                   WHERE si.selection_id = ?1 AND si.image_id = ci.image_id
               )",
            params![selection_id],
            |row| row.get(0),
        )?;
        if outside > 0 {
            return Err(invalid_request(format!(
                "Selection collection contains {outside} image(s) outside the captured selection source. \
                 Remove them from the collection before continuing as Selection; nothing is deleted"
            )));
        }
        let now = chrono::Utc::now().to_rfc3339();
        let updated = tx.execute(
            "UPDATE selection_runs SET status = 'active', finished_at = NULL, updated_at = ?2
             WHERE id = ?1 AND status = 'finished'",
            params![selection_id, now],
        )?;
        if updated != 1 {
            return Err(invalid_request(
                "Selection run changed concurrently; retry the reopen",
            ));
        }
        let updated = tx.execute(
            "UPDATE projects SET collection_type = 'selection'
             WHERE id = ?1 AND collection_type = 'manual'",
            params![selection_id],
        )?;
        if updated != 1 {
            return Err(invalid_request(
                "Selection run backing collection is missing; retry the reopen",
            ));
        }
        tx.commit()?;
        Ok(())
    }

    /// Archives a run from `active` or `finished`. The previous status is
    /// recorded so restore returns the run to the exact lifecycle it had. An
    /// archived finished run's collection leaves the normal list until
    /// restored.
    pub fn archive_selection_run(&self, selection_id: &str) -> Result<()> {
        let mut conn = self.conn.lock();
        let tx = conn.transaction()?;
        let status: Option<String> = tx
            .query_row(
                "SELECT status FROM selection_runs WHERE id = ?1",
                params![selection_id],
                |row| row.get(0),
            )
            .optional()?;
        match status.as_deref() {
            None => {
                return Err(invalid_request(format!(
                    "Selection run '{selection_id}' was not found"
                )))
            }
            Some(ARCHIVED) => {
                return Err(invalid_request(format!(
                    "Selection run '{selection_id}' is already archived"
                )))
            }
            Some(previous) => {
                let now = chrono::Utc::now().to_rfc3339();
                let updated = tx.execute(
                    "UPDATE selection_runs
                     SET status = 'archived', archived_from = ?2, updated_at = ?3
                     WHERE id = ?1 AND status = ?2",
                    params![selection_id, previous, now],
                )?;
                if updated != 1 {
                    return Err(invalid_request(
                        "Selection run changed concurrently; retry the archive",
                    ));
                }
                if previous == FINISHED {
                    let updated = tx.execute(
                        "UPDATE projects SET collection_type = 'selection'
                         WHERE id = ?1 AND collection_type = 'manual'",
                        params![selection_id],
                    )?;
                    if updated != 1 {
                        return Err(invalid_request(
                            "Selection run backing collection is missing; retry the archive",
                        ));
                    }
                }
            }
        }
        tx.commit()?;
        Ok(())
    }

    /// Restores an archived run to the lifecycle it had when archived
    /// (`active` or `finished`), including the collection visibility that
    /// belongs to that lifecycle.
    pub fn restore_selection_run(&self, selection_id: &str) -> Result<()> {
        let mut conn = self.conn.lock();
        let tx = conn.transaction()?;
        let row: Option<(String, Option<String>)> = tx
            .query_row(
                "SELECT status, archived_from FROM selection_runs WHERE id = ?1",
                params![selection_id],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .optional()?;
        match row {
            None => {
                return Err(invalid_request(format!(
                    "Selection run '{selection_id}' was not found"
                )))
            }
            Some((status, archived_from)) => {
                if status != ARCHIVED {
                    return Err(invalid_request(format!(
                        "Selection run '{selection_id}' is {status}, not archived"
                    )));
                }
                let previous = archived_from.ok_or_else(|| {
                    invalid_request(format!(
                        "Archived selection run '{selection_id}' is missing its previous \
                         lifecycle; it cannot be restored safely"
                    ))
                })?;
                let now = chrono::Utc::now().to_rfc3339();
                let updated = tx.execute(
                    "UPDATE selection_runs
                     SET status = ?2, archived_from = NULL, updated_at = ?3
                     WHERE id = ?1 AND status = 'archived'",
                    params![selection_id, previous, now],
                )?;
                if updated != 1 {
                    return Err(invalid_request(
                        "Selection run changed concurrently; retry the restore",
                    ));
                }
                let collection_type = if previous == FINISHED {
                    "manual"
                } else {
                    "selection"
                };
                let updated = tx.execute(
                    "UPDATE projects SET collection_type = ?2
                     WHERE id = ?1 AND collection_type = 'selection'",
                    params![selection_id, collection_type],
                )?;
                if updated != 1 {
                    return Err(invalid_request(
                        "Selection run backing collection is missing; retry the restore",
                    ));
                }
            }
        }
        tx.commit()?;
        Ok(())
    }

    /// Ordered shortlist IDs (addition order) for markers and optimistic UI.
    pub fn selection_shortlist_ids(&self, selection_id: &str) -> Result<Vec<String>> {
        let conn = self.read_connection();
        let mut stmt = conn.prepare(
            "SELECT image_id FROM collection_items
             WHERE collection_id = ?1
             ORDER BY position ASC, image_id ASC",
        )?;
        let ids = stmt
            .query_map(params![selection_id], |row| row.get::<_, String>(0))?
            .collect::<Result<Vec<_>>>()?;
        Ok(ids)
    }

    /// Replays one membership delta atomically, preserving unrelated edits
    /// and refusing conflicts on affected members. Deleted source IDs are
    /// ignored; the run must be active.
    pub fn shortlist_restore(
        &self,
        selection_id: &str,
        expected_ids: &[String],
        ordered_ids: &[String],
    ) -> Result<()> {
        let mut conn = self.conn.lock();
        let tx = conn.transaction()?;
        shortlist_restore_conn(&tx, selection_id, expected_ids, ordered_ids)?;
        tx.commit()?;
        Ok(())
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use crate::db_core::models::{Image, ImageFile};

    fn open_db() -> Database {
        Database::open(std::path::Path::new(":memory:")).unwrap()
    }

    fn seed_image(db: &Database, id: &str, imported_at: &str) {
        db.insert_image(&Image {
            id: id.to_string(),
            sha256_hash: format!("hash-{id}"),
            width: 800,
            height: 600,
            format: "jpg".to_string(),
            file_size: 1024,
            created_at: "2026-09-01T00:00:00Z".to_string(),
            imported_at: imported_at.to_string(),
            ai_prompt: None,
            raw_metadata: None,
        })
        .unwrap();
        db.insert_image_file(&ImageFile {
            id: format!("file-{id}"),
            image_id: id.to_string(),
            path: format!("/library/shoot/{id}.jpg"),
            last_seen_at: "2026-09-01T00:00:00Z".to_string(),
            missing_at: None,
            last_seen_size: Some(1024),
            last_seen_mtime: None,
        })
        .unwrap();
    }

    fn all_scope() -> SelectionSourceScope {
        SelectionSourceScope::All {
            include_rejected: false,
        }
    }

    fn create_run_with_images(db: &Database, name: &str, ids: &[&str]) -> String {
        for id in ids {
            seed_image(db, id, "2026-09-01T00:00:00Z");
        }
        let scope = SelectionSourceScope::Folder {
            path: "/library/shoot".into(),
            min_size: 0,
            include_rejected: false,
        };
        db.create_selection_run(name, &scope, None).unwrap()
    }

    #[test]
    fn resolver_captures_complete_ordered_source_over_200_images() {
        let db = open_db();
        // Insert 250 images across two folders with distinct import order.
        let mut imported = 0;
        for i in 0..250 {
            imported += 1;
            let folder = if i % 2 == 0 { "shoot-a" } else { "shoot-b" };
            let id = format!("img-{i:03}");
            db.insert_image(&Image {
                id: id.clone(),
                sha256_hash: format!("hash-{id}"),
                width: 800,
                height: 600,
                format: "jpg".to_string(),
                file_size: 1024,
                created_at: "2026-09-01T00:00:00Z".to_string(),
                imported_at: format!("2026-09-01T00:{:02}:{:02}Z", imported / 60, imported % 60),
                ai_prompt: None,
                raw_metadata: None,
            })
            .unwrap();
            db.insert_image_file(&ImageFile {
                id: format!("file-{id}"),
                image_id: id.clone(),
                path: format!("/library/{folder}/{id}.jpg"),
                last_seen_at: "2026-09-01T00:00:00Z".to_string(),
                missing_at: None,
                last_seen_size: None,
                last_seen_mtime: None,
            })
            .unwrap();
        }

        let all = db.resolve_selection_scope_ids(&all_scope()).unwrap();
        assert_eq!(all.len(), 250);

        let folder = db
            .resolve_selection_scope_ids(&SelectionSourceScope::Folder {
                path: "/library/shoot-a".into(),
                min_size: 0,
                include_rejected: false,
            })
            .unwrap();
        assert_eq!(folder.len(), 125);
        assert!(folder.iter().all(|id| {
            let index: usize = id.trim_start_matches("img-").parse().unwrap();
            index % 2 == 0
        }));
        // Ordering: newest import first.
        assert_eq!(folder.first().unwrap(), "img-248");
    }

    #[test]
    fn search_intersection_narrows_base_scope() {
        let db = open_db();
        seed_image(&db, "match-1", "2026-09-01T00:00:01Z");
        seed_image(&db, "match-2", "2026-09-01T00:00:02Z");
        seed_image(&db, "other-1", "2026-09-01T00:00:03Z");
        let conn = db.conn.lock();
        conn.execute(
            "UPDATE image_files SET path = '/library/shoot/final-match-1.jpg' WHERE image_id = 'match-1'",
            [],
        )
        .unwrap();
        conn.execute(
            "UPDATE image_files SET path = '/library/shoot/final-match-2.jpg' WHERE image_id = 'match-2'",
            [],
        )
        .unwrap();
        conn.execute(
            "UPDATE image_files SET path = '/library/shoot/other.jpg' WHERE image_id = 'other-1'",
            [],
        )
        .unwrap();
        drop(conn);

        let scope = SelectionSourceScope::Search {
            base: Box::new(SelectionSourceScope::Folder {
                path: "/library/shoot".into(),
                min_size: 0,
                include_rejected: false,
            }),
            query: "match".into(),
        };
        let ids = db.resolve_selection_scope_ids(&scope).unwrap();
        assert_eq!(ids.len(), 2);
        assert!(ids.contains(&"match-1".to_string()));
        assert!(ids.contains(&"match-2".to_string()));
    }

    #[test]
    fn create_run_captures_snapshot_and_starts_empty() {
        let db = open_db();
        let selection_id = create_run_with_images(&db, "Client final", &["a", "b", "c"]);

        let run = db.get_selection_run(&selection_id).unwrap().unwrap();
        assert_eq!(run.status, "active");
        assert_eq!(run.name, "Client final");
        assert_eq!(run.source_count, 3);
        assert_eq!(run.shortlist_count, 0);
        assert_eq!(run.target_count, None);
        assert_eq!(run.finished_at, None);
        assert_eq!(run.rejected_shortlist_count, 0);
        match run.source_scope {
            SelectionSourceScope::Folder { path, .. } => assert_eq!(path, "/library/shoot"),
            other => panic!("unexpected scope {other:?}"),
        }

        let conn = db.conn.lock();
        let items: i64 = conn
            .query_row("SELECT COUNT(*) FROM collection_items", [], |r| r.get(0))
            .unwrap();
        let selections: i64 = conn
            .query_row("SELECT COUNT(*) FROM selections", [], |r| r.get(0))
            .unwrap();
        assert_eq!(items, 0);
        assert_eq!(selections, 0);
    }

    #[test]
    fn create_run_from_empty_scope_is_rejected() {
        let db = open_db();
        let scope = SelectionSourceScope::Folder {
            path: "/library/empty".into(),
            min_size: 0,
            include_rejected: false,
        };
        let error = db
            .create_selection_run("Nothing", &scope, None)
            .unwrap_err();
        assert!(error.to_string().contains("no images"));
    }

    #[test]
    fn active_selection_run_is_hidden_from_normal_collections() {
        let db = open_db();
        let selection_id = create_run_with_images(&db, "Hidden run", &["a"]);
        let collections = db.list_collections().unwrap();
        assert!(collections.iter().all(|(id, _, _)| id != &selection_id));
    }

    #[test]
    fn group_add_preserves_addition_order_and_is_idempotent() {
        let db = open_db();
        let selection_id = create_run_with_images(&db, "Run", &["a", "b", "c"]);
        let conn = db.conn.lock();
        let added =
            shortlist_append_conn(&conn, &selection_id, &["c".to_string(), "a".to_string()])
                .unwrap();
        assert_eq!(added, vec!["c", "a"]);
        // Re-adding existing members is a no-op.
        let again =
            shortlist_append_conn(&conn, &selection_id, &["a".to_string(), "c".to_string()])
                .unwrap();
        assert!(again.is_empty());
        let mut stmt = conn
            .prepare(&format!(
                "SELECT image_id FROM collection_items WHERE collection_id = '{selection_id}' ORDER BY position"
            ))
            .unwrap();
        let ids = stmt
            .query_map([], |row| row.get::<_, String>(0))
            .unwrap()
            .collect::<Result<Vec<_>>>()
            .unwrap();
        assert_eq!(ids, vec!["c", "a"]);
    }

    #[test]
    fn source_page_survives_missing_files_and_applies_filters() {
        let db = open_db();
        let selection_id = create_run_with_images(&db, "Offline run", &["a", "b"]);
        {
            let conn = db.conn.lock();
            conn.execute(
                "UPDATE image_files SET missing_at = '2026-09-02' WHERE image_id = 'a'",
                [],
            )
            .unwrap();
        }
        let (items, total) = db
            .list_selection_source_page(&selection_id, 10, 0, PageFilters::default())
            .unwrap();
        assert_eq!(total, 2);
        assert_eq!(items.len(), 2);
        let missing = items
            .iter()
            .find(|item| item.image.id == "a")
            .unwrap()
            .missing_at
            .clone();
        assert_eq!(missing.as_deref(), Some("2026-09-02"));

        // Search filter layers within the source.
        let (filtered, filtered_total) = db
            .list_selection_source_page(
                &selection_id,
                10,
                0,
                PageFilters {
                    query: Some("b.jpg".to_string()),
                    ..PageFilters::default()
                },
            )
            .unwrap();
        assert_eq!(filtered_total, 1);
        assert_eq!(filtered[0].image.id, "b");
    }

    #[test]
    fn finish_requires_members_flips_lifecycle_and_preserves_decisions() {
        let db = open_db();
        let selection_id = create_run_with_images(&db, "Final", &["a", "b"]);
        db.set_decision("a", "reject").unwrap();

        // Empty shortlist cannot finish.
        let empty = db.finish_selection_run(&selection_id).unwrap_err();
        assert!(empty.to_string().contains("empty"));

        {
            let mut conn = db.conn.lock();
            let tx = conn.transaction().unwrap();
            shortlist_append_conn(&tx, &selection_id, &["a".to_string()]).unwrap();
            tx.commit().unwrap();
        }

        db.finish_selection_run(&selection_id).unwrap();
        let run = db.get_selection_run(&selection_id).unwrap().unwrap();
        assert_eq!(run.status, "finished");
        assert!(run.finished_at.is_some());
        assert_eq!(run.rejected_shortlist_count, 1);
        // Backing project is now a normal manual collection.
        let collections = db.list_collections().unwrap();
        assert!(collections.iter().any(|(id, _, _)| id == &selection_id));
        // Decision untouched; no file rows changed.
        let decision = db.get_selection_for_image("a").unwrap().unwrap();
        assert_eq!(decision.decision, "reject");
        let conn = db.conn.lock();
        let missing: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM image_files WHERE missing_at IS NOT NULL",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(missing, 0);
    }

    #[test]
    fn reopen_restores_active_membership_and_visibility() {
        let db = open_db();
        let selection_id = create_run_with_images(&db, "Reopen", &["a", "b"]);
        {
            let mut conn = db.conn.lock();
            let tx = conn.transaction().unwrap();
            shortlist_append_conn(&tx, &selection_id, &["b".to_string(), "a".to_string()]).unwrap();
            tx.commit().unwrap();
        }
        db.finish_selection_run(&selection_id).unwrap();
        db.reopen_selection_run(&selection_id).unwrap();
        let run = db.get_selection_run(&selection_id).unwrap().unwrap();
        assert_eq!(run.status, "active");
        assert_eq!(run.shortlist_count, 2);
        assert_eq!(run.finished_at, None);
        let collections = db.list_collections().unwrap();
        assert!(collections.iter().all(|(id, _, _)| id != &selection_id));
        let ids = db.selection_shortlist_ids(&selection_id).unwrap();
        assert_eq!(ids, vec!["b", "a"]);
    }

    #[test]
    fn archive_restore_roundtrips_both_lifecycles() {
        let db = open_db();
        // Archived active run restores to active and stays hidden.
        let active_run = create_run_with_images(&db, "Active archive", &["a"]);
        db.archive_selection_run(&active_run).unwrap();
        let archived = db.get_selection_run(&active_run).unwrap().unwrap();
        assert_eq!(archived.status, "archived");
        db.restore_selection_run(&active_run).unwrap();
        let restored = db.get_selection_run(&active_run).unwrap().unwrap();
        assert_eq!(restored.status, "active");

        // Archived finished run hides its collection and restores to finished.
        let finished_run = create_run_with_images(&db, "Finished archive", &["b"]);
        {
            let mut conn = db.conn.lock();
            let tx = conn.transaction().unwrap();
            shortlist_append_conn(&tx, &finished_run, &["b".to_string()]).unwrap();
            tx.commit().unwrap();
        }
        db.finish_selection_run(&finished_run).unwrap();
        db.archive_selection_run(&finished_run).unwrap();
        let collections = db.list_collections().unwrap();
        assert!(collections.iter().all(|(id, _, _)| id != &finished_run));
        db.restore_selection_run(&finished_run).unwrap();
        let restored = db.get_selection_run(&finished_run).unwrap().unwrap();
        assert_eq!(restored.status, "finished");
        assert!(restored.finished_at.is_some());
        let collections = db.list_collections().unwrap();
        assert!(collections.iter().any(|(id, _, _)| id == &finished_run));
    }

    #[test]
    fn counts_reflect_surviving_foreign_key_references() {
        let db = open_db();
        let selection_id = create_run_with_images(&db, "Drift", &["a", "b", "c"]);
        {
            let mut conn = db.conn.lock();
            let tx = conn.transaction().unwrap();
            shortlist_append_conn(&tx, &selection_id, &["b".to_string()]).unwrap();
            tx.commit().unwrap();
        }
        // Simulate an authorized independent deletion of image "a".
        {
            let conn = db.conn.lock();
            conn.execute("DELETE FROM images WHERE id = 'a'", [])
                .unwrap();
        }
        let run = db.get_selection_run(&selection_id).unwrap().unwrap();
        assert_eq!(run.source_count, 2);
        assert_eq!(run.shortlist_count, 1);
    }

    #[test]
    fn generic_collection_mutations_cannot_touch_selection_runs() {
        let db = open_db();
        let selection_id = create_run_with_images(&db, "Guarded", &["a"]);
        let add = db.add_to_collection(&selection_id, &["a"]);
        assert!(add.is_err());
        let remove = db.remove_from_collection(&selection_id, "a");
        assert!(remove.is_err());
        let delete = db.delete_collection(&selection_id);
        assert!(delete.is_err());
        // Manual collections still accept the generic paths.
        let manual = db.create_collection("Manual").unwrap();
        seed_image(&db, "m-1", "2026-09-01T00:00:00Z");
        db.add_to_collection(&manual, &["m-1"]).unwrap();
        db.remove_from_collection(&manual, "m-1").unwrap();
        db.delete_collection(&manual).unwrap();
    }

    #[test]
    fn reopen_rejects_outside_source_members_added_after_finish() {
        let db = open_db();
        let selection_id = create_run_with_images(&db, "Continue later", &["a", "b"]);
        {
            let mut conn = db.conn.lock();
            let tx = conn.transaction().unwrap();
            shortlist_append_conn(&tx, &selection_id, &["a".to_string()]).unwrap();
            tx.commit().unwrap();
        }
        db.finish_selection_run(&selection_id).unwrap();

        // A finished collection is a normal manual collection; ordinary edits
        // may add a brand-new image to it.
        seed_image(&db, "late-add", "2026-09-03T00:00:00Z");
        db.add_to_collection(&selection_id, &["late-add"]).unwrap();

        // Continue as Selection must not silently admit the outside member
        // or alter the snapshot.
        let error = db.reopen_selection_run(&selection_id).unwrap_err();
        assert!(error
            .to_string()
            .contains("outside the captured selection source"));
        let run = db.get_selection_run(&selection_id).unwrap().unwrap();
        assert_eq!(run.status, "finished", "reopen was refused");
        assert_eq!(run.source_count, 2, "snapshot unchanged");
        {
            let conn = db.conn.lock();
            let count: i64 = conn
                .query_row(
                    "SELECT COUNT(*) FROM selection_run_source_items WHERE selection_id = ?1",
                    rusqlite::params![selection_id],
                    |row| row.get(0),
                )
                .unwrap();
            assert_eq!(count, 2, "snapshot rows retained");
        }
        // Removing the outside member unblocks reopening; all data retained.
        db.remove_from_collection(&selection_id, "late-add")
            .unwrap();
        db.reopen_selection_run(&selection_id).unwrap();
        let run = db.get_selection_run(&selection_id).unwrap().unwrap();
        assert_eq!(run.status, "active");
        assert_eq!(run.shortlist_count, 1);
        assert_eq!(run.source_count, 2);
    }

    #[test]
    fn referenced_folder_scope_resolves_offline_members() {
        use crate::db_core::models::{ReferencedSource, ReferencedSourceKind};
        let db = open_db();
        db.upsert_referenced_source(&ReferencedSource {
            id: "src-1".into(),
            platform_volume_id: Some("vol-1".into()),
            display_name: "SD CARD".into(),
            last_mount_path: Some("/Volumes/SD".into()),
            source_kind: ReferencedSourceKind::SdCard,
            capacity_bytes: None,
            recursive_default: false,
            settings_json: "{}".into(),
            last_seen_at: "2026-09-01T00:00:00Z".into(),
            offline_at: None,
        })
        .unwrap();
        {
            let conn = db.conn.lock();
            conn.execute(
                "INSERT INTO images (id, sha256_hash, width, height, format, file_size, created_at, imported_at)
                 VALUES ('ref-1', 'hash-ref-1', 800, 600, 'jpg', 10, datetime('now'), datetime('now'))",
                [],
            )
            .unwrap();
            conn.execute(
                "INSERT INTO image_files (id, image_id, path, last_seen_at, library_member)
                 VALUES ('ref-file-1', 'ref-1', '/Volumes/SD/DCIM/ref-1.jpg', '2026-09-01', 0)",
                [],
            )
            .unwrap();
        }
        db.attach_referenced_file("src-1", "ref-file-1", "DCIM/ref-1.jpg")
            .unwrap();

        let scope = SelectionSourceScope::ReferencedFolder {
            source_id: "src-1".into(),
            relative_path: "/".into(),
            recursive: true,
            include_rejected: false,
        };
        let ids = db.resolve_selection_scope_ids(&scope).unwrap();
        assert_eq!(ids, vec!["ref-1"]);
    }
}
