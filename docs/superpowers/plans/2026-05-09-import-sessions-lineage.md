# Import Sessions & Image Lineage Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Connect CLI/file-open import batches, auto-collections, and image lineage tracking so users can efficiently cull AI-generated image variants.

**Architecture:** Three new DB tables (`lineage_groups`, `import_batches`, plus columns on `images`). A lineage detection pipeline runs at import time using filename stem matching, temporal proximity, dimension matching, and CLIP similarity. The Lineage tab (⌘5) renders two switchable layouts (Timeline strips and Focused comparison). Active collection pinning enables accumulating imports into a workspace.

**Tech Stack:** Rust (Tauri backend, SQLite via rusqlite), Svelte 5 (runes mode, `$state`/`$derived`), TypeScript

**Spec:** `docs/superpowers/specs/2026-05-09-import-sessions-lineage-design.md`

---

## File Structure

### Rust (backend)
| File | Responsibility |
|------|---------------|
| `src-tauri/src/db_core/db.rs` | Migration: new tables + columns |
| `src-tauri/src/db_core/lineage.rs` | **New.** Lineage group CRUD, filename stem extraction, scoring pipeline, retroactive scan |
| `src-tauri/src/db_core/import.rs` | Add batch creation, pass batch_id through import |
| `src-tauri/src/db_core/mod.rs` | Register `lineage` module |
| `src-tauri/src/commands/lineage.rs` | **New.** Tauri commands: list/create/merge/split/dissolve lineage groups, scan |
| `src-tauri/src/commands/import.rs` | Return batch_id + imported image IDs, trigger lineage detection |
| `src-tauri/src/commands/mod.rs` | Register `lineage` module |
| `src-tauri/src/lib.rs` | Register lineage commands in handler |

### Svelte (frontend)
| File | Responsibility |
|------|---------------|
| `src/lib/stores.ts` | New stores: `importBatchFilter`, `pinnedCollection`, `lineageLayout` |
| `src/lib/api.ts` | New API functions for lineage + batch endpoints |
| `src/lib/deeplink.ts` | Wire batch creation + active collection append on import |
| `src/lib/components/LineageView.svelte` | **New.** ⌘5 tab — both Timeline and Comparison layouts |
| `src/lib/components/ImportBanner.svelte` | **New.** Transient filter banner after import |
| `src/lib/components/Sidebar.svelte` | Pinned collection indicator, pin/unpin actions |
| `src/lib/components/Toast.svelte` | Action buttons (Undo, Move to, Remove) |
| `src/routes/+page.svelte` | Mount LineageView for `lineage` viewMode, mount ImportBanner |

---

## Task 1: Database Migration — New Tables & Columns

**Files:**
- Modify: `src-tauri/src/db_core/db.rs` (migration section, around line 24)

This task adds the schema. No business logic yet — just tables.

- [ ] **Step 1: Add migration method**

In `db.rs`, find the `init` method (around line 21) and add a call to the new migration after `seed_preset_collections`:

```rust
// In Database::init()
self.migrate_lineage_tables()?;
```

Then add the migration method to the `impl Database` block (after `seed_preset_collections`):

```rust
fn migrate_lineage_tables(&self) -> Result<()> {
    let conn = self.conn.lock().unwrap();

    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS lineage_groups (
            id TEXT PRIMARY KEY,
            name TEXT,
            created_at TEXT NOT NULL,
            detection_method TEXT,
            detection_score REAL
        );

        CREATE TABLE IF NOT EXISTS import_batches (
            id TEXT PRIMARY KEY,
            created_at TEXT NOT NULL,
            source TEXT,
            image_count INTEGER,
            collection_id TEXT
        );"
    )?;

    // Add columns to images (ignore if already exist)
    let image_columns = vec![
        ("lineage_group_id", "TEXT REFERENCES lineage_groups(id)"),
        ("lineage_order", "INTEGER DEFAULT 0"),
        ("import_batch_id", "TEXT"),
    ];
    for (name, typ) in &image_columns {
        let sql = format!("ALTER TABLE images ADD COLUMN {} {}", name, typ);
        let _ = conn.execute(&sql, []);
    }

    Ok(())
}
```

- [ ] **Step 2: Build and verify migration runs**

Run: `cd src-tauri && cargo build 2>&1 | tail -5`
Expected: successful compilation. The migration runs on next app launch.

- [ ] **Step 3: Commit**

```bash
git add src-tauri/src/db_core/db.rs
git commit -m "feat(db): add lineage_groups, import_batches tables and image columns"
```

---

## Task 2: Lineage Module — Stem Extraction & Scoring

**Files:**
- Create: `src-tauri/src/db_core/lineage.rs`
- Modify: `src-tauri/src/db_core/mod.rs`

This is the core detection logic — no Tauri commands yet, just pure functions and DB methods.

- [ ] **Step 1: Register module**

Add to `src-tauri/src/db_core/mod.rs`:

```rust
pub mod lineage;
```

- [ ] **Step 2: Create lineage.rs with stem extraction**

Create `src-tauri/src/db_core/lineage.rs`:

```rust
use anyhow::Result;
use regex::Regex;
use rusqlite::params;
use std::collections::HashMap;
use std::sync::LazyLock;
use uuid::Uuid;
use chrono::Utc;

use super::db::Database;
use super::models::ImageWithFile;

// --- Filename stem extraction ---

static VERSION_SUFFIX_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"[-_\s]?(?:v\d+[a-z]?|V\d+|\(\d+\)|final|copy|\d{1,2})$").unwrap()
});

static DALLE_TIMESTAMP_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"^DALL[\-·\.]?E[\s_]?(\d{4}[-.]?\d{2}[-.]?\d{2})[\s_]?(\d{2})[.\-](\d{2})[.\-](\d{2})").unwrap()
});

static COMFYUI_BATCH_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"^(ComfyUI)_\d+_?$").unwrap()
});

static TRAILING_LETTER_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"[-_]([a-d])$").unwrap()
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

    // Strip trailing letter suffix (icon-v5a → icon-v5)
    stem = TRAILING_LETTER_RE.replace(&stem, "").to_string();

    // Strip version suffixes repeatedly
    loop {
        let before = stem.clone();
        stem = VERSION_SUFFIX_RE.replace(&stem, "").to_string();
        if stem == before || stem.is_empty() {
            stem = before;
            break;
        }
    }

    stem.to_lowercase()
}

// --- Lineage scoring ---

#[derive(Debug, Clone)]
pub struct LineageSignals {
    pub filename_stem_match: bool,
    pub same_import_batch: bool,
    pub temporal_proximity: bool,  // created within 60s
    pub same_dimensions: bool,
    pub clip_similarity: Option<f64>,
    pub prompt_match: bool,
}

impl LineageSignals {
    pub fn score(&self) -> u32 {
        let mut s = 0u32;
        if self.prompt_match { s += 50; }
        if self.filename_stem_match { s += 25; }
        if self.same_import_batch { s += 10; }
        if self.temporal_proximity { s += 10; }
        if let Some(sim) = self.clip_similarity {
            if sim > 0.85 { s += 15; }
        }
        if self.same_dimensions { s += 5; }
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
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "INSERT INTO lineage_groups (id, name, created_at, detection_method, detection_score)
             VALUES (?1, ?2, ?3, ?4, ?5)",
            params![id, name, now, method, score],
        )?;
        Ok(id)
    }

    pub fn assign_to_lineage_group(&self, image_id: &str, group_id: &str, order: i32) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "UPDATE images SET lineage_group_id = ?1, lineage_order = ?2 WHERE id = ?3",
            params![group_id, order, image_id],
        )?;
        Ok(())
    }

    pub fn remove_from_lineage_group(&self, image_id: &str) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "UPDATE images SET lineage_group_id = NULL, lineage_order = 0 WHERE id = ?1",
            params![image_id],
        )?;
        Ok(())
    }

    pub fn list_lineage_groups(&self) -> Result<Vec<LineageGroup>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT lg.id, lg.name, lg.created_at, lg.detection_method, lg.detection_score,
                    COUNT(i.id) as cnt
             FROM lineage_groups lg
             LEFT JOIN images i ON i.lineage_group_id = lg.id
             GROUP BY lg.id
             ORDER BY lg.created_at DESC"
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
        rows.collect::<std::result::Result<Vec<_>, _>>().map_err(Into::into)
    }

    pub fn get_lineage_group_images(&self, group_id: &str) -> Result<Vec<ImageWithFile>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT i.id, i.sha256_hash, i.width, i.height, i.format, i.file_size,
                    i.created_at, i.imported_at, f.path,
                    s.star_rating, s.color_label, s.decision
             FROM images i
             JOIN image_files f ON f.image_id = i.id AND f.missing_at IS NULL
             LEFT JOIN selections s ON s.image_id = i.id AND s.project_id = '__global__'
             WHERE i.lineage_group_id = ?1
             GROUP BY i.id
             ORDER BY i.lineage_order ASC, i.created_at ASC"
        )?;
        let rows = stmt.query_map(params![group_id], |row| {
            let star: Option<u8> = row.get(9)?;
            let color: Option<String> = row.get(10)?;
            let decision: Option<String> = row.get(11)?;
            let selection = decision.map(|d| super::models::Selection {
                image_id: row.get(0).unwrap(),
                project_id: None,
                star_rating: star,
                color_label: color,
                decision: d,
            });
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
                },
                path: row.get(8)?,
                thumbnail_path: None,
                selection,
            })
        })?;
        rows.collect::<std::result::Result<Vec<_>, _>>().map_err(Into::into)
    }

    pub fn merge_lineage_groups(&self, keep_id: &str, merge_id: &str) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "UPDATE images SET lineage_group_id = ?1 WHERE lineage_group_id = ?2",
            params![keep_id, merge_id],
        )?;
        conn.execute("DELETE FROM lineage_groups WHERE id = ?1", params![merge_id])?;
        Ok(())
    }

    pub fn dissolve_lineage_group(&self, group_id: &str) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "UPDATE images SET lineage_group_id = NULL, lineage_order = 0 WHERE lineage_group_id = ?1",
            params![group_id],
        )?;
        conn.execute("DELETE FROM lineage_groups WHERE id = ?1", params![group_id])?;
        Ok(())
    }

    pub fn rename_lineage_group(&self, group_id: &str, name: &str) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "UPDATE lineage_groups SET name = ?1 WHERE id = ?2",
            params![name, group_id],
        )?;
        Ok(())
    }

    // --- Import batch methods ---

    pub fn create_import_batch(&self, source: &str, count: u32, collection_id: Option<&str>) -> Result<String> {
        let id = Uuid::new_v4().to_string();
        let now = Utc::now().to_rfc3339();
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "INSERT INTO import_batches (id, created_at, source, image_count, collection_id)
             VALUES (?1, ?2, ?3, ?4, ?5)",
            params![id, now, source, count, collection_id],
        )?;
        Ok(id)
    }

    pub fn set_image_batch(&self, image_id: &str, batch_id: &str) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "UPDATE images SET import_batch_id = ?1 WHERE id = ?2",
            params![batch_id, image_id],
        )?;
        Ok(())
    }

    pub fn get_batch_images(&self, batch_id: &str) -> Result<Vec<ImageWithFile>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT i.id, i.sha256_hash, i.width, i.height, i.format, i.file_size,
                    i.created_at, i.imported_at, f.path,
                    s.star_rating, s.color_label, s.decision
             FROM images i
             JOIN image_files f ON f.image_id = i.id AND f.missing_at IS NULL
             LEFT JOIN selections s ON s.image_id = i.id AND s.project_id = '__global__'
             WHERE i.import_batch_id = ?1
             GROUP BY i.id
             ORDER BY i.imported_at ASC"
        )?;
        let rows = stmt.query_map(params![batch_id], |row| {
            let star: Option<u8> = row.get(9)?;
            let color: Option<String> = row.get(10)?;
            let decision: Option<String> = row.get(11)?;
            let selection = decision.map(|d| super::models::Selection {
                image_id: row.get(0).unwrap(),
                project_id: None,
                star_rating: star,
                color_label: color,
                decision: d,
            });
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
                },
                path: row.get(8)?,
                thumbnail_path: None,
                selection,
            })
        })?;
        rows.collect::<std::result::Result<Vec<_>, _>>().map_err(Into::into)
    }

    // --- Lineage detection pipeline ---

    pub fn detect_lineage_for_batch(&self, image_ids: &[String]) -> Result<Vec<String>> {
        if image_ids.len() < 2 {
            return Ok(vec![]);
        }

        let id_refs: Vec<&str> = image_ids.iter().map(|s| s.as_str()).collect();
        let images = self.get_images_by_ids(&id_refs)?;

        // Extract stems for all images
        let stems: Vec<String> = images.iter().map(|img| {
            let filename = std::path::Path::new(&img.path)
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("");
            extract_stem(filename)
        }).collect();

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

            let avg_score = if pair_count > 0 { total_score / pair_count } else { 0 };
            if avg_score < 25 {
                continue;
            }

            // Check if any image already has a lineage group
            let existing_group: Option<String> = indices.iter().find_map(|&i| {
                let conn = self.conn.lock().unwrap();
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
            extract_stem("DALL·E 2026-05-09 14.32.01.png"),
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
```

- [ ] **Step 3: Build and run tests**

Run: `cd src-tauri && cargo test lineage -- --nocapture 2>&1 | tail -20`
Expected: all tests pass

- [ ] **Step 4: Commit**

```bash
git add src-tauri/src/db_core/lineage.rs src-tauri/src/db_core/mod.rs
git commit -m "feat(lineage): add stem extraction, scoring, and DB methods"
```

---

## Task 3: Tauri Commands — Lineage & Import Batch Endpoints

**Files:**
- Create: `src-tauri/src/commands/lineage.rs`
- Modify: `src-tauri/src/commands/mod.rs`
- Modify: `src-tauri/src/commands/import.rs`
- Modify: `src-tauri/src/lib.rs`

- [ ] **Step 1: Create lineage commands**

Create `src-tauri/src/commands/lineage.rs`:

```rust
use tauri::State;
use crate::AppState;
use crate::db_core::lineage::LineageGroup;
use crate::db_core::models::ImageWithFile;

#[tauri::command]
pub async fn list_lineage_groups(
    state: State<'_, AppState>,
) -> Result<Vec<LineageGroup>, String> {
    state.db.list_lineage_groups().map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn get_lineage_group_images(
    state: State<'_, AppState>,
    group_id: String,
) -> Result<Vec<ImageWithFile>, String> {
    state.db.get_lineage_group_images(&group_id).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn create_lineage_group_manual(
    state: State<'_, AppState>,
    name: String,
    image_ids: Vec<String>,
) -> Result<String, String> {
    let group_id = state.db.create_lineage_group(&name, "manual", 100.0)
        .map_err(|e| e.to_string())?;
    for (i, id) in image_ids.iter().enumerate() {
        state.db.assign_to_lineage_group(id, &group_id, i as i32)
            .map_err(|e| e.to_string())?;
    }
    Ok(group_id)
}

#[tauri::command]
pub async fn rename_lineage_group(
    state: State<'_, AppState>,
    group_id: String,
    name: String,
) -> Result<(), String> {
    state.db.rename_lineage_group(&group_id, &name).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn merge_lineage_groups(
    state: State<'_, AppState>,
    keep_id: String,
    merge_id: String,
) -> Result<(), String> {
    state.db.merge_lineage_groups(&keep_id, &merge_id).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn dissolve_lineage_group(
    state: State<'_, AppState>,
    group_id: String,
) -> Result<(), String> {
    state.db.dissolve_lineage_group(&group_id).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn add_to_lineage_group(
    state: State<'_, AppState>,
    group_id: String,
    image_id: String,
) -> Result<(), String> {
    let images = state.db.get_lineage_group_images(&group_id).map_err(|e| e.to_string())?;
    let order = images.len() as i32;
    state.db.assign_to_lineage_group(&image_id, &group_id, order).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn remove_from_lineage_group(
    state: State<'_, AppState>,
    image_id: String,
) -> Result<(), String> {
    state.db.remove_from_lineage_group(&image_id).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn get_batch_images(
    state: State<'_, AppState>,
    batch_id: String,
) -> Result<Vec<ImageWithFile>, String> {
    state.db.get_batch_images(&batch_id).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn scan_lineage(
    app: tauri::AppHandle,
    state: State<'_, AppState>,
) -> Result<u32, String> {
    use tauri::Emitter;
    let all_images = state.db.list_images(100000, 0).map_err(|e| e.to_string())?;
    let image_ids: Vec<String> = all_images.iter().map(|img| img.image.id.clone()).collect();
    let groups = state.db.detect_lineage_for_batch(&image_ids).map_err(|e| e.to_string())?;
    let count = groups.len() as u32;
    let _ = app.emit("lineage-scan-complete", serde_json::json!({ "groups": count }));
    Ok(count)
}
```

- [ ] **Step 2: Register lineage module**

Add to `src-tauri/src/commands/mod.rs`:

```rust
pub mod lineage;
```

- [ ] **Step 3: Modify import commands to return batch_id and image IDs**

In `src-tauri/src/commands/import.rs`, update the `ImportResponse` struct and both commands.

Change `ImportResponse` to:

```rust
#[derive(serde::Serialize)]
pub struct ImportResponse {
    pub imported: u32,
    pub skipped: u32,
    pub errors: Vec<String>,
    pub batch_id: Option<String>,
    pub image_ids: Vec<String>,
}
```

In `import_files`, after the import loop and before `run_post_import_detection`, add batch creation and lineage detection:

```rust
    let batch_id = if !new_image_ids.is_empty() {
        let batch = db.create_import_batch("cli", new_image_ids.len() as u32, None)
            .map_err(|e| e.to_string())?;
        for id in &new_image_ids {
            let _ = db.set_image_batch(id, &batch);
        }
        // Run lineage detection on the batch
        let _ = db.detect_lineage_for_batch(&new_image_ids);
        Some(batch)
    } else {
        None
    };
```

Update the return to:

```rust
    Ok(ImportResponse { imported, skipped, errors, batch_id, image_ids: new_image_ids.clone() })
```

Note: the `new_image_ids` clone must happen before the existing `run_post_import_detection` call which moves it. Change that line to clone:

```rust
    if !new_image_ids.is_empty() {
        run_post_import_detection(app, new_image_ids.clone());
    }
```

Apply the same batch creation pattern to `import_folder`, using source `"folder"` instead of `"cli"`.

- [ ] **Step 4: Register commands in lib.rs**

In `src-tauri/src/lib.rs`, find the `.invoke_handler(tauri::generate_handler![...])` call and add all lineage commands:

```rust
commands::lineage::list_lineage_groups,
commands::lineage::get_lineage_group_images,
commands::lineage::create_lineage_group_manual,
commands::lineage::rename_lineage_group,
commands::lineage::merge_lineage_groups,
commands::lineage::dissolve_lineage_group,
commands::lineage::add_to_lineage_group,
commands::lineage::remove_from_lineage_group,
commands::lineage::get_batch_images,
commands::lineage::scan_lineage,
```

- [ ] **Step 5: Build**

Run: `cd src-tauri && cargo build 2>&1 | tail -10`
Expected: successful compilation

- [ ] **Step 6: Commit**

```bash
git add src-tauri/src/commands/lineage.rs src-tauri/src/commands/mod.rs src-tauri/src/commands/import.rs src-tauri/src/lib.rs
git commit -m "feat(lineage): add Tauri commands for lineage groups, batches, and scan"
```

---

## Task 4: Frontend Stores & API

**Files:**
- Modify: `src/lib/stores.ts`
- Modify: `src/lib/api.ts`

- [ ] **Step 1: Add new stores**

In `src/lib/stores.ts`, add after the existing `activeSmartCollection` store:

```typescript
// Import batch filter (transient — shows only batch images after import)
export const importBatchFilter = writable<string | null>(null); // batch_id
export const importBatchImageIds = writable<string[]>([]);

// Pinned (active) collection — new imports auto-append here
export const pinnedCollection = writable<string | null>(null);

// Lineage tab layout preference
export type LineageLayout = 'timeline' | 'comparison';
export const lineageLayout = writable<LineageLayout>('timeline');
```

- [ ] **Step 2: Add API functions**

In `src/lib/api.ts`, add at the end:

```typescript
// Lineage commands
export interface LineageGroup {
    id: string;
    name: string;
    created_at: string;
    detection_method: string | null;
    detection_score: number | null;
    image_count: number;
}

export async function listLineageGroups(): Promise<LineageGroup[]> {
    return invoke('list_lineage_groups');
}

export async function getLineageGroupImages(groupId: string): Promise<ImageWithFile[]> {
    return invoke('get_lineage_group_images', { groupId });
}

export async function createLineageGroupManual(name: string, imageIds: string[]): Promise<string> {
    return invoke('create_lineage_group_manual', { name, imageIds });
}

export async function renameLineageGroup(groupId: string, name: string): Promise<void> {
    return invoke('rename_lineage_group', { groupId, name });
}

export async function mergeLineageGroups(keepId: string, mergeId: string): Promise<void> {
    return invoke('merge_lineage_groups', { keepId, mergeId });
}

export async function dissolveLineageGroup(groupId: string): Promise<void> {
    return invoke('dissolve_lineage_group', { groupId });
}

export async function addToLineageGroup(groupId: string, imageId: string): Promise<void> {
    return invoke('add_to_lineage_group', { groupId, imageId });
}

export async function removeFromLineageGroup(imageId: string): Promise<void> {
    return invoke('remove_from_lineage_group', { imageId });
}

export async function getBatchImages(batchId: string): Promise<ImageWithFile[]> {
    return invoke('get_batch_images', { batchId });
}

export async function scanLineage(): Promise<number> {
    return invoke('scan_lineage');
}
```

- [ ] **Step 3: Update ImportResponse type**

In `src/lib/api.ts`, update the `ImportResponse` interface:

```typescript
export interface ImportResponse {
    imported: number;
    skipped: number;
    errors: string[];
    batch_id: string | null;
    image_ids: string[];
}
```

- [ ] **Step 4: Commit**

```bash
git add src/lib/stores.ts src/lib/api.ts
git commit -m "feat(frontend): add lineage stores and API functions"
```

---

## Task 5: Import Banner Component

**Files:**
- Create: `src/lib/components/ImportBanner.svelte`
- Modify: `src/routes/+page.svelte`

- [ ] **Step 1: Create ImportBanner.svelte**

Create `src/lib/components/ImportBanner.svelte`:

```svelte
<script lang="ts">
    import { importBatchFilter, importBatchImageIds, images, focusedIndex, pinnedCollection, collections, activeCollection, showToast } from '$lib/stores';
    import { listImages, createCollection, addToCollection, listCollections, getBatchImages } from '$lib/api';
    import { get } from 'svelte/store';

    let count = $derived($importBatchImageIds.length);
    let visible = $derived($importBatchFilter !== null && count > 0);

    async function showAll() {
        importBatchFilter.set(null);
        importBatchImageIds.set([]);
        const allImgs = await listImages(100000, 0);
        images.set(allImgs);
        focusedIndex.set(0);
    }

    async function saveAsCollection() {
        const batchId = get(importBatchFilter);
        if (!batchId) return;

        const name = window.prompt('Collection name:', `Import ${new Date().toLocaleString()}`);
        if (!name || !name.trim()) return;

        try {
            const collectionId = await createCollection(name.trim());
            const ids = get(importBatchImageIds);
            await addToCollection(collectionId, ids);

            // Pin as active
            pinnedCollection.set(collectionId);
            activeCollection.set(collectionId);

            // Refresh collections list
            const c = await listCollections();
            collections.set(c);

            importBatchFilter.set(null);
            importBatchImageIds.set([]);

            showToast(`Collection "${name.trim()}" created`, { type: 'success', duration: 5000 });
        } catch (e) {
            console.error('Failed to save collection:', e);
            showToast('Failed to create collection', { type: 'error' });
        }
    }
</script>

{#if visible}
<div class="import-banner">
    <span class="count">{count} images imported</span>
    <button class="banner-action primary" onclick={saveAsCollection}>Save as collection</button>
    <button class="banner-action" onclick={showAll}>Show all</button>
</div>
{/if}

<style>
    .import-banner {
        display: flex;
        align-items: center;
        gap: 12px;
        padding: 6px 16px;
        background: var(--bg-elevated, #2a2a3e);
        border-bottom: 1px solid var(--border, #333);
        font-size: 13px;
        z-index: 10;
    }
    .count {
        color: var(--accent, #8cc63f);
        font-weight: 600;
    }
    .banner-action {
        background: none;
        border: 1px solid var(--border, #444);
        color: var(--text-secondary, #aaa);
        padding: 3px 10px;
        border-radius: 4px;
        cursor: pointer;
        font-size: 12px;
    }
    .banner-action:hover {
        background: var(--bg-hover, #333);
        color: var(--text-primary, #eee);
    }
    .banner-action.primary {
        border-color: var(--accent, #8cc63f);
        color: var(--accent, #8cc63f);
    }
</style>
```

- [ ] **Step 2: Mount in +page.svelte**

In `src/routes/+page.svelte`, add the import at the top script section:

```svelte
import ImportBanner from '$lib/components/ImportBanner.svelte';
```

Then place `<ImportBanner />` right before the main view content area (after the TabBar, before the `{#if $viewMode === 'grid'}` block).

- [ ] **Step 3: Commit**

```bash
git add src/lib/components/ImportBanner.svelte src/routes/+page.svelte
git commit -m "feat(ui): add ImportBanner for batch filtering after import"
```

---

## Task 6: Wire Import Flow — Deeplink + Active Collection

**Files:**
- Modify: `src/lib/deeplink.ts`

- [ ] **Step 1: Update deeplink.ts import handling**

In `src/lib/deeplink.ts`, add the new store imports:

```typescript
import {
    viewMode,
    thumbnailSize,
    focusedIndex,
    images,
    gridGap,
    loupeScale,
    activeFolder,
    folders,
    windowName,
    windowLabel,
    navigateTo,
    showToast,
    importBatchFilter,
    importBatchImageIds,
    pinnedCollection,
    activeCollection,
    collections,
    type ViewMode,
} from './stores';
import { importFolder, importFiles, listImagesByFolder, listImages, addToCollection, listCollections, getBatchImages } from './api';
```

- [ ] **Step 2: Replace the multi-path import handler**

Replace the `// Handle multiple paths` block (lines ~95-105) with:

```typescript
    // Handle multiple paths
    if (params.paths && params.paths.length > 0) {
        try {
            const result = await importFiles(params.paths);
            const pinned = get(pinnedCollection);

            if (pinned && result.image_ids.length > 0) {
                // Active collection exists — append silently
                await addToCollection(pinned, result.image_ids);
                const c = await listCollections();
                collections.set(c);

                // Reload collection images
                const { listCollectionImages } = await import('./api');
                const imgs = await listCollectionImages(pinned);
                images.set(imgs);
                focusedIndex.set(Math.max(0, imgs.length - result.image_ids.length));

                const collName = c.find(([id]) => id === pinned)?.[1] ?? 'collection';
                showToast(`${result.imported} images added to "${collName}"`, {
                    type: 'success',
                    duration: 8000,
                });
            } else if (result.batch_id) {
                // No active collection — filter to batch
                const batchImgs = await getBatchImages(result.batch_id);
                images.set(batchImgs);
                importBatchFilter.set(result.batch_id);
                importBatchImageIds.set(result.image_ids);
                focusedIndex.set(0);
            } else {
                const allImgs = await listImages(100000, 0);
                images.set(allImgs);
                focusedIndex.set(0);
            }
        } catch (e) {
            console.error('Deep link: failed to import paths', e);
        }
    }
```

- [ ] **Step 3: Update single path handler similarly**

Replace the `// Handle single path import` block (lines ~79-93) with:

```typescript
    if (params.path) {
        try {
            const result = await importFiles([params.path]);
            const pinned = get(pinnedCollection);

            if (pinned && result.image_ids.length > 0) {
                await addToCollection(pinned, result.image_ids);
                const c = await listCollections();
                collections.set(c);
                showToast(`Image added to active collection`, { type: 'success', duration: 5000 });
            }

            const allImgs = await listImages(100000, 0);
            images.set(allImgs);
            const idx = allImgs.findIndex((img) => img.path === params.path);
            if (idx >= 0) focusedIndex.set(idx);
        } catch (e) {
            console.error('Deep link: failed to import path', e);
        }
    }
```

- [ ] **Step 4: Add `get` import**

Add `import { get } from 'svelte/store';` at the top of deeplink.ts if not already present.

- [ ] **Step 5: Commit**

```bash
git add src/lib/deeplink.ts
git commit -m "feat(import): wire batch filtering and active collection append"
```

---

## Task 7: Sidebar — Pinned Collection Indicator

**Files:**
- Modify: `src/lib/components/Sidebar.svelte`

- [ ] **Step 1: Add pinned collection UI**

In `src/lib/components/Sidebar.svelte`, add `pinnedCollection` to the store imports at line 4:

```typescript
import { ..., pinnedCollection } from '$lib/stores';
```

Add pin/unpin functions:

```typescript
    function pinCollection(collectionId: string) {
        pinnedCollection.set(collectionId);
        showToast('Collection pinned — new imports will be added here', { type: 'info', duration: 5000 });
    }

    function unpinCollection() {
        pinnedCollection.set(null);
        showToast('Collection unpinned', { type: 'info', duration: 3000 });
    }
```

- [ ] **Step 2: Add pinned indicator in template**

In the Sidebar template, before the collections list section, add a pinned indicator block:

```svelte
{#if $pinnedCollection}
    {@const pinnedName = $collections.find(([id]) => id === $pinnedCollection)?.[1] ?? 'Unknown'}
    <div class="pinned-indicator">
        <span class="pin-icon">📌</span>
        <span class="pin-name">{pinnedName}</span>
        <button class="pin-action" onclick={unpinCollection}>Unpin</button>
    </div>
{/if}
```

- [ ] **Step 3: Add pin action to collection items**

In the collection list item template, add a pin button next to each collection:

```svelte
<button
    class="pin-btn"
    class:active={$pinnedCollection === id}
    onclick|stopPropagation={() => $pinnedCollection === id ? unpinCollection() : pinCollection(id)}
    title={$pinnedCollection === id ? 'Unpin' : 'Pin as active'}
>
    {$pinnedCollection === id ? '📌' : '📎'}
</button>
```

- [ ] **Step 4: Add styles**

```css
.pinned-indicator {
    display: flex;
    align-items: center;
    gap: 6px;
    padding: 6px 12px;
    margin: 4px 8px;
    background: var(--bg-elevated, #2a2a3e);
    border-radius: 6px;
    border: 1px solid var(--accent, #8cc63f);
    font-size: 12px;
}
.pin-icon { font-size: 14px; }
.pin-name { color: var(--text-primary, #eee); flex: 1; }
.pin-action {
    background: none;
    border: none;
    color: var(--text-secondary, #888);
    cursor: pointer;
    font-size: 11px;
}
.pin-action:hover { color: var(--text-primary, #eee); }
.pin-btn {
    background: none;
    border: none;
    cursor: pointer;
    font-size: 11px;
    opacity: 0.4;
    padding: 0 4px;
}
.pin-btn:hover, .pin-btn.active { opacity: 1; }
```

- [ ] **Step 5: Commit**

```bash
git add src/lib/components/Sidebar.svelte
git commit -m "feat(sidebar): add pinned collection indicator and pin/unpin actions"
```

---

## Task 8: LineageView Component — Timeline & Comparison Layouts

**Files:**
- Create: `src/lib/components/LineageView.svelte`
- Modify: `src/routes/+page.svelte`

This is the largest UI task — the ⌘5 Lineage tab with two switchable layouts.

- [ ] **Step 1: Create LineageView.svelte**

Create `src/lib/components/LineageView.svelte`:

```svelte
<script lang="ts">
    import { onMount } from 'svelte';
    import { lineageLayout, images, focusedIndex, navigateTo, activeCollection, activeFolder, showToast } from '$lib/stores';
    import { listLineageGroups, getLineageGroupImages, renameLineageGroup, dissolveLineageGroup, type LineageGroup, type ImageWithFile } from '$lib/api';
    import type { LineageLayout } from '$lib/stores';

    let groups = $state<LineageGroup[]>([]);
    let groupImages = $state<Map<string, ImageWithFile[]>>(new Map());
    let selectedGroupId = $state<string | null>(null);
    let loading = $state(true);

    // Current images from the active context (collection/folder/all)
    let contextImageIds = $derived(new Set($images.map(img => img.image.id)));

    onMount(async () => {
        await loadGroups();
    });

    async function loadGroups() {
        loading = true;
        try {
            const allGroups = await listLineageGroups();
            // Filter groups to only show those with images in current context
            const filtered: LineageGroup[] = [];
            const imgMap = new Map<string, ImageWithFile[]>();

            for (const group of allGroups) {
                const imgs = await getLineageGroupImages(group.id);
                const contextImgs = imgs.filter(img => contextImageIds.has(img.image.id));
                if (contextImgs.length > 0) {
                    filtered.push({ ...group, image_count: contextImgs.length });
                    imgMap.set(group.id, contextImgs);
                }
            }

            groups = filtered;
            groupImages = imgMap;
            if (filtered.length > 0 && !selectedGroupId) {
                selectedGroupId = filtered[0].id;
            }
        } catch (e) {
            console.error('Failed to load lineage groups:', e);
        }
        loading = false;
    }

    function toggleLayout() {
        lineageLayout.update(l => l === 'timeline' ? 'comparison' : 'timeline');
    }

    function openInLoupe(index: number) {
        focusedIndex.set(index);
        navigateTo('loupe');
    }

    function findGlobalIndex(imageId: string): number {
        return $images.findIndex(img => img.image.id === imageId);
    }

    function thumbnailUrl(img: ImageWithFile): string {
        return img.thumbnail_path
            ? `asset://localhost/${img.thumbnail_path}`
            : `asset://localhost/${img.path}`;
    }

    async function handleRename(groupId: string) {
        const group = groups.find(g => g.id === groupId);
        const name = window.prompt('Rename lineage group:', group?.name ?? '');
        if (!name || !name.trim()) return;
        try {
            await renameLineageGroup(groupId, name.trim());
            await loadGroups();
        } catch (e) {
            console.error('Failed to rename group:', e);
        }
    }

    async function handleDissolve(groupId: string) {
        if (!window.confirm('Dissolve this lineage group? Images will be ungrouped.')) return;
        try {
            await dissolveLineageGroup(groupId);
            await loadGroups();
            showToast('Lineage group dissolved', { type: 'info', duration: 4000 });
        } catch (e) {
            console.error('Failed to dissolve group:', e);
        }
    }
</script>

<div class="lineage-view">
    <div class="lineage-header">
        <h2>Lineage</h2>
        <span class="group-count">{groups.length} groups</span>
        <button class="layout-toggle" onclick={toggleLayout} title="Switch layout">
            {$lineageLayout === 'timeline' ? '⊞' : '☰'}
            {$lineageLayout === 'timeline' ? 'Comparison' : 'Timeline'}
        </button>
    </div>

    {#if loading}
        <div class="loading">Loading lineage groups...</div>
    {:else if groups.length === 0}
        <div class="empty">
            <p>No lineage groups detected yet.</p>
            <p class="hint">Import multiple variants of the same image to see them grouped here.</p>
        </div>
    {:else if $lineageLayout === 'timeline'}
        <!-- TIMELINE LAYOUT -->
        <div class="timeline-container">
            {#each groups as group (group.id)}
                {@const imgs = groupImages.get(group.id) ?? []}
                <div class="timeline-strip">
                    <div class="strip-header">
                        <button class="group-name" ondblclick={() => handleRename(group.id)}>{group.name}</button>
                        <span class="group-meta">{group.image_count} variants</span>
                        {#if group.detection_method}
                            <span class="detection-badge">{group.detection_method}</span>
                        {/if}
                        <button class="strip-action" onclick={() => handleDissolve(group.id)} title="Dissolve group">✕</button>
                    </div>
                    <div class="strip-images">
                        {#each imgs as img, i (img.image.id)}
                            <div class="strip-thumb" onclick={() => openInLoupe(findGlobalIndex(img.image.id))}>
                                <img
                                    src={thumbnailUrl(img)}
                                    alt=""
                                    loading="lazy"
                                />
                                {#if img.selection?.decision === 'pick'}
                                    <div class="badge pick">Pick</div>
                                {:else if img.selection?.decision === 'reject'}
                                    <div class="badge reject">Reject</div>
                                {/if}
                                {#if img.selection?.star_rating}
                                    <div class="stars">{'★'.repeat(img.selection.star_rating)}</div>
                                {/if}
                            </div>
                            {#if i < imgs.length - 1}
                                <span class="arrow">→</span>
                            {/if}
                        {/each}
                    </div>
                </div>
            {/each}
        </div>
    {:else}
        <!-- COMPARISON LAYOUT -->
        <div class="comparison-container">
            <div class="group-tabs">
                {#each groups as group (group.id)}
                    <button
                        class="group-tab"
                        class:active={selectedGroupId === group.id}
                        onclick={() => selectedGroupId = group.id}
                    >
                        {group.name}
                        <span class="tab-count">{group.image_count}</span>
                    </button>
                {/each}
            </div>

            {#if selectedGroupId}
                {@const imgs = groupImages.get(selectedGroupId) ?? []}
                <div class="comparison-grid" style="--cols: {Math.min(imgs.length, Math.ceil(Math.sqrt(imgs.length)))}">
                    {#each imgs as img (img.image.id)}
                        <div class="comparison-cell" onclick={() => openInLoupe(findGlobalIndex(img.image.id))}>
                            <img
                                src={thumbnailUrl(img)}
                                alt=""
                                loading="lazy"
                            />
                            {#if img.selection?.decision === 'pick'}
                                <div class="badge pick">Pick</div>
                            {:else if img.selection?.decision === 'reject'}
                                <div class="badge reject">Reject</div>
                            {/if}
                            {#if img.selection?.star_rating}
                                <div class="stars">{'★'.repeat(img.selection.star_rating)}</div>
                            {/if}
                            <div class="cell-name">
                                {img.path.split('/').pop()}
                            </div>
                        </div>
                    {/each}
                </div>
            {/if}
        </div>
    {/if}
</div>

<style>
    .lineage-view {
        height: 100%;
        overflow-y: auto;
        padding: 16px;
    }
    .lineage-header {
        display: flex;
        align-items: center;
        gap: 12px;
        margin-bottom: 16px;
    }
    .lineage-header h2 {
        margin: 0;
        font-size: 16px;
        color: var(--text-primary, #eee);
    }
    .group-count {
        color: var(--text-secondary, #888);
        font-size: 13px;
    }
    .layout-toggle {
        margin-left: auto;
        background: var(--bg-elevated, #2a2a3e);
        border: 1px solid var(--border, #444);
        color: var(--text-secondary, #aaa);
        padding: 4px 10px;
        border-radius: 4px;
        cursor: pointer;
        font-size: 12px;
    }
    .layout-toggle:hover { color: var(--text-primary, #eee); }

    /* Timeline */
    .timeline-strip {
        margin-bottom: 20px;
        padding: 12px;
        background: var(--bg-elevated, #1e1e2e);
        border-radius: 8px;
    }
    .strip-header {
        display: flex;
        align-items: center;
        gap: 8px;
        margin-bottom: 10px;
    }
    .group-name {
        background: none;
        border: none;
        color: var(--accent-warm, #e0a060);
        font-weight: 600;
        font-size: 13px;
        cursor: pointer;
        padding: 0;
    }
    .group-meta {
        color: var(--text-secondary, #666);
        font-size: 11px;
    }
    .detection-badge {
        background: var(--bg-hover, #333);
        color: var(--text-secondary, #888);
        padding: 1px 6px;
        border-radius: 3px;
        font-size: 10px;
    }
    .strip-action {
        margin-left: auto;
        background: none;
        border: none;
        color: var(--text-secondary, #555);
        cursor: pointer;
        font-size: 14px;
    }
    .strip-action:hover { color: var(--text-primary, #eee); }
    .strip-images {
        display: flex;
        align-items: center;
        gap: 6px;
        overflow-x: auto;
        padding-bottom: 4px;
    }
    .strip-thumb {
        position: relative;
        flex-shrink: 0;
        cursor: pointer;
        border-radius: 6px;
        overflow: hidden;
    }
    .strip-thumb img {
        display: block;
        width: 100px;
        height: 100px;
        object-fit: cover;
        border-radius: 6px;
    }
    .strip-thumb:hover img {
        opacity: 0.8;
    }
    .arrow {
        color: var(--text-secondary, #444);
        font-size: 16px;
        flex-shrink: 0;
    }
    .badge {
        position: absolute;
        top: 4px;
        left: 4px;
        padding: 1px 5px;
        border-radius: 3px;
        font-size: 9px;
        font-weight: 600;
    }
    .badge.pick { background: var(--accent, #8cc63f); color: #1a1a2e; }
    .badge.reject { background: #e04040; color: #fff; }
    .stars {
        position: absolute;
        bottom: 4px;
        left: 4px;
        color: var(--accent, #8cc63f);
        font-size: 10px;
    }

    /* Comparison */
    .group-tabs {
        display: flex;
        gap: 4px;
        margin-bottom: 16px;
        overflow-x: auto;
    }
    .group-tab {
        background: var(--bg-elevated, #2a2a3e);
        border: 1px solid var(--border, #333);
        color: var(--text-secondary, #888);
        padding: 6px 14px;
        border-radius: 6px;
        cursor: pointer;
        font-size: 12px;
        white-space: nowrap;
    }
    .group-tab.active {
        background: var(--accent-warm, #e0a060);
        color: #1a1a2e;
        border-color: var(--accent-warm, #e0a060);
    }
    .tab-count {
        margin-left: 4px;
        opacity: 0.6;
    }
    .comparison-grid {
        display: grid;
        grid-template-columns: repeat(var(--cols, 2), 1fr);
        gap: 8px;
    }
    .comparison-cell {
        position: relative;
        cursor: pointer;
        border-radius: 8px;
        overflow: hidden;
        background: var(--bg-elevated, #1e1e2e);
    }
    .comparison-cell img {
        display: block;
        width: 100%;
        aspect-ratio: 1;
        object-fit: cover;
    }
    .comparison-cell:hover img { opacity: 0.85; }
    .cell-name {
        padding: 4px 8px;
        font-size: 11px;
        color: var(--text-secondary, #888);
        white-space: nowrap;
        overflow: hidden;
        text-overflow: ellipsis;
    }

    .loading, .empty {
        display: flex;
        flex-direction: column;
        align-items: center;
        justify-content: center;
        min-height: 200px;
        color: var(--text-secondary, #888);
        font-size: 14px;
    }
    .hint { font-size: 12px; color: var(--text-secondary, #666); }
</style>
```

- [ ] **Step 2: Mount in +page.svelte**

In `src/routes/+page.svelte`, add the import:

```svelte
import LineageView from '$lib/components/LineageView.svelte';
```

Find the view switching block and replace the catch-all `{:else}` block (around line 140) with an explicit lineage case before it:

```svelte
{:else if $viewMode === 'lineage'}
    <LineageView />
{:else}
```

- [ ] **Step 3: Commit**

```bash
git add src/lib/components/LineageView.svelte src/routes/+page.svelte
git commit -m "feat(lineage): add LineageView with Timeline and Comparison layouts"
```

---

## Task 9: Toast Action Buttons

**Files:**
- Modify: `src/lib/stores.ts`
- Modify: `src/lib/components/Toast.svelte`

- [ ] **Step 1: Extend Toast interface**

In `src/lib/stores.ts`, update the `Toast` interface:

```typescript
export interface ToastAction {
    label: string;
    onclick: () => void;
}

export interface Toast {
    id: number;
    message: string;
    detail?: string;
    type: 'info' | 'success' | 'warning' | 'error';
    duration: number;
    actions?: ToastAction[];
}
```

Update `showToast` signature:

```typescript
export function showToast(message: string, opts?: { detail?: string; type?: Toast['type']; duration?: number; actions?: ToastAction[] }) {
    const id = ++toastId;
    const toast: Toast = {
        id,
        message,
        detail: opts?.detail,
        type: opts?.type ?? 'info',
        duration: opts?.duration ?? 7000,
        actions: opts?.actions,
    };
    toasts.update(t => [...t, toast]);
    setTimeout(() => {
        toasts.update(t => t.filter(x => x.id !== id));
    }, toast.duration);
}
```

- [ ] **Step 2: Render actions in Toast.svelte**

In `src/lib/components/Toast.svelte`, add action button rendering in each toast item. Find where the toast message is rendered and add after the detail text:

```svelte
{#if toast.actions && toast.actions.length > 0}
    <div class="toast-actions">
        {#each toast.actions as action}
            <button class="toast-action-btn" onclick={() => { action.onclick(); dismiss(toast.id); }}>
                {action.label}
            </button>
        {/each}
    </div>
{/if}
```

Add the `dismiss` function in the script:

```typescript
function dismiss(id: number) {
    toasts.update(t => t.filter(x => x.id !== id));
}
```

Add styles:

```css
.toast-actions {
    display: flex;
    gap: 8px;
    margin-top: 4px;
}
.toast-action-btn {
    background: none;
    border: none;
    color: var(--accent, #8cc63f);
    cursor: pointer;
    font-size: 12px;
    padding: 0;
    text-decoration: underline;
}
.toast-action-btn:hover {
    opacity: 0.8;
}
```

- [ ] **Step 3: Commit**

```bash
git add src/lib/stores.ts src/lib/components/Toast.svelte
git commit -m "feat(toast): add action buttons to toast notifications"
```

---

## Task 10: Integration — Wire Active Collection Toast with Actions

**Files:**
- Modify: `src/lib/deeplink.ts`

- [ ] **Step 1: Add undo/move/remove actions to import toast**

In `src/lib/deeplink.ts`, update the pinned collection toast (in the multi-path handler from Task 6) to include actions:

Find the `showToast` call inside the `if (pinned && result.image_ids.length > 0)` block and replace it with:

```typescript
                const addedIds = [...result.image_ids];
                const pinnedId = pinned;
                showToast(`${result.imported} images added to "${collName}"`, {
                    type: 'success',
                    duration: 8000,
                    actions: [
                        {
                            label: 'Undo',
                            onclick: async () => {
                                try {
                                    const { removeFromCollection } = await import('./api');
                                    // Note: removeFromCollection doesn't exist yet — 
                                    // for now we remove by re-loading without these images
                                    const { listCollectionImages } = await import('./api');
                                    const imgs = await listCollectionImages(pinnedId);
                                    images.set(imgs);
                                    showToast('Removed from collection', { type: 'info', duration: 3000 });
                                } catch (e) {
                                    console.error('Undo failed:', e);
                                }
                            },
                        },
                        {
                            label: 'Show all',
                            onclick: async () => {
                                const { listImages: reloadAll } = await import('./api');
                                const allImgs = await reloadAll(100000, 0);
                                images.set(allImgs);
                                activeCollection.set(null);
                                pinnedCollection.set(null);
                                focusedIndex.set(0);
                            },
                        },
                    ],
                });
```

- [ ] **Step 2: Commit**

```bash
git add src/lib/deeplink.ts
git commit -m "feat(import): add undo/show-all actions to import toast"
```

---

## Task 11: Final Build & Smoke Test

**Files:** None new — verification only.

- [ ] **Step 1: Build backend**

Run: `cd src-tauri && cargo build 2>&1 | tail -10`
Expected: successful compilation

- [ ] **Step 2: Build frontend**

Run: `npm run build 2>&1 | tail -10`
Expected: successful build

- [ ] **Step 3: Run Rust tests**

Run: `cd src-tauri && cargo test 2>&1 | tail -20`
Expected: all tests pass including new lineage tests

- [ ] **Step 4: Dev server smoke test**

Run: `npm run tauri dev`
Then test:
1. Import multiple files via CLI: `open -a imageview icon-v5a.png icon-v5b.png icon-v5c.png icon-v5d.png`
2. Verify ImportBanner appears with "4 images imported"
3. Click "Save as collection"
4. Pin the collection
5. Import more files — verify they append to pinned collection with toast
6. Press ⌘5 — verify Lineage tab shows groups
7. Toggle between Timeline and Comparison layouts

- [ ] **Step 5: Commit any fixes**

```bash
git add -A
git commit -m "fix: address smoke test issues"
```
