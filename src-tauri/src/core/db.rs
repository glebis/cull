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
             LEFT JOIN selections s ON s.image_id = i.id AND s.project_id IS NULL
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
             VALUES (?1, NULL, ?2, 'undecided')
             ON CONFLICT(image_id, COALESCE(project_id, '__global__'))
             DO UPDATE SET star_rating = ?2",
            params![image_id, rating],
        )?;
        Ok(())
    }

    pub fn set_decision(&self, image_id: &str, decision: &str) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "INSERT INTO selections (image_id, project_id, decision)
             VALUES (?1, NULL, ?2)
             ON CONFLICT(image_id, COALESCE(project_id, '__global__'))
             DO UPDATE SET decision = ?2",
            params![image_id, decision],
        )?;
        Ok(())
    }

    pub fn image_count(&self) -> Result<u32> {
        let conn = self.conn.lock().unwrap();
        conn.query_row("SELECT COUNT(*) FROM images", [], |row| row.get(0))
    }
}
