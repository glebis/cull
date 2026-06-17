use chrono::Utc;
use regex::Regex;
use rusqlite::{params, Result};
use std::collections::{HashMap, HashSet};
use std::sync::LazyLock;
use uuid::Uuid;

use super::db::Database;
use super::models::ImageWithFile;

// --- Filename stem extraction ---

static VERSION_SUFFIX_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"[-_\s]?(?:v\d+[a-z]?|V\d+|\(\d+\)|final|copy|\d{1,2})$").unwrap()
});

static DALLE_TIMESTAMP_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r"^DALL[\-·\.]?E[\s_]?(\d{4}[-.]?\d{2}[-.]?\d{2})[\s_]?(\d{2})[.\-](\d{2})[.\-](\d{2})",
    )
    .unwrap()
});

static COMFYUI_BATCH_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^(ComfyUI)_\d+_?$").unwrap());

static TRAILING_LETTER_RE: LazyLock<Regex> = LazyLock::new(|| {
    // Match trailing letter variant: icon-v5a, icon-v5-a, thing_3b
    Regex::new(r"(\d)[-_]?([a-d])$").unwrap()
});

static UUID_RUN_DIR_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"^[0-9a-fA-F]{8}-[0-9a-fA-F]{4}-[0-9a-fA-F]{4}-[0-9a-fA-F]{4}-[0-9a-fA-F]{12}$")
        .unwrap()
});

pub fn extract_stem(filename: &str) -> String {
    let name = std::path::Path::new(filename)
        .file_stem()
        .and_then(|n| n.to_str())
        .unwrap_or(filename);

    // DALL-E: group by date
    if let Some(caps) = DALLE_TIMESTAMP_RE.captures(name) {
        return format!("dalle-{}", &caps[1]);
    }

    // ComfyUI: group by prefix
    if let Some(caps) = COMFYUI_BATCH_RE.captures(name) {
        return caps[1].to_lowercase();
    }

    let mut stem = name.to_string();

    // Strip trailing letter suffix first (icon-v5a → icon-v5)
    let stripped = TRAILING_LETTER_RE.replace(&stem, "$1").to_string();
    if !stripped.is_empty() {
        stem = stripped;
    }

    // Strip version suffix once (favicon-v2 → favicon), but not if
    // the letter strip already narrowed it (icon-v5 stays icon-v5)
    if stem == name {
        let stripped = VERSION_SUFFIX_RE.replace(&stem, "").to_string();
        if !stripped.is_empty() {
            stem = stripped;
        }
    }

    stem.to_lowercase()
}

pub fn extract_run_folder_key(path: &str) -> Option<String> {
    let parent = std::path::Path::new(path).parent()?;
    let dir_name = parent.file_name()?.to_str()?;
    if UUID_RUN_DIR_RE.is_match(dir_name) {
        Some(dir_name.to_lowercase())
    } else {
        None
    }
}

fn run_folder_group_name(key: &str) -> String {
    let short = key.split('-').next().unwrap_or(key);
    format!("Run {}", short)
}

// --- Lineage scoring ---

#[derive(Debug, Clone)]
pub struct LineageSignals {
    pub filename_stem_match: bool,
    pub same_import_batch: bool,
    pub temporal_proximity: bool, // created within 60s
    pub same_dimensions: bool,
    pub clip_similarity: Option<f64>,
    pub prompt_match: bool,
}

impl LineageSignals {
    pub fn score(&self) -> u32 {
        let mut s = 0u32;
        if self.prompt_match {
            s += 50;
        }
        if self.filename_stem_match {
            s += 25;
        }
        if self.same_import_batch {
            s += 10;
        }
        if self.temporal_proximity {
            s += 10;
        }
        if let Some(sim) = self.clip_similarity {
            if sim > 0.85 {
                s += 15;
            }
        }
        if self.same_dimensions {
            s += 5;
        }
        s
    }
}

pub fn compare_images_for_lineage(
    a: &ImageWithFile,
    b: &ImageWithFile,
    a_stem: &str,
    b_stem: &str,
    same_batch: bool,
) -> LineageSignals {
    let temporal = match (
        chrono::DateTime::parse_from_rfc3339(&a.image.created_at),
        chrono::DateTime::parse_from_rfc3339(&b.image.created_at),
    ) {
        (Ok(ta), Ok(tb)) => (ta - tb).num_seconds().unsigned_abs() < 60,
        _ => false,
    };

    LineageSignals {
        filename_stem_match: !a_stem.is_empty() && a_stem == b_stem,
        same_import_batch: same_batch,
        temporal_proximity: temporal,
        same_dimensions: a.image.width == b.image.width && a.image.height == b.image.height,
        clip_similarity: None, // filled in by caller if embeddings available
        prompt_match: false,   // filled in by caller if metadata available
    }
}

// --- Database methods ---

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct LineageGroup {
    pub id: String,
    pub name: String,
    pub created_at: String,
    pub detection_method: Option<String>,
    pub detection_score: Option<f64>,
    pub image_count: u32,
}

impl Database {
    pub fn create_lineage_group(&self, name: &str, method: &str, score: f64) -> Result<String> {
        let id = Uuid::new_v4().to_string();
        let now = Utc::now().to_rfc3339();
        let conn = self.conn.lock();
        conn.execute(
            "INSERT INTO lineage_groups (id, name, created_at, detection_method, detection_score)
             VALUES (?1, ?2, ?3, ?4, ?5)",
            params![id, name, now, method, score],
        )?;
        Ok(id)
    }

    pub fn assign_to_lineage_group(
        &self,
        image_id: &str,
        group_id: &str,
        order: i32,
    ) -> Result<()> {
        let conn = self.conn.lock();
        conn.execute(
            "UPDATE images SET lineage_group_id = ?1, lineage_order = ?2 WHERE id = ?3",
            params![group_id, order, image_id],
        )?;
        Ok(())
    }

    pub fn remove_from_lineage_group(&self, image_id: &str) -> Result<()> {
        let conn = self.conn.lock();
        conn.execute(
            "UPDATE images SET lineage_group_id = NULL, lineage_order = 0 WHERE id = ?1",
            params![image_id],
        )?;
        Ok(())
    }

    pub fn list_lineage_groups(&self) -> Result<Vec<LineageGroup>> {
        let conn = self.conn.lock();
        let mut stmt = conn.prepare(
            "SELECT lg.id, lg.name, lg.created_at, lg.detection_method, lg.detection_score,
                    COUNT(DISTINCT i.id) as cnt
             FROM lineage_groups lg
             JOIN images i ON i.lineage_group_id = lg.id
             JOIN image_files f ON f.image_id = i.id AND f.missing_at IS NULL
             GROUP BY lg.id
             HAVING COUNT(DISTINCT i.id) >= 2
             ORDER BY lg.created_at DESC",
        )?;
        let rows = stmt.query_map([], |row| {
            Ok(LineageGroup {
                id: row.get(0)?,
                name: row.get(1)?,
                created_at: row.get(2)?,
                detection_method: row.get(3)?,
                detection_score: row.get(4)?,
                image_count: row.get(5)?,
            })
        })?;
        rows.collect::<std::result::Result<Vec<_>, _>>()
    }

    pub fn get_lineage_group_images(&self, group_id: &str) -> Result<Vec<ImageWithFile>> {
        let conn = self.conn.lock();
        let mut stmt = conn.prepare(
            "SELECT i.id, i.sha256_hash, i.width, i.height, i.format, i.file_size,
                    i.created_at, i.imported_at, f.path,
                    s.star_rating, s.color_label, s.decision, i.source_label, i.ai_prompt,
                    i.raw_metadata, f.missing_at
             FROM images i
             JOIN image_files f ON f.image_id = i.id AND f.missing_at IS NULL
             LEFT JOIN selections s ON s.image_id = i.id AND s.project_id = '__global__'
             WHERE i.lineage_group_id = ?1
             GROUP BY i.id
             ORDER BY i.lineage_order ASC, i.created_at ASC",
        )?;
        let rows = stmt.query_map(params![group_id], |row| {
            let star: Option<u8> = row.get(9)?;
            let color: Option<String> = row.get(10)?;
            let decision: Option<String> = row.get(11)?;
            let selection = super::models::Selection::from_nullable_parts(
                row.get(0)?,
                None,
                star,
                color,
                decision,
            );
            Ok(ImageWithFile {
                image: super::models::Image {
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
        rows.collect::<std::result::Result<Vec<_>, _>>()
    }

    pub fn merge_lineage_groups(&self, keep_id: &str, merge_id: &str) -> Result<()> {
        let conn = self.conn.lock();
        conn.execute(
            "UPDATE images SET lineage_group_id = ?1 WHERE lineage_group_id = ?2",
            params![keep_id, merge_id],
        )?;
        conn.execute(
            "DELETE FROM lineage_groups WHERE id = ?1",
            params![merge_id],
        )?;
        Ok(())
    }

    pub fn dissolve_lineage_group(&self, group_id: &str) -> Result<()> {
        let conn = self.conn.lock();
        conn.execute(
            "UPDATE images SET lineage_group_id = NULL, lineage_order = 0 WHERE lineage_group_id = ?1",
            params![group_id],
        )?;
        conn.execute(
            "DELETE FROM lineage_groups WHERE id = ?1",
            params![group_id],
        )?;
        Ok(())
    }

    pub fn rename_lineage_group(&self, group_id: &str, name: &str) -> Result<()> {
        let conn = self.conn.lock();
        conn.execute(
            "UPDATE lineage_groups SET name = ?1 WHERE id = ?2",
            params![name, group_id],
        )?;
        Ok(())
    }

    // --- Import batch methods ---

    pub fn create_import_batch(
        &self,
        source: &str,
        count: u32,
        collection_id: Option<&str>,
    ) -> Result<String> {
        let id = Uuid::new_v4().to_string();
        let now = Utc::now().to_rfc3339();
        let conn = self.conn.lock();
        conn.execute(
            "INSERT INTO import_batches (id, created_at, source, image_count, collection_id)
             VALUES (?1, ?2, ?3, ?4, ?5)",
            params![id, now, source, count, collection_id],
        )?;
        Ok(id)
    }

    pub fn set_image_batch(&self, image_id: &str, batch_id: &str) -> Result<()> {
        let conn = self.conn.lock();
        conn.execute(
            "UPDATE images SET import_batch_id = ?1 WHERE id = ?2",
            params![batch_id, image_id],
        )?;
        Ok(())
    }

    pub fn get_batch_images(&self, batch_id: &str) -> Result<Vec<ImageWithFile>> {
        let conn = self.conn.lock();
        let mut stmt = conn.prepare(
            "SELECT i.id, i.sha256_hash, i.width, i.height, i.format, i.file_size,
                    i.created_at, i.imported_at, f.path,
                    s.star_rating, s.color_label, s.decision, i.source_label, i.ai_prompt,
                    i.raw_metadata, f.missing_at
             FROM images i
             JOIN image_files f ON f.image_id = i.id AND f.missing_at IS NULL
             LEFT JOIN selections s ON s.image_id = i.id AND s.project_id = '__global__'
             WHERE i.import_batch_id = ?1
             GROUP BY i.id
             ORDER BY i.imported_at ASC",
        )?;
        let rows = stmt.query_map(params![batch_id], |row| {
            let star: Option<u8> = row.get(9)?;
            let color: Option<String> = row.get(10)?;
            let decision: Option<String> = row.get(11)?;
            let selection = super::models::Selection::from_nullable_parts(
                row.get(0)?,
                None,
                star,
                color,
                decision,
            );
            Ok(ImageWithFile {
                image: super::models::Image {
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
        rows.collect::<std::result::Result<Vec<_>, _>>()
    }

    // --- Lineage detection pipeline ---

    pub fn detect_lineage_for_batch(&self, image_ids: &[String]) -> Result<Vec<String>> {
        if image_ids.len() < 2 {
            return Ok(vec![]);
        }

        let id_refs: Vec<&str> = image_ids.iter().map(|s| s.as_str()).collect();
        let images = self.get_images_by_ids(&id_refs)?;

        // Extract stems for all images
        let stems: Vec<String> = images
            .iter()
            .map(|img| {
                let filename = std::path::Path::new(&img.path)
                    .file_name()
                    .and_then(|n| n.to_str())
                    .unwrap_or("");
                extract_stem(filename)
            })
            .collect();

        let mut created_groups = vec![];
        let mut assigned_indices: HashSet<usize> = HashSet::new();

        // Codex image generation writes sibling outputs into UUID-like run
        // directories. That folder is not a parent-child proof, but it is a
        // strong "generated together" signal for culling variants.
        let mut run_folder_groups: HashMap<String, Vec<usize>> = HashMap::new();
        for (i, img) in images.iter().enumerate() {
            if let Some(key) = extract_run_folder_key(&img.path) {
                run_folder_groups.entry(key).or_default().push(i);
            }
        }

        for (folder_key, indices) in &run_folder_groups {
            if indices.len() < 2 {
                continue;
            }

            let existing_group: Option<String> = indices.iter().find_map(|&i| {
                let conn = self.conn.lock();
                conn.query_row(
                    "SELECT lineage_group_id FROM images WHERE id = ?1 AND lineage_group_id IS NOT NULL",
                    params![images[i].image.id],
                    |row| row.get(0),
                )
                .ok()
            });

            let group_id = if let Some(existing) = existing_group {
                existing
            } else {
                self.create_lineage_group(&run_folder_group_name(folder_key), "run_folder", 95.0)?
            };

            let mut ordered_indices = indices.clone();
            ordered_indices.sort_by(|a, b| images[*a].path.cmp(&images[*b].path));

            for (order, idx) in ordered_indices.into_iter().enumerate() {
                self.assign_to_lineage_group(&images[idx].image.id, &group_id, order as i32)?;
                assigned_indices.insert(idx);
            }

            created_groups.push(group_id);
        }

        // Group by stem
        let mut stem_groups: HashMap<String, Vec<usize>> = HashMap::new();
        for (i, stem) in stems.iter().enumerate() {
            if !assigned_indices.contains(&i) && !stem.is_empty() {
                stem_groups.entry(stem.clone()).or_default().push(i);
            }
        }

        for (stem, indices) in &stem_groups {
            if indices.len() < 2 {
                continue;
            }

            // Score all pairs in this stem group
            let mut total_score = 0u32;
            let mut pair_count = 0u32;
            for i in 0..indices.len() {
                for j in (i + 1)..indices.len() {
                    let signals = compare_images_for_lineage(
                        &images[indices[i]],
                        &images[indices[j]],
                        &stems[indices[i]],
                        &stems[indices[j]],
                        true, // same batch
                    );
                    total_score += signals.score();
                    pair_count += 1;
                }
            }

            let avg_score = if pair_count > 0 {
                total_score / pair_count
            } else {
                0
            };
            if avg_score < 25 {
                continue;
            }

            // Check if any image already has a lineage group
            let existing_group: Option<String> = indices.iter().find_map(|&i| {
                let conn = self.conn.lock();
                conn.query_row(
                    "SELECT lineage_group_id FROM images WHERE id = ?1 AND lineage_group_id IS NOT NULL",
                    params![images[i].image.id],
                    |row| row.get(0),
                ).ok()
            });

            let group_id = if let Some(existing) = existing_group {
                existing
            } else {
                let name = format!("{} series", stem);
                self.create_lineage_group(&name, "auto", avg_score as f64)?
            };

            for (order, &idx) in indices.iter().enumerate() {
                self.assign_to_lineage_group(&images[idx].image.id, &group_id, order as i32)?;
            }

            created_groups.push(group_id);
        }

        Ok(created_groups)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db_core::models::{Image, ImageFile};
    use std::path::Path;

    #[test]
    fn test_extract_stem_version_suffix() {
        assert_eq!(extract_stem("icon-v5a.png"), "icon-v5");
        assert_eq!(extract_stem("icon-v5b.png"), "icon-v5");
        assert_eq!(extract_stem("favicon-v2.png"), "favicon");
        assert_eq!(extract_stem("favicon-v3-grid.png"), "favicon-v3-grid");
    }

    #[test]
    fn test_extract_stem_parenthetical() {
        assert_eq!(extract_stem("image(1).png"), "image");
        assert_eq!(extract_stem("image(2).png"), "image");
    }

    #[test]
    fn test_extract_stem_dalle() {
        assert_eq!(
            extract_stem("DALL\u{b7}E 2026-05-09 14.32.01.png"),
            "dalle-2026-05-09"
        );
        assert_eq!(
            extract_stem("DALL-E 2026-05-09 14.35.22.png"),
            "dalle-2026-05-09"
        );
    }

    #[test]
    fn test_extract_stem_comfyui() {
        assert_eq!(extract_stem("ComfyUI_00042_.png"), "comfyui");
        assert_eq!(extract_stem("ComfyUI_00043_.png"), "comfyui");
    }

    #[test]
    fn test_extract_stem_preserves_meaningful_names() {
        assert_eq!(extract_stem("hero-banner.png"), "hero-banner");
        assert_eq!(extract_stem("logo.png"), "logo");
    }

    #[test]
    fn test_extract_run_folder_key_codex_generated_images() {
        let run_id = "019e2166-0bb7-79e0-b00a-37ebdb28d4a0";
        let path = format!(
            "/Users/example/.codex/generated_images/{}/ig_0c29e1a8045487e3016a048af48cac8191b863fce2bd8960aa.png",
            run_id
        );
        assert_eq!(extract_run_folder_key(&path).as_deref(), Some(run_id));
        assert_eq!(
            extract_run_folder_key("/Users/example/.codex/generated_images/not-a-run/ig.png"),
            None
        );
    }

    fn insert_test_image(db: &Database, id: &str, path: &str) {
        let now = "2026-05-13T10:00:00Z".to_string();
        db.insert_image(&Image {
            id: id.to_string(),
            sha256_hash: format!("hash-{}", id),
            width: 1024,
            height: 1024,
            format: "png".to_string(),
            file_size: 1024,
            created_at: now.clone(),
            imported_at: now.clone(),
            ai_prompt: None,
            raw_metadata: None,
        })
        .unwrap();
        db.insert_image_file(&ImageFile {
            id: format!("file-{}", id),
            image_id: id.to_string(),
            path: path.to_string(),
            last_seen_at: now,
            missing_at: None,
            last_seen_size: Some(1024),
            last_seen_mtime: Some("2026-05-13T10:00:00Z".to_string()),
        })
        .unwrap();
    }

    #[test]
    fn test_detect_lineage_groups_uuid_run_folder() {
        let db = Database::open(Path::new(":memory:")).unwrap();
        let run_id = "019e2166-0bb7-79e0-b00a-37ebdb28d4a0";
        insert_test_image(
            &db,
            "img-a",
            &format!("/tmp/generated_images/{}/ig_b.png", run_id),
        );
        insert_test_image(
            &db,
            "img-b",
            &format!("/tmp/generated_images/{}/ig_a.png", run_id),
        );
        insert_test_image(
            &db,
            "img-c",
            "/tmp/generated_images/019dd433-3196-7ef1-aac5-fe3eb33e15b9/ig_c.png",
        );

        let image_ids = vec![
            "img-a".to_string(),
            "img-b".to_string(),
            "img-c".to_string(),
        ];
        let touched = db.detect_lineage_for_batch(&image_ids).unwrap();
        assert_eq!(touched.len(), 1);

        let groups = db.list_lineage_groups().unwrap();
        assert_eq!(groups.len(), 1);
        assert_eq!(groups[0].name, "Run 019e2166");
        assert_eq!(groups[0].detection_method.as_deref(), Some("run_folder"));
        assert_eq!(groups[0].image_count, 2);

        let images = db.get_lineage_group_images(&groups[0].id).unwrap();
        assert_eq!(images.len(), 2);
        assert!(images[0].path.ends_with("ig_a.png"));
        assert!(images[1].path.ends_with("ig_b.png"));
    }

    #[test]
    fn test_list_lineage_groups_excludes_singletons() {
        let db = Database::open(Path::new(":memory:")).unwrap();
        insert_test_image(&db, "solo", "/tmp/solo.png");
        insert_test_image(&db, "pair-a", "/tmp/pair_a.png");
        insert_test_image(&db, "pair-b", "/tmp/pair_b.png");

        let solo_group = db
            .create_lineage_group("solo series", "manual", 100.0)
            .unwrap();
        db.assign_to_lineage_group("solo", &solo_group, 0).unwrap();

        let pair_group = db
            .create_lineage_group("pair series", "manual", 100.0)
            .unwrap();
        db.assign_to_lineage_group("pair-a", &pair_group, 0)
            .unwrap();
        db.assign_to_lineage_group("pair-b", &pair_group, 1)
            .unwrap();

        let groups = db.list_lineage_groups().unwrap();
        assert_eq!(groups.len(), 1);
        assert_eq!(groups[0].id, pair_group);
        assert_eq!(groups[0].image_count, 2);
    }

    #[test]
    fn test_lineage_score() {
        let signals = LineageSignals {
            filename_stem_match: true,
            same_import_batch: true,
            temporal_proximity: true,
            same_dimensions: true,
            clip_similarity: None,
            prompt_match: false,
        };
        assert_eq!(signals.score(), 50); // 25 + 10 + 10 + 5
    }

    #[test]
    fn test_lineage_score_with_prompt() {
        let signals = LineageSignals {
            filename_stem_match: true,
            same_import_batch: false,
            temporal_proximity: false,
            same_dimensions: true,
            clip_similarity: Some(0.9),
            prompt_match: true,
        };
        assert_eq!(signals.score(), 95); // 50 + 25 + 15 + 5
    }
}
