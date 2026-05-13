use crate::AppState;
use tauri::{AppHandle, Emitter, State};

#[tauri::command]
pub async fn backfill_raw_previews(
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<u32, String> {
    let db = &state.db;
    let app_data_dir = &state.app_data_dir;

    let raw_exts: Vec<String> = crate::extensions::RAW_EXTENSIONS
        .iter()
        .map(|e| format!("'{}'", e))
        .collect();
    let in_clause = raw_exts.join(",");

    let images: Vec<(String, String)> = {
        let conn = db.conn.lock();
        let query = format!(
            "SELECT i.id, f.path FROM images i
             JOIN image_files f ON f.image_id = i.id AND f.missing_at IS NULL
             WHERE i.format IN ({}) AND i.width = 0
             GROUP BY i.id",
            in_clause
        );
        let mut stmt = conn.prepare(&query).map_err(|e| e.to_string())?;
        let rows = stmt
            .query_map([], |row| {
                Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
            })
            .map_err(|e| e.to_string())?;
        rows.filter_map(|r| r.ok()).collect()
    };

    let total = images.len() as u32;
    let mut backfilled = 0u32;

    for (i, (image_id, path_str)) in images.iter().enumerate() {
        let path = std::path::Path::new(path_str);
        if !path.exists() {
            continue;
        }

        match crate::raw::decode_raw_preview(path) {
            Ok(preview) => {
                let w = preview.image.width();
                let h = preview.image.height();
                let _ = crate::db_core::thumbnails::generate_thumbnail_from_image(
                    &preview.image,
                    app_data_dir,
                    image_id,
                );
                {
                    let conn = db.conn.lock();
                    let _ = conn.execute(
                        "UPDATE images SET width = ?1, height = ?2 WHERE id = ?3",
                        rusqlite::params![w, h, image_id],
                    );
                }
                if let Ok(meta_json) = serde_json::to_string(&preview.metadata) {
                    let _ = db.update_raw_metadata(image_id, &meta_json);
                }
                backfilled += 1;
            }
            Err(e) => {
                eprintln!("[backfill] Failed for {}: {}", path_str, e);
            }
        }

        let _ = app.emit(
            "backfill-progress",
            serde_json::json!({
                "current": i + 1, "total": total
            }),
        );
    }

    Ok(backfilled)
}
