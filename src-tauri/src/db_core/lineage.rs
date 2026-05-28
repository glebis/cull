use chrono::Utc;
use regex::Regex;
use rusqlite::{params, Result};
use std::collections::HashMap;
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
                    COUNT(i.id) as cnt
             FROM lineage_groups lg
             LEFT JOIN images i ON i.lineage_group_id = lg.id
             GROUP BY lg.id
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
            .map_err(Into::into)
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
            .map_err(Into::into)
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
            .map_err(Into::into)
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

        // Group by stem
        let mut stem_groups: HashMap<String, Vec<usize>> = HashMap::new();
        for (i, stem) in stems.iter().enumerate() {
            if !stem.is_empty() {
                stem_groups.entry(stem.clone()).or_default().push(i);
            }
        }

        let mut created_groups = vec![];

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
