//! The sole unsafe/native boundary for the read-only PhotoKit catalog.

use super::{
    PhotosAlbum, PhotosAlbumKind, PhotosAsset, PhotosAuthorizationStatus, PhotosError, PhotosPage,
};
use block2::RcBlock;
use objc2_foundation::{NSArray, NSDate, NSPredicate, NSSortDescriptor, NSString};
use objc2_photos::{
    PHAccessLevel, PHAsset, PHAssetCollection, PHAssetCollectionSubtype, PHAssetCollectionType,
    PHAssetMediaType, PHAssetResource, PHAuthorizationStatus, PHFetchOptions, PHPhotoLibrary,
};

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
    // SAFETY: This is a class query with no object lifetime or callback requirements.
    let status =
        unsafe { PHPhotoLibrary::authorizationStatusForAccessLevel(PHAccessLevel::ReadWrite) };
    Ok(normalize_status(status))
}

pub(super) fn request_authorization() -> Result<PhotosAuthorizationStatus, PhotosError> {
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
}

pub(super) fn list_albums() -> Result<Vec<PhotosAlbum>, PhotosError> {
    require_read_access()?;
    // SAFETY: Fetch results and every retained object are created, read, and dropped
    // on this worker thread. No PhotoKit object escapes this adapter.
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
    // SAFETY: All PhotoKit objects stay within this worker-thread scope. Fetching
    // asset metadata does not request image bytes or permit an iCloud download.
    unsafe {
        let options = PHFetchOptions::new();
        let creation_key = NSString::from_str("creationDate");
        let creation_sort =
            NSSortDescriptor::sortDescriptorWithKey_ascending(Some(&creation_key), false);
        let descriptors = NSArray::from_retained_slice(&[creation_sort]);
        options.setSortDescriptors(Some(&descriptors));
        let image_predicate_format = NSString::from_str("mediaType == 1");
        let image_predicate =
            NSPredicate::predicateWithFormat_argumentArray(&image_predicate_format, None);
        options.setPredicate(Some(&image_predicate));

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
        // PhotoKit does not guarantee a stable tie order for equal creation dates.
        // Read only lightweight keys across the fetch result, then bound the more
        // expensive PHAssetResource lookup to the requested page.
        let mut ordered_indices = Vec::with_capacity(result_count);
        for index in 0..result_count {
            let asset = result.objectAtIndex(index);
            ordered_indices.push((
                index,
                asset.creationDate().as_deref().and_then(date_to_rfc3339),
                asset.localIdentifier().to_string(),
            ));
        }
        ordered_indices.sort_by(|a, b| match (&a.1, &b.1) {
            (Some(a_date), Some(b_date)) => b_date.cmp(a_date).then_with(|| a.2.cmp(&b.2)),
            (Some(_), None) => std::cmp::Ordering::Less,
            (None, Some(_)) => std::cmp::Ordering::Greater,
            (None, None) => a.2.cmp(&b.2),
        });
        let start = usize::try_from(offset)
            .unwrap_or(usize::MAX)
            .min(ordered_indices.len());
        let end = start
            .saturating_add(limit as usize)
            .min(ordered_indices.len());
        let mut assets = Vec::new();
        for (index, _, _) in &ordered_indices[start..end] {
            let asset = result.objectAtIndex(*index);
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
            has_more: end < ordered_indices.len(),
        })
    }
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
