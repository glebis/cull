use crate::db_core::db::{row_u64, sql_u64, Database};
use crate::db_core::models::{GenerationRun, Image, Selection};
use crate::db_core::smart_collections::SmartCollection;
use crate::exchange::xmp::{serialize_xmp, XmpMetadata};
use rusqlite::params;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};

const FORMAT: &str = "com.glebkalinin.cull.exchange";
const VERSION: u32 = 1;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum OriginalMode {
    ReferenceOriginals,
    CopyOriginals,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CullExchangeManifest {
    pub format: String,
    pub version: u32,
    pub created_by: String,
    pub created_at: String,
    pub original_mode: OriginalMode,
    pub capabilities: Vec<String>,
    pub images: Vec<ExchangeImage>,
    pub collections: Vec<ExchangeCollection>,
    pub smart_collections: Vec<SmartCollection>,
    pub generation_runs: Vec<GenerationRun>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExchangeImage {
    pub image: Image,
    pub original_path: String,
    pub bundle_path: Option<String>,
    pub selection: Option<Selection>,
    pub source_label: Option<String>,
    pub source_evidence_json: Option<String>,
    pub generation_run_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExchangeCollection {
    pub id: String,
    pub name: String,
    pub description: Option<String>,
    pub items: Vec<ExchangeCollectionItem>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExchangeCollectionItem {
    pub image_id: String,
    pub position: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExchangeExportOptions {
    pub target_dir: String,
    pub image_ids: Option<Vec<String>>,
    pub copy_originals: bool,
    pub include_xmp: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExchangeImportPreview {
    pub valid: bool,
    pub format: String,
    pub version: u32,
    pub image_count: usize,
    pub collection_count: usize,
    pub smart_collection_count: usize,
    pub generation_run_count: usize,
    pub missing_originals: Vec<String>,
    pub errors: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExchangeImportResult {
    pub imported_images: usize,
    pub imported_collections: usize,
    pub imported_smart_collections: usize,
    pub imported_generation_runs: usize,
}

pub fn build_manifest(
    db: &Database,
    image_ids: Option<&[String]>,
    original_mode: OriginalMode,
) -> rusqlite::Result<CullExchangeManifest> {
    let selected: Option<BTreeSet<String>> = image_ids.map(|ids| ids.iter().cloned().collect());
    let conn = db.conn.lock();
    let mut stmt = conn.prepare(
        "SELECT i.id, i.sha256_hash, i.width, i.height, i.format, i.file_size,
                i.created_at, i.imported_at, i.ai_prompt, i.raw_metadata, f.path,
                s.star_rating, s.color_label, s.decision, i.source_label,
                i.source_evidence_json, i.generation_run_id
         FROM images i
         JOIN image_files f ON f.image_id = i.id AND f.missing_at IS NULL
         LEFT JOIN selections s ON s.image_id = i.id AND s.project_id = '__global__'
         GROUP BY i.id
         ORDER BY i.id ASC",
    )?;
    let images = stmt
        .query_map([], |row| {
            let id: String = row.get(0)?;
            let star_rating: Option<u8> = row.get(11)?;
            let color_label: Option<String> = row.get(12)?;
            let decision: Option<String> = row.get(13)?;
            Ok(ExchangeImage {
                image: Image {
                    id: id.clone(),
                    sha256_hash: row.get(1)?,
                    width: row.get(2)?,
                    height: row.get(3)?,
                    format: row.get(4)?,
                    file_size: row_u64(row, 5)?,
                    created_at: row.get(6)?,
                    imported_at: row.get(7)?,
                    ai_prompt: row.get(8)?,
                    raw_metadata: row.get(9)?,
                },
                original_path: row.get(10)?,
                bundle_path: None,
                selection: decision.map(|d| Selection {
                    image_id: id,
                    project_id: None,
                    star_rating,
                    color_label,
                    decision: d,
                }),
                source_label: row.get(14)?,
                source_evidence_json: row.get(15)?,
                generation_run_id: row.get(16)?,
            })
        })?
        .filter_map(|row| row.ok())
        .filter(|img| {
            selected
                .as_ref()
                .map_or(true, |ids| ids.contains(&img.image.id))
        })
        .collect::<Vec<_>>();

    let selected_ids: BTreeSet<String> = images.iter().map(|img| img.image.id.clone()).collect();
    let generation_runs = load_generation_runs(&conn, &selected_ids)?;
    let collections = load_collections(&conn, &selected_ids)?;
    let smart_collections = load_smart_collections(&conn)?;

    Ok(CullExchangeManifest {
        format: FORMAT.to_string(),
        version: VERSION,
        created_by: "Cull".to_string(),
        created_at: chrono::Utc::now().to_rfc3339(),
        original_mode,
        capabilities: vec![
            "original_references".to_string(),
            "ratings".to_string(),
            "color_labels".to_string(),
            "decisions".to_string(),
            "collections".to_string(),
            "smart_collections".to_string(),
            "generation_runs".to_string(),
            "source_detection".to_string(),
            "xmp_sidecars".to_string(),
        ],
        images,
        collections,
        smart_collections,
        generation_runs,
    })
}

pub fn export_bundle(db: &Database, options: ExchangeExportOptions) -> Result<String, String> {
    let mode = if options.copy_originals {
        OriginalMode::CopyOriginals
    } else {
        OriginalMode::ReferenceOriginals
    };
    let target_dir = PathBuf::from(&options.target_dir);
    fs::create_dir_all(&target_dir).map_err(|e| e.to_string())?;
    fs::create_dir_all(target_dir.join("sidecars")).map_err(|e| e.to_string())?;
    fs::create_dir_all(target_dir.join("metadata")).map_err(|e| e.to_string())?;
    if options.copy_originals {
        fs::create_dir_all(target_dir.join("images")).map_err(|e| e.to_string())?;
    }

    let mut manifest = build_manifest(db, options.image_ids.as_deref(), mode)
        .map_err(|e| format!("Failed to build exchange manifest: {}", e))?;
    let runs_by_id = manifest
        .generation_runs
        .iter()
        .map(|run| (run.id.clone(), run.clone()))
        .collect::<BTreeMap<_, _>>();

    for image in &mut manifest.images {
        if options.copy_originals {
            let ext = Path::new(&image.original_path)
                .extension()
                .and_then(|e| e.to_str())
                .unwrap_or(&image.image.format);
            let rel = format!("images/{}.{}", image.image.id, ext);
            fs::copy(&image.original_path, target_dir.join(&rel))
                .map_err(|e| format!("Failed to copy original '{}': {}", image.original_path, e))?;
            image.bundle_path = Some(rel);
        }

        let cull_path = target_dir
            .join("metadata")
            .join(format!("{}.cull.json", image.image.id));
        let cull_json = serde_json::to_string_pretty(&image).map_err(|e| e.to_string())?;
        fs::write(cull_path, cull_json).map_err(|e| e.to_string())?;

        if options.include_xmp {
            let run = image
                .generation_run_id
                .as_ref()
                .and_then(|id| runs_by_id.get(id));
            let meta = xmp_for_image(image, run);
            let xmp = serialize_xmp(&meta);
            fs::write(
                target_dir
                    .join("sidecars")
                    .join(format!("{}.xmp", image.image.id)),
                xmp,
            )
            .map_err(|e| e.to_string())?;
        }
    }

    let manifest_json = serde_json::to_string_pretty(&manifest).map_err(|e| e.to_string())?;
    let manifest_path = target_dir.join("manifest.json");
    fs::write(&manifest_path, manifest_json).map_err(|e| e.to_string())?;
    Ok(manifest_path.to_string_lossy().to_string())
}

pub fn preview_import(bundle_dir: &Path) -> ExchangeImportPreview {
    let mut errors = vec![];
    let manifest = match read_manifest(bundle_dir) {
        Ok(manifest) => manifest,
        Err(e) => {
            return ExchangeImportPreview {
                valid: false,
                format: String::new(),
                version: 0,
                image_count: 0,
                collection_count: 0,
                smart_collection_count: 0,
                generation_run_count: 0,
                missing_originals: vec![],
                errors: vec![e],
            }
        }
    };

    if manifest.format != FORMAT {
        errors.push(format!("Unsupported format '{}'", manifest.format));
    }
    if manifest.version != VERSION {
        errors.push(format!("Unsupported version {}", manifest.version));
    }

    let missing_originals = manifest
        .images
        .iter()
        .filter_map(|img| {
            resolve_original(bundle_dir, img)
                .ok()
                .filter(|p| !p.exists())
        })
        .map(|p| p.to_string_lossy().to_string())
        .collect::<Vec<_>>();

    ExchangeImportPreview {
        valid: errors.is_empty(),
        format: manifest.format,
        version: manifest.version,
        image_count: manifest.images.len(),
        collection_count: manifest.collections.len(),
        smart_collection_count: manifest.smart_collections.len(),
        generation_run_count: manifest.generation_runs.len(),
        missing_originals,
        errors,
    }
}

pub fn import_bundle(db: &Database, bundle_dir: &Path) -> Result<ExchangeImportResult, String> {
    let preview = preview_import(bundle_dir);
    if !preview.valid {
        return Err(preview.errors.join("; "));
    }
    let manifest = read_manifest(bundle_dir)?;
    let conn = db.conn.lock();

    for run in &manifest.generation_runs {
        conn.execute(
            "INSERT OR REPLACE INTO generation_runs (id, prompt, negative_prompt, provider, model, settings_json, seed, parent_run_id, source_type, source_path, raw_metadata_json, created_at, imported_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13)",
            params![run.id, run.prompt, run.negative_prompt, run.provider, run.model, run.settings_json, run.seed, run.parent_run_id, run.source_type, run.source_path, run.raw_metadata_json, run.created_at, run.imported_at],
        )
        .map_err(|e| e.to_string())?;
    }

    for item in &manifest.images {
        conn.execute(
            "INSERT OR IGNORE INTO images (id, sha256_hash, width, height, format, file_size, created_at, imported_at, ai_prompt, raw_metadata, source_label, source_evidence_json, generation_run_id)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13)",
            params![
                item.image.id,
                item.image.sha256_hash,
                item.image.width,
                item.image.height,
                item.image.format,
                sql_u64(item.image.file_size).map_err(|e| e.to_string())?,
                item.image.created_at,
                item.image.imported_at,
                item.image.ai_prompt,
                item.image.raw_metadata,
                item.source_label,
                item.source_evidence_json,
                item.generation_run_id
            ],
        )
        .map_err(|e| e.to_string())?;
        conn.execute(
            "UPDATE images SET ai_prompt = COALESCE(?2, ai_prompt), raw_metadata = COALESCE(?3, raw_metadata), source_label = COALESCE(?4, source_label), source_evidence_json = COALESCE(?5, source_evidence_json), generation_run_id = COALESCE(?6, generation_run_id)
             WHERE id = ?1",
            params![item.image.id, item.image.ai_prompt, item.image.raw_metadata, item.source_label, item.source_evidence_json, item.generation_run_id],
        )
        .map_err(|e| e.to_string())?;

        let original = resolve_original(bundle_dir, item)?;
        let file_id = format!("exchange_{}", item.image.id);
        conn.execute(
            "INSERT OR REPLACE INTO image_files (id, image_id, path, last_seen_at, missing_at, last_seen_size, last_seen_mtime)
             VALUES (?1, ?2, ?3, datetime('now'), NULL, ?4, NULL)",
            params![
                file_id,
                item.image.id,
                original.to_string_lossy(),
                sql_u64(item.image.file_size).map_err(|e| e.to_string())?
            ],
        )
        .map_err(|e| e.to_string())?;

        if let Some(selection) = &item.selection {
            conn.execute(
                "INSERT INTO selections (image_id, project_id, star_rating, color_label, decision)
                 VALUES (?1, '__global__', ?2, ?3, ?4)
                 ON CONFLICT(image_id, project_id)
                 DO UPDATE SET star_rating = ?2, color_label = ?3, decision = ?4",
                params![
                    item.image.id,
                    selection.star_rating,
                    selection.color_label,
                    selection.decision
                ],
            )
            .map_err(|e| e.to_string())?;
        }
    }

    for collection in &manifest.collections {
        conn.execute(
            "INSERT OR REPLACE INTO projects (id, name, description, collection_type, created_at)
             VALUES (?1, ?2, ?3, 'manual', datetime('now'))",
            params![collection.id, collection.name, collection.description],
        )
        .map_err(|e| e.to_string())?;
        conn.execute(
            "DELETE FROM collection_items WHERE collection_id = ?1",
            params![collection.id],
        )
        .map_err(|e| e.to_string())?;
        for item in &collection.items {
            conn.execute(
                "INSERT OR IGNORE INTO collection_items (collection_id, image_id, position)
                 VALUES (?1, ?2, ?3)",
                params![collection.id, item.image_id, item.position],
            )
            .map_err(|e| e.to_string())?;
        }
    }

    for smart in &manifest.smart_collections {
        conn.execute(
            "INSERT OR REPLACE INTO projects (id, name, description, collection_type, filter_json, nl_query, is_preset, sort_order, created_at)
             VALUES (?1, ?2, ?3, 'smart', ?4, ?5, ?6, ?7, ?8)",
            params![
                smart.id,
                smart.name,
                smart.description,
                smart.filter_json,
                smart.nl_query,
                smart.is_preset as i32,
                smart.sort_order,
                smart.created_at
            ],
        )
        .map_err(|e| e.to_string())?;
    }

    Ok(ExchangeImportResult {
        imported_images: manifest.images.len(),
        imported_collections: manifest.collections.len(),
        imported_smart_collections: manifest.smart_collections.len(),
        imported_generation_runs: manifest.generation_runs.len(),
    })
}

fn read_manifest(bundle_dir: &Path) -> Result<CullExchangeManifest, String> {
    let path = bundle_dir.join("manifest.json");
    let json = fs::read_to_string(&path)
        .map_err(|e| format!("Failed to read '{}': {}", path.display(), e))?;
    serde_json::from_str(&json).map_err(|e| format!("Invalid manifest.json: {}", e))
}

fn resolve_original(bundle_dir: &Path, image: &ExchangeImage) -> Result<PathBuf, String> {
    if let Some(bundle_path) = &image.bundle_path {
        Ok(bundle_dir.join(bundle_path))
    } else {
        Ok(PathBuf::from(&image.original_path))
    }
}

fn xmp_for_image(image: &ExchangeImage, run: Option<&GenerationRun>) -> XmpMetadata {
    XmpMetadata {
        rating: image.selection.as_ref().and_then(|s| s.star_rating),
        color_label: image.selection.as_ref().and_then(|s| s.color_label.clone()),
        keywords: image.source_label.iter().cloned().collect(),
        title: None,
        description: image.image.ai_prompt.clone(),
        creator: None,
        copyright: None,
        prompt: run
            .and_then(|r| r.prompt.clone())
            .or_else(|| image.image.ai_prompt.clone()),
        provider: run.and_then(|r| r.provider.clone()),
        model: run.and_then(|r| r.model.clone()),
        seed: run.and_then(|r| r.seed.clone()),
    }
}

fn load_generation_runs(
    conn: &rusqlite::Connection,
    selected_ids: &BTreeSet<String>,
) -> rusqlite::Result<Vec<GenerationRun>> {
    if selected_ids.is_empty() {
        return Ok(vec![]);
    }
    let mut stmt = conn.prepare(
        "SELECT DISTINCT g.id, g.prompt, g.negative_prompt, g.provider, g.model, g.settings_json, g.seed, g.parent_run_id, g.source_type, g.source_path, g.raw_metadata_json, g.created_at, g.imported_at
         FROM generation_runs g
         JOIN images i ON i.generation_run_id = g.id
         ORDER BY g.id ASC",
    )?;
    let runs = stmt
        .query_map([], |row| {
            Ok(GenerationRun {
                id: row.get(0)?,
                prompt: row.get(1)?,
                negative_prompt: row.get(2)?,
                provider: row.get(3)?,
                model: row.get(4)?,
                settings_json: row.get(5)?,
                seed: row.get(6)?,
                parent_run_id: row.get(7)?,
                source_type: row.get(8)?,
                source_path: row.get(9)?,
                raw_metadata_json: row.get(10)?,
                created_at: row.get(11)?,
                imported_at: row.get(12)?,
            })
        })?
        .filter_map(|row| row.ok())
        .collect();
    Ok(runs)
}

fn load_collections(
    conn: &rusqlite::Connection,
    selected_ids: &BTreeSet<String>,
) -> rusqlite::Result<Vec<ExchangeCollection>> {
    let mut stmt = conn.prepare(
        "SELECT id, name, description FROM projects
         WHERE (collection_type IS NULL OR collection_type = 'manual')
         ORDER BY id ASC",
    )?;
    let mut collections = vec![];
    let rows = stmt.query_map([], |row| {
        Ok((
            row.get::<_, String>(0)?,
            row.get::<_, String>(1)?,
            row.get::<_, Option<String>>(2)?,
        ))
    })?;
    for row in rows {
        let (id, name, description) = row?;
        let mut item_stmt = conn.prepare(
            "SELECT image_id, position FROM collection_items
             WHERE collection_id = ?1 ORDER BY position ASC",
        )?;
        let items = item_stmt
            .query_map(params![id], |row| {
                Ok(ExchangeCollectionItem {
                    image_id: row.get(0)?,
                    position: row.get(1)?,
                })
            })?
            .filter_map(|row| row.ok())
            .filter(|item| selected_ids.contains(&item.image_id))
            .collect::<Vec<_>>();
        if !items.is_empty() {
            collections.push(ExchangeCollection {
                id,
                name,
                description,
                items,
            });
        }
    }
    Ok(collections)
}

fn load_smart_collections(conn: &rusqlite::Connection) -> rusqlite::Result<Vec<SmartCollection>> {
    let mut stmt = conn.prepare(
        "SELECT id, name, description, collection_type, filter_json, nl_query,
                is_preset, sort_order, created_at
         FROM projects
         WHERE collection_type = 'smart'
         ORDER BY sort_order ASC, id ASC",
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
    rows.collect::<rusqlite::Result<Vec<_>>>()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db_core::models::GenerationRun;

    fn db() -> Database {
        Database::open(Path::new(":memory:")).unwrap()
    }

    fn insert_image(db: &Database, id: &str, path: &Path) {
        let conn = db.conn.lock();
        conn.execute(
            "INSERT INTO images (id, sha256_hash, width, height, format, file_size, created_at, imported_at, ai_prompt, source_label, source_evidence_json)
             VALUES (?1, ?2, 640, 480, 'png', 12, '2026-01-01', '2026-01-02', 'prompt text', 'midjourney', '{\"layer\":\"filename\"}')",
            params![id, format!("hash_{}", id)],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO image_files (id, image_id, path, last_seen_at, last_seen_size)
             VALUES (?1, ?2, ?3, '2026-01-02', 12)",
            params![format!("file_{}", id), id, path.to_string_lossy()],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO selections (image_id, project_id, star_rating, color_label, decision)
             VALUES (?1, '__global__', 5, 'green', 'accept')",
            params![id],
        )
        .unwrap();
    }

    #[test]
    fn manifest_preserves_curation_and_generation_metadata() {
        let db = db();
        let tmp = tempfile::tempdir().unwrap();
        let original = tmp.path().join("a.png");
        fs::write(&original, b"fake").unwrap();
        insert_image(&db, "img_a", &original);
        let run = GenerationRun {
            id: "run_a".to_string(),
            prompt: Some("prompt text".to_string()),
            negative_prompt: None,
            provider: Some("openai".to_string()),
            model: Some("gpt-image-1".to_string()),
            settings_json: "{\"quality\":\"high\"}".to_string(),
            seed: Some("42".to_string()),
            parent_run_id: None,
            source_type: "sidecar".to_string(),
            source_path: Some("a.json".to_string()),
            raw_metadata_json: Some("{\"provider\":\"openai\"}".to_string()),
            created_at: Some("2026-01-01".to_string()),
            imported_at: "2026-01-02".to_string(),
        };
        db.insert_generation_run(&run).unwrap();
        db.link_image_to_run("img_a", "run_a").unwrap();
        let col = db.create_collection("Portfolio").unwrap();
        db.add_to_collection(&col, &["img_a"]).unwrap();
        db.create_smart_collection(
            "Picks",
            r#"{"type":"rule","field":"decision","op":"eq","value":"accept"}"#,
            Some("picks"),
            false,
        )
        .unwrap();

        let manifest = build_manifest(&db, None, OriginalMode::ReferenceOriginals).unwrap();
        assert_eq!(manifest.format, FORMAT);
        assert_eq!(manifest.version, VERSION);
        assert_eq!(manifest.images.len(), 1);
        assert_eq!(
            manifest.images[0].selection.as_ref().unwrap().decision,
            "accept"
        );
        assert_eq!(manifest.collections[0].items[0].image_id, "img_a");
        assert!(manifest
            .smart_collections
            .iter()
            .any(|smart| smart.name == "Picks"));
        assert_eq!(
            manifest.generation_runs[0].provider.as_deref(),
            Some("openai")
        );
    }

    #[test]
    fn export_writes_manifest_sidecars_and_copied_originals() {
        let db = db();
        let tmp = tempfile::tempdir().unwrap();
        let original = tmp.path().join("a.png");
        fs::write(&original, b"fake image").unwrap();
        insert_image(&db, "img_a", &original);
        let out = tempfile::tempdir().unwrap();

        let manifest_path = export_bundle(
            &db,
            ExchangeExportOptions {
                target_dir: out.path().to_string_lossy().to_string(),
                image_ids: None,
                copy_originals: true,
                include_xmp: true,
            },
        )
        .unwrap();

        assert!(Path::new(&manifest_path).exists());
        assert!(out.path().join("images/img_a.png").exists());
        assert!(out.path().join("metadata/img_a.cull.json").exists());
        let xmp = fs::read_to_string(out.path().join("sidecars/img_a.xmp")).unwrap();
        assert!(xmp.contains("<xmp:Rating>5</xmp:Rating>"));
        let preview = preview_import(out.path());
        assert!(preview.valid, "{:?}", preview.errors);
        assert_eq!(preview.image_count, 1);
    }

    #[test]
    fn import_round_trips_without_deleting_existing_data() {
        let source = db();
        let tmp = tempfile::tempdir().unwrap();
        let original = tmp.path().join("a.png");
        fs::write(&original, b"fake image").unwrap();
        insert_image(&source, "img_a", &original);
        let out = tempfile::tempdir().unwrap();
        export_bundle(
            &source,
            ExchangeExportOptions {
                target_dir: out.path().to_string_lossy().to_string(),
                image_ids: None,
                copy_originals: true,
                include_xmp: true,
            },
        )
        .unwrap();

        let dest = db();
        let result = import_bundle(&dest, out.path()).unwrap();
        assert_eq!(result.imported_images, 1);
        let imported = dest.get_images_by_ids(&["img_a"]).unwrap();
        assert_eq!(imported[0].selection.as_ref().unwrap().star_rating, Some(5));
        assert_eq!(imported[0].source_label.as_deref(), Some("midjourney"));
    }
}
