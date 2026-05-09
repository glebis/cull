# Smart Collections Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add smart collections with NL command bar, evidence-based AI source detection, and a nested-capable filter engine to the ImageView desktop app.

**Architecture:** Extend the existing `projects` table with a `collection_type` discriminator and `filter_json` column for smart collection rules. Build a `FilterNode` tree that compiles to dynamic SQL using the existing `list_images_filtered` pattern. Add a deterministic NL parser (regex + synonyms) that converts text to `FilterNode` structs. Source detection runs at import time, storing evidence JSON per image.

**Tech Stack:** Rust (rusqlite, serde, serde_json, regex), SvelteKit 5, Svelte 5 stores, Tauri 2 commands.

**Spec:** `docs/smart-collections-design-brief.md` (v2)

---

### Task 1: DB Schema — Add Source Detection Columns and Smart Collection Support

**Files:**
- Modify: `src-tauri/src/db_core/schema.sql`
- Modify: `src-tauri/src/db_core/db.rs` (migration logic)

- [ ] **Step 1: Add source detection columns to images table**

In `src-tauri/src/db_core/schema.sql`, add after the `images` CREATE TABLE (these will be applied as migrations in db.rs):

```sql
-- Source detection evidence (added to images table via migration)
-- source_label TEXT,           -- 'midjourney', 'stable_diffusion', 'dalle', 'comfyui', 'nanobanana', 'photo', NULL
-- source_confidence REAL,      -- 0.0 to 1.0
-- source_evidence_json TEXT,   -- JSON array of detector results
-- is_ai_generated INTEGER,     -- 0/1/NULL (NULL = unknown)
-- ai_prompt TEXT,              -- extracted generation prompt if available
-- aspect_ratio REAL,           -- width/height
-- orientation TEXT,            -- 'landscape', 'portrait', 'square'
-- original_date TEXT,          -- from EXIF if available
```

- [ ] **Step 2: Add migration function in db.rs**

In `src-tauri/src/db_core/db.rs`, add a migration method to `Database`:

```rust
fn migrate_smart_collections(&self) -> Result<()> {
    let conn = self.conn.lock().unwrap();

    // Add source detection columns to images
    let columns = vec![
        ("source_label", "TEXT"),
        ("source_confidence", "REAL"),
        ("source_evidence_json", "TEXT"),
        ("source_detected_at", "TEXT"),
        ("source_detector_version", "TEXT"),
        ("is_ai_generated", "INTEGER"),
        ("ai_prompt", "TEXT"),
        ("aspect_ratio", "REAL"),
        ("orientation", "TEXT"),
        ("original_date", "TEXT"),
        ("megapixels", "REAL"),
    ];

    for (name, typ) in &columns {
        let sql = format!("ALTER TABLE images ADD COLUMN {} {}", name, typ);
        match conn.execute(&sql, []) {
            Ok(_) => {},
            Err(e) if e.to_string().contains("duplicate column") => {},
            Err(e) => return Err(e.into()),
        }
    }

    // Add smart collection columns to projects
    let project_columns = vec![
        ("collection_type", "TEXT DEFAULT 'manual'"), // 'manual' or 'smart'
        ("filter_json", "TEXT"),                       // serialized FilterNode tree
        ("nl_query", "TEXT"),                          // original NL if used
        ("is_preset", "INTEGER DEFAULT 0"),
        ("sort_order", "INTEGER DEFAULT 0"),
    ];

    for (name, typ) in &project_columns {
        let sql = format!("ALTER TABLE projects ADD COLUMN {} {}", name, typ);
        match conn.execute(&sql, []) {
            Ok(_) => {},
            Err(e) if e.to_string().contains("duplicate column") => {},
            Err(e) => return Err(e.into()),
        }
    }

    Ok(())
}
```

- [ ] **Step 3: Call migration in run_migrations()**

In `db.rs`, call `self.migrate_smart_collections()?;` at the end of the existing `run_migrations()` method (after `conn.execute_batch(schema)?;`). Note: `Database` uses `open()` not `new()` — the migration chain is `Database::open()` → `run_migrations()`. Since `run_migrations` holds the lock, `migrate_smart_collections` must accept the already-locked connection or release/reacquire:

```rust
fn run_migrations(&self) -> Result<()> {
    let conn = self.conn.lock().unwrap();
    let schema = include_str!("schema.sql");
    conn.execute_batch(schema)?;
    drop(conn); // release before calling method that also locks
    self.migrate_smart_collections()?;
    self.seed_preset_collections()?;
    Ok(())
}
```

- [ ] **Step 4: Verify migration runs**

Run: `cd src-tauri && cargo build`
Expected: compiles without errors.

- [ ] **Step 5: Commit**

```bash
git add src-tauri/src/db_core/schema.sql src-tauri/src/db_core/db.rs
git commit -m "feat: add DB schema for source detection and smart collections"
```

---

### Task 2: Rust Models — FilterNode, SmartCollection, SourceEvidence

**Files:**
- Create: `src-tauri/src/db_core/smart_collections.rs`
- Modify: `src-tauri/src/db_core/mod.rs`
- Create: `src-tauri/src/db_core/source_detection.rs`
- Test: `src-tauri/src/db_core/smart_collections.rs` (inline #[cfg(test)])

- [ ] **Step 1: Write FilterNode types with serialization tests**

Create `src-tauri/src/db_core/smart_collections.rs`:

```rust
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "type")]
pub enum FilterNode {
    #[serde(rename = "group")]
    Group { op: GroupOp, children: Vec<FilterNode> },
    #[serde(rename = "not")]
    Not { child: Box<FilterNode> },
    #[serde(rename = "rule")]
    Rule { field: Field, op: RuleOp, value: FilterValue },
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum GroupOp {
    And,
    Or,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum Field {
    Rating,
    ColorLabel,
    Decision,
    Format,
    Width,
    Height,
    AspectRatio,
    Orientation,
    SourceLabel,
    SourceConfidence,
    IsAiGenerated,
    AiPrompt,
    Folder,
    ImportedAt,
    OriginalDate,
    FileSize,
    ClipSimilarTo,
    ClipTextMatch,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum RuleOp {
    Eq,
    Neq,
    Gt,
    Gte,
    Lt,
    Lte,
    Contains,
    NotContains,
    In,
    NotIn,
    Between,
    IsEmpty,
    IsNotEmpty,
    LastNDays,
    ThisWeek,
    ThisMonth,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(untagged)]
pub enum FilterValue {
    String(String),
    Number(f64),
    Bool(bool),
    StringArray(Vec<String>),
    Range { from: String, to: String },
    ClipImage { image_id: i64, threshold: Option<f64> },
    ClipText { text: String, threshold: f64 },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SmartCollection {
    pub id: String,
    pub name: String,
    pub description: Option<String>,
    pub collection_type: String,
    pub filter_json: Option<String>,
    pub nl_query: Option<String>,
    pub is_preset: bool,
    pub sort_order: i32,
    pub created_at: String,
    pub image_count: Option<i64>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_filter_node_serialization_roundtrip() {
        let filter = FilterNode::Group {
            op: GroupOp::And,
            children: vec![
                FilterNode::Rule {
                    field: Field::Rating,
                    op: RuleOp::Gte,
                    value: FilterValue::Number(4.0),
                },
                FilterNode::Rule {
                    field: Field::SourceLabel,
                    op: RuleOp::Eq,
                    value: FilterValue::String("midjourney".to_string()),
                },
            ],
        };

        let json = serde_json::to_string(&filter).unwrap();
        let deserialized: FilterNode = serde_json::from_str(&json).unwrap();
        assert_eq!(filter, deserialized);
    }

    #[test]
    fn test_nested_not_group() {
        let filter = FilterNode::Group {
            op: GroupOp::And,
            children: vec![
                FilterNode::Rule {
                    field: Field::Rating,
                    op: RuleOp::Gte,
                    value: FilterValue::Number(4.0),
                },
                FilterNode::Not {
                    child: Box::new(FilterNode::Rule {
                        field: Field::SourceLabel,
                        op: RuleOp::Eq,
                        value: FilterValue::String("midjourney".to_string()),
                    }),
                },
            ],
        };

        let json = serde_json::to_string(&filter).unwrap();
        let deserialized: FilterNode = serde_json::from_str(&json).unwrap();
        assert_eq!(filter, deserialized);
    }

    #[test]
    fn test_date_filter_last_n_days() {
        let filter = FilterNode::Rule {
            field: Field::ImportedAt,
            op: RuleOp::LastNDays,
            value: FilterValue::Number(7.0),
        };

        let json = serde_json::to_string(&filter).unwrap();
        assert!(json.contains("last_n_days"));
        assert!(json.contains("imported_at"));
    }
}
```

- [ ] **Step 2: Run tests to verify serialization**

Run: `cd src-tauri && cargo test smart_collections --lib -- --nocapture`
Expected: 3 tests pass.

- [ ] **Step 3: Create source detection types**

Create `src-tauri/src/db_core/source_detection.rs`:

```rust
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SourceEvidence {
    pub detector: String,
    pub source_label: Option<String>,
    pub confidence: f64,
    pub details: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SourceDetectionResult {
    pub source_label: Option<String>,
    pub confidence: f64,
    pub is_ai_generated: Option<bool>,
    pub ai_prompt: Option<String>,
    pub evidence: Vec<SourceEvidence>,
}

impl SourceDetectionResult {
    pub fn unknown() -> Self {
        Self {
            source_label: None,
            confidence: 0.0,
            is_ai_generated: None,
            ai_prompt: None,
            evidence: vec![],
        }
    }

    pub fn to_evidence_json(&self) -> String {
        serde_json::to_string(&self.evidence).unwrap_or_else(|_| "[]".to_string())
    }
}
```

- [ ] **Step 4: Register modules**

In `src-tauri/src/db_core/mod.rs`, add:

```rust
pub mod smart_collections;
pub mod source_detection;
```

- [ ] **Step 5: Verify compilation**

Run: `cd src-tauri && cargo build`
Expected: compiles.

- [ ] **Step 6: Commit**

```bash
git add src-tauri/src/db_core/smart_collections.rs src-tauri/src/db_core/source_detection.rs src-tauri/src/db_core/mod.rs
git commit -m "feat: add FilterNode, SmartCollection, and SourceEvidence models"
```

---

### Task 3: Source Detection — Metadata and Filename Parsers

**Files:**
- Modify: `src-tauri/src/db_core/source_detection.rs`
- Modify: `src-tauri/Cargo.toml` (add `regex` if not present)

- [ ] **Step 1: Write tests for metadata detection**

Add to `src-tauri/src/db_core/source_detection.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_filename_dalle() {
        let result = detect_from_filename("DALL·E 2024-01-15 14.32.05.png");
        assert_eq!(result.source_label, Some("dalle".to_string()));
        assert!(result.confidence > 0.5);
    }

    #[test]
    fn test_filename_comfyui() {
        let result = detect_from_filename("ComfyUI_00042_.png");
        assert_eq!(result.source_label, Some("comfyui".to_string()));
    }

    #[test]
    fn test_filename_midjourney_discord() {
        let result = detect_from_filename("username_a_portrait_of_a_cat_abc123def.png");
        // Midjourney Discord filenames are too ambiguous for high confidence
        assert!(result.confidence < 0.5 || result.source_label.is_none());
    }

    #[test]
    fn test_filename_generic() {
        let result = detect_from_filename("IMG_2024.jpg");
        assert_eq!(result.source_label, None);
    }

    #[test]
    fn test_png_text_stable_diffusion() {
        let chunks = vec![
            ("parameters".to_string(), "masterpiece, best quality\nNegative prompt: bad\nSteps: 20, Sampler: Euler".to_string()),
        ];
        let result = detect_from_png_text_chunks(&chunks);
        assert_eq!(result.source_label, Some("stable_diffusion".to_string()));
        assert!(result.confidence > 0.8);
        assert!(result.ai_prompt.is_some());
    }

    #[test]
    fn test_png_text_comfyui_workflow() {
        let chunks = vec![
            ("prompt".to_string(), r#"{"3": {"class_type": "KSampler"}}"#.to_string()),
            ("workflow".to_string(), r#"{"nodes": []}"#.to_string()),
        ];
        let result = detect_from_png_text_chunks(&chunks);
        assert_eq!(result.source_label, Some("comfyui".to_string()));
    }

    #[test]
    fn test_combine_evidence() {
        let results = vec![
            SourceDetectionResult {
                source_label: Some("stable_diffusion".to_string()),
                confidence: 0.9,
                is_ai_generated: Some(true),
                ai_prompt: Some("a cat".to_string()),
                evidence: vec![SourceEvidence {
                    detector: "png_text".to_string(),
                    source_label: Some("stable_diffusion".to_string()),
                    confidence: 0.9,
                    details: "Found parameters chunk".to_string(),
                }],
            },
            SourceDetectionResult {
                source_label: None,
                confidence: 0.0,
                is_ai_generated: None,
                ai_prompt: None,
                evidence: vec![],
            },
        ];
        let combined = combine_detection_results(results);
        assert_eq!(combined.source_label, Some("stable_diffusion".to_string()));
        assert!(combined.confidence > 0.8);
    }
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cd src-tauri && cargo test source_detection --lib -- --nocapture`
Expected: FAIL — functions not defined.

- [ ] **Step 3: Implement filename detection**

Add to `source_detection.rs`:

```rust
use regex::Regex;
use std::sync::LazyLock;

static DALLE_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)DALL[\-·\.]?E[\s_]?\d{4}").unwrap()
});

static COMFYUI_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)^ComfyUI_\d+").unwrap()
});

static SD_SEED_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"^\d{5,10}-\d+\.png$").unwrap()
});

pub fn detect_from_filename(filename: &str) -> SourceDetectionResult {
    let name = std::path::Path::new(filename)
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or(filename);

    if DALLE_RE.is_match(name) {
        return SourceDetectionResult {
            source_label: Some("dalle".to_string()),
            confidence: 0.7,
            is_ai_generated: Some(true),
            ai_prompt: None,
            evidence: vec![SourceEvidence {
                detector: "filename".to_string(),
                source_label: Some("dalle".to_string()),
                confidence: 0.7,
                details: format!("Filename matches DALL-E pattern: {}", name),
            }],
        };
    }

    if COMFYUI_RE.is_match(name) {
        return SourceDetectionResult {
            source_label: Some("comfyui".to_string()),
            confidence: 0.6,
            is_ai_generated: Some(true),
            ai_prompt: None,
            evidence: vec![SourceEvidence {
                detector: "filename".to_string(),
                source_label: Some("comfyui".to_string()),
                confidence: 0.6,
                details: format!("Filename matches ComfyUI pattern: {}", name),
            }],
        };
    }

    if SD_SEED_RE.is_match(name) {
        return SourceDetectionResult {
            source_label: Some("stable_diffusion".to_string()),
            confidence: 0.3,
            is_ai_generated: Some(true),
            ai_prompt: None,
            evidence: vec![SourceEvidence {
                detector: "filename".to_string(),
                source_label: Some("stable_diffusion".to_string()),
                confidence: 0.3,
                details: format!("Filename matches SD seed pattern: {}", name),
            }],
        };
    }

    SourceDetectionResult::unknown()
}
```

- [ ] **Step 4: Implement PNG text chunk detection**

Add to `source_detection.rs`:

```rust
pub fn detect_from_png_text_chunks(chunks: &[(String, String)]) -> SourceDetectionResult {
    for (key, value) in chunks {
        match key.as_str() {
            "parameters" if value.contains("Steps:") && value.contains("Sampler:") => {
                let prompt = value.lines().next().map(|l| l.to_string());
                return SourceDetectionResult {
                    source_label: Some("stable_diffusion".to_string()),
                    confidence: 0.95,
                    is_ai_generated: Some(true),
                    ai_prompt: prompt,
                    evidence: vec![SourceEvidence {
                        detector: "png_text".to_string(),
                        source_label: Some("stable_diffusion".to_string()),
                        confidence: 0.95,
                        details: "Found A1111-style parameters chunk".to_string(),
                    }],
                };
            }
            "prompt" if value.contains("class_type") => {
                return SourceDetectionResult {
                    source_label: Some("comfyui".to_string()),
                    confidence: 0.95,
                    is_ai_generated: Some(true),
                    ai_prompt: None,
                    evidence: vec![SourceEvidence {
                        detector: "png_text".to_string(),
                        source_label: Some("comfyui".to_string()),
                        confidence: 0.95,
                        details: "Found ComfyUI prompt JSON chunk".to_string(),
                    }],
                };
            }
            "Dream" => {
                return SourceDetectionResult {
                    source_label: Some("stable_diffusion".to_string()),
                    confidence: 0.9,
                    is_ai_generated: Some(true),
                    ai_prompt: Some(value.clone()),
                    evidence: vec![SourceEvidence {
                        detector: "png_text".to_string(),
                        source_label: Some("stable_diffusion".to_string()),
                        confidence: 0.9,
                        details: "Found Dream chunk (InvokeAI)".to_string(),
                    }],
                };
            }
            "Software" if value.to_lowercase().contains("nanobanana") => {
                return SourceDetectionResult {
                    source_label: Some("nanobanana".to_string()),
                    confidence: 0.9,
                    is_ai_generated: Some(true),
                    ai_prompt: None,
                    evidence: vec![SourceEvidence {
                        detector: "png_text".to_string(),
                        source_label: Some("nanobanana".to_string()),
                        confidence: 0.9,
                        details: "Software field contains Nanobanana".to_string(),
                    }],
                };
            }
            _ => {}
        }
    }

    SourceDetectionResult::unknown()
}
```

- [ ] **Step 5: Implement evidence combiner**

Add to `source_detection.rs`:

```rust
pub fn combine_detection_results(results: Vec<SourceDetectionResult>) -> SourceDetectionResult {
    let mut all_evidence: Vec<SourceEvidence> = vec![];
    let mut best: Option<SourceDetectionResult> = None;

    for r in results {
        all_evidence.extend(r.evidence.clone());
        match &best {
            None if r.source_label.is_some() => best = Some(r),
            Some(b) if r.confidence > b.confidence && r.source_label.is_some() => {
                best = Some(r);
            }
            _ => {}
        }
    }

    match best {
        Some(mut b) => {
            b.evidence = all_evidence;
            b
        }
        None => {
            let mut unknown = SourceDetectionResult::unknown();
            unknown.evidence = all_evidence;
            unknown
        }
    }
}

pub fn detect_source(filename: &str, png_text_chunks: &[(String, String)]) -> SourceDetectionResult {
    let filename_result = detect_from_filename(filename);
    let png_result = detect_from_png_text_chunks(png_text_chunks);
    combine_detection_results(vec![png_result, filename_result])
}
```

- [ ] **Step 6: Add regex dependency if needed**

Check `src-tauri/Cargo.toml` for `regex`. If missing, add: `regex = "1"`

- [ ] **Step 7: Run tests**

Run: `cd src-tauri && cargo test source_detection --lib -- --nocapture`
Expected: all 7 tests pass.

- [ ] **Step 8: Commit**

```bash
git add src-tauri/src/db_core/source_detection.rs src-tauri/Cargo.toml
git commit -m "feat: source detection — filename patterns and PNG text chunk parsing"
```

---

### Task 4: Smart Collection Query Engine — FilterNode to SQL

**Files:**
- Modify: `src-tauri/src/db_core/smart_collections.rs`
- Modify: `src-tauri/src/db_core/db.rs`

- [ ] **Step 1: Write tests for SQL compilation**

Add to `smart_collections.rs` tests module:

```rust
#[test]
fn test_simple_rating_filter_to_sql() {
    let filter = FilterNode::Rule {
        field: Field::Rating,
        op: RuleOp::Gte,
        value: FilterValue::Number(4.0),
    };
    let (sql, params) = filter.to_sql_clause().unwrap();
    assert!(sql.contains("s.star_rating"));
    assert!(sql.contains(">="));
    assert_eq!(params.len(), 1);
}

#[test]
fn test_and_group_to_sql() {
    let filter = FilterNode::Group {
        op: GroupOp::And,
        children: vec![
            FilterNode::Rule {
                field: Field::Rating,
                op: RuleOp::Gte,
                value: FilterValue::Number(4.0),
            },
            FilterNode::Rule {
                field: Field::SourceLabel,
                op: RuleOp::Eq,
                value: FilterValue::String("midjourney".to_string()),
            },
        ],
    };
    let (sql, params) = filter.to_sql_clause().unwrap();
    assert!(sql.contains("AND"));
    assert_eq!(params.len(), 2);
}

#[test]
fn test_not_to_sql() {
    let filter = FilterNode::Not {
        child: Box::new(FilterNode::Rule {
            field: Field::SourceLabel,
            op: RuleOp::Eq,
            value: FilterValue::String("midjourney".to_string()),
        }),
    };
    let (sql, _) = filter.to_sql_clause().unwrap();
    assert!(sql.contains("NOT"));
}

#[test]
fn test_last_n_days_to_sql() {
    let filter = FilterNode::Rule {
        field: Field::ImportedAt,
        op: RuleOp::LastNDays,
        value: FilterValue::Number(7.0),
    };
    let (sql, _) = filter.to_sql_clause().unwrap();
    assert!(sql.contains("datetime"));
}

#[test]
fn test_orientation_filter() {
    let filter = FilterNode::Rule {
        field: Field::Orientation,
        op: RuleOp::Eq,
        value: FilterValue::String("landscape".to_string()),
    };
    let (sql, params) = filter.to_sql_clause().unwrap();
    assert!(sql.contains("i.orientation"));
    assert_eq!(params.len(), 1);
}

#[test]
fn test_in_operator() {
    let filter = FilterNode::Rule {
        field: Field::Format,
        op: RuleOp::In,
        value: FilterValue::StringArray(vec!["png".to_string(), "webp".to_string()]),
    };
    let (sql, params) = filter.to_sql_clause().unwrap();
    assert!(sql.contains("IN"));
    assert_eq!(params.len(), 2);
}

#[test]
fn test_clip_field_returns_error() {
    let filter = FilterNode::Rule {
        field: Field::ClipSimilarTo,
        op: RuleOp::Eq,
        value: FilterValue::String("test".to_string()),
    };
    assert!(filter.to_sql_clause().is_err());
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cd src-tauri && cargo test smart_collections --lib -- --nocapture`
Expected: FAIL — `to_sql_clause` not defined.

- [ ] **Step 3: Implement FilterNode::to_sql_clause()**

Add to `smart_collections.rs`:

```rust
use rusqlite::types::Value as SqlValue;

impl Field {
    fn to_column(&self) -> &str {
        match self {
            Field::Rating => "s.star_rating",
            Field::ColorLabel => "s.color_label",
            Field::Decision => "s.decision",
            Field::Format => "i.format",
            Field::Width => "i.width",
            Field::Height => "i.height",
            Field::AspectRatio => "i.aspect_ratio",
            Field::Orientation => "i.orientation",
            Field::SourceLabel => "i.source_label",
            Field::SourceConfidence => "i.source_confidence",
            Field::IsAiGenerated => "i.is_ai_generated",
            Field::AiPrompt => "i.ai_prompt",
            Field::Folder => "f.path",
            Field::ImportedAt => "i.imported_at",
            Field::OriginalDate => "i.original_date",
            Field::FileSize => "i.file_size",
            // CLIP fields are not SQL-based — they use embedding search.
            // to_sql_clause() returns Err for these; handle them in evaluate_smart_collection separately.
            Field::ClipSimilarTo | Field::ClipTextMatch => "unsupported",
        }
    }
}

impl FilterNode {
    pub fn to_sql_clause(&self) -> std::result::Result<(String, Vec<SqlValue>), String> {
        match self {
            FilterNode::Group { op, children } => {
                if children.is_empty() {
                    return Ok(("1=1".to_string(), vec![]));
                }
                let separator = match op {
                    GroupOp::And => " AND ",
                    GroupOp::Or => " OR ",
                };
                let mut parts = vec![];
                let mut all_params = vec![];
                for child in children {
                    let (sql, params) = child.to_sql_clause()?;
                    parts.push(format!("({})", sql));
                    all_params.extend(params);
                }
                Ok((parts.join(separator), all_params))
            }
            FilterNode::Not { child } => {
                let (sql, params) = child.to_sql_clause()?;
                Ok((format!("NOT ({})", sql), params))
            }
            FilterNode::Rule { field, op, value } => {
                let col = field.to_column();
                if col == "unsupported" {
                    return Err(format!("Field {:?} requires embedding search, not SQL filtering", field));
                }

                match (op, value) {
                    (RuleOp::Eq, FilterValue::String(v)) => {
                        Ok((format!("{} = ?", col), vec![SqlValue::Text(v.clone())]))
                    }
                    (RuleOp::Eq, FilterValue::Number(v)) => {
                        Ok((format!("{} = ?", col), vec![SqlValue::Real(*v)]))
                    }
                    (RuleOp::Eq, FilterValue::Bool(v)) => {
                        Ok((format!("{} = ?", col), vec![SqlValue::Integer(*v as i64)]))
                    }
                    (RuleOp::Neq, FilterValue::String(v)) => {
                        Ok((format!("({} IS NULL OR {} != ?)", col, col), vec![SqlValue::Text(v.clone())]))
                    }
                    (RuleOp::Neq, FilterValue::Number(v)) => {
                        Ok((format!("({} IS NULL OR {} != ?)", col, col), vec![SqlValue::Real(*v)]))
                    }
                    (RuleOp::Gt, FilterValue::Number(v)) => {
                        Ok((format!("{} > ?", col), vec![SqlValue::Real(*v)]))
                    }
                    (RuleOp::Gte, FilterValue::Number(v)) => {
                        Ok((format!("{} >= ?", col), vec![SqlValue::Real(*v)]))
                    }
                    (RuleOp::Lt, FilterValue::Number(v)) => {
                        Ok((format!("{} < ?", col), vec![SqlValue::Real(*v)]))
                    }
                    (RuleOp::Lte, FilterValue::Number(v)) => {
                        Ok((format!("{} <= ?", col), vec![SqlValue::Real(*v)]))
                    }
                    (RuleOp::Contains, FilterValue::String(v)) => {
                        Ok((format!("{} LIKE ?", col), vec![SqlValue::Text(format!("%{}%", v))]))
                    }
                    (RuleOp::NotContains, FilterValue::String(v)) => {
                        Ok((format!("({} IS NULL OR {} NOT LIKE ?)", col, col),
                         vec![SqlValue::Text(format!("%{}%", v))]))
                    }
                    (RuleOp::In, FilterValue::StringArray(vals)) => {
                        let placeholders: Vec<&str> = vals.iter().map(|_| "?").collect();
                        let params: Vec<SqlValue> = vals.iter()
                            .map(|v| SqlValue::Text(v.clone()))
                            .collect();
                        Ok((format!("{} IN ({})", col, placeholders.join(",")), params))
                    }
                    (RuleOp::NotIn, FilterValue::StringArray(vals)) => {
                        let placeholders: Vec<&str> = vals.iter().map(|_| "?").collect();
                        let params: Vec<SqlValue> = vals.iter()
                            .map(|v| SqlValue::Text(v.clone()))
                            .collect();
                        Ok((format!("({} IS NULL OR {} NOT IN ({}))", col, col, placeholders.join(",")), params))
                    }
                    (RuleOp::IsEmpty, _) => {
                        Ok((format!("({} IS NULL OR {} = '')", col, col), vec![]))
                    }
                    (RuleOp::IsNotEmpty, _) => {
                        Ok((format!("({} IS NOT NULL AND {} != '')", col, col), vec![]))
                    }
                    (RuleOp::LastNDays, FilterValue::Number(days)) => {
                        Ok((format!("{} >= datetime('now', '-{} days')", col, *days as i64), vec![]))
                    }
                    (RuleOp::ThisWeek, _) => {
                        Ok((format!("{} >= datetime('now', 'weekday 0', '-7 days')", col), vec![]))
                    }
                    (RuleOp::ThisMonth, _) => {
                        Ok((format!("{} >= datetime('now', 'start of month')", col), vec![]))
                    }
                    (RuleOp::Between, FilterValue::Range { from, to }) => {
                        Ok((format!("{} BETWEEN ? AND ?", col),
                         vec![SqlValue::Text(from.clone()), SqlValue::Text(to.clone())]))
                    }
                    _ => Err(format!("Unsupported operator {:?} for field {:?}", op, field)),
                }
            }
        }
    }
}
```

- [ ] **Step 4: Run tests**

Run: `cd src-tauri && cargo test smart_collections --lib -- --nocapture`
Expected: all tests pass.

- [ ] **Step 5: Add evaluate_smart_collection to Database**

In `src-tauri/src/db_core/db.rs`, add. **Important**: follow the existing `ImageWithFile` nesting pattern from `list_images()` at line 94, use `s.project_id = '__global__'` in the JOIN, and use `Result<Vec<_>>` (one generic param):

```rust
use crate::db_core::smart_collections::{FilterNode, SmartCollection};

impl Database {
    pub fn evaluate_smart_collection(&self, filter_json: &str) -> Result<Vec<ImageWithFile>> {
        let filter: FilterNode = serde_json::from_str(filter_json)
            .map_err(|e| rusqlite::Error::InvalidParameterName(e.to_string()))?;

        let (where_clause, params) = filter.to_sql_clause()
            .map_err(|e| rusqlite::Error::InvalidParameterName(e))?;

        let conn = self.conn.lock().unwrap();
        let sql = format!(
            "SELECT i.id, i.sha256_hash, i.width, i.height, i.format, i.file_size,
                    i.created_at, i.imported_at, f.path,
                    s.star_rating, s.color_label, s.decision
             FROM images i
             JOIN image_files f ON f.image_id = i.id AND f.missing_at IS NULL
             LEFT JOIN selections s ON s.image_id = i.id AND s.project_id = '__global__'
             WHERE ({})
             GROUP BY i.id
             ORDER BY i.imported_at DESC",
            where_clause
        );

        let param_refs: Vec<&dyn rusqlite::types::ToSql> = params.iter()
            .map(|p| p as &dyn rusqlite::types::ToSql)
            .collect();

        let mut stmt = conn.prepare(&sql)?;
        let rows = stmt.query_map(param_refs.as_slice(), |row| {
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
```

- [ ] **Step 6: Commit**

```bash
git add src-tauri/src/db_core/smart_collections.rs src-tauri/src/db_core/db.rs
git commit -m "feat: FilterNode to SQL compiler with evaluate_smart_collection"
```

---

### Task 5: Smart Collection CRUD in Database

**Files:**
- Modify: `src-tauri/src/db_core/db.rs`

- [ ] **Step 1: Add create_smart_collection**

```rust
pub fn create_smart_collection(
    &self,
    name: &str,
    filter_json: &str,
    nl_query: Option<&str>,
    is_preset: bool,
) -> Result<String> {
    let conn = self.conn.lock().unwrap();
    let id = uuid::Uuid::new_v4().to_string();
    conn.execute(
        "INSERT INTO projects (id, name, collection_type, filter_json, nl_query, is_preset, created_at)
         VALUES (?1, ?2, 'smart', ?3, ?4, ?5, datetime('now'))",
        rusqlite::params![id, name, filter_json, nl_query, is_preset as i32],
    )?;
    Ok(id)
}
```

- [ ] **Step 2: Add list_smart_collections**

```rust
pub fn list_smart_collections(&self) -> Result<Vec<SmartCollection>> {
    let conn = self.conn.lock().unwrap();
    let mut stmt = conn.prepare(
        "SELECT id, name, description, collection_type, filter_json, nl_query,
                is_preset, sort_order, created_at
         FROM projects
         WHERE collection_type = 'smart'
         ORDER BY sort_order ASC, created_at DESC"
    )?;
    let rows = stmt.query_map([], |row| {
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
    })?;
    rows.collect::<Result<Vec<_>>>()
}
```

- [ ] **Step 3: Add delete_smart_collection and update_smart_collection**

```rust
pub fn delete_smart_collection(&self, id: &str) -> Result<()> {
    let conn = self.conn.lock().unwrap();
    conn.execute(
        "DELETE FROM projects WHERE id = ?1 AND collection_type = 'smart' AND is_preset = 0",
        [id],
    )?;
    Ok(())
}

pub fn update_smart_collection(&self, id: &str, name: &str, filter_json: &str, nl_query: Option<&str>) -> Result<()> {
    let conn = self.conn.lock().unwrap();
    conn.execute(
        "UPDATE projects SET name = ?2, filter_json = ?3, nl_query = ?4
         WHERE id = ?1 AND collection_type = 'smart'",
        rusqlite::params![id, name, filter_json, nl_query],
    )?;
    Ok(())
}
```

- [ ] **Step 4: Verify compilation**

Run: `cd src-tauri && cargo build`
Expected: compiles.

- [ ] **Step 5: Commit**

```bash
git add src-tauri/src/db_core/db.rs
git commit -m "feat: smart collection CRUD — create, list, update, delete"
```

---

### Task 6: Deterministic NL Parser

**Files:**
- Create: `src-tauri/src/db_core/nl_parser.rs`
- Modify: `src-tauri/src/db_core/mod.rs`

- [ ] **Step 1: Write parser tests**

Create `src-tauri/src/db_core/nl_parser.rs`:

```rust
use crate::db_core::smart_collections::*;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_five_stars() {
        let result = parse_query("5 stars");
        let (sql, _) = result.to_sql_clause().unwrap();
        assert!(sql.contains("star_rating"));
    }

    #[test]
    fn test_parse_midjourney() {
        let result = parse_query("midjourney");
        let (sql, _) = result.to_sql_clause().unwrap();
        assert!(sql.contains("source_label"));
    }

    #[test]
    fn test_parse_recent() {
        let result = parse_query("recent imports");
        let (sql, _) = result.to_sql_clause().unwrap();
        assert!(sql.contains("imported_at"));
    }

    #[test]
    fn test_parse_landscape_4_stars() {
        let result = parse_query("landscape 4 stars or more");
        let (sql, _) = result.to_sql_clause().unwrap();
        assert!(sql.contains("orientation"));
        assert!(sql.contains("star_rating"));
    }

    #[test]
    fn test_parse_not_midjourney() {
        let result = parse_query("not midjourney");
        let (sql, _) = result.to_sql_clause().unwrap();
        assert!(sql.contains("NOT"));
    }

    #[test]
    fn test_parse_png_images() {
        let result = parse_query("png images");
        let (sql, _) = result.to_sql_clause().unwrap();
        assert!(sql.contains("format"));
    }

    #[test]
    fn test_parse_picks() {
        let result = parse_query("picks");
        let (sql, _) = result.to_sql_clause().unwrap();
        assert!(sql.contains("decision"));
    }

    #[test]
    fn test_parse_empty_returns_all() {
        let result = parse_query("");
        let (sql, _) = result.to_sql_clause().unwrap();
        assert_eq!(sql, "1=1");
    }
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cd src-tauri && cargo test nl_parser --lib -- --nocapture`
Expected: FAIL — `parse_query` not defined.

- [ ] **Step 3: Implement the deterministic parser**

Add to `nl_parser.rs`:

```rust
use regex::Regex;
use std::sync::LazyLock;

struct PatternRule {
    pattern: Regex,
    build: fn(&regex::Captures) -> Option<FilterNode>,
}

static PATTERNS: LazyLock<Vec<PatternRule>> = LazyLock::new(|| {
    vec![
        // Rating patterns
        PatternRule {
            pattern: Regex::new(r"(?i)(\d)\s*\+?\s*stars?\s*(or\s*more|and\s*above|\+)?").unwrap(),
            build: |caps| {
                let n: f64 = caps[1].parse().ok()?;
                let has_plus = caps.get(2).is_some() || caps[0].contains('+');
                Some(FilterNode::Rule {
                    field: Field::Rating,
                    op: if has_plus || n >= 4.0 { RuleOp::Gte } else { RuleOp::Eq },
                    value: FilterValue::Number(n),
                })
            },
        },
        // Source patterns
        PatternRule {
            pattern: Regex::new(r"(?i)\b(midjourney|mj)\b").unwrap(),
            build: |_| Some(FilterNode::Rule {
                field: Field::SourceLabel,
                op: RuleOp::Eq,
                value: FilterValue::String("midjourney".to_string()),
            }),
        },
        PatternRule {
            pattern: Regex::new(r"(?i)\b(stable\s*diffusion|sd|a1111|automatic1111)\b").unwrap(),
            build: |_| Some(FilterNode::Rule {
                field: Field::SourceLabel,
                op: RuleOp::Eq,
                value: FilterValue::String("stable_diffusion".to_string()),
            }),
        },
        PatternRule {
            pattern: Regex::new(r"(?i)\b(dall[\-·]?e|chatgpt|openai)\b").unwrap(),
            build: |_| Some(FilterNode::Rule {
                field: Field::SourceLabel,
                op: RuleOp::Eq,
                value: FilterValue::String("dalle".to_string()),
            }),
        },
        PatternRule {
            pattern: Regex::new(r"(?i)\b(comfyui|comfy)\b").unwrap(),
            build: |_| Some(FilterNode::Rule {
                field: Field::SourceLabel,
                op: RuleOp::Eq,
                value: FilterValue::String("comfyui".to_string()),
            }),
        },
        PatternRule {
            pattern: Regex::new(r"(?i)\bnanobanana\b").unwrap(),
            build: |_| Some(FilterNode::Rule {
                field: Field::SourceLabel,
                op: RuleOp::Eq,
                value: FilterValue::String("nanobanana".to_string()),
            }),
        },
        // Orientation
        PatternRule {
            pattern: Regex::new(r"(?i)\b(landscape|horizontal|wide)\b").unwrap(),
            build: |_| Some(FilterNode::Rule {
                field: Field::Orientation,
                op: RuleOp::Eq,
                value: FilterValue::String("landscape".to_string()),
            }),
        },
        PatternRule {
            pattern: Regex::new(r"(?i)\b(portrait|vertical|tall)\b").unwrap(),
            build: |_| Some(FilterNode::Rule {
                field: Field::Orientation,
                op: RuleOp::Eq,
                value: FilterValue::String("portrait".to_string()),
            }),
        },
        PatternRule {
            pattern: Regex::new(r"(?i)\bsquare\b").unwrap(),
            build: |_| Some(FilterNode::Rule {
                field: Field::Orientation,
                op: RuleOp::Eq,
                value: FilterValue::String("square".to_string()),
            }),
        },
        // Format
        PatternRule {
            pattern: Regex::new(r"(?i)\b(png|jpg|jpeg|webp|gif|bmp|tiff)\b").unwrap(),
            build: |caps| Some(FilterNode::Rule {
                field: Field::Format,
                op: RuleOp::Eq,
                value: FilterValue::String(caps[1].to_lowercase()),
            }),
        },
        // Recency
        PatternRule {
            pattern: Regex::new(r"(?i)\b(recent|today|new)\s*(imports?|images?)?").unwrap(),
            build: |_| Some(FilterNode::Rule {
                field: Field::ImportedAt,
                op: RuleOp::LastNDays,
                value: FilterValue::Number(7.0),
            }),
        },
        PatternRule {
            pattern: Regex::new(r"(?i)\bthis\s*week\b").unwrap(),
            build: |_| Some(FilterNode::Rule {
                field: Field::ImportedAt,
                op: RuleOp::ThisWeek,
                value: FilterValue::Bool(true),
            }),
        },
        PatternRule {
            pattern: Regex::new(r"(?i)\bthis\s*month\b").unwrap(),
            build: |_| Some(FilterNode::Rule {
                field: Field::ImportedAt,
                op: RuleOp::ThisMonth,
                value: FilterValue::Bool(true),
            }),
        },
        // Decision
        PatternRule {
            pattern: Regex::new(r"(?i)\b(picks?|accepted|selected)\b").unwrap(),
            build: |_| Some(FilterNode::Rule {
                field: Field::Decision,
                op: RuleOp::Eq,
                value: FilterValue::String("accept".to_string()),
            }),
        },
        PatternRule {
            pattern: Regex::new(r"(?i)\b(rejects?|rejected)\b").unwrap(),
            build: |_| Some(FilterNode::Rule {
                field: Field::Decision,
                op: RuleOp::Eq,
                value: FilterValue::String("reject".to_string()),
            }),
        },
        // Color labels
        PatternRule {
            pattern: Regex::new(r"(?i)\b(red|green|blue|yellow)\s*(label)?\b").unwrap(),
            build: |caps| Some(FilterNode::Rule {
                field: Field::ColorLabel,
                op: RuleOp::Eq,
                value: FilterValue::String(caps[1].to_lowercase()),
            }),
        },
        // AI generated
        PatternRule {
            pattern: Regex::new(r"(?i)\b(ai\s*generated|ai\s*images?|generated)\b").unwrap(),
            build: |_| Some(FilterNode::Rule {
                field: Field::IsAiGenerated,
                op: RuleOp::Eq,
                value: FilterValue::Bool(true),
            }),
        },
        PatternRule {
            pattern: Regex::new(r"(?i)\bphotos?\b").unwrap(),
            build: |_| Some(FilterNode::Rule {
                field: Field::IsAiGenerated,
                op: RuleOp::Eq,
                value: FilterValue::Bool(false),
            }),
        },
    ]
});

static NOT_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)\bnot\s+").unwrap()
});

pub fn parse_query(input: &str) -> FilterNode {
    let input = input.trim();
    if input.is_empty() {
        return FilterNode::Group {
            op: GroupOp::And,
            children: vec![],
        };
    }

    let mut rules: Vec<FilterNode> = vec![];
    let mut remaining = input.to_string();

    for pattern_rule in PATTERNS.iter() {
        if let Some(caps) = pattern_rule.pattern.captures(&remaining) {
            if let Some(node) = (pattern_rule.build)(&caps) {
                let matched_text = &caps[0];
                let is_negated = {
                    let before = &remaining[..caps.get(0).unwrap().start()];
                    NOT_RE.is_match(before.trim_end())
                };

                if is_negated {
                    rules.push(FilterNode::Not {
                        child: Box::new(node),
                    });
                } else {
                    rules.push(node);
                }

                remaining = remaining.replace(matched_text, " ");
            }
        }
    }

    match rules.len() {
        0 => FilterNode::Group { op: GroupOp::And, children: vec![] },
        1 => rules.remove(0),
        _ => FilterNode::Group { op: GroupOp::And, children: rules },
    }
}
```

- [ ] **Step 4: Register module**

In `src-tauri/src/db_core/mod.rs`, add: `pub mod nl_parser;`

- [ ] **Step 5: Run tests**

Run: `cd src-tauri && cargo test nl_parser --lib -- --nocapture`
Expected: all 8 tests pass.

- [ ] **Step 6: Commit**

```bash
git add src-tauri/src/db_core/nl_parser.rs src-tauri/src/db_core/mod.rs
git commit -m "feat: deterministic NL parser — regex-based query to FilterNode"
```

---

### Task 7: Tauri Commands for Smart Collections

**Files:**
- Create: `src-tauri/src/commands/smart_collections.rs`
- Modify: `src-tauri/src/commands/mod.rs`
- Modify: `src-tauri/src/main.rs` (or `lib.rs` — wherever commands are registered)

- [ ] **Step 1: Create smart collection commands**

Create `src-tauri/src/commands/smart_collections.rs`:

```rust
use tauri::State;
use crate::AppState;
use crate::db_core::smart_collections::SmartCollection;
use crate::db_core::models::ImageWithFile;
use crate::db_core::nl_parser::parse_query;

#[tauri::command]
pub fn create_smart_collection(
    state: State<'_, AppState>,
    name: String,
    filter_json: String,
    nl_query: Option<String>,
) -> Result<String, String> {
    state.db.create_smart_collection(&name, &filter_json, nl_query.as_deref(), false)
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn list_smart_collections(
    state: State<'_, AppState>,
) -> Result<Vec<SmartCollection>, String> {
    state.db.list_smart_collections()
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn evaluate_smart_collection(
    state: State<'_, AppState>,
    filter_json: String,
) -> Result<Vec<ImageWithFile>, String> {
    state.db.evaluate_smart_collection(&filter_json)
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn delete_smart_collection(
    state: State<'_, AppState>,
    id: String,
) -> Result<(), String> {
    state.db.delete_smart_collection(&id)
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn update_smart_collection(
    state: State<'_, AppState>,
    id: String,
    name: String,
    filter_json: String,
    nl_query: Option<String>,
) -> Result<(), String> {
    state.db.update_smart_collection(&id, &name, &filter_json, nl_query.as_deref())
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn parse_nl_query(query: String) -> Result<String, String> {
    let filter = parse_query(&query);
    serde_json::to_string(&filter).map_err(|e| e.to_string())
}
```

- [ ] **Step 2: Register module**

In `src-tauri/src/commands/mod.rs`, add: `pub mod smart_collections;`

- [ ] **Step 3: Register commands in main.rs/lib.rs**

Find the `.invoke_handler(tauri::generate_handler![...])` call and add:

```rust
commands::smart_collections::create_smart_collection,
commands::smart_collections::list_smart_collections,
commands::smart_collections::evaluate_smart_collection,
commands::smart_collections::delete_smart_collection,
commands::smart_collections::update_smart_collection,
commands::smart_collections::parse_nl_query,
```

- [ ] **Step 4: Verify compilation**

Run: `cd src-tauri && cargo build`
Expected: compiles.

- [ ] **Step 5: Commit**

```bash
git add src-tauri/src/commands/smart_collections.rs src-tauri/src/commands/mod.rs src-tauri/src/main.rs
git commit -m "feat: Tauri commands for smart collections CRUD and NL parsing"
```

---

### Task 8: Frontend API and Stores

**Files:**
- Modify: `src/lib/api.ts`
- Modify: `src/lib/stores.ts`

- [ ] **Step 1: Add TypeScript types**

At the top of `src/lib/api.ts`, add discriminated union types matching the Rust structs:

```typescript
export type FilterNode = FilterGroup | FilterNot | FilterRule;

export interface FilterGroup {
  type: 'group';
  op: 'and' | 'or';
  children: FilterNode[];
}

export interface FilterNot {
  type: 'not';
  child: FilterNode;
}

export interface FilterRule {
  type: 'rule';
  field: string;
  op: string;
  value: any;
}

export interface SmartCollection {
  id: string;
  name: string;
  description: string | null;
  collection_type: string;
  filter_json: string | null;
  nl_query: string | null;
  is_preset: boolean;
  sort_order: number;
  created_at: string;
  image_count: number | null;
}
```

- [ ] **Step 2: Add API functions**

Add to `src/lib/api.ts`:

```typescript
export async function listSmartCollections(): Promise<SmartCollection[]> {
  return invoke('list_smart_collections');
}

export async function createSmartCollection(
  name: string,
  filterJson: string,
  nlQuery?: string,
): Promise<string> {
  return invoke('create_smart_collection', { name, filterJson, nlQuery });
}

export async function evaluateSmartCollection(filterJson: string): Promise<ImageWithFile[]> {
  return invoke('evaluate_smart_collection', { filterJson });
}

export async function deleteSmartCollectionApi(id: string): Promise<void> {
  return invoke('delete_smart_collection', { id });
}

export async function updateSmartCollectionApi(
  id: string,
  name: string,
  filterJson: string,
  nlQuery?: string,
): Promise<void> {
  return invoke('update_smart_collection', { id, name, filterJson, nlQuery });
}

export async function parseNlQuery(query: string): Promise<string> {
  return invoke('parse_nl_query', { query });
}
```

- [ ] **Step 3: Add stores**

Add to `src/lib/stores.ts`:

```typescript
import type { SmartCollection } from './api';

export const smartCollections = writable<SmartCollection[]>([]);
export const activeSmartCollection = writable<SmartCollection | null>(null);
```

- [ ] **Step 4: Commit**

```bash
git add src/lib/api.ts src/lib/stores.ts
git commit -m "feat: frontend API and stores for smart collections"
```

---

### Task 9: Sidebar — Smart Collections List

**Files:**
- Modify: `src/lib/components/Sidebar.svelte`

- [ ] **Step 1: Read current Sidebar.svelte structure**

Read `src/lib/components/Sidebar.svelte` to understand the existing pattern for folders and collections rendering.

- [ ] **Step 2: Add smart collections section**

Add a new "SMART" section in Sidebar.svelte, between the FILTERS section and the COLLECTIONS section. **Use Svelte 5 patterns** matching the existing code: `$state()` rune, `onclick` (not `on:click`), `.section`/`.section-header`/`.section-item`/`.count`/`.active` CSS classes.

In the `<script>` block, add imports and state:

```typescript
import { smartCollections, activeSmartCollection } from '$lib/stores';
import { listSmartCollections, evaluateSmartCollection } from '$lib/api';
import type { SmartCollection } from '$lib/api';

let smartExpanded = $state(true);
```

In the `onMount`, add: `$smartCollections = await listSmartCollections();`

Add the helper function:

```typescript
async function selectSmartCollection(sc: SmartCollection) {
    activeFolder.set(null);
    activeCollection.set(null);
    activeSmartCollection.set(sc);
    if (sc.filter_json) {
        const imgs = await evaluateSmartCollection(sc.filter_json);
        images.set(imgs);
    }
    focusedIndex.set(0);
}
```

Add in the template (between FILTERS and COLLECTIONS sections):

```svelte
<div class="section">
    <button class="folders-toggle" onclick={() => smartExpanded = !smartExpanded}>
        <span class="toggle-arrow">{smartExpanded ? '▾' : '▸'}</span>
        <span class="folders-toggle-label">Smart</span>
        <span class="count">({$smartCollections.length})</span>
    </button>

    {#if smartExpanded}
        {#each $smartCollections as sc}
            <button
                class="section-item"
                class:active={$activeSmartCollection?.id === sc.id}
                onclick={() => selectSmartCollection(sc)}
            >
                {sc.name}
            </button>
        {/each}
    {/if}
</div>
```

- [ ] **Step 3: Commit**

```bash
git add src/lib/components/Sidebar.svelte
git commit -m "feat: sidebar smart collections list with evaluation on click"
```

---

### Task 10: Command Bar and Rule Builder UI

**Files:**
- Create: `src/lib/components/CommandBar.svelte`
- Create: `src/lib/components/RuleBuilder.svelte`
- Modify: `src/routes/+page.svelte`

- [ ] **Step 1: Create CommandBar component**

Create `src/lib/components/CommandBar.svelte`. **Key design requirement**: On Enter, show parsed rules as a preview. User must click "Apply" to actually execute the filter. This matches the spec's query-confirmation UX. Uses Svelte 5 `$state()`:

```svelte
<script lang="ts">
  import { parseNlQuery, evaluateSmartCollection, createSmartCollection } from '$lib/api';
  import { images, focusedIndex, activeSmartCollection, activeFolder, activeCollection } from '$lib/stores';
  import type { FilterNode } from '$lib/api';
  import RuleBuilder from './RuleBuilder.svelte';

  let query = $state('');
  let parsedFilter: FilterNode | null = $state(null);
  let filterJson = $state('');
  let matchCount = $state(0);
  let showRules = $state(false);
  let applied = $state(false);

  async function handleParse() {
    if (!query.trim()) {
      parsedFilter = null;
      showRules = false;
      applied = false;
      return;
    }

    filterJson = await parseNlQuery(query);
    parsedFilter = JSON.parse(filterJson);
    showRules = true;
    applied = false;
  }

  async function handleApply() {
    if (!filterJson) return;
    activeFolder.set(null);
    activeCollection.set(null);
    activeSmartCollection.set(null);
    const results = await evaluateSmartCollection(filterJson);
    images.set(results);
    matchCount = results.length;
    focusedIndex.set(0);
    applied = true;
  }

  async function handleSave() {
    if (!filterJson) return;
    const name = query.trim() || 'Untitled';
    await createSmartCollection(name, filterJson, query);
  }

  function handleKeydown(e: KeyboardEvent) {
    if (e.key === 'Enter') {
      handleParse();
    }
  }
</script>

<div class="command-bar-wrapper">
  <div class="command-bar">
    <span class="command-icon">/</span>
    <input
      type="text"
      bind:value={query}
      onkeydown={handleKeydown}
      placeholder="landscape midjourney 4 stars or more..."
      class="command-input"
    />
    <span class="hint">voice | type</span>
  </div>

  {#if showRules && parsedFilter}
    <div class="parsed-rules">
      <div class="rules-header">
        <span class="parsed-label">Parsed as:</span>
        <div class="rules-actions">
          {#if applied}
            <span class="match-count">{matchCount} images match</span>
            <button class="save-btn" onclick={handleSave}>Save</button>
          {:else}
            <button class="apply-btn" onclick={handleApply}>Apply</button>
          {/if}
        </div>
      </div>
      <RuleBuilder filter={parsedFilter} />
    </div>
  {/if}
</div>

<style>
  .command-bar-wrapper { display: flex; flex-direction: column; }
  .command-bar {
    display: flex; align-items: center; gap: 8px;
    padding: 8px 12px; border: 2px solid var(--accent, #4a9eed);
    border-radius: 8px; background: var(--bg, #fff);
  }
  .command-icon { color: var(--accent, #4a9eed); font-size: 18px; font-weight: bold; }
  .command-input { flex: 1; border: none; outline: none; font-size: 16px; background: transparent; }
  .hint { font-size: 12px; color: var(--muted, #999); }
  .parsed-rules {
    padding: 12px; background: var(--surface, #fafafa);
    border: 1px solid var(--border, #e0e0e0); border-radius: 0 0 8px 8px; border-top: none;
  }
  .rules-header { display: flex; justify-content: space-between; margin-bottom: 8px; }
  .parsed-label { color: var(--muted, #757575); }
  .rules-actions { display: flex; gap: 8px; align-items: center; }
  .match-count { color: var(--muted, #757575); font-size: 14px; }
  .apply-btn {
    padding: 4px 16px; border-radius: 6px; border: 1px solid var(--accent, #4a9eed);
    background: var(--accent, #4a9eed); color: white; cursor: pointer; font-size: 13px;
  }
  .save-btn {
    padding: 4px 12px; border-radius: 6px; border: 1px dashed var(--accent, #4a9eed);
    background: transparent; color: var(--accent, #4a9eed); cursor: pointer; font-size: 13px;
  }
</style>
```

- [ ] **Step 2: Create RuleBuilder component**

Create `src/lib/components/RuleBuilder.svelte`. **Must be editable** with `<select>` dropdowns for field/op and an AND/OR toggle. Uses Svelte 5 `$state()` and `onclick`:

```svelte
<script lang="ts">
  import type { FilterNode, FilterGroup, FilterRule, FilterNot } from '$lib/api';

  interface Props { filter: FilterNode; }
  let { filter }: Props = $props();

  const FIELD_OPTIONS: [string, string][] = [
    ['rating', 'Rating'], ['color_label', 'Color Label'], ['decision', 'Decision'],
    ['format', 'Format'], ['width', 'Width'], ['height', 'Height'],
    ['orientation', 'Orientation'], ['source_label', 'Source'],
    ['is_ai_generated', 'AI Generated'], ['imported_at', 'Imported'],
    ['ai_prompt', 'Prompt'], ['aspect_ratio', 'Aspect Ratio'],
  ];

  const OP_OPTIONS: [string, string][] = [
    ['eq', 'is'], ['neq', 'is not'], ['gt', '>'], ['gte', '>='],
    ['lt', '<'], ['lte', '<='], ['contains', 'contains'],
    ['last_n_days', 'in last N days'], ['this_week', 'this week'],
    ['this_month', 'this month'], ['is_empty', 'is empty'],
  ];

  function getRules(node: FilterNode): FilterNode[] {
    if (node.type === 'group') return node.children;
    if (node.type === 'rule') return [node];
    return [];
  }

  function getGroupOp(node: FilterNode): string {
    return node.type === 'group' ? node.op : 'and';
  }

  function formatValue(value: any): string {
    if (typeof value === 'boolean') return value ? 'Yes' : 'No';
    if (typeof value === 'number') return String(value);
    if (typeof value === 'string') return value;
    if (Array.isArray(value)) return value.join(', ');
    return JSON.stringify(value);
  }

  function getRuleData(node: FilterNode): { field: string; op: string; value: any; negated: boolean } {
    if (node.type === 'not' && node.child.type === 'rule') {
      return { field: node.child.field, op: node.child.op, value: node.child.value, negated: true };
    }
    if (node.type === 'rule') {
      return { field: node.field, op: node.op, value: node.value, negated: false };
    }
    return { field: '', op: '', value: '', negated: false };
  }
</script>

<div class="rule-builder">
  <div class="match-toggle">
    <span class="match-label">Match</span>
    <select class="match-select" value={getGroupOp(filter)}>
      <option value="and">All (AND)</option>
      <option value="or">Any (OR)</option>
    </select>
  </div>

  {#each getRules(filter) as rule, i}
    {@const data = getRuleData(rule)}
    <div class="rule-row">
      {#if data.negated}
        <span class="not-badge">NOT</span>
      {/if}
      <select class="field-select" value={data.field}>
        {#each FIELD_OPTIONS as [val, label]}
          <option value={val}>{label}</option>
        {/each}
      </select>
      <select class="op-select" value={data.op}>
        {#each OP_OPTIONS as [val, label]}
          <option value={val}>{label}</option>
        {/each}
      </select>
      <input class="value-input" type="text" value={formatValue(data.value)} />
      <button class="remove-rule" onclick={() => {}}>&times;</button>
    </div>
  {/each}
  <button class="add-rule" onclick={() => {}}>+ Add rule</button>
</div>

<style>
  .rule-builder { display: flex; flex-direction: column; gap: 6px; }
  .match-toggle { display: flex; align-items: center; gap: 6px; margin-bottom: 4px; }
  .match-label { font-size: 13px; color: var(--muted, #757575); }
  .match-select { padding: 2px 8px; border-radius: 4px; border: 1px solid var(--accent, #4a9eed); font-size: 13px; color: var(--accent, #4a9eed); }
  .rule-row { display: flex; align-items: center; gap: 6px; }
  .field-select { padding: 4px 8px; border-radius: 6px; background: #d0bfff; border: 1px solid #8b5cf6; font-size: 13px; }
  .op-select { padding: 4px 8px; border-radius: 6px; background: #fff; border: 1px solid #ccc; font-size: 13px; }
  .value-input { padding: 4px 8px; border-radius: 6px; background: #fff; border: 1px solid #ccc; font-size: 13px; width: 120px; }
  .not-badge { padding: 2px 6px; border-radius: 4px; background: #ffc9c9; border: 1px solid #ef4444; font-size: 11px; font-weight: bold; color: #dc2626; }
  .remove-rule { background: none; border: none; color: #999; cursor: pointer; font-size: 16px; }
  .add-rule { background: none; border: 1px dashed var(--accent, #4a9eed); border-radius: 6px; color: var(--accent, #4a9eed); padding: 4px 12px; cursor: pointer; font-size: 13px; width: fit-content; }
</style>
```

- [ ] **Step 3: Integrate into page**

In `src/routes/+page.svelte`, read the existing layout structure first. The app uses a CSS grid with sidebar + content area. Add the CommandBar import and place it **inside the existing content area container**, above the image grid. Wrap it in a div that fits the existing grid layout:

```svelte
<script>
  import CommandBar from '$lib/components/CommandBar.svelte';
</script>

<!-- Inside the existing content area (read +page.svelte to find the right container) -->
<!-- Add this div above the Grid component, inside the content area wrapper -->
<div class="command-bar-area">
  <CommandBar />
</div>
```

Add CSS to the page's `<style>` block:
```css
.command-bar-area {
  padding: 8px 12px 0;
}
```

**Important**: Read `+page.svelte` before editing to understand the existing grid structure. Do not break the sidebar/content layout.

- [ ] **Step 4: Verify in dev mode**

Run: `npm run tauri dev`
Test: type "5 stars" in the command bar, press Enter. Verify rules appear and image grid filters.

- [ ] **Step 5: Commit**

```bash
git add src/lib/components/CommandBar.svelte src/lib/components/RuleBuilder.svelte src/routes/+page.svelte
git commit -m "feat: command bar and rule builder UI components"
```

---

### Task 11: Preset Smart Collections (Seeding)

**Files:**
- Modify: `src-tauri/src/db_core/db.rs`

Note: `seed_preset_collections()` is already called from `run_migrations()` (Task 1 Step 3). This task defines the seeding function.

- [ ] **Step 1: Add preset seeding function**

In `db.rs`, add the `seed_preset_collections` method to `Database`:

```rust
pub fn seed_preset_collections(&self) -> Result<()> {
    let conn = self.conn.lock().unwrap();

    let existing: i64 = conn.query_row(
        "SELECT COUNT(*) FROM projects WHERE is_preset = 1",
        [],
        |row| row.get(0),
    )?;

    if existing > 0 {
        return Ok(());
    }

    let presets = vec![
        ("5 Stars", r#"{"type":"rule","field":"rating","op":"eq","value":5.0}"#, 1),
        ("4 Stars+", r#"{"type":"rule","field":"rating","op":"gte","value":4.0}"#, 2),
        ("Picks", r#"{"type":"rule","field":"decision","op":"eq","value":"accept"}"#, 3),
        ("Rejects", r#"{"type":"rule","field":"decision","op":"eq","value":"reject"}"#, 4),
        ("Unrated", r#"{"type":"rule","field":"rating","op":"is_empty","value":true}"#, 5),
        ("Recent Imports", r#"{"type":"rule","field":"imported_at","op":"last_n_days","value":7.0}"#, 6),
        ("Imported Today", r#"{"type":"rule","field":"imported_at","op":"last_n_days","value":1.0}"#, 7),
        ("This Week", r#"{"type":"rule","field":"imported_at","op":"this_week","value":true}"#, 8),
        ("This Month", r#"{"type":"rule","field":"imported_at","op":"this_month","value":true}"#, 9),
        ("Landscape", r#"{"type":"rule","field":"orientation","op":"eq","value":"landscape"}"#, 10),
        ("Portrait", r#"{"type":"rule","field":"orientation","op":"eq","value":"portrait"}"#, 11),
        ("Square", r#"{"type":"rule","field":"orientation","op":"eq","value":"square"}"#, 12),
        ("Panoramic", r#"{"type":"rule","field":"aspect_ratio","op":"gt","value":2.0}"#, 13),
        ("PNG", r#"{"type":"rule","field":"format","op":"eq","value":"png"}"#, 14),
        ("WebP", r#"{"type":"rule","field":"format","op":"eq","value":"webp"}"#, 15),
        ("Large (>4K)", r#"{"type":"rule","field":"width","op":"gte","value":3840.0}"#, 16),
        ("Small (<1024px)", r#"{"type":"rule","field":"width","op":"lt","value":1024.0}"#, 17),
        ("AI Generated", r#"{"type":"rule","field":"is_ai_generated","op":"eq","value":true}"#, 18),
        ("Red Label", r#"{"type":"rule","field":"color_label","op":"eq","value":"red"}"#, 19),
        ("Green Label", r#"{"type":"rule","field":"color_label","op":"eq","value":"green"}"#, 20),
        ("Blue Label", r#"{"type":"rule","field":"color_label","op":"eq","value":"blue"}"#, 21),
        ("Yellow Label", r#"{"type":"rule","field":"color_label","op":"eq","value":"yellow"}"#, 22),
        // CLIP-powered presets (Near-Duplicates, Outliers, Visual Clusters) deferred to v2
    ];

    for (name, filter, order) in presets {
        let id = uuid::Uuid::new_v4().to_string();
        conn.execute(
            "INSERT INTO projects (id, name, collection_type, filter_json, is_preset, sort_order, created_at)
             VALUES (?1, ?2, 'smart', ?3, 1, ?4, datetime('now'))",
            rusqlite::params![id, name, filter, order],
        )?;
    }

    Ok(())
}
```

- [ ] **Step 2: Exclude smart collections from list_collections()**

Modify the existing `list_collections()` method to exclude smart collections:

```rust
pub fn list_collections(&self) -> Result<Vec<(String, String, u32)>> {
    let conn = self.conn.lock().unwrap();
    let mut stmt = conn.prepare(
        "SELECT p.id, p.name, COUNT(ci.image_id) as cnt
         FROM projects p
         LEFT JOIN collection_items ci ON ci.collection_id = p.id
         WHERE p.collection_type IS NULL OR p.collection_type = 'manual'
         GROUP BY p.id
         ORDER BY p.created_at DESC"
    )?;
    // ... rest unchanged
```

- [ ] **Step 3: Verify build and commit**

Run: `cd src-tauri && cargo build`

```bash
git add src-tauri/src/db_core/db.rs
git commit -m "feat: 22 preset smart collections + exclude smart from manual list"
```

---

### Task 12: Source Detection on Import

**Files:**
- Modify: `src-tauri/src/db_core/db.rs` (add `update_source_detection` method)
- Modify: `src-tauri/src/db_core/import.rs`
- Modify: `src-tauri/Cargo.toml` (add `png` crate if needed)

- [ ] **Step 1: Add update_source_detection to Database**

Since `Database.conn` is private, import.rs cannot access it directly. Add a method to `db.rs`:

```rust
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
    let conn = self.conn.lock().unwrap();
    conn.execute(
        "UPDATE images SET source_label = ?1, source_confidence = ?2,
         source_evidence_json = ?3, is_ai_generated = ?4, ai_prompt = ?5,
         aspect_ratio = ?6, orientation = ?7, megapixels = ?8,
         source_detected_at = datetime('now'), source_detector_version = 'v1'
         WHERE id = ?9",
        rusqlite::params![
            source_label,
            source_confidence,
            source_evidence_json,
            is_ai_generated.map(|b| b as i32),
            ai_prompt,
            aspect_ratio,
            orientation,
            megapixels,
            image_id,
        ],
    )?;
    Ok(())
}
```

- [ ] **Step 2: Add source detection call in import.rs**

In `src-tauri/src/db_core/import.rs`, after the image is inserted into the DB via `db.insert_image(...)`, add source detection using the `Database` method:

```rust
use crate::db_core::source_detection::{detect_source, read_png_text_chunks};

// After image insert, detect source and compute metadata
let filename = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
let png_chunks = if format == "png" {
    read_png_text_chunks(&path).unwrap_or_default()
} else {
    vec![]
};

let detection = detect_source(filename, &png_chunks);

let aspect_ratio = width as f64 / height.max(1) as f64;
let orientation = if (aspect_ratio - 1.0).abs() < 0.05 {
    "square"
} else if aspect_ratio > 1.0 {
    "landscape"
} else {
    "portrait"
};
let megapixels = (width as f64 * height as f64) / 1_000_000.0;

db.update_source_detection(
    &image_id,
    detection.source_label.as_deref(),
    detection.confidence,
    &detection.to_evidence_json(),
    detection.is_ai_generated,
    detection.ai_prompt.as_deref(),
    aspect_ratio,
    orientation,
    megapixels,
)?;
```

- [ ] **Step 3: Add PNG text chunk reader helper**

Add to `src-tauri/src/db_core/source_detection.rs`:

```rust
pub fn read_png_text_chunks(path: &std::path::Path) -> std::result::Result<Vec<(String, String)>, Box<dyn std::error::Error>> {
    let file = std::fs::File::open(path)?;
    let decoder = png::Decoder::new(file);
    let reader = decoder.read_info()?;
    let info = reader.info();

    let mut chunks = vec![];
    for text in &info.uncompressed_latin1_text {
        chunks.push((text.keyword.clone(), text.text.clone()));
    }
    for text in &info.utf8_text {
        chunks.push((text.keyword.clone(), text.text.clone()));
    }
    Ok(chunks)
}
```

Note: Requires `png` crate. Add `png = "0.17"` to `Cargo.toml` if not already present.

- [ ] **Step 4: Verify build**

Run: `cd src-tauri && cargo build`

- [ ] **Step 5: Commit**

```bash
git add src-tauri/src/db_core/db.rs src-tauri/src/db_core/import.rs src-tauri/src/db_core/source_detection.rs src-tauri/Cargo.toml
git commit -m "feat: source detection on import with evidence storage"
```

---

### Task 13: Backfill Existing Images

**Files:**
- Modify: `src-tauri/src/db_core/db.rs`
- Modify: `src-tauri/src/commands/smart_collections.rs`

Existing images have NULL orientation/aspect_ratio/megapixels. Presets like Landscape/Portrait will miss them without a backfill.

- [ ] **Step 1: Add backfill method to Database**

```rust
pub fn backfill_image_metadata(&self) -> Result<u32> {
    let conn = self.conn.lock().unwrap();
    let mut stmt = conn.prepare(
        "SELECT i.id, i.width, i.height, f.path
         FROM images i
         JOIN image_files f ON f.image_id = i.id AND f.missing_at IS NULL
         WHERE i.orientation IS NULL"
    )?;
    let rows: Vec<(String, u32, u32, String)> = stmt.query_map([], |row| {
        Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?))
    })?.collect::<Result<Vec<_>>>()?;

    let count = rows.len() as u32;
    for (id, width, height, _path) in &rows {
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
            "UPDATE images SET aspect_ratio = ?1, orientation = ?2, megapixels = ?3 WHERE id = ?4",
            rusqlite::params![aspect_ratio, orientation, megapixels, id],
        )?;
    }
    Ok(count)
}
```

- [ ] **Step 2: Add Tauri command for backfill**

In `src-tauri/src/commands/smart_collections.rs`:

```rust
#[tauri::command]
pub fn backfill_image_metadata(
    state: State<'_, AppState>,
) -> Result<u32, String> {
    state.db.backfill_image_metadata()
        .map_err(|e| e.to_string())
}
```

Register in `main.rs`/`lib.rs` invoke handler.

- [ ] **Step 3: Call backfill on migration**

In `run_migrations()`, after `seed_preset_collections`, add:

```rust
self.backfill_image_metadata()?;
```

This runs once — subsequent imports set these fields directly.

- [ ] **Step 4: Commit**

```bash
git add src-tauri/src/db_core/db.rs src-tauri/src/commands/smart_collections.rs
git commit -m "feat: backfill orientation/aspect_ratio/megapixels for existing images"
```

---

### Task 14: End-to-End Testing

- [ ] **Step 1: Run app and test full flow**

Run: `npm run tauri dev`
Test:
1. Verify existing images got backfilled (orientation presets work)
2. Import a folder with some AI-generated PNGs
3. Check the sidebar shows 22 smart collection presets
4. Click "5 Stars" preset — should filter to 5-star images
5. Click "Landscape" — should show landscape images
6. Type "midjourney" in command bar — parsed rules should appear
7. Click "Apply" to confirm — image grid updates
8. Check that newly imported images have source_label populated
9. Verify manual collections list does NOT show smart collections

- [ ] **Step 2: Commit any fixes**

```bash
git add -A
git commit -m "fix: address integration test findings"
```

---

## Summary

| Task | Description | Key Files |
|------|-------------|-----------|
| 1 | DB schema migration | schema.sql, db.rs |
| 2 | FilterNode + SmartCollection models | smart_collections.rs, source_detection.rs |
| 3 | Source detection (filename + PNG text) | source_detection.rs |
| 4 | FilterNode → SQL compiler | smart_collections.rs, db.rs |
| 5 | Smart collection CRUD | db.rs |
| 6 | Deterministic NL parser | nl_parser.rs |
| 7 | Tauri commands | commands/smart_collections.rs |
| 8 | Frontend API + stores | api.ts, stores.ts |
| 9 | Sidebar smart collections | Sidebar.svelte |
| 10 | Command bar + rule builder | CommandBar.svelte, RuleBuilder.svelte |
| 11 | Preset smart collections (22 presets) | db.rs |
| 12 | Source detection on import | db.rs, import.rs, source_detection.rs |
| 13 | Backfill existing images | db.rs, commands/smart_collections.rs |
| 14 | End-to-end testing | — |
