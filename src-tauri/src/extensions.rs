use std::path::Path;

pub const BASE_IMAGE_EXTENSIONS: &[&str] = &[
    "jpg", "jpeg", "png", "gif", "webp", "bmp", "tiff", "tif", "heic", "heif", "avif", "svg",
    "ico", "psd",
];

pub const RAW_EXTENSIONS: &[&str] = &["cr2", "cr3", "nef", "arw", "dng", "orf", "raf", "rw2"];

pub const BASE_DECODABLE_EXTENSIONS: &[&str] = &["jpg", "jpeg", "png", "webp", "gif"];

pub fn supported_extensions(module_raw: bool) -> Vec<&'static str> {
    let mut exts = BASE_IMAGE_EXTENSIONS.to_vec();
    if module_raw {
        exts.extend_from_slice(RAW_EXTENSIONS);
    }
    exts
}

pub fn is_raw_extension(ext: &str) -> bool {
    RAW_EXTENSIONS.contains(&ext.to_lowercase().as_str())
}

pub fn is_image_path(path: &Path, module_raw: bool) -> bool {
    path.extension()
        .and_then(|ext| ext.to_str())
        .map(|ext| {
            let lower = ext.to_lowercase();
            BASE_IMAGE_EXTENSIONS.contains(&lower.as_str())
                || (module_raw && RAW_EXTENSIONS.contains(&lower.as_str()))
        })
        .unwrap_or(false)
}

pub fn is_decodable(ext: &str, module_raw: bool) -> bool {
    let lower = ext.to_lowercase();
    BASE_DECODABLE_EXTENSIONS.contains(&lower.as_str())
        || (module_raw && RAW_EXTENSIONS.contains(&lower.as_str()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    #[test]
    fn supported_without_raw_excludes_raf() {
        let exts = supported_extensions(false);
        assert!(exts.contains(&"jpg"));
        assert!(exts.contains(&"png"));
        assert!(exts.contains(&"psd"));
        assert!(!exts.contains(&"raf"));
        assert!(!exts.contains(&"cr2"));
    }

    #[test]
    fn supported_with_raw_includes_raf() {
        let exts = supported_extensions(true);
        assert!(exts.contains(&"jpg"));
        assert!(exts.contains(&"raf"));
        assert!(exts.contains(&"cr2"));
        assert!(exts.contains(&"nef"));
        assert!(exts.contains(&"arw"));
        assert!(exts.contains(&"dng"));
    }

    #[test]
    fn is_raw_extension_checks() {
        assert!(is_raw_extension("raf"));
        assert!(is_raw_extension("RAF"));
        assert!(is_raw_extension("cr2"));
        assert!(!is_raw_extension("jpg"));
        assert!(!is_raw_extension("psd"));
        assert!(!is_raw_extension(""));
    }

    #[test]
    fn is_image_path_respects_module() {
        assert!(is_image_path(Path::new("photo.jpg"), false));
        assert!(!is_image_path(Path::new("photo.raf"), false));
        assert!(is_image_path(Path::new("photo.raf"), true));
        assert!(is_image_path(Path::new("photo.RAF"), true));
        assert!(!is_image_path(Path::new("doc.txt"), true));
    }

    #[test]
    fn is_decodable_raw_only_when_enabled() {
        assert!(is_decodable("jpg", false));
        assert!(!is_decodable("raf", false));
        assert!(is_decodable("raf", true));
        assert!(is_decodable("cr2", true));
        assert!(!is_decodable("bmp", false));
    }
}
