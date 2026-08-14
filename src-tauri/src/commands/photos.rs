use crate::apple_photos::{
    self, PhotosAlbum, PhotosAsset, PhotosAuthorizationStatus, PhotosPage, SystemPhotosCatalog,
};

const DEFAULT_PAGE_LIMIT: u32 = 50;

async fn blocking<T, F>(operation: F) -> Result<T, String>
where
    T: Send + 'static,
    F: FnOnce() -> Result<T, crate::apple_photos::PhotosError> + Send + 'static,
{
    tauri::async_runtime::spawn_blocking(operation)
        .await
        .map_err(|error| format!("Apple Photos worker failed: {error}"))?
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub async fn photos_authorization_status() -> Result<PhotosAuthorizationStatus, String> {
    blocking(|| apple_photos::authorization_status(&SystemPhotosCatalog)).await
}

#[tauri::command]
pub async fn photos_request_authorization() -> Result<PhotosAuthorizationStatus, String> {
    blocking(|| apple_photos::request_authorization(&SystemPhotosCatalog)).await
}

#[tauri::command]
pub async fn photos_list_albums(
    offset: Option<u32>,
    limit: Option<u32>,
) -> Result<PhotosPage<PhotosAlbum>, String> {
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
) -> Result<PhotosPage<PhotosAsset>, String> {
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

#[tauri::command]
pub async fn photos_load_local_preview(
    asset_id: String,
    size: Option<u32>,
) -> Result<Option<String>, String> {
    blocking(move || {
        apple_photos::load_local_preview(&SystemPhotosCatalog, &asset_id, size.unwrap_or(320))
    })
    .await
}

#[cfg(test)]
mod tests {
    #[test]
    fn unsupported_platform_operation_uses_standard_command_error_string() {
        let error = crate::apple_photos::PhotosError::Unsupported.to_string();
        assert_eq!(error, "Apple Photos is unsupported on this platform");
    }
}
