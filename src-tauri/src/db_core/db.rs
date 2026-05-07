use rusqlite::{Connection, Result, params};
use std::path::Path;
use std::sync::Mutex;
use super::models::*;

pub struct Database {
    conn: Mutex<Connection>,
}

impl Database {
    pub fn open(db_path: &Path) -> Result<Self> {
        let conn = Connection::open(db_path)?;
        let db = Database { conn: Mutex::new(conn) };
        db.run_migrations()?;
        Ok(db)
    }

    fn run_migrations(&self) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        let schema = include_str!("schema.sql");
        conn.execute_batch(schema)?;
        Ok(())
    }

    pub fn insert_image(&self, image: &Image) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "INSERT OR IGNORE INTO images (id, sha256_hash, width, height, format, file_size, created_at, imported_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
            params![image.id, image.sha256_hash, image.width, image.height,
                    image.format, image.file_size, image.created_at, image.imported_at],
        )?;
        Ok(())
    }

    pub fn insert_image_file(&self, file: &ImageFile) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "INSERT OR REPLACE INTO image_files (id, image_id, path, last_seen_at, missing_at)
             VALUES (?1, ?2, ?3, ?4, ?5)",
            params![file.id, file.image_id, file.path, file.last_seen_at, file.missing_at],
        )?;
        Ok(())
    }

    pub fn find_by_hash(&self, hash: &str) -> Result<Option<Image>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT id, sha256_hash, width, height, format, file_size, created_at, imported_at
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
            })
        })?;
        match rows.next() {
            Some(Ok(img)) => Ok(Some(img)),
            _ => Ok(None),
        }
    }

    pub fn list_images(&self, limit: u32, offset: u32) -> Result<Vec<ImageWithFile>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT i.id, i.sha256_hash, i.width, i.height, i.format, i.file_size,
                    i.created_at, i.imported_at, f.path,
                    s.star_rating, s.color_label, s.decision
             FROM images i
             JOIN image_files f ON f.image_id = i.id AND f.missing_at IS NULL
             LEFT JOIN selections s ON s.image_id = i.id AND s.project_id = '__global__'
             GROUP BY i.id
             ORDER BY i.imported_at DESC
             LIMIT ?1 OFFSET ?2"
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
                },
                path: row.get(8)?,
                thumbnail_path: None,
                selection,
            })
        })?;
        rows.collect::<Result<Vec<_>>>()
    }

    pub fn set_rating(&self, image_id: &str, rating: u8) -> Result<()> {
        let conn = self.conn.lock().unwrap();
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
        let conn = self.conn.lock().unwrap();
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
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT f.path, f.image_id FROM image_files f WHERE f.missing_at IS NULL"
        )?;
        let rows = stmt.query_map([], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
        })?;

        let mut folder_counts: std::collections::HashMap<String, u32> = std::collections::HashMap::new();
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

    pub fn list_images_by_folder(&self, folder: &str, limit: u32, offset: u32) -> Result<Vec<ImageWithFile>> {
        let conn = self.conn.lock().unwrap();
        let pattern = format!("{}/%", folder);
        let mut stmt = conn.prepare(
            "SELECT i.id, i.sha256_hash, i.width, i.height, i.format, i.file_size,
                    i.created_at, i.imported_at, f.path,
                    s.star_rating, s.color_label, s.decision
             FROM images i
             JOIN image_files f ON f.image_id = i.id AND f.missing_at IS NULL
             LEFT JOIN selections s ON s.image_id = i.id AND s.project_id = '__global__'
             WHERE f.path LIKE ?1
             GROUP BY i.id
             ORDER BY i.imported_at DESC
             LIMIT ?2 OFFSET ?3"
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
                },
                path: row.get(8)?,
                thumbnail_path: None,
                selection,
            })
        })?;
        rows.collect::<Result<Vec<_>>>()
    }

    pub fn list_images_filtered(&self, min_width: Option<u32>, min_height: Option<u32>, limit: u32, offset: u32) -> Result<Vec<ImageWithFile>> {
        let conn = self.conn.lock().unwrap();
        let mut sql = String::from(
            "SELECT i.id, i.sha256_hash, i.width, i.height, i.format, i.file_size,
                    i.created_at, i.imported_at, f.path,
                    s.star_rating, s.color_label, s.decision
             FROM images i
             JOIN image_files f ON f.image_id = i.id AND f.missing_at IS NULL
             LEFT JOIN selections s ON s.image_id = i.id AND s.project_id = '__global__'
             WHERE 1=1"
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
                },
                path: row.get(8)?,
                thumbnail_path: None,
                selection,
            })
        })?;
        rows.collect::<Result<Vec<_>>>()
    }

    // ---- Collection methods ----

    pub fn create_collection(&self, name: &str) -> Result<String> {
        let id = uuid::Uuid::new_v4().to_string();
        let now = chrono::Utc::now().to_rfc3339();
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "INSERT INTO projects (id, name, description, created_at) VALUES (?1, ?2, NULL, ?3)",
            params![id, name, now],
        )?;
        Ok(id)
    }

    pub fn list_collections(&self) -> Result<Vec<(String, String, u32)>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT p.id, p.name, COUNT(ci.image_id) as cnt
             FROM projects p
             LEFT JOIN collection_items ci ON ci.collection_id = p.id
             GROUP BY p.id
             ORDER BY p.created_at DESC"
        )?;
        let rows = stmt.query_map([], |row| {
            Ok((row.get(0)?, row.get(1)?, row.get(2)?))
        })?;
        rows.collect::<Result<Vec<_>>>()
    }

    pub fn add_to_collection(&self, collection_id: &str, image_ids: &[&str]) -> Result<()> {
        let conn = self.conn.lock().unwrap();
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
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT i.id, i.sha256_hash, i.width, i.height, i.format, i.file_size,
                    i.created_at, i.imported_at, f.path,
                    s.star_rating, s.color_label, s.decision
             FROM collection_items ci
             JOIN images i ON i.id = ci.image_id
             JOIN image_files f ON f.image_id = i.id AND f.missing_at IS NULL
             LEFT JOIN selections s ON s.image_id = i.id AND s.project_id = '__global__'
             WHERE ci.collection_id = ?1
             GROUP BY i.id
             ORDER BY ci.position ASC"
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
                },
                path: row.get(8)?,
                thumbnail_path: None,
                selection,
            })
        })?;
        rows.collect::<Result<Vec<_>>>()
    }

    pub fn delete_collection(&self, collection_id: &str) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute("DELETE FROM collection_items WHERE collection_id = ?1", params![collection_id])?;
        conn.execute("DELETE FROM projects WHERE id = ?1", params![collection_id])?;
        Ok(())
    }

    // ---- Embedding methods ----

    pub fn store_embedding(&self, image_id: &str, model_name: &str, vector: &[f32]) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        let bytes: Vec<u8> = vector.iter().flat_map(|f| f.to_le_bytes()).collect();
        conn.execute(
            "INSERT OR REPLACE INTO embeddings (id, image_id, model_name, vector, dims, dtype, normalized, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5, 'float32', 1, ?6)",
            params![
                uuid::Uuid::new_v4().to_string(),
                image_id,
                model_name,
                bytes,
                vector.len() as u32,
                chrono::Utc::now().to_rfc3339(),
            ],
        )?;
        Ok(())
    }

    pub fn get_all_embeddings(&self, model_name: &str) -> Result<Vec<(String, Vec<f32>)>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT image_id, vector, dims FROM embeddings WHERE model_name = ?1"
        )?;
        let rows = stmt.query_map(params![model_name], |row| {
            let image_id: String = row.get(0)?;
            let bytes: Vec<u8> = row.get(1)?;
            let _dims: u32 = row.get(2)?;
            let vector: Vec<f32> = bytes.chunks(4)
                .map(|chunk| f32::from_le_bytes([chunk[0], chunk[1], chunk[2], chunk[3]]))
                .collect();
            Ok((image_id, vector))
        })?;
        rows.collect::<Result<Vec<_>>>()
    }

    pub fn find_similar(&self, vector: &[f32], model_name: &str, top_k: usize) -> Result<Vec<(String, f32)>> {
        let all = self.get_all_embeddings(model_name)?;
        let mut scores: Vec<(String, f32)> = all.iter().map(|(id, emb)| {
            let score = cosine_similarity(vector, emb);
            (id.clone(), score)
        }).collect();
        scores.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
        scores.truncate(top_k);
        Ok(scores)
    }

    pub fn embedding_count(&self, model_name: &str) -> Result<u32> {
        let conn = self.conn.lock().unwrap();
        conn.query_row(
            "SELECT COUNT(*) FROM embeddings WHERE model_name = ?1",
            params![model_name],
            |row| row.get(0),
        )
    }

    pub fn remove_from_collection(&self, collection_id: &str, image_id: &str) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "DELETE FROM collection_items WHERE collection_id = ?1 AND image_id = ?2",
            params![collection_id, image_id],
        )?;
        Ok(())
    }

    pub fn delete_images_by_folder(&self, folder: &str) -> Result<u32> {
        let conn = self.conn.lock().unwrap();
        let pattern = format!("{}/%", folder);

        // Get image IDs that ONLY exist in this folder (no other paths)
        let mut stmt = conn.prepare(
            "SELECT DISTINCT f.image_id FROM image_files f
             WHERE f.path LIKE ?1 AND f.missing_at IS NULL
             AND f.image_id NOT IN (
                 SELECT image_id FROM image_files
                 WHERE path NOT LIKE ?1 AND missing_at IS NULL
             )"
        )?;
        let image_ids: Vec<String> = stmt.query_map(params![pattern], |row| {
            row.get(0)
        })?.filter_map(|r| r.ok()).collect();

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

    pub fn image_count(&self) -> Result<u32> {
        let conn = self.conn.lock().unwrap();
        conn.query_row("SELECT COUNT(*) FROM images", [], |row| row.get(0))
    }

    pub fn get_images_by_ids(&self, ids: &[&str]) -> Result<Vec<ImageWithFile>> {
        if ids.is_empty() {
            return Ok(vec![]);
        }
        let conn = self.conn.lock().unwrap();
        let placeholders: Vec<String> = ids.iter().enumerate().map(|(i, _)| format!("?{}", i + 1)).collect();
        let sql = format!(
            "SELECT i.id, i.sha256_hash, i.width, i.height, i.format, i.file_size,
                    i.created_at, i.imported_at, f.path,
                    s.star_rating, s.color_label, s.decision
             FROM images i
             JOIN image_files f ON f.image_id = i.id AND f.missing_at IS NULL
             LEFT JOIN selections s ON s.image_id = i.id AND s.project_id = '__global__'
             WHERE i.id IN ({})
             GROUP BY i.id",
            placeholders.join(", ")
        );
        let params: Vec<&dyn rusqlite::types::ToSql> = ids.iter().map(|id| id as &dyn rusqlite::types::ToSql).collect();
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
                },
                path: row.get(8)?,
                thumbnail_path: None,
                selection,
            })
        })?;
        rows.collect::<Result<Vec<_>>>()
    }

    pub fn get_iteration_siblings(&self, parent_id: &str) -> Result<Vec<ImageWithFile>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT i.id, i.sha256_hash, i.width, i.height, i.format, i.file_size,
                    i.created_at, i.imported_at, f.path,
                    s.star_rating, s.color_label, s.decision
             FROM iterations it
             JOIN images i ON i.id = it.child_id
             JOIN image_files f ON f.image_id = i.id AND f.missing_at IS NULL
             LEFT JOIN selections s ON s.image_id = i.id AND s.project_id = '__global__'
             WHERE it.parent_id = ?1
             GROUP BY i.id
             ORDER BY it.created_at ASC"
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
                },
                path: row.get(8)?,
                thumbnail_path: None,
                selection,
            })
        })?;
        rows.collect::<Result<Vec<_>>>()
    }
}

fn cosine_similarity(a: &[f32], b: &[f32]) -> f32 {
    if a.len() != b.len() { return 0.0; }
    let dot: f32 = a.iter().zip(b.iter()).map(|(x, y)| x * y).sum();
    let norm_a: f32 = a.iter().map(|x| x * x).sum::<f32>().sqrt();
    let norm_b: f32 = b.iter().map(|x| x * x).sum::<f32>().sqrt();
    if norm_a == 0.0 || norm_b == 0.0 { 0.0 } else { dot / (norm_a * norm_b) }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

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
        };
        db.insert_image(&img).unwrap();
        let file = ImageFile {
            id: format!("f-{}", id),
            image_id: id.to_string(),
            path: format!("/tmp/{}.png", id),
            last_seen_at: "2026-05-07T00:00:00Z".to_string(),
            missing_at: None,
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
        let conn = db.conn.lock().unwrap();
        conn.execute(
            "INSERT INTO iterations (id, parent_id, child_id, prompt, model_used, created_at)
             VALUES ('it-1', 'parent', 'child-1', 'make it blue', 'flux', '2026-05-07T00:00:00Z')",
            [],
        ).unwrap();
        conn.execute(
            "INSERT INTO iterations (id, parent_id, child_id, prompt, model_used, created_at)
             VALUES ('it-2', 'parent', 'child-2', 'make it red', 'flux', '2026-05-07T00:00:00Z')",
            [],
        ).unwrap();
        drop(conn);

        let results = db.get_iteration_siblings("parent").unwrap();
        assert_eq!(results.len(), 2);
    }
}
