use crate::db_core::db::Database;
use crate::db_core::models::{ImageWithFile, UndoRecord};
use crate::services::library::enrich_thumbnails;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct HistoryTarget {
    pub kind: String,
    pub display_name: String,
    pub context: Option<String>,
    pub unavailable: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct HistoryImagePreview {
    pub image_id: String,
    pub display_name: String,
    pub thumbnail_path: Option<String>,
    pub missing: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UndoHistoryEntry {
    pub record: UndoRecord,
    pub action_title: String,
    pub target: HistoryTarget,
    pub change_summary: Option<String>,
    pub previews: Vec<HistoryImagePreview>,
    pub affected_count: u32,
    pub can_undo: bool,
}

fn affected_ids(record: &UndoRecord) -> Vec<String> {
    record
        .affected_image_ids
        .as_deref()
        .unwrap_or_default()
        .split(',')
        .map(str::trim)
        .filter(|id| !id.is_empty())
        .map(str::to_string)
        .collect()
}

fn filename(path: &str) -> String {
    Path::new(path)
        .file_name()
        .and_then(|name| name.to_str())
        .filter(|name| !name.is_empty())
        .unwrap_or("Unavailable image")
        .to_string()
}

fn action_title(action_type: &str) -> String {
    match action_type {
        "set_rating" => "Set rating".to_string(),
        "set_decision" => "Set decision".to_string(),
        "trash_image" => "Move to Trash".to_string(),
        other => other.replace('_', " "),
    }
}

fn parsed_json(raw: &str) -> Option<serde_json::Value> {
    serde_json::from_str(raw).ok()
}

fn string_field(value: Option<&serde_json::Value>, field: &str) -> Option<String> {
    value?.get(field)?.as_str().map(str::to_string)
}

fn decision_label(value: String) -> String {
    match value.as_str() {
        "accept" | "selected" => "accepted".to_string(),
        "reject" | "rejected" => "rejected".to_string(),
        "none" => "undecided".to_string(),
        _ => value,
    }
}

fn rating_field(value: Option<&serde_json::Value>) -> Option<String> {
    value?
        .get("rating")?
        .as_u64()
        .map(|rating| rating.to_string())
}

fn change_summary(record: &UndoRecord) -> Option<String> {
    let before = parsed_json(&record.before_json);
    let after = parsed_json(&record.after_json);
    match record.action_type.as_str() {
        "set_rating" => match (rating_field(before.as_ref()), rating_field(after.as_ref())) {
            (Some(before), Some(after)) => Some(format!("Rating: {before} → {after}")),
            _ => Some("Rating: Unknown previous value".to_string()),
        },
        "set_decision" => match (
            string_field(before.as_ref(), "decision"),
            string_field(after.as_ref(), "decision"),
        ) {
            (Some(before), Some(after)) => Some(format!(
                "Decision: {} → {}",
                decision_label(before),
                decision_label(after)
            )),
            _ => Some("Decision: Unknown previous value".to_string()),
        },
        "trash_image" => Some("Moved to Trash".to_string()),
        _ => None,
    }
}

fn preview_for(id: &str, images: &HashMap<String, ImageWithFile>) -> HistoryImagePreview {
    if let Some(image) = images.get(id) {
        return HistoryImagePreview {
            image_id: id.to_string(),
            display_name: filename(&image.path),
            thumbnail_path: image.thumbnail_path.clone(),
            missing: false,
        };
    }
    HistoryImagePreview {
        image_id: id.to_string(),
        display_name: "Unavailable image".to_string(),
        thumbnail_path: None,
        missing: true,
    }
}

fn target_for(record: &UndoRecord, previews: &[HistoryImagePreview]) -> HistoryTarget {
    if record.action_type == "trash_image" {
        let before = parsed_json(&record.before_json);
        let path = string_field(before.as_ref(), "path").or_else(|| {
            parsed_json(&record.after_json)
                .as_ref()
                .and_then(|value| string_field(Some(value), "path"))
        });
        return HistoryTarget {
            kind: "image".to_string(),
            display_name: path.as_deref().map(filename).unwrap_or_else(|| {
                previews
                    .first()
                    .map(|preview| preview.display_name.clone())
                    .unwrap_or_else(|| "Unavailable image".to_string())
            }),
            context: Some("Moved to Trash".to_string()),
            unavailable: previews
                .first()
                .map(|preview| preview.missing)
                .unwrap_or(true),
        };
    }
    let first = previews.first();
    HistoryTarget {
        kind: "image".to_string(),
        display_name: first
            .map(|preview| preview.display_name.clone())
            .unwrap_or_else(|| "Unavailable image".to_string()),
        context: (previews.len() > 1).then(|| format!("{} images affected", previews.len())),
        unavailable: first.map(|preview| preview.missing).unwrap_or(true),
    }
}

pub fn enrich_undo_history(
    db: &Database,
    app_data_dir: &Path,
    limit: u32,
) -> Result<Vec<UndoHistoryEntry>, String> {
    let records = db
        .list_undo_records(limit)
        .map_err(|error| error.to_string())?;
    let all_ids: Vec<String> = records.iter().flat_map(affected_ids).collect();
    let id_refs: Vec<&str> = all_ids.iter().map(String::as_str).collect();
    let mut image_rows = db
        .get_images_by_ids(&id_refs)
        .map_err(|error| error.to_string())?;
    enrich_thumbnails(&mut image_rows, &PathBuf::from(app_data_dir));
    let images: HashMap<String, ImageWithFile> = image_rows
        .into_iter()
        .map(|image| (image.image.id.clone(), image))
        .collect();

    Ok(records
        .into_iter()
        .map(|record| {
            let ids = affected_ids(&record);
            let previews: Vec<HistoryImagePreview> =
                ids.iter().map(|id| preview_for(id, &images)).collect();
            let target = target_for(&record, &previews);
            UndoHistoryEntry {
                action_title: action_title(&record.action_type),
                change_summary: change_summary(&record),
                affected_count: ids.len() as u32,
                target,
                previews,
                record,
                can_undo: true,
            }
        })
        .collect())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db_core::db::Database;
    use crate::db_core::models::{Image, ImageFile};
    use crate::services::undo::ActionManager;

    fn insert_image(db: &Database, image_id: &str, path: &std::path::Path) {
        let now = "2026-07-10T12:00:00Z".to_string();
        db.insert_image(&Image {
            id: image_id.to_string(),
            sha256_hash: format!("hash-{image_id}"),
            width: 100,
            height: 100,
            format: "jpg".to_string(),
            file_size: 10,
            created_at: now.clone(),
            imported_at: now.clone(),
            ai_prompt: None,
            raw_metadata: None,
        })
        .unwrap();
        db.insert_image_file(&ImageFile {
            id: format!("file-{image_id}"),
            image_id: image_id.to_string(),
            path: path.to_string_lossy().to_string(),
            last_seen_at: now,
            missing_at: None,
            last_seen_size: Some(10),
            last_seen_mtime: None,
        })
        .unwrap();
    }

    #[test]
    fn enriches_image_action_with_filename_preview_and_change() {
        let dir = tempfile::tempdir().unwrap();
        let db = Database::open(&dir.path().join("history.db")).unwrap();
        let image_path = dir.path().join("portrait.jpg");
        std::fs::write(&image_path, b"image").unwrap();
        insert_image(&db, "img-1", &image_path);
        let manager = ActionManager::new();
        manager
            .record_action(
                &db,
                "set_decision",
                "Set decision to accept".to_string(),
                serde_json::json!({"image_id":"img-1","decision":"undecided"}).to_string(),
                serde_json::json!({"image_id":"img-1","decision":"accept"}).to_string(),
                "img-1".to_string(),
                false,
            )
            .unwrap();

        let entries = enrich_undo_history(&db, dir.path(), 20).unwrap();

        assert_eq!(entries[0].action_title, "Set decision");
        assert_eq!(entries[0].target.display_name, "portrait.jpg");
        assert_eq!(
            entries[0].change_summary.as_deref(),
            Some("Decision: undecided → accepted")
        );
        assert_eq!(entries[0].affected_count, 1);
        assert_eq!(entries[0].previews[0].image_id, "img-1");
    }

    #[test]
    fn enriches_multi_image_and_missing_legacy_records() {
        let dir = tempfile::tempdir().unwrap();
        let db = Database::open(&dir.path().join("history.db")).unwrap();
        let image_path = dir.path().join("known.jpg");
        std::fs::write(&image_path, b"image").unwrap();
        insert_image(&db, "known-image", &image_path);
        let manager = ActionManager::new();
        manager
            .record_action(
                &db,
                "set_rating",
                "Set rating".to_string(),
                "not-json".to_string(),
                "also-not-json".to_string(),
                "known-image,missing-image".to_string(),
                false,
            )
            .unwrap();

        let entries = enrich_undo_history(&db, dir.path(), 20).unwrap();

        assert_eq!(entries[0].affected_count, 2);
        assert_eq!(entries[0].previews.len(), 2);
        assert_eq!(entries[0].previews[0].display_name, "known.jpg");
        assert_eq!(entries[0].previews[1].display_name, "Unavailable image");
        assert!(entries[0].previews[1].missing);
        assert_eq!(
            entries[0].change_summary.as_deref(),
            Some("Rating: Unknown previous value")
        );
    }

    #[test]
    fn trash_action_uses_filename_instead_of_full_path() {
        let dir = tempfile::tempdir().unwrap();
        let db = Database::open(&dir.path().join("history.db")).unwrap();
        let manager = ActionManager::new();
        manager
            .record_action(
                &db,
                "trash_image",
                "Trash private.jpg".to_string(),
                serde_json::json!({"path":"/Users/test/private/folder/private.jpg","trashed":false}).to_string(),
                serde_json::json!({"path":"/Users/test/private/folder/private.jpg","trashed":true}).to_string(),
                "missing-image".to_string(),
                true,
            )
            .unwrap();

        let entries = enrich_undo_history(&db, dir.path(), 20).unwrap();

        assert_eq!(entries[0].target.display_name, "private.jpg");
        assert_eq!(entries[0].target.context.as_deref(), Some("Moved to Trash"));
        assert!(!entries[0].target.display_name.contains("/Users/"));
    }
}
