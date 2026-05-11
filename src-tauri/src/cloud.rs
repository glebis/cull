use std::path::Path;

#[derive(Debug, Clone, PartialEq)]
pub enum CloudProvider {
    ICloud,
    Dropbox,
    GoogleDrive,
}

pub fn detect_cloud_provider(path: &Path) -> Option<CloudProvider> {
    let path_str = path.to_string_lossy();

    if path_str.contains("/Library/CloudStorage/") {
        if path_str.contains("/CloudStorage/Dropbox") {
            return Some(CloudProvider::Dropbox);
        }
        if path_str.contains("/CloudStorage/GoogleDrive") {
            return Some(CloudProvider::GoogleDrive);
        }
        if path_str.contains("/CloudStorage/iCloud") {
            return Some(CloudProvider::ICloud);
        }
    }

    if path_str.contains("/Library/Mobile Documents/") {
        return Some(CloudProvider::ICloud);
    }

    if let Some(home) = dirs::home_dir() {
        let home_str = home.to_string_lossy();
        if path_str.starts_with(&format!("{}/Dropbox", home_str)) {
            return Some(CloudProvider::Dropbox);
        }
    }

    if path_str.starts_with("/Volumes/GoogleDrive") {
        return Some(CloudProvider::GoogleDrive);
    }

    None
}

pub fn is_cloud_placeholder(path: &Path) -> bool {
    if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
        if name.starts_with('.') && name.ends_with(".icloud") {
            return true;
        }
    }

    if let Ok(metadata) = std::fs::metadata(path) {
        if metadata.len() == 0 && detect_cloud_provider(path).is_some() {
            return true;
        }
    }

    false
}

pub fn is_cloud_internal_file(path: &Path) -> bool {
    if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
        if name.starts_with('.') && name.ends_with(".icloud") {
            return true;
        }
        if name == ".dropbox" || name == ".dropbox.cache" || name == ".dropbox.attr" {
            return true;
        }
        if name.starts_with(".gdrive") {
            return true;
        }
    }

    let path_str = path.to_string_lossy();
    if path_str.contains("/.dropbox.cache/") {
        return true;
    }

    false
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn test_detect_icloud_mobile_documents() {
        let p = PathBuf::from("/Users/user/Library/Mobile Documents/com~apple~CloudDocs/Photos/test.png");
        assert_eq!(detect_cloud_provider(&p), Some(CloudProvider::ICloud));
    }

    #[test]
    fn test_detect_icloud_cloud_storage() {
        let p = PathBuf::from("/Users/user/Library/CloudStorage/iCloudDrive/photos/test.png");
        assert_eq!(detect_cloud_provider(&p), Some(CloudProvider::ICloud));
    }

    #[test]
    fn test_detect_dropbox_cloud_storage() {
        let p = PathBuf::from("/Users/user/Library/CloudStorage/Dropbox/Camera Uploads/photo.jpg");
        assert_eq!(detect_cloud_provider(&p), Some(CloudProvider::Dropbox));
    }

    #[test]
    fn test_detect_google_drive_cloud_storage() {
        let p = PathBuf::from("/Users/user/Library/CloudStorage/GoogleDrive-user@gmail.com/My Drive/photos/img.png");
        assert_eq!(detect_cloud_provider(&p), Some(CloudProvider::GoogleDrive));
    }

    #[test]
    fn test_detect_local_path_returns_none() {
        let p = PathBuf::from("/Users/user/Pictures/local/photo.png");
        assert_eq!(detect_cloud_provider(&p), None);
    }

    #[test]
    fn test_detect_legacy_google_drive_volume() {
        let p = PathBuf::from("/Volumes/GoogleDrive/My Drive/photos/test.png");
        assert_eq!(detect_cloud_provider(&p), Some(CloudProvider::GoogleDrive));
    }

    #[test]
    fn test_icloud_stub_is_placeholder() {
        let p = PathBuf::from("/Users/user/Library/Mobile Documents/com~apple~CloudDocs/.photo.png.icloud");
        assert!(is_cloud_placeholder(&p));
    }

    #[test]
    fn test_cloud_internal_files() {
        assert!(is_cloud_internal_file(Path::new("/path/.dropbox")));
        assert!(is_cloud_internal_file(Path::new("/path/.dropbox.cache")));
        assert!(is_cloud_internal_file(Path::new("/path/.photo.png.icloud")));
        assert!(is_cloud_internal_file(Path::new("/path/to/.dropbox.cache/some_file")));
        assert!(!is_cloud_internal_file(Path::new("/path/photo.png")));
    }
}
