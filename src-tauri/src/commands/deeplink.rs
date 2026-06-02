use serde::Serialize;
use std::path::{Path, PathBuf};
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Mutex, OnceLock,
};
use tauri::{AppHandle, Emitter};

#[derive(Serialize, Clone, Default, Debug)]
pub struct OpenParams {
    pub path: Option<String>,
    pub paths: Option<Vec<String>>,
    pub folder: Option<String>,
    pub view: Option<String>,
    pub size: Option<u32>,
    pub zoom: Option<u32>,
    pub fullscreen: Option<bool>,
    pub focus: Option<u32>,
    pub image_id: Option<String>,
    pub gap: Option<u32>,
}

/// Sensitive directories that must never be accessed via deep links.
const SENSITIVE_DIRS: &[&str] = &[
    ".ssh",
    ".gnupg",
    ".aws",
    ".config/gcloud",
    "Library/Keychains",
];

/// Validate that a single path is safe for deep-link access.
/// Returns the canonicalized path string on success, or an error message.
fn validate_path(raw: &str) -> Result<String, String> {
    let path = Path::new(raw);

    // Canonicalize resolves symlinks and normalizes ".."
    let canonical =
        std::fs::canonicalize(path).map_err(|e| format!("Cannot resolve path '{}': {}", raw, e))?;

    let home = dirs::home_dir().ok_or("Cannot determine home directory")?;

    // Must be under $HOME
    if !canonical.starts_with(&home) {
        return Err(format!(
            "Deep link path '{}' is outside the home directory",
            canonical.display()
        ));
    }

    // Get the portion relative to $HOME for checking sensitive dirs and hidden components
    let relative = canonical
        .strip_prefix(&home)
        .map_err(|_| "Internal error stripping home prefix")?;

    // Check sensitive directories
    for sensitive in SENSITIVE_DIRS {
        let sensitive_path = Path::new(sensitive);
        if relative.starts_with(sensitive_path) {
            return Err(format!(
                "Deep link access to '{}' is blocked (sensitive directory)",
                canonical.display()
            ));
        }
    }

    // Reject hidden files/directories (components starting with '.')
    for component in relative.components() {
        if let std::path::Component::Normal(name) = component {
            if let Some(s) = name.to_str() {
                if s.starts_with('.') {
                    return Err(format!(
                        "Deep link access to '{}' is blocked (hidden path component '{}')",
                        canonical.display(),
                        s
                    ));
                }
            }
        }
    }

    Ok(canonical.to_string_lossy().into_owned())
}

/// Validate all file-system paths in OpenParams received from a deep link.
/// Non-path fields (view, size, zoom, etc.) are passed through unchanged.
pub fn validate_open_params(params: OpenParams) -> Result<OpenParams, String> {
    let path = match params.path {
        Some(ref p) => Some(validate_path(p)?),
        None => None,
    };

    let paths = match params.paths {
        Some(ref ps) => {
            let mut validated = Vec::with_capacity(ps.len());
            for p in ps {
                validated.push(validate_path(p)?);
            }
            Some(validated)
        }
        None => None,
    };

    let folder = match params.folder {
        Some(ref f) => Some(validate_path(f)?),
        None => None,
    };

    Ok(OpenParams {
        path,
        paths,
        folder,
        ..params
    })
}

static FRONTEND_OPEN_LISTENER_READY: AtomicBool = AtomicBool::new(false);
static PENDING_OPEN_PARAMS: OnceLock<Mutex<Vec<OpenParams>>> = OnceLock::new();

fn pending_open_params() -> &'static Mutex<Vec<OpenParams>> {
    PENDING_OPEN_PARAMS.get_or_init(|| Mutex::new(Vec::new()))
}

pub fn emit_open_params(app: &AppHandle, params: OpenParams) -> tauri::Result<()> {
    if !FRONTEND_OPEN_LISTENER_READY.load(Ordering::SeqCst) {
        if let Ok(mut pending) = pending_open_params().lock() {
            pending.push(params.clone());
        }
    }
    app.emit("open-with-params", params)
}

pub fn open_params_for_file_paths(file_paths: Vec<String>) -> Option<OpenParams> {
    if file_paths.is_empty() {
        return None;
    }

    let params = OpenParams {
        path: if file_paths.len() == 1 {
            Some(file_paths[0].clone())
        } else {
            None
        },
        paths: if file_paths.len() > 1 {
            Some(file_paths)
        } else {
            None
        },
        folder: None,
        view: Some("loupe".to_string()),
        size: None,
        zoom: None,
        fullscreen: None,
        focus: None,
        image_id: None,
        gap: None,
    };

    validate_open_params(params).ok()
}

pub fn open_params_for_urls(urls: &[String]) -> Vec<OpenParams> {
    let mut params = Vec::new();
    let file_paths: Vec<String> = urls
        .iter()
        .filter_map(|url| file_path_from_url(url))
        .collect();

    if let Some(file_params) = open_params_for_file_paths(file_paths) {
        params.push(file_params);
    }

    for url in urls {
        if url.starts_with("cull://") {
            match parse_deep_link(url) {
                Ok(parsed) => params.push(parsed),
                Err(e) => crate::safe_eprintln!("[deep-link] Deep link rejected: {}", e),
            }
        }
    }

    params
}

pub fn open_params_for_drag_drop_paths(paths: &[PathBuf]) -> Vec<OpenParams> {
    let dirs: Vec<String> = paths
        .iter()
        .filter(|p| p.is_dir())
        .filter_map(|p| validate_path(&p.to_string_lossy()).ok())
        .collect();
    let files: Vec<String> = paths
        .iter()
        .filter(|p| !p.is_dir() && crate::extensions::is_image_path(p, false))
        .filter_map(|p| validate_path(&p.to_string_lossy()).ok())
        .collect();

    if dirs.len() == 1 && files.is_empty() {
        return vec![OpenParams {
            folder: Some(dirs[0].clone()),
            view: Some("grid".to_string()),
            ..OpenParams::default()
        }];
    }

    let mut params = Vec::new();
    if !files.is_empty() {
        let file_count = files.len();
        params.push(OpenParams {
            path: if file_count == 1 {
                Some(files[0].clone())
            } else {
                None
            },
            paths: if file_count > 1 { Some(files) } else { None },
            view: Some(if file_count == 1 { "loupe" } else { "grid" }.to_string()),
            ..OpenParams::default()
        });
    }

    if !params.is_empty() || dirs.len() > 1 {
        params.extend(dirs.into_iter().map(|folder| OpenParams {
            folder: Some(folder),
            ..OpenParams::default()
        }));
    }

    params
}

pub fn file_path_from_url(url: &str) -> Option<String> {
    let raw_path = url.strip_prefix("file://")?;
    let without_host = raw_path.strip_prefix("localhost").unwrap_or(raw_path);
    percent_decode(without_host).ok()
}

#[tauri::command]
pub async fn drain_pending_open_params() -> Result<Vec<OpenParams>, String> {
    FRONTEND_OPEN_LISTENER_READY.store(true, Ordering::SeqCst);
    let mut pending = pending_open_params()
        .lock()
        .map_err(|_| "pending open params lock poisoned".to_string())?;
    Ok(std::mem::take(&mut *pending))
}

/// Tauri command that agents can call via IPC to control the app.
#[tauri::command]
pub async fn open_with_params(
    app: AppHandle,
    path: Option<String>,
    paths: Option<Vec<String>>,
    folder: Option<String>,
    view: Option<String>,
    size: Option<u32>,
    zoom: Option<u32>,
    fullscreen: Option<bool>,
    focus: Option<u32>,
    image_id: Option<String>,
    gap: Option<u32>,
) -> Result<(), String> {
    let params = OpenParams {
        path,
        paths,
        folder,
        view,
        size,
        zoom,
        fullscreen,
        focus,
        image_id,
        gap,
    };
    let validated = validate_open_params(params)?;
    emit_open_params(&app, validated).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn open_deep_link_urls(app: AppHandle, urls: Vec<String>) -> Result<(), String> {
    for params in open_params_for_urls(&urls) {
        emit_open_params(&app, params).map_err(|e| e.to_string())?;
    }
    Ok(())
}

/// Parse a deep link URL into OpenParams.
/// Returns an error if any file-system path fails validation.
pub fn parse_deep_link(url: &str) -> Result<OpenParams, String> {
    let mut params = OpenParams {
        path: None,
        paths: None,
        folder: None,
        view: None,
        size: None,
        zoom: None,
        fullscreen: None,
        focus: None,
        image_id: None,
        gap: None,
    };

    // Extract the action from the URL (e.g., "open", "grid", "loupe")
    // cull://open?path=... or cull://grid?size=280
    let action = if let Some(scheme_end) = url.find("://") {
        let after_scheme = &url[scheme_end + 3..];
        let action_end = after_scheme.find('?').unwrap_or(after_scheme.len());
        Some(after_scheme[..action_end].to_string())
    } else {
        None
    };

    // Map action to view mode if not explicitly set
    match action.as_deref() {
        Some("grid") => params.view = Some("grid".to_string()),
        Some("loupe") => params.view = Some("loupe".to_string()),
        Some("compare") => params.view = Some("compare".to_string()),
        _ => {}
    }

    if let Some(query_start) = url.find('?') {
        let query = &url[query_start + 1..];
        for pair in query.split('&') {
            let mut parts = pair.splitn(2, '=');
            let key = parts.next().unwrap_or("");
            let value = parts.next().unwrap_or("");
            let decoded = percent_decode(value)?;
            match key {
                "path" => params.path = Some(decoded),
                "paths" => {
                    params.paths = Some(decoded.split(',').map(|s| s.to_string()).collect());
                }
                "folder" => params.folder = Some(decoded),
                "view" => params.view = Some(decoded),
                "zoom" => params.zoom = decoded.parse().ok(),
                "size" => params.size = decoded.parse().ok(),
                "fullscreen" => params.fullscreen = Some(decoded == "true"),
                "focus" => params.focus = decoded.parse().ok(),
                "image_id" | "imageId" => params.image_id = Some(decoded),
                "gap" => params.gap = decoded.parse().ok(),
                _ => {}
            }
        }
    }

    validate_open_params(params)
}

fn percent_decode(s: &str) -> Result<String, String> {
    let input = s.as_bytes();
    let mut output = Vec::with_capacity(input.len());
    let mut i = 0;
    while i < input.len() {
        match input[i] {
            b'%' => {
                if i + 2 >= input.len() {
                    return Err(format!("Malformed percent encoding in '{}'", s));
                }
                let hi = hex_value(input[i + 1])
                    .ok_or_else(|| format!("Malformed percent encoding in '{}'", s))?;
                let lo = hex_value(input[i + 2])
                    .ok_or_else(|| format!("Malformed percent encoding in '{}'", s))?;
                output.push((hi << 4) | lo);
                i += 3;
            }
            b'+' => {
                output.push(b' ');
                i += 1;
            }
            byte => {
                output.push(byte);
                i += 1;
            }
        }
    }

    String::from_utf8(output).map_err(|_| format!("Invalid UTF-8 percent encoding in '{}'", s))
}

fn hex_value(byte: u8) -> Option<u8> {
    match byte {
        b'0'..=b'9' => Some(byte - b'0'),
        b'a'..=b'f' => Some(byte - b'a' + 10),
        b'A'..=b'F' => Some(byte - b'A' + 10),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn home_tempdir(prefix: &str) -> tempfile::TempDir {
        let home = dirs::home_dir().unwrap();
        tempfile::Builder::new()
            .prefix(prefix)
            .tempdir_in(home)
            .unwrap()
    }

    #[test]
    fn parses_file_url_into_path() {
        assert_eq!(
            file_path_from_url("file:///tmp/Cull%20Test/image.png").as_deref(),
            Some("/tmp/Cull Test/image.png")
        );
    }

    #[test]
    fn ignores_non_file_url_for_file_path() {
        assert!(file_path_from_url("cull://loupe?image_id=img-1").is_none());
    }

    #[test]
    fn file_url_rejects_malformed_percent_encoding() {
        assert!(file_path_from_url("file:///tmp/Cull%ZZTest/image.png").is_none());
    }

    // --- Deep link path validation tests ---

    #[test]
    fn valid_home_path_passes_validation() {
        let home = dirs::home_dir().unwrap();
        // Create a non-hidden temp directory under $HOME
        let test_dir = home.join("cull_deeplink_test_tmp");
        std::fs::create_dir_all(&test_dir).unwrap();
        let image = test_dir.join("photo.jpg");
        std::fs::write(&image, b"fake image").unwrap();

        let result = validate_path(image.to_str().unwrap());
        // Clean up before asserting so we don't leave files on failure
        let _ = std::fs::remove_file(&image);
        let _ = std::fs::remove_dir(&test_dir);
        assert!(result.is_ok(), "Expected Ok, got: {:?}", result);
    }

    #[test]
    fn etc_passwd_is_rejected() {
        let result = validate_path("/etc/passwd");
        assert!(result.is_err());
        assert!(
            result.unwrap_err().contains("outside the home directory"),
            "Should mention outside home directory"
        );
    }

    #[test]
    fn ssh_dir_is_rejected() {
        let home = dirs::home_dir().unwrap();
        let ssh_path = home.join(".ssh");
        // Only test if the directory actually exists (it does on most dev machines)
        if ssh_path.exists() {
            let result = validate_path(ssh_path.to_str().unwrap());
            assert!(result.is_err());
            let err = result.unwrap_err();
            assert!(
                err.contains("sensitive directory") || err.contains("hidden path component"),
                "Should block .ssh: {}",
                err
            );
        }
    }

    #[test]
    fn dotdot_traversal_rejected() {
        // Build a path that tries to traverse out of home via ..
        let result = validate_path("/tmp/../etc/passwd");
        assert!(result.is_err());
        assert!(
            result.unwrap_err().contains("outside the home directory"),
            "Traversal to /etc should be rejected"
        );
    }

    #[test]
    fn malformed_percent_encoding_is_rejected() {
        let result = parse_deep_link("cull://open?path=/Users/test/Cull%ZZ.png");
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("percent encoding"));
    }

    #[test]
    fn invalid_utf8_percent_encoding_is_rejected() {
        let result = parse_deep_link("cull://open?path=/Users/test/%E0%A4%A.png");
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("percent encoding"));
    }

    #[test]
    fn hidden_directory_rejected() {
        let home = dirs::home_dir().unwrap();
        let hidden = home.join(".hidden_test_dir_deeplink");
        let _ = std::fs::create_dir(&hidden);
        if hidden.exists() {
            let result = validate_path(hidden.to_str().unwrap());
            assert!(result.is_err());
            assert!(result.unwrap_err().contains("hidden path component"));
            let _ = std::fs::remove_dir(&hidden);
        }
    }

    #[test]
    fn validate_open_params_passes_no_paths() {
        // OpenParams with no file-system paths should pass through fine
        let params = OpenParams {
            view: Some("grid".to_string()),
            size: Some(280),
            ..OpenParams::default()
        };
        let result = validate_open_params(params);
        assert!(result.is_ok());
        let p = result.unwrap();
        assert_eq!(p.view.as_deref(), Some("grid"));
        assert_eq!(p.size, Some(280));
    }

    #[test]
    fn open_params_for_urls_routes_cull_urls_through_rust_parser() {
        let urls = vec!["cull://grid?size=280".to_string()];

        let params = open_params_for_urls(&urls);

        assert_eq!(params.len(), 1);
        assert_eq!(params[0].view.as_deref(), Some("grid"));
        assert_eq!(params[0].size, Some(280));
    }

    #[test]
    fn open_params_for_urls_rejects_invalid_cull_url_paths() {
        let urls = vec!["cull://open?path=/Users/test/Cull%ZZ.png".to_string()];

        let params = open_params_for_urls(&urls);

        assert!(params.is_empty());
    }

    #[test]
    fn drag_drop_paths_use_deep_link_validation() {
        let dir = tempfile::tempdir().unwrap();
        let image = dir.path().join("image.jpg");
        std::fs::write(&image, b"not a real jpeg").unwrap();

        let params = open_params_for_drag_drop_paths(&[image.clone()]);
        assert!(params.is_empty());

        let file_params = open_params_for_file_paths(vec![image.to_string_lossy().into_owned()]);
        assert!(file_params.is_none());
    }

    #[test]
    fn builds_loupe_params_for_opened_file() {
        let dir = home_tempdir("cull_open_file_");
        let image = dir.path().join("image.png");
        std::fs::write(&image, b"not a real png").unwrap();

        let params =
            open_params_for_file_paths(vec![image.to_string_lossy().into_owned()]).unwrap();
        let canonical = image.canonicalize().unwrap().to_string_lossy().into_owned();
        assert_eq!(params.path.as_deref(), Some(canonical.as_str()));
        assert_eq!(params.view.as_deref(), Some("loupe"));
        assert!(params.paths.is_none());
    }

    #[test]
    fn drag_drop_single_image_opens_loupe() {
        let dir = home_tempdir("cull_drag_single_");
        let image = dir.path().join("image.jpg");
        std::fs::write(&image, b"not a real jpeg").unwrap();

        let params = open_params_for_drag_drop_paths(&[image.clone()]);

        let canonical = image.canonicalize().unwrap().to_string_lossy().into_owned();
        assert_eq!(params.len(), 1);
        assert_eq!(params[0].path.as_deref(), Some(canonical.as_str()));
        assert_eq!(params[0].paths, None);
        assert_eq!(params[0].folder, None);
        assert_eq!(params[0].view.as_deref(), Some("loupe"));
    }

    #[test]
    fn drag_drop_multiple_images_opens_grid_batch() {
        let dir = home_tempdir("cull_drag_multi_");
        let first = dir.path().join("first.jpg");
        let second = dir.path().join("second.png");
        std::fs::write(&first, b"image").unwrap();
        std::fs::write(&second, b"image").unwrap();

        let params = open_params_for_drag_drop_paths(&[first.clone(), second.clone()]);

        assert_eq!(params.len(), 1);
        assert_eq!(params[0].path, None);
        assert_eq!(
            params[0].paths,
            Some(vec![
                first.to_string_lossy().into_owned(),
                second.to_string_lossy().into_owned(),
            ])
        );
        assert_eq!(params[0].folder, None);
        assert_eq!(params[0].view.as_deref(), Some("grid"));
    }

    #[test]
    fn drag_drop_single_folder_opens_folder_grid() {
        let dir = home_tempdir("cull_drag_folder_");
        let folder = dir.path().join("Library");
        std::fs::create_dir(&folder).unwrap();

        let params = open_params_for_drag_drop_paths(&[folder.clone()]);

        assert_eq!(params.len(), 1);
        assert_eq!(params[0].path, None);
        assert_eq!(params[0].paths, None);
        assert_eq!(
            params[0].folder,
            Some(folder.to_string_lossy().into_owned())
        );
        assert_eq!(params[0].view.as_deref(), Some("grid"));
    }

    #[test]
    fn drag_drop_mixed_files_and_folders_keeps_both_import_actions() {
        let dir = home_tempdir("cull_drag_mixed_");
        let image = dir.path().join("image.webp");
        let folder = dir.path().join("Folder");
        let ignored = dir.path().join("notes.txt");
        std::fs::write(&image, b"image").unwrap();
        std::fs::create_dir(&folder).unwrap();
        std::fs::write(&ignored, b"text").unwrap();

        let params = open_params_for_drag_drop_paths(&[image.clone(), folder.clone(), ignored]);

        assert_eq!(params.len(), 2);
        assert_eq!(params[0].path, Some(image.to_string_lossy().into_owned()));
        assert_eq!(params[0].view.as_deref(), Some("loupe"));
        assert_eq!(
            params[1].folder,
            Some(folder.to_string_lossy().into_owned())
        );
        assert_eq!(params[1].view, None);
    }
}
