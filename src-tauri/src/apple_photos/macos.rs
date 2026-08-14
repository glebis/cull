//! The sole unsafe/native boundary for the read-only PhotoKit catalog.

use super::{
    PhotosAlbum, PhotosAlbumKind, PhotosAlbumRole, PhotosAsset, PhotosAssetFilter, PhotosAssetSort,
    PhotosAuthorizationStatus, PhotosCurrentResource, PhotosError, PhotosImportError,
    PhotosMaterializedMetadata, PhotosPage,
};
use base64::Engine as _;
use block2::RcBlock;
use objc2::rc::{autoreleasepool, Retained};
use objc2_app_kit::{NSBitmapImageFileType, NSBitmapImageRep, NSImage};
use objc2_foundation::{
    NSArray, NSDate, NSDictionary, NSPredicate, NSSize, NSSortDescriptor, NSString,
};
use objc2_photos::{
    PHAccessLevel, PHAsset, PHAssetCollection, PHAssetCollectionSubtype, PHAssetCollectionType,
    PHAssetMediaType, PHAssetResource, PHAssetResourceType, PHAuthorizationStatus, PHFetchOptions,
    PHImageContentMode, PHImageManager, PHImageRequestOptions, PHImageRequestOptionsDeliveryMode,
    PHImageRequestOptionsResizeMode, PHImageRequestOptionsVersion, PHPhotoLibrary,
};
use std::io::Write;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

static PHOTO_KIT_OPERATION: parking_lot::Mutex<()> = parking_lot::Mutex::new(());

fn catch_native<T>(operation: impl FnOnce() -> Result<T, PhotosError>) -> Result<T, PhotosError> {
    let _operation_guard = PHOTO_KIT_OPERATION.lock();
    autoreleasepool(
        |_| match objc2::exception::catch(std::panic::AssertUnwindSafe(operation)) {
            Ok(result) => result,
            Err(Some(exception)) => Err(PhotosError::Native(format!(
                "PhotoKit raised a native exception: {exception}"
            ))),
            Err(None) => Err(PhotosError::Native(
                "PhotoKit raised an unknown native exception".to_string(),
            )),
        },
    )
}

fn import_native<T>(
    operation: impl FnOnce() -> Result<T, PhotosImportError>,
) -> Result<T, PhotosImportError> {
    let _operation_guard = PHOTO_KIT_OPERATION.lock();
    autoreleasepool(
        |_| match objc2::exception::catch(std::panic::AssertUnwindSafe(operation)) {
            Ok(result) => result,
            Err(Some(exception)) => Err(PhotosImportError::Native(format!(
                "PhotoKit raised a native exception: {exception}"
            ))),
            Err(None) => Err(PhotosImportError::Native(
                "PhotoKit raised an unknown native exception".into(),
            )),
        },
    )
}

fn normalize_status(status: PHAuthorizationStatus) -> PhotosAuthorizationStatus {
    match status {
        PHAuthorizationStatus::NotDetermined => PhotosAuthorizationStatus::NotDetermined,
        PHAuthorizationStatus::Restricted => PhotosAuthorizationStatus::Restricted,
        PHAuthorizationStatus::Denied => PhotosAuthorizationStatus::Denied,
        PHAuthorizationStatus::Limited => PhotosAuthorizationStatus::Limited,
        PHAuthorizationStatus::Authorized => PhotosAuthorizationStatus::Authorized,
        _ => PhotosAuthorizationStatus::Restricted,
    }
}

fn require_read_access() -> Result<(), PhotosError> {
    match authorization_status()? {
        PhotosAuthorizationStatus::Limited | PhotosAuthorizationStatus::Authorized => Ok(()),
        _ => Err(PhotosError::PermissionDenied),
    }
}

pub(super) fn authorization_status() -> Result<PhotosAuthorizationStatus, PhotosError> {
    catch_native(|| {
        // SAFETY: This is a class query with no object lifetime or callback requirements.
        let status =
            unsafe { PHPhotoLibrary::authorizationStatusForAccessLevel(PHAccessLevel::ReadWrite) };
        Ok(normalize_status(status))
    })
}

pub(super) fn request_authorization() -> Result<PhotosAuthorizationStatus, PhotosError> {
    catch_native(|| {
        let (sender, receiver) = std::sync::mpsc::sync_channel(1);
        let handler = RcBlock::new(move |status: PHAuthorizationStatus| {
            let _ = sender.send(normalize_status(status));
        });
        // SAFETY: PhotoKit retains the block for the asynchronous request. RcBlock owns
        // the captured sender, and this function keeps the block alive while waiting.
        unsafe {
            PHPhotoLibrary::requestAuthorizationForAccessLevel_handler(
                PHAccessLevel::ReadWrite,
                &handler,
            )
        };
        receiver
            .recv()
            .map_err(|error| PhotosError::Native(format!("authorization callback failed: {error}")))
    })
}

pub(super) fn list_albums() -> Result<Vec<PhotosAlbum>, PhotosError> {
    require_read_access()?;
    catch_native(|| {
        // SAFETY: Fetch results and every retained object are created, read, and dropped
        // inside this autorelease pool. No PhotoKit object escapes this adapter.
        unsafe {
            let mut albums = Vec::new();
            collect_albums(
                PHAssetCollectionType::Album,
                PhotosAlbumKind::User,
                &mut albums,
            );
            collect_albums(
                PHAssetCollectionType::SmartAlbum,
                PhotosAlbumKind::Smart,
                &mut albums,
            );
            Ok(albums)
        }
    })
}

unsafe fn collect_albums(
    collection_type: PHAssetCollectionType,
    kind: PhotosAlbumKind,
    output: &mut Vec<PhotosAlbum>,
) {
    let result = PHAssetCollection::fetchAssetCollectionsWithType_subtype_options(
        collection_type,
        PHAssetCollectionSubtype::Any,
        None,
    );
    for index in 0..result.count() {
        let album = result.objectAtIndex(index);
        let id = album.localIdentifier().to_string();
        let title = album.localizedTitle().map(|value| value.to_string());
        let role = match album.assetCollectionSubtype() {
            PHAssetCollectionSubtype::SmartAlbumFavorites => Some(PhotosAlbumRole::Favorites),
            PHAssetCollectionSubtype::SmartAlbumScreenshots => Some(PhotosAlbumRole::Screenshots),
            _ => None,
        };
        output.push(PhotosAlbum {
            id,
            title,
            kind,
            role,
        });
    }
}

pub(super) fn list_assets_page(
    album_id: Option<&str>,
    offset: u32,
    limit: u32,
    filter: PhotosAssetFilter,
    sort: PhotosAssetSort,
) -> Result<PhotosPage<PhotosAsset>, PhotosError> {
    require_read_access()?;
    catch_native(|| unsafe {
        // SAFETY: All PhotoKit objects stay within this serialized autorelease-pool
        // scope. Fetching metadata does not request bytes or permit an iCloud download.
        let options = asset_fetch_options(filter, sort);

        let result = if let Some(album_id) = album_id {
            let identifier = NSString::from_str(album_id);
            let identifiers = NSArray::from_retained_slice(&[identifier]);
            let albums = PHAssetCollection::fetchAssetCollectionsWithLocalIdentifiers_options(
                &identifiers,
                None,
            );
            let album = albums
                .firstObject()
                .ok_or_else(|| PhotosError::InvalidAlbum(album_id.to_string()))?;
            PHAsset::fetchAssetsInAssetCollection_options(&album, Some(&options))
        } else {
            PHAsset::fetchAssetsWithMediaType_options(PHAssetMediaType::Image, Some(&options))
        };

        let result_count = result.count();
        let total = u32::try_from(result_count).unwrap_or(u32::MAX);
        let start = usize::try_from(offset)
            .unwrap_or(usize::MAX)
            .min(result_count);
        let end = start.saturating_add(limit as usize).min(result_count);
        let mut assets = Vec::new();
        for index in start..end {
            let asset = result.objectAtIndex(index);
            let resources = PHAssetResource::assetResourcesForAsset(&asset);
            let filename = resources
                .firstObject()
                .map(|resource| resource.originalFilename().to_string());
            assets.push(PhotosAsset {
                id: asset.localIdentifier().to_string(),
                filename,
                created_at: asset.creationDate().as_deref().and_then(date_to_rfc3339),
                modified_at: asset
                    .modificationDate()
                    .as_deref()
                    .and_then(date_to_rfc3339),
                pixel_width: u32::try_from(asset.pixelWidth()).unwrap_or(u32::MAX),
                pixel_height: u32::try_from(asset.pixelHeight()).unwrap_or(u32::MAX),
                media_subtypes: u64::try_from(asset.mediaSubtypes().0).unwrap_or(u64::MAX),
                favorite: asset.isFavorite(),
            });
        }
        Ok(PhotosPage {
            items: assets,
            total,
            offset,
            next_offset: u32::try_from(end).unwrap_or(u32::MAX),
            has_more: end < result_count,
        })
    })
}

pub(super) fn load_local_preview(asset_id: &str, size: u32) -> Result<Option<String>, PhotosError> {
    require_read_access()?;
    catch_native(|| unsafe {
        // SAFETY: The request is synchronous, serialized, and scoped to this
        // autorelease pool. Network access is explicitly disabled so an
        // iCloud-only asset returns no image instead of starting a download.
        let identifier = NSString::from_str(asset_id);
        let identifiers = NSArray::from_retained_slice(&[identifier]);
        let assets = PHAsset::fetchAssetsWithLocalIdentifiers_options(&identifiers, None);
        let Some(asset) = assets.firstObject() else {
            return Ok(None);
        };

        let options = local_preview_options();
        let result = std::sync::Arc::new(std::sync::Mutex::new(None));
        let result_for_handler = result.clone();
        let handler = RcBlock::new(move |image: *mut NSImage, _info: *mut NSDictionary| {
            let data_url = image.as_ref().and_then(|image| encode_preview_png(image));
            if let Ok(mut result) = result_for_handler.lock() {
                *result = Some(data_url);
            }
        });
        PHImageManager::defaultManager()
            .requestImageForAsset_targetSize_contentMode_options_resultHandler(
                &asset,
                NSSize::new(size as f64, size as f64),
                PHImageContentMode::AspectFill,
                Some(&options),
                &handler,
            );

        let preview = result
            .lock()
            .map_err(|_| PhotosError::Native("local preview callback state was poisoned".into()))?
            .take()
            .flatten();
        Ok(preview)
    })
}

pub(super) fn describe_current(asset_id: &str) -> Result<PhotosCurrentResource, PhotosImportError> {
    match authorization_status().map_err(|error| PhotosImportError::Native(error.to_string()))? {
        PhotosAuthorizationStatus::Authorized | PhotosAuthorizationStatus::Limited => {}
        _ => return Err(PhotosImportError::PermissionDenied),
    }
    import_native(|| unsafe {
        let identifier = NSString::from_str(asset_id);
        let identifiers = NSArray::from_retained_slice(&[identifier]);
        let assets = PHAsset::fetchAssetsWithLocalIdentifiers_options(&identifiers, None);
        let asset = assets
            .firstObject()
            .ok_or(PhotosImportError::Inaccessible)?;
        if asset.mediaType() != PHAssetMediaType::Image {
            return Err(PhotosImportError::UnsupportedResource("video".into()));
        }
        let resources = PHAssetResource::assetResourcesForAsset(&asset);
        let selected = resources
            .iter()
            .find(|resource| resource.r#type() == PHAssetResourceType::FullSizePhoto)
            .or_else(|| {
                resources
                    .iter()
                    .find(|resource| resource.r#type() == PHAssetResourceType::Photo)
            })
            .ok_or_else(|| {
                PhotosImportError::UnsupportedResource("no still-image resource".into())
            })?;
        let content_type = selected.uniformTypeIdentifier().to_string();
        let filename =
            normalized_resource_filename(&selected.originalFilename().to_string(), &content_type)?;
        Ok(PhotosCurrentResource {
            asset_id: asset_id.to_string(),
            filename,
            content_type,
            modified_at: asset
                .modificationDate()
                .as_deref()
                .and_then(date_to_rfc3339),
            pixel_width: u32::try_from(asset.pixelWidth()).unwrap_or(u32::MAX),
            pixel_height: u32::try_from(asset.pixelHeight()).unwrap_or(u32::MAX),
        })
    })
}

pub(super) fn materialize_current(
    resource: &PhotosCurrentResource,
    output: &mut std::fs::File,
    cancel: &tokio_util::sync::CancellationToken,
    progress: &mut dyn FnMut(Option<u64>, Option<u64>, Option<f64>),
) -> Result<PhotosMaterializedMetadata, PhotosImportError> {
    import_native(|| unsafe {
        let identifier = NSString::from_str(&resource.asset_id);
        let identifiers = NSArray::from_retained_slice(&[identifier]);
        let assets = PHAsset::fetchAssetsWithLocalIdentifiers_options(&identifiers, None);
        let asset = assets
            .firstObject()
            .ok_or(PhotosImportError::Inaccessible)?;
        let options = PHImageRequestOptions::new();
        options.setVersion(PHImageRequestOptionsVersion::Current);
        options.setNetworkAccessAllowed(true);
        options.setSynchronous(false);

        let progress_value = Arc::new(AtomicU64::new(0));
        let progress_for_block = progress_value.clone();
        let progress_block = RcBlock::new(
            move |value: f64,
                  _error: *mut objc2_foundation::NSError,
                  _stop: std::ptr::NonNull<objc2::runtime::Bool>,
                  _info: *mut NSDictionary| {
                progress_for_block
                    .store((value.clamp(0.0, 1.0) * 10_000.0) as u64, Ordering::Relaxed);
            },
        );
        options.setProgressHandler(&*progress_block as *const _ as *mut _);

        let (sender, receiver) = std::sync::mpsc::sync_channel(1);
        let handler = RcBlock::new(
            move |data: *mut objc2_foundation::NSData,
                  uti: *mut NSString,
                  _orientation,
                  _info: *mut NSDictionary| {
                let result = match (data.as_ref(), uti.as_ref()) {
                    (Some(data), Some(uti)) => Ok((data.to_vec(), uti.to_string())),
                    _ => Err(PhotosImportError::Native(
                        "PhotoKit returned no current image data".into(),
                    )),
                };
                let _ = sender.send(result);
            },
        );
        let manager = PHImageManager::defaultManager();
        let request_id = manager.requestImageDataAndOrientationForAsset_options_resultHandler(
            &asset,
            Some(&options),
            &handler,
        );
        let mut last_progress = u64::MAX;
        let mut cancelled_at = None;
        let result: Result<(Vec<u8>, String), PhotosImportError> = loop {
            let current = progress_value.load(Ordering::Relaxed);
            if current != last_progress {
                progress(None, None, Some(current as f64 / 10_000.0));
                last_progress = current;
            }
            if cancel.is_cancelled() && cancelled_at.is_none() {
                manager.cancelImageRequest(request_id);
                cancelled_at = Some(Instant::now());
            }
            match receiver.recv_timeout(Duration::from_millis(50)) {
                Ok(result) => break result,
                Err(std::sync::mpsc::RecvTimeoutError::Timeout) => {
                    if cancelled_at.is_some_and(|at| at.elapsed() > Duration::from_secs(5)) {
                        break Err(PhotosImportError::Cancelled);
                    }
                }
                Err(error) => {
                    break Err(PhotosImportError::Native(format!(
                        "PhotoKit callback failed: {error}"
                    )))
                }
            }
        };
        let result = resolve_materialization_result(result, cancel.is_cancelled())?;
        if cancel.is_cancelled() {
            return Err(PhotosImportError::Cancelled);
        }
        let (bytes, returned_type) = result;
        let returned_extension = extension_for_uti(&returned_type)
            .ok_or_else(|| PhotosImportError::UnsupportedResource(returned_type.clone()))?;
        output
            .write_all(&bytes)
            .map_err(|error| PhotosImportError::Io(error.to_string()))?;
        progress(
            Some(bytes.len() as u64),
            Some(bytes.len() as u64),
            Some(1.0),
        );
        Ok(PhotosMaterializedMetadata {
            content_type: returned_type,
            extension: returned_extension.to_string(),
        })
    })
}

fn resolve_materialization_result<T>(
    result: Result<T, PhotosImportError>,
    cancelled: bool,
) -> Result<T, PhotosImportError> {
    if cancelled {
        Err(PhotosImportError::Cancelled)
    } else {
        result
    }
}

fn normalized_resource_filename(filename: &str, uti: &str) -> Result<String, PhotosImportError> {
    let extension = extension_for_uti(uti)
        .ok_or_else(|| PhotosImportError::UnsupportedResource(uti.to_string()))?;
    let stem = std::path::Path::new(filename)
        .file_stem()
        .and_then(|value| value.to_str())
        .unwrap_or("current-image");
    Ok(format!("{stem}.{extension}"))
}

fn extension_for_uti(uti: &str) -> Option<&'static str> {
    match uti.to_ascii_lowercase().as_str() {
        "public.jpeg" | "public.jpg" => Some("jpg"),
        "public.png" => Some("png"),
        "public.tiff" => Some("tiff"),
        "public.heic" | "public.heif" => Some("heic"),
        "public.avif" => Some("avif"),
        "com.adobe.raw-image" | "com.adobe.dng" => Some("dng"),
        value if value.contains("canon") && value.contains("cr2") => Some("cr2"),
        value if value.contains("canon") && value.contains("cr3") => Some("cr3"),
        value if value.contains("nikon") && value.contains("raw") => Some("nef"),
        value if value.contains("sony") && value.contains("raw") => Some("arw"),
        _ => None,
    }
}

unsafe fn local_preview_options() -> Retained<PHImageRequestOptions> {
    let options = PHImageRequestOptions::new();
    options.setNetworkAccessAllowed(false);
    options.setSynchronous(true);
    options.setVersion(PHImageRequestOptionsVersion::Current);
    options.setDeliveryMode(PHImageRequestOptionsDeliveryMode::HighQualityFormat);
    options.setResizeMode(PHImageRequestOptionsResizeMode::Exact);
    options
}

unsafe fn encode_preview_png(image: &NSImage) -> Option<String> {
    let tiff = image.TIFFRepresentation()?;
    let bitmap = NSBitmapImageRep::imageRepWithData(&tiff)?;
    let properties = NSDictionary::new();
    let png = bitmap.representationUsingType_properties(NSBitmapImageFileType::PNG, &properties)?;
    let encoded = base64::engine::general_purpose::STANDARD.encode(png.to_vec());
    Some(format!("data:image/png;base64,{encoded}"))
}

unsafe fn asset_fetch_options(
    filter: PhotosAssetFilter,
    sort: PhotosAssetSort,
) -> Retained<PHFetchOptions> {
    let options = PHFetchOptions::new();
    let creation_key = NSString::from_str("creationDate");
    let creation_sort = NSSortDescriptor::sortDescriptorWithKey_ascending(
        Some(&creation_key),
        matches!(sort, PhotosAssetSort::Oldest),
    );
    // PhotoKit only accepts a restricted set of PHAsset sort keys. In particular,
    // localIdentifier raises an Objective-C "Unsupported sort descriptor" exception.
    let descriptors = NSArray::from_retained_slice(&[creation_sort]);
    options.setSortDescriptors(Some(&descriptors));
    let image_predicate_format = NSString::from_str(match filter {
        PhotosAssetFilter::All => "mediaType == 1",
        PhotosAssetFilter::Favorites => "mediaType == 1 AND favorite == YES",
    });
    let image_predicate =
        NSPredicate::predicateWithFormat_argumentArray(&image_predicate_format, None);
    options.setPredicate(Some(&image_predicate));
    options
}

fn date_to_rfc3339(date: &NSDate) -> Option<String> {
    let timestamp = date.timeIntervalSince1970();
    if !timestamp.is_finite() {
        return None;
    }
    let seconds = timestamp.floor() as i64;
    let nanos = ((timestamp - seconds as f64) * 1_000_000_000.0).round() as u32;
    chrono::DateTime::<chrono::Utc>::from_timestamp(seconds, nanos).map(|date| date.to_rfc3339())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn native_exception_becomes_a_recoverable_photos_error() {
        let error = catch_native(|| {
            let values = NSArray::<NSString>::new();
            let _ = values.objectAtIndex(0);
            Ok(())
        })
        .unwrap_err();

        assert!(matches!(error, PhotosError::Native(_)));
    }

    #[test]
    fn asset_fetch_options_exclude_unsupported_identifier_sort() {
        let options =
            unsafe { asset_fetch_options(PhotosAssetFilter::All, PhotosAssetSort::Newest) };
        let descriptors = unsafe { options.sortDescriptors() }.unwrap();
        let keys: Vec<String> = descriptors
            .iter()
            .filter_map(|descriptor| descriptor.key())
            .map(|key| key.to_string())
            .collect();

        assert_eq!(keys, ["creationDate"]);
    }

    #[test]
    fn local_preview_options_are_synchronous_and_never_allow_network_access() {
        let options = unsafe { local_preview_options() };

        assert!(unsafe { options.isSynchronous() });
        assert!(!unsafe { options.isNetworkAccessAllowed() });
        assert_eq!(
            unsafe { options.resizeMode() },
            PHImageRequestOptionsResizeMode::Exact
        );
    }

    #[test]
    fn cancellation_wins_over_a_nil_data_callback_error() {
        let result = resolve_materialization_result::<()>(
            Err(PhotosImportError::Native(
                "PhotoKit returned no current image data".into(),
            )),
            true,
        );

        assert!(matches!(result, Err(PhotosImportError::Cancelled)));
    }
}
