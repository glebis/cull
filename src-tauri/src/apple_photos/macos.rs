//! The sole unsafe/native boundary for the read-only PhotoKit catalog.

use super::{
    PhotosAlbum, PhotosAlbumKind, PhotosAsset, PhotosAuthorizationStatus, PhotosError, PhotosPage,
};
use block2::RcBlock;
use objc2::rc::{autoreleasepool, Retained};
use objc2_foundation::{NSArray, NSDate, NSPredicate, NSSortDescriptor, NSString};
use objc2_photos::{
    PHAccessLevel, PHAsset, PHAssetCollection, PHAssetCollectionSubtype, PHAssetCollectionType,
    PHAssetMediaType, PHAssetResource, PHAuthorizationStatus, PHFetchOptions, PHPhotoLibrary,
};

static PHOTO_KIT_OPERATION: parking_lot::Mutex<()> = parking_lot::Mutex::new(());

fn catch_native<T>(
    operation: impl FnOnce() -> Result<T, PhotosError> + std::panic::UnwindSafe,
) -> Result<T, PhotosError> {
    let _operation_guard = PHOTO_KIT_OPERATION.lock();
    autoreleasepool(|_| match objc2::exception::catch(operation) {
        Ok(result) => result,
        Err(Some(exception)) => Err(PhotosError::Native(format!(
            "PhotoKit raised a native exception: {exception}"
        ))),
        Err(None) => Err(PhotosError::Native(
            "PhotoKit raised an unknown native exception".to_string(),
        )),
    })
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
        output.push(PhotosAlbum { id, title, kind });
    }
}

pub(super) fn list_assets_page(
    album_id: Option<&str>,
    offset: u32,
    limit: u32,
) -> Result<PhotosPage<PhotosAsset>, PhotosError> {
    require_read_access()?;
    catch_native(|| unsafe {
        // SAFETY: All PhotoKit objects stay within this serialized autorelease-pool
        // scope. Fetching metadata does not request bytes or permit an iCloud download.
        let options = asset_fetch_options();

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
            if asset.mediaType() != PHAssetMediaType::Image {
                continue;
            }
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
            has_more: end < result_count,
        })
    })
}

unsafe fn asset_fetch_options() -> Retained<PHFetchOptions> {
    let options = PHFetchOptions::new();
    let creation_key = NSString::from_str("creationDate");
    let creation_sort =
        NSSortDescriptor::sortDescriptorWithKey_ascending(Some(&creation_key), false);
    // PhotoKit only accepts a restricted set of PHAsset sort keys. In particular,
    // localIdentifier raises an Objective-C "Unsupported sort descriptor" exception.
    let descriptors = NSArray::from_retained_slice(&[creation_sort]);
    options.setSortDescriptors(Some(&descriptors));
    let image_predicate_format = NSString::from_str("mediaType == 1");
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
        let options = unsafe { asset_fetch_options() };
        let descriptors = unsafe { options.sortDescriptors() }.unwrap();
        let keys: Vec<String> = descriptors
            .iter()
            .filter_map(|descriptor| descriptor.key())
            .map(|key| key.to_string())
            .collect();

        assert_eq!(keys, ["creationDate"]);
    }
}
