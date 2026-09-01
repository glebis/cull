use super::db::{map_image_with_file_row, row_opt_u64, sql_opt_u64, Database};
use super::models::{ImageWithFile, ReferencedFile, ReferencedSource, ReferencedSourceKind};
use super::visibility::RejectedVisibility;
use rusqlite::{params, params_from_iter, types::Type, Error, Result};
use std::path::{Component, Path, PathBuf};

pub(crate) const NORMAL_LIBRARY_FILE_PREDICATE: &str =
    "NOT EXISTS (SELECT 1 FROM referenced_files rf_library WHERE rf_library.image_file_id = f.id)";

fn invalid_source_kind(value: String) -> Error {
    Error::FromSqlConversionFailure(
        4,
        Type::Text,
        std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            format!("unknown referenced source kind: {value}"),
        )
        .into(),
    )
}

fn validate_relative_path(relative_path: &str) -> Result<()> {
    let path = Path::new(relative_path);
    if relative_path.is_empty()
        || path.is_absolute()
        || path.components().any(|component| {
            matches!(
                component,
                Component::ParentDir | Component::RootDir | Component::Prefix(_)
            )
        })
    {
        return Err(Error::InvalidParameterName(
            "relative_path must remain inside the referenced source".to_string(),
        ));
    }
    Ok(())
}

impl Database {
    pub fn original_file_candidates(&self, image_id: &str) -> Result<Vec<(String, bool)>> {
        let conn = self.read_connection();
        let mut stmt = conn.prepare(
            "SELECT f.path, rf.image_file_id IS NOT NULL
             FROM image_files f
             LEFT JOIN referenced_files rf ON rf.image_file_id = f.id
             WHERE f.image_id = ?1
             ORDER BY (rf.image_file_id IS NOT NULL) ASC, (f.missing_at IS NOT NULL) ASC, f.id ASC",
        )?;
        let candidates = stmt
            .query_map(params![image_id], |row| Ok((row.get(0)?, row.get(1)?)))?
            .collect();
        candidates
    }

    pub fn ensure_original_mutation_allowed(&self, image_id: &str) -> Result<()> {
        if self.referenced_source_for_image(image_id)?.is_some() {
            return Err(Error::InvalidParameterName(
                "This original is on a browsed source. Cull will not move, rename, trash, or delete it."
                    .to_string(),
            ));
        }
        Ok(())
    }

    pub fn list_images_in_referenced_folder(
        &self,
        source_id: &str,
        relative_path: &str,
        recursive: bool,
        limit: u32,
        offset: u32,
        include_rejected: bool,
    ) -> Result<Vec<ImageWithFile>> {
        let normalized = relative_path.trim_matches('/');
        let prefix = if normalized.is_empty() {
            String::new()
        } else {
            format!("{normalized}/")
        };
        let sql = format!(
            "SELECT i.id, i.sha256_hash, i.width, i.height, i.format, i.file_size,
                    i.created_at, i.imported_at, f.path,
                    s.star_rating, s.color_label, s.decision, i.source_label, i.ai_prompt,
                    i.raw_metadata, f.missing_at
             FROM referenced_files rf
             JOIN image_files f ON f.id = rf.image_file_id
             JOIN images i ON i.id = f.image_id
             LEFT JOIN selections s ON s.image_id = i.id AND s.project_id = '__global__'
             WHERE rf.source_id = ?1
               AND substr(rf.relative_path, 1, length(?2)) = ?2
               AND (?3 OR instr(substr(rf.relative_path, length(?2) + 1), '/') = 0)
               AND {}
             GROUP BY i.id
             ORDER BY rf.relative_path COLLATE NOCASE
             LIMIT ?4 OFFSET ?5",
            RejectedVisibility::from_include_rejected(include_rejected).sql_predicate()
        );
        let conn = self.read_connection();
        let mut stmt = conn.prepare(&sql)?;
        let rows = stmt.query_map(
            params![source_id, prefix, recursive, limit, offset],
            map_image_with_file_row,
        )?;
        rows.collect()
    }

    pub fn upsert_referenced_source(&self, source: &ReferencedSource) -> Result<()> {
        let conn = self.conn.lock();
        conn.execute(
            "INSERT INTO referenced_sources (
                id, platform_volume_id, display_name, last_mount_path, source_kind,
                capacity_bytes, recursive_default, settings_json, last_seen_at, offline_at
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)
             ON CONFLICT(id) DO UPDATE SET
                platform_volume_id = excluded.platform_volume_id,
                display_name = excluded.display_name,
                last_mount_path = excluded.last_mount_path,
                source_kind = excluded.source_kind,
                capacity_bytes = excluded.capacity_bytes,
                recursive_default = excluded.recursive_default,
                settings_json = excluded.settings_json,
                last_seen_at = excluded.last_seen_at,
                offline_at = excluded.offline_at",
            params![
                source.id,
                source.platform_volume_id,
                source.display_name,
                source.last_mount_path,
                source.source_kind.as_str(),
                sql_opt_u64(source.capacity_bytes)?,
                source.recursive_default,
                source.settings_json,
                source.last_seen_at,
                source.offline_at,
            ],
        )?;
        Ok(())
    }

    pub fn list_referenced_sources(&self) -> Result<Vec<ReferencedSource>> {
        let conn = self.read_connection();
        let mut stmt = conn.prepare(
            "SELECT id, platform_volume_id, display_name, last_mount_path, source_kind,
                    capacity_bytes, recursive_default, settings_json, last_seen_at, offline_at
             FROM referenced_sources
             ORDER BY display_name COLLATE NOCASE, id",
        )?;
        let sources = stmt
            .query_map([], |row| {
                let source_kind: String = row.get(4)?;
                Ok(ReferencedSource {
                    id: row.get(0)?,
                    platform_volume_id: row.get(1)?,
                    display_name: row.get(2)?,
                    last_mount_path: row.get(3)?,
                    source_kind: ReferencedSourceKind::from_db(&source_kind)
                        .ok_or_else(|| invalid_source_kind(source_kind))?,
                    capacity_bytes: row_opt_u64(row, 5)?,
                    recursive_default: row.get(6)?,
                    settings_json: row.get(7)?,
                    last_seen_at: row.get(8)?,
                    offline_at: row.get(9)?,
                })
            })?
            .collect();
        sources
    }

    pub fn thumbnail_purge_candidates_for_offline_sources(
        &self,
        offline_source_ids: &[String],
    ) -> Result<Vec<String>> {
        if offline_source_ids.is_empty() {
            return Ok(Vec::new());
        }

        let placeholders = std::iter::repeat("?")
            .take(offline_source_ids.len())
            .collect::<Vec<_>>()
            .join(", ");
        let sql = format!(
            "SELECT DISTINCT f.image_id
             FROM referenced_files rf
             JOIN image_files f ON f.id = rf.image_file_id
             WHERE rf.source_id IN ({placeholders})
               AND NOT EXISTS (
                   SELECT 1 FROM image_files normal
                   WHERE normal.image_id = f.image_id
                     AND NOT EXISTS (
                         SELECT 1 FROM referenced_files normal_rf
                         WHERE normal_rf.image_file_id = normal.id
                     )
               )
               AND NOT EXISTS (
                   SELECT 1
                   FROM referenced_files online_rf
                   JOIN image_files online_file ON online_file.id = online_rf.image_file_id
                   JOIN referenced_sources online_source ON online_source.id = online_rf.source_id
                   WHERE online_file.image_id = f.image_id
                     AND online_source.offline_at IS NULL
               )
             ORDER BY f.image_id"
        );
        let conn = self.read_connection();
        let mut stmt = conn.prepare(&sql)?;
        let image_ids = stmt
            .query_map(params_from_iter(offline_source_ids.iter()), |row| {
                row.get(0)
            })?
            .collect();
        image_ids
    }

    pub fn attach_referenced_file(
        &self,
        source_id: &str,
        image_file_id: &str,
        relative_path: &str,
    ) -> Result<ReferencedFile> {
        validate_relative_path(relative_path)?;
        let conn = self.conn.lock();
        conn.execute(
            "INSERT INTO referenced_files (source_id, image_file_id, relative_path)
             VALUES (?1, ?2, ?3)
             ON CONFLICT(source_id, relative_path) DO UPDATE SET
                 image_file_id = excluded.image_file_id",
            params![source_id, image_file_id, relative_path],
        )?;
        Ok(ReferencedFile {
            source_id: source_id.to_string(),
            image_file_id: image_file_id.to_string(),
            relative_path: relative_path.to_string(),
        })
    }

    pub fn referenced_source_for_image(&self, image_id: &str) -> Result<Option<ReferencedSource>> {
        let conn = self.read_connection();
        let mut stmt = conn.prepare(
            "SELECT rs.id, rs.platform_volume_id, rs.display_name, rs.last_mount_path,
                    rs.source_kind, rs.capacity_bytes, rs.recursive_default, rs.settings_json,
                    rs.last_seen_at, rs.offline_at
             FROM referenced_sources rs
             JOIN referenced_files rf ON rf.source_id = rs.id
             JOIN image_files f ON f.id = rf.image_file_id
             WHERE f.image_id = ?1
             ORDER BY rs.id
             LIMIT 1",
        )?;
        let mut rows = stmt.query(params![image_id])?;
        let Some(row) = rows.next()? else {
            return Ok(None);
        };
        let source_kind: String = row.get(4)?;
        Ok(Some(ReferencedSource {
            id: row.get(0)?,
            platform_volume_id: row.get(1)?,
            display_name: row.get(2)?,
            last_mount_path: row.get(3)?,
            source_kind: ReferencedSourceKind::from_db(&source_kind)
                .ok_or_else(|| invalid_source_kind(source_kind))?,
            capacity_bytes: row_opt_u64(row, 5)?,
            recursive_default: row.get(6)?,
            settings_json: row.get(7)?,
            last_seen_at: row.get(8)?,
            offline_at: row.get(9)?,
        }))
    }

    pub fn reconnect_referenced_source(
        &self,
        source_id: &str,
        platform_volume_id: Option<&str>,
        new_mount_path: &Path,
        last_seen_at: &str,
    ) -> Result<()> {
        let mut conn = self.conn.lock();
        let tx = conn.transaction()?;
        let stored_platform_id: Option<String> = tx.query_row(
            "SELECT platform_volume_id FROM referenced_sources WHERE id = ?1",
            params![source_id],
            |row| row.get(0),
        )?;
        if stored_platform_id.as_deref() != platform_volume_id {
            return Err(Error::InvalidParameterName(
                "mounted volume identity does not match the remembered source".to_string(),
            ));
        }

        let linked_files = {
            let mut stmt = tx.prepare(
                "SELECT image_file_id, relative_path FROM referenced_files WHERE source_id = ?1",
            )?;
            let files = stmt
                .query_map(params![source_id], |row| {
                    Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
                })?
                .collect::<Result<Vec<_>>>()?;
            files
        };
        for (image_file_id, relative_path) in linked_files {
            validate_relative_path(&relative_path)?;
            let path = new_mount_path.join(PathBuf::from(relative_path));
            tx.execute(
                "UPDATE image_files SET path = ?1, missing_at = NULL, last_seen_at = ?2 WHERE id = ?3",
                params![path.to_string_lossy(), last_seen_at, image_file_id],
            )?;
        }
        tx.execute(
            "UPDATE referenced_sources
             SET last_mount_path = ?1, last_seen_at = ?2, offline_at = NULL
             WHERE id = ?3",
            params![new_mount_path.to_string_lossy(), last_seen_at, source_id],
        )?;
        tx.commit()
    }

    pub fn remove_referenced_source(&self, source_id: &str) -> Result<Vec<String>> {
        let mut conn = self.conn.lock();
        let tx = conn.transaction()?;
        let linked = {
            let mut stmt = tx.prepare(
                "SELECT rf.image_file_id, f.image_id
                 FROM referenced_files rf
                 JOIN image_files f ON f.id = rf.image_file_id
                 WHERE rf.source_id = ?1",
            )?;
            let files = stmt
                .query_map(params![source_id], |row| {
                    Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
                })?
                .collect::<Result<Vec<_>>>()?;
            files
        };
        tx.execute(
            "DELETE FROM referenced_sources WHERE id = ?1",
            params![source_id],
        )?;
        for (file_id, _) in &linked {
            tx.execute("DELETE FROM image_files WHERE id = ?1", params![file_id])?;
        }

        let mut orphaned = Vec::new();
        for (_, image_id) in linked {
            let remaining: i64 = tx.query_row(
                "SELECT COUNT(*) FROM image_files WHERE image_id = ?1",
                params![image_id],
                |row| row.get(0),
            )?;
            if remaining == 0 && !orphaned.contains(&image_id) {
                tx.execute("DELETE FROM images WHERE id = ?1", params![image_id])?;
                orphaned.push(image_id);
            }
        }
        tx.commit()?;
        Ok(orphaned)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rusqlite::Connection;
    use tempfile::tempdir;

    fn test_db() -> (tempfile::TempDir, Database) {
        let dir = tempdir().unwrap();
        let db = Database::open(&dir.path().join("cull.db")).unwrap();
        (dir, db)
    }

    fn sample_source() -> ReferencedSource {
        ReferencedSource {
            id: "source-1".into(),
            platform_volume_id: Some("volume-uuid-1".into()),
            display_name: "UNTITLED".into(),
            last_mount_path: Some("/Volumes/UNTITLED".into()),
            source_kind: ReferencedSourceKind::SdCard,
            capacity_bytes: Some(64_000_000_000),
            recursive_default: false,
            settings_json: "{}".into(),
            last_seen_at: "2026-08-30T10:00:00Z".into(),
            offline_at: None,
        }
    }

    fn insert_image_file(db: &Database, image_id: &str, file_id: &str, path: &str) {
        let conn = db.conn.lock();
        conn.execute(
            "INSERT INTO images (id, sha256_hash, width, height, format, file_size, created_at, imported_at)
             VALUES (?1, ?2, 100, 100, 'jpg', 10, datetime('now'), datetime('now'))",
            params![image_id, format!("hash-{image_id}")],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO image_files (id, image_id, path, last_seen_at)
             VALUES (?1, ?2, ?3, '2026-08-30')",
            params![file_id, image_id, path],
        )
        .unwrap();
    }

    #[test]
    fn migration_27_upgrades_a_full_version_26_database() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("cull.db");
        drop(Database::open(&path).unwrap());
        let conn = Connection::open(&path).unwrap();
        conn.execute_batch(
            "DROP TABLE referenced_files;
             DROP TABLE referenced_sources;
             DELETE FROM schema_migrations WHERE version = 27;
             DELETE FROM schema_migration_steps WHERE version = 27;
             PRAGMA user_version = 26;",
        )
        .unwrap();
        drop(conn);

        let db = Database::open(&path).unwrap();
        let conn = db.conn.lock();
        for name in [
            "referenced_sources",
            "referenced_files",
            "idx_referenced_sources_mount_path",
            "idx_referenced_files_source",
            "idx_referenced_files_image_file",
        ] {
            let count: i64 = conn
                .query_row(
                    "SELECT COUNT(*) FROM sqlite_master WHERE name = ?1",
                    params![name],
                    |row| row.get(0),
                )
                .unwrap();
            assert_eq!(count, 1, "missing schema object {name}");
        }
    }

    #[test]
    fn referenced_source_round_trips() {
        let (_dir, db) = test_db();
        let source = sample_source();
        db.upsert_referenced_source(&source).unwrap();
        assert_eq!(db.list_referenced_sources().unwrap(), vec![source]);
    }

    #[test]
    fn original_file_candidates_prefer_normal_rows_before_referenced_rows() {
        let (_dir, db) = test_db();
        let normal_path = "/library/normal.jpg";
        let referenced_path = "/Volumes/UNTITLED/referenced.jpg";
        insert_image_file(&db, "image-1", "z-normal", normal_path);
        {
            let conn = db.conn.lock();
            conn.execute(
                "INSERT INTO image_files (id, image_id, path, last_seen_at)
                 VALUES ('a-referenced', 'image-1', ?1, '2026-08-30')",
                params![referenced_path],
            )
            .unwrap();
        }
        db.upsert_referenced_source(&sample_source()).unwrap();
        db.attach_referenced_file("source-1", "a-referenced", "referenced.jpg")
            .unwrap();

        assert_eq!(
            db.original_file_candidates("image-1").unwrap(),
            vec![
                (normal_path.to_string(), false),
                (referenced_path.to_string(), true),
            ]
        );
    }

    #[test]
    fn reconnect_updates_paths_without_losing_image_identity_or_review() {
        let (_dir, db) = test_db();
        let source = sample_source();
        db.upsert_referenced_source(&source).unwrap();
        insert_image_file(
            &db,
            "image-1",
            "file-1",
            "/Volumes/UNTITLED/DCIM/100CANON/IMG_0001.JPG",
        );
        db.attach_referenced_file("source-1", "file-1", "DCIM/100CANON/IMG_0001.JPG")
            .unwrap();
        db.set_decision("image-1", "accept").unwrap();

        db.reconnect_referenced_source(
            "source-1",
            Some("volume-uuid-1"),
            Path::new("/Volumes/UNTITLED 1"),
            "2026-08-30T11:00:00Z",
        )
        .unwrap();

        let conn = db.conn.lock();
        let (path, image_id): (String, String) = conn
            .query_row(
                "SELECT path, image_id FROM image_files WHERE id = 'file-1'",
                [],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .unwrap();
        assert_eq!(path, "/Volumes/UNTITLED 1/DCIM/100CANON/IMG_0001.JPG");
        assert_eq!(image_id, "image-1");
        let decision: String = conn
            .query_row(
                "SELECT decision FROM selections WHERE image_id = 'image-1' AND project_id = '__global__'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(decision, "accept");
    }

    #[test]
    fn attach_rejects_paths_that_escape_the_source() {
        let (_dir, db) = test_db();
        db.upsert_referenced_source(&sample_source()).unwrap();
        insert_image_file(&db, "image-1", "file-1", "/tmp/image.jpg");
        assert!(db
            .attach_referenced_file("source-1", "file-1", "../image.jpg")
            .is_err());
        assert!(db
            .attach_referenced_file("source-1", "file-1", "/tmp/image.jpg")
            .is_err());
    }

    #[test]
    fn referenced_originals_are_protected_from_file_mutation_commands() {
        let (_dir, db) = test_db();
        db.upsert_referenced_source(&sample_source()).unwrap();
        insert_image_file(&db, "image-1", "file-1", "/Volumes/UNTITLED/DCIM/image.jpg");
        db.attach_referenced_file("source-1", "file-1", "DCIM/image.jpg")
            .unwrap();
        let error = db.ensure_original_mutation_allowed("image-1").unwrap_err();
        assert!(error
            .to_string()
            .contains("will not move, rename, trash, or delete"));
        assert!(db
            .ensure_original_mutation_allowed("not-referenced")
            .is_ok());
    }

    #[test]
    fn referenced_only_images_are_not_permanent_library_members() {
        let (_dir, db) = test_db();
        db.upsert_referenced_source(&sample_source()).unwrap();
        insert_image_file(
            &db,
            "image-1",
            "file-1",
            "/Volumes/UNTITLED/DCIM/100CANON/IMG_0001.JPG",
        );
        db.attach_referenced_file("source-1", "file-1", "DCIM/100CANON/IMG_0001.JPG")
            .unwrap();

        assert!(db
            .list_images_with_visibility(20, 0, true)
            .unwrap()
            .is_empty());
        assert_eq!(db.image_count_with_visibility(true).unwrap(), 0);
        assert!(db
            .evaluate_smart_collection(
                r#"{"type":"rule","field":"imported_at","op":"last_n_days","value":7.0}"#
            )
            .unwrap()
            .is_empty());
        assert_eq!(
            db.list_images_in_referenced_folder("source-1", "", true, 20, 0, true)
                .unwrap()
                .len(),
            1
        );
    }

    #[test]
    fn normal_file_keeps_a_referenced_image_in_permanent_scopes() {
        let (_dir, db) = test_db();
        db.upsert_referenced_source(&sample_source()).unwrap();
        insert_image_file(
            &db,
            "image-1",
            "file-1",
            "/Volumes/UNTITLED/DCIM/100CANON/IMG_0001.JPG",
        );
        db.attach_referenced_file("source-1", "file-1", "DCIM/100CANON/IMG_0001.JPG")
            .unwrap();
        {
            let conn = db.conn.lock();
            conn.execute(
                "INSERT INTO image_files (id, image_id, path, last_seen_at)
                 VALUES ('kept-file', 'image-1', '/Pictures/kept.jpg', '2026-08-30')",
                [],
            )
            .unwrap();
        }

        let all_images = db.list_images_with_visibility(20, 0, true).unwrap();
        assert_eq!(all_images.len(), 1);
        assert_eq!(all_images[0].path, "/Pictures/kept.jpg");

        let recent_imports = db
            .evaluate_smart_collection(
                r#"{"type":"rule","field":"imported_at","op":"last_n_days","value":7.0}"#,
            )
            .unwrap();
        assert_eq!(recent_imports.len(), 1);
        assert_eq!(recent_imports[0].path, "/Pictures/kept.jpg");
    }

    #[test]
    fn offline_thumbnail_purge_keeps_normal_and_online_references() {
        let (_dir, db) = test_db();
        let source_1 = sample_source();
        let source_2 = ReferencedSource {
            id: "source-2".into(),
            platform_volume_id: Some("volume-uuid-2".into()),
            display_name: "SECOND CARD".into(),
            last_mount_path: Some("/Volumes/SECOND".into()),
            ..sample_source()
        };
        db.upsert_referenced_source(&source_1).unwrap();
        db.upsert_referenced_source(&source_2).unwrap();

        insert_image_file(
            &db,
            "referenced-only",
            "referenced-only-file",
            "/Volumes/UNTITLED/referenced-only.jpg",
        );
        db.attach_referenced_file("source-1", "referenced-only-file", "referenced-only.jpg")
            .unwrap();

        insert_image_file(
            &db,
            "also-online",
            "also-online-file",
            "/Volumes/UNTITLED/also-online.jpg",
        );
        db.attach_referenced_file("source-1", "also-online-file", "also-online.jpg")
            .unwrap();
        {
            let conn = db.conn.lock();
            conn.execute(
                "INSERT INTO image_files (id, image_id, path, last_seen_at)
                 VALUES ('also-online-file-source-2', 'also-online', '/Volumes/SECOND/also-online.jpg', '2026-08-30')",
                [],
            )
            .unwrap();
        }
        db.attach_referenced_file("source-2", "also-online-file-source-2", "also-online.jpg")
            .unwrap();

        insert_image_file(
            &db,
            "mixed",
            "mixed-referenced-file",
            "/Volumes/UNTITLED/mixed.jpg",
        );
        db.attach_referenced_file("source-1", "mixed-referenced-file", "mixed.jpg")
            .unwrap();
        {
            let conn = db.conn.lock();
            conn.execute(
                "INSERT INTO image_files (id, image_id, path, last_seen_at)
                 VALUES ('mixed-normal-file', 'mixed', '/Pictures/mixed.jpg', '2026-08-30')",
                [],
            )
            .unwrap();
        }

        let mut offline_source = source_1;
        offline_source.offline_at = Some("2026-09-01T10:00:00Z".into());
        db.upsert_referenced_source(&offline_source).unwrap();

        assert_eq!(
            db.thumbnail_purge_candidates_for_offline_sources(&["source-1".into()])
                .unwrap(),
            vec!["referenced-only".to_string()]
        );
    }

    #[test]
    fn scoped_library_browsing_excludes_referenced_files_in_all_or_branches() {
        let (_dir, db) = test_db();
        db.upsert_referenced_source(&sample_source()).unwrap();
        insert_image_file(
            &db,
            "referenced-image",
            "referenced-file",
            "/Volumes/UNTITLED/DCIM/IMG_0001.JPG",
        );
        db.attach_referenced_file("source-1", "referenced-file", "DCIM/IMG_0001.JPG")
            .unwrap();
        insert_image_file(&db, "kept-image", "kept-file", "/Pictures/kept.jpg");

        let images = db
            .list_images_in_scope(
                &["/Volumes/UNTITLED".to_string(), "/Pictures".to_string()],
                &[],
                &[],
                20,
                0,
            )
            .unwrap();

        assert_eq!(images.len(), 1);
        assert_eq!(images[0].path, "/Pictures/kept.jpg");
    }

    #[test]
    fn removing_source_removes_only_its_file_references() {
        let (_dir, db) = test_db();
        db.upsert_referenced_source(&sample_source()).unwrap();
        insert_image_file(&db, "orphan", "external-file", "/Volumes/UNTITLED/a.jpg");
        insert_image_file(&db, "shared", "external-shared", "/Volumes/UNTITLED/b.jpg");
        {
            let conn = db.conn.lock();
            conn.execute(
                "INSERT INTO image_files (id, image_id, path, last_seen_at) VALUES ('local-shared', 'shared', '/Pictures/b.jpg', '2026-08-30')",
                [],
            )
            .unwrap();
        }
        db.attach_referenced_file("source-1", "external-file", "a.jpg")
            .unwrap();
        db.attach_referenced_file("source-1", "external-shared", "b.jpg")
            .unwrap();

        let orphaned = db.remove_referenced_source("source-1").unwrap();
        assert_eq!(orphaned, vec!["orphan"]);
        let conn = db.conn.lock();
        let shared_count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM images WHERE id = 'shared'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(shared_count, 1);
    }
}
