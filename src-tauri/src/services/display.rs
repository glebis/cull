use crate::commands::deeplink::{emit_open_params, OpenParams};
use crate::services::ServiceError;
use tauri::Emitter;

pub fn show_image(app_handle: &tauri::AppHandle, image_id: &str) -> Result<(), ServiceError> {
    let params = OpenParams {
        view: Some("loupe".to_string()),
        image_id: Some(image_id.to_string()),
        ..OpenParams::default()
    };
    emit_open_params(app_handle, params).map_err(|e| ServiceError::Engine(e.to_string()))
}

pub fn navigate_to_folder(
    app_handle: &tauri::AppHandle,
    folder_path: &str,
) -> Result<(), ServiceError> {
    let params = OpenParams {
        folder: Some(folder_path.to_string()),
        view: Some("grid".to_string()),
        ..OpenParams::default()
    };
    emit_open_params(app_handle, params).map_err(|e| ServiceError::Engine(e.to_string()))
}

pub fn show_collection(
    app_handle: &tauri::AppHandle,
    collection_id: &str,
) -> Result<(), ServiceError> {
    app_handle
        .emit(
            "navigate-collection",
            serde_json::json!({
                "collection_id": collection_id,
            }),
        )
        .map_err(|e| ServiceError::Engine(e.to_string()))
}

#[cfg(test)]
mod tests {
    #[test]
    fn test_show_image_payload_structure() {
        let image_id = "img_abc123";
        let payload = serde_json::json!({
            "path": null,
            "paths": null,
            "folder": null,
            "view": "loupe",
            "image_id": image_id,
        });
        assert_eq!(payload["view"], "loupe");
        assert_eq!(payload["image_id"], "img_abc123");
        assert!(payload["path"].is_null());
        assert!(payload["folder"].is_null());
    }

    #[test]
    fn test_navigate_to_folder_payload_structure() {
        let folder = "/art/midjourney";
        let payload = serde_json::json!({
            "folder": folder,
            "view": "grid",
        });
        assert_eq!(payload["view"], "grid");
        assert_eq!(payload["folder"], "/art/midjourney");
    }

    #[test]
    fn test_show_collection_payload_structure() {
        let collection_id = "col_xyz";
        let payload = serde_json::json!({
            "collection_id": collection_id,
        });
        assert_eq!(payload["collection_id"], "col_xyz");
    }

    #[test]
    fn test_show_image_payload_with_special_chars() {
        let image_id = "img_with spaces & stuff";
        let payload = serde_json::json!({
            "image_id": image_id,
            "view": "loupe",
        });
        assert_eq!(payload["image_id"], "img_with spaces & stuff");
    }

    #[test]
    fn test_folder_payload_with_unicode_path() {
        let folder = "/Users/gleb/фото/природа";
        let payload = serde_json::json!({
            "folder": folder,
            "view": "grid",
        });
        assert_eq!(payload["folder"], "/Users/gleb/фото/природа");
    }
}
