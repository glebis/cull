use serde::Serialize;
use std::fmt;

#[cfg(target_os = "macos")]
mod macos;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum PhotosAuthorizationStatus {
    Unsupported,
    NotDetermined,
    Restricted,
    Denied,
    Limited,
    Authorized,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct PhotosAlbum {
    pub id: String,
    pub title: Option<String>,
    pub kind: PhotosAlbumKind,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum PhotosAlbumKind {
    User,
    Smart,
}

impl PhotosAlbum {
    fn new(id: impl Into<String>, title: impl Into<String>, kind: PhotosAlbumKind) -> Self {
        Self {
            id: id.into(),
            title: Some(title.into()),
            kind,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct PhotosAsset {
    pub id: String,
    pub filename: Option<String>,
    pub created_at: Option<String>,
    pub modified_at: Option<String>,
    pub pixel_width: u32,
    pub pixel_height: u32,
    pub media_subtypes: u64,
    pub favorite: bool,
}

impl PhotosAsset {
    fn new(id: impl Into<String>, created_at: Option<&str>) -> Self {
        Self {
            id: id.into(),
            filename: None,
            created_at: created_at.map(str::to_owned),
            modified_at: None,
            pixel_width: 0,
            pixel_height: 0,
            media_subtypes: 0,
            favorite: false,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct PhotosPage<T> {
    pub items: Vec<T>,
    pub total: u32,
    pub offset: u32,
    pub has_more: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PhotosError {
    Unsupported,
    PermissionDenied,
    InvalidAlbum(String),
    Native(String),
}

impl fmt::Display for PhotosError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Unsupported => write!(f, "Apple Photos is unsupported on this platform"),
            Self::PermissionDenied => write!(f, "Apple Photos permission is not granted"),
            Self::InvalidAlbum(id) => write!(f, "Apple Photos album not found: {id}"),
            Self::Native(message) => write!(f, "Apple Photos error: {message}"),
        }
    }
}

impl std::error::Error for PhotosError {}

pub trait PhotosCatalog {
    fn authorization_status(&self) -> Result<PhotosAuthorizationStatus, PhotosError>;
    fn request_authorization(&self) -> Result<PhotosAuthorizationStatus, PhotosError>;
    fn list_albums(&self) -> Result<Vec<PhotosAlbum>, PhotosError>;
    fn list_assets_page(
        &self,
        album_id: Option<&str>,
        offset: u32,
        limit: u32,
    ) -> Result<PhotosPage<PhotosAsset>, PhotosError>;
}

pub fn authorization_status(
    catalog: &impl PhotosCatalog,
) -> Result<PhotosAuthorizationStatus, PhotosError> {
    catalog.authorization_status()
}

pub fn request_authorization(
    catalog: &impl PhotosCatalog,
) -> Result<PhotosAuthorizationStatus, PhotosError> {
    catalog.request_authorization()
}

pub fn list_albums(
    catalog: &impl PhotosCatalog,
    offset: u32,
    limit: u32,
) -> Result<PhotosPage<PhotosAlbum>, PhotosError> {
    let mut albums = catalog.list_albums()?;
    albums.sort_by(|a, b| {
        a.kind
            .cmp(&b.kind)
            .then_with(|| match (&a.title, &b.title) {
                (Some(a_title), Some(b_title)) => {
                    a_title.to_lowercase().cmp(&b_title.to_lowercase())
                }
                (Some(_), None) => std::cmp::Ordering::Less,
                (None, Some(_)) => std::cmp::Ordering::Greater,
                (None, None) => std::cmp::Ordering::Equal,
            })
            .then_with(|| a.id.cmp(&b.id))
    });
    Ok(paginate(albums, offset, limit))
}

pub fn list_assets(
    catalog: &impl PhotosCatalog,
    album_id: Option<&str>,
    offset: u32,
    limit: u32,
) -> Result<PhotosPage<PhotosAsset>, PhotosError> {
    catalog.list_assets_page(album_id, offset, limit.clamp(1, 100))
}

fn paginate<T>(items: Vec<T>, offset: u32, limit: u32) -> PhotosPage<T> {
    let limit = limit.clamp(1, 100) as usize;
    let total = u32::try_from(items.len()).unwrap_or(u32::MAX);
    let start = usize::try_from(offset)
        .unwrap_or(usize::MAX)
        .min(items.len());
    let end = start.saturating_add(limit).min(items.len());
    let has_more = end < items.len();
    PhotosPage {
        items: items.into_iter().skip(start).take(limit).collect(),
        total,
        offset,
        has_more,
    }
}

pub struct SystemPhotosCatalog;

#[cfg(target_os = "macos")]
impl PhotosCatalog for SystemPhotosCatalog {
    fn authorization_status(&self) -> Result<PhotosAuthorizationStatus, PhotosError> {
        macos::authorization_status()
    }

    fn request_authorization(&self) -> Result<PhotosAuthorizationStatus, PhotosError> {
        macos::request_authorization()
    }

    fn list_albums(&self) -> Result<Vec<PhotosAlbum>, PhotosError> {
        macos::list_albums()
    }

    fn list_assets_page(
        &self,
        album_id: Option<&str>,
        offset: u32,
        limit: u32,
    ) -> Result<PhotosPage<PhotosAsset>, PhotosError> {
        macos::list_assets_page(album_id, offset, limit)
    }
}

#[cfg(not(target_os = "macos"))]
impl PhotosCatalog for SystemPhotosCatalog {
    fn authorization_status(&self) -> Result<PhotosAuthorizationStatus, PhotosError> {
        Ok(PhotosAuthorizationStatus::Unsupported)
    }

    fn request_authorization(&self) -> Result<PhotosAuthorizationStatus, PhotosError> {
        Err(PhotosError::Unsupported)
    }

    fn list_albums(&self) -> Result<Vec<PhotosAlbum>, PhotosError> {
        Err(PhotosError::Unsupported)
    }

    fn list_assets_page(
        &self,
        _album_id: Option<&str>,
        _offset: u32,
        _limit: u32,
    ) -> Result<PhotosPage<PhotosAsset>, PhotosError> {
        Err(PhotosError::Unsupported)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Default)]
    struct FakeCatalog {
        request_count: std::sync::atomic::AtomicUsize,
    }

    impl PhotosCatalog for FakeCatalog {
        fn authorization_status(&self) -> Result<PhotosAuthorizationStatus, PhotosError> {
            Ok(PhotosAuthorizationStatus::Limited)
        }

        fn request_authorization(&self) -> Result<PhotosAuthorizationStatus, PhotosError> {
            self.request_count
                .fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            Ok(PhotosAuthorizationStatus::Authorized)
        }

        fn list_albums(&self) -> Result<Vec<PhotosAlbum>, PhotosError> {
            Ok(vec![
                PhotosAlbum::new("z", "Trips", PhotosAlbumKind::User),
                PhotosAlbum::new("b", "Favorites", PhotosAlbumKind::Smart),
                PhotosAlbum::new("a", "favorites", PhotosAlbumKind::Smart),
            ])
        }

        fn list_assets_page(
            &self,
            _album_id: Option<&str>,
            offset: u32,
            limit: u32,
        ) -> Result<PhotosPage<PhotosAsset>, PhotosError> {
            Ok(paginate(
                vec![
                    PhotosAsset::new("new-a", Some("2026-02-01T00:00:00Z")),
                    PhotosAsset::new("new-b", Some("2026-02-01T00:00:00Z")),
                    PhotosAsset::new("old", Some("2026-01-01T00:00:00Z")),
                    PhotosAsset::new("null", None),
                ],
                offset,
                limit,
            ))
        }
    }

    #[test]
    fn authorization_request_is_explicit() {
        let catalog = FakeCatalog::default();
        assert_eq!(
            authorization_status(&catalog).unwrap(),
            PhotosAuthorizationStatus::Limited
        );
        assert_eq!(
            catalog
                .request_count
                .load(std::sync::atomic::Ordering::SeqCst),
            0
        );
        assert_eq!(
            request_authorization(&catalog).unwrap(),
            PhotosAuthorizationStatus::Authorized
        );
        assert_eq!(
            catalog
                .request_count
                .load(std::sync::atomic::Ordering::SeqCst),
            1
        );
    }

    #[test]
    fn album_page_clamps_and_orders_by_kind_title_then_id() {
        let page = list_albums(&FakeCatalog::default(), 0, 500).unwrap();
        let ids: Vec<&str> = page.items.iter().map(|item| item.id.as_str()).collect();
        assert_eq!(ids, vec!["z", "a", "b"]);
        assert_eq!(page.offset, 0);
        assert!(!page.has_more);
    }

    #[test]
    fn asset_adapter_page_preserves_created_desc_null_last_then_id() {
        let page = list_assets(&FakeCatalog::default(), None, 0, 100).unwrap();
        let ids: Vec<&str> = page.items.iter().map(|item| item.id.as_str()).collect();
        assert_eq!(ids, vec!["new-a", "new-b", "old", "null"]);
    }

    #[test]
    fn pagination_clamps_limit_and_reports_total_and_more() {
        let page = paginate(vec![1, 2, 3], 1, 0);
        assert_eq!(page.items, vec![2]);
        assert_eq!(page.total, 3);
        assert_eq!(page.offset, 1);
        assert!(page.has_more);
    }
}
