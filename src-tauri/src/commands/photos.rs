use crate::apple_photos::{
    self, PhotosAlbum, PhotosAsset, PhotosAuthorizationStatus, PhotosPage, SystemPhotosCatalog,
};

const DEFAULT_PAGE_LIMIT: u32 = 50;

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct PhotosCommandError {
    pub code: &'static str,
    pub message: String,
}

impl From<crate::apple_photos::PhotosError> for PhotosCommandError {
    fn from(error: crate::apple_photos::PhotosError) -> Self {
        let code = match &error {
            crate::apple_photos::PhotosError::Unsupported => "unsupported_platform",
            crate::apple_photos::PhotosError::PermissionDenied => "permission_denied",
            crate::apple_photos::PhotosError::InvalidAlbum(_) => "invalid_album",
            crate::apple_photos::PhotosError::Native(_) => "native_error",
        };
        Self {
            code,
            message: error.to_string(),
        }
    }
}

async fn blocking<T, F>(operation: F) -> Result<T, PhotosCommandError>
where
    T: Send + 'static,
    F: FnOnce() -> Result<T, crate::apple_photos::PhotosError> + Send + 'static,
{
    tauri::async_runtime::spawn_blocking(operation)
        .await
        .map_err(|error| PhotosCommandError {
            code: "worker_failed",
            message: format!("Apple Photos worker failed: {error}"),
        })?
        .map_err(PhotosCommandError::from)
}

#[tauri::command]
pub async fn photos_authorization_status() -> Result<PhotosAuthorizationStatus, PhotosCommandError>
{
    blocking(|| apple_photos::authorization_status(&SystemPhotosCatalog)).await
}

#[tauri::command]
pub async fn photos_request_authorization() -> Result<PhotosAuthorizationStatus, PhotosCommandError>
{
    blocking(|| apple_photos::request_authorization(&SystemPhotosCatalog)).await
}

#[tauri::command]
pub async fn photos_list_albums(
    offset: Option<u32>,
    limit: Option<u32>,
) -> Result<PhotosPage<PhotosAlbum>, PhotosCommandError> {
    blocking(move || {
        apple_photos::list_albums(
            &SystemPhotosCatalog,
            offset.unwrap_or(0),
            limit.unwrap_or(DEFAULT_PAGE_LIMIT),
        )
    })
    .await
}

#[tauri::command]
pub async fn photos_list_assets(
    album_id: Option<String>,
    offset: Option<u32>,
    limit: Option<u32>,
) -> Result<PhotosPage<PhotosAsset>, PhotosCommandError> {
    blocking(move || {
        apple_photos::list_assets(
            &SystemPhotosCatalog,
            album_id.as_deref(),
            offset.unwrap_or(0),
            limit.unwrap_or(DEFAULT_PAGE_LIMIT),
        )
    })
    .await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unsupported_platform_error_is_machine_readable_and_frontend_consumable() {
        let error = PhotosCommandError::from(crate::apple_photos::PhotosError::Unsupported);
        let json = serde_json::to_value(error).unwrap();
        assert_eq!(json["code"], "unsupported_platform");
        assert_eq!(
            json["message"],
            "Apple Photos is unsupported on this platform"
        );
    }
}
