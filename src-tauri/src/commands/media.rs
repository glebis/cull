use crate::db_core::models::{MediaAsset, MediaFile, PdfPage};
use crate::AppState;
use tauri::State;

#[tauri::command]
pub async fn list_media_assets(
    state: State<'_, AppState>,
    media_type: Option<String>,
    limit: u32,
    offset: u32,
) -> Result<Vec<MediaAsset>, String> {
    state
        .db
        .list_media_assets(media_type.as_deref(), limit, offset)
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn get_media_asset(
    state: State<'_, AppState>,
    media_asset_id: String,
) -> Result<Option<MediaAsset>, String> {
    state
        .db
        .media_asset(&media_asset_id)
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn get_media_asset_for_image(
    state: State<'_, AppState>,
    image_id: String,
) -> Result<Option<MediaAsset>, String> {
    state
        .db
        .media_asset_for_image(&image_id)
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn list_media_files(
    state: State<'_, AppState>,
    media_asset_id: String,
) -> Result<Vec<MediaFile>, String> {
    state
        .db
        .list_media_files(&media_asset_id)
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn list_pdf_pages(
    state: State<'_, AppState>,
    media_asset_id: String,
) -> Result<Vec<PdfPage>, String> {
    state
        .db
        .list_pdf_pages(&media_asset_id)
        .map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use crate::db_core::db::Database;
    use crate::db_core::models::Image;
    use tempfile::tempdir;

    #[test]
    fn media_queries_roundtrip() {
        let dir = tempdir().unwrap();
        let db = Database::open(&dir.path().join("media.db")).unwrap();

        db.insert_image(&Image {
            id: "image-1".to_string(),
            sha256_hash: "sha256".to_string(),
            width: 640,
            height: 480,
            format: "png".to_string(),
            file_size: 42,
            created_at: "2026-01-01T00:00:00Z".to_string(),
            imported_at: "2026-01-01T00:00:00Z".to_string(),
            ai_prompt: None,
            raw_metadata: None,
        })
        .unwrap();

        let media_asset_id = "ma_image-1";
        db.insert_media_asset(&crate::db_core::models::MediaAsset {
            id: media_asset_id.to_string(),
            media_type: "image".to_string(),
            primary_image_id: "image-1".to_string(),
            sha256_hash: "sha256".to_string(),
            format: "png".to_string(),
            file_size: 42,
            page_count: None,
            title: None,
            created_at: "2026-01-01T00:00:00Z".to_string(),
            imported_at: "2026-01-01T00:00:00Z".to_string(),
        })
        .unwrap();

        db.insert_media_file(&crate::db_core::models::MediaFile {
            id: "mf_image-1".to_string(),
            media_asset_id: media_asset_id.to_string(),
            path: "/tmp/image.png".to_string(),
            last_seen_at: "2026-01-01T00:00:00Z".to_string(),
            missing_at: None,
            last_seen_size: None,
            last_seen_mtime: None,
        })
        .unwrap();

        db.upsert_pdf_page(&crate::db_core::models::PdfPage {
            id: "pp_image-1_0".to_string(),
            media_asset_id: media_asset_id.to_string(),
            page_index: 0,
            width_points: Some(612.0),
            height_points: Some(792.0),
            thumbnail_path: None,
            preview_path: None,
            extracted_text: None,
            text_extracted_at: None,
        })
        .unwrap();

        assert_eq!(db.list_media_assets(None, 10, 0).unwrap().len(), 1);
        assert_eq!(
            db.media_asset(media_asset_id).unwrap().unwrap().media_type,
            "image"
        );
        assert_eq!(
            db.media_asset_for_image("image-1").unwrap().unwrap().id,
            media_asset_id
        );
        assert_eq!(db.list_media_files(media_asset_id).unwrap().len(), 1);
        assert_eq!(db.list_pdf_pages(media_asset_id).unwrap().len(), 1);
    }
}
