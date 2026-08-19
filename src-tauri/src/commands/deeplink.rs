use serde::Serialize;
use std::path::PathBuf;
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
    pub settings_tab: Option<String>,
    pub collection: Option<String>,
    pub view: Option<String>,
    pub size: Option<u32>,
    pub zoom: Option<u32>,
    pub fullscreen: Option<bool>,
    pub focus: Option<u32>,
    pub image_id: Option<String>,
    pub gap: Option<u32>,
    pub drag_drop: Option<bool>,
    pub drop_x: Option<f64>,
    pub drop_y: Option<f64>,
    /// Correlation id for the navigation ack round-trip. When set, the frontend
    /// must report back via `complete_deep_link_navigation` so the caller can
    /// tell whether the navigation actually happened. See `services::display`.
    pub request_id: Option<String>,
}

/// Validate that a single path is safe for deep-link access.
/// Returns the canonicalized path string on success, or an error message.
/// Delegates to the shared [`path_policy`] in the strictest (`Deeplink`) mode.
fn validate_path(raw: &str) -> Result<String, String> {
    crate::db_core::path_policy::validate_path(raw, crate::db_core::path_policy::PathMode::Deeplink)
        .map(|p| p.to_string_lossy().into_owned())
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

pub fn emit_open_params<R: tauri::Runtime>(
    app: &AppHandle<R>,
    params: OpenParams,
) -> tauri::Result<()> {
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

    validate_open_params(build_file_open_params(file_paths)).ok()
}

fn build_file_open_params(file_paths: Vec<String>) -> OpenParams {
    OpenParams {
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
        settings_tab: None,
        collection: None,
        view: Some("loupe".to_string()),
        size: None,
        zoom: None,
        fullscreen: None,
        focus: None,
        image_id: None,
        gap: None,
        drag_drop: None,
        drop_x: None,
        drop_y: None,
        request_id: None,
    }
}

pub fn open_params_for_launch_path(path: &std::path::Path) -> Option<OpenParams> {
    let canonical = crate::db_core::path_policy::validate_path(
        path.to_string_lossy().as_ref(),
        crate::db_core::path_policy::PathMode::UserPicked,
    )
    .ok()?;

    if canonical.is_dir() {
        return Some(OpenParams {
            folder: Some(canonical.to_string_lossy().into_owned()),
            view: Some("grid".to_string()),
            ..OpenParams::default()
        });
    }

    if canonical.is_file() && crate::extensions::is_image_path(&canonical, false) {
        Some(build_file_open_params(vec![canonical
            .to_string_lossy()
            .into_owned()]))
    } else {
        None
    }
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
    open_params_for_drag_drop_paths_at(paths, None)
}

pub fn open_params_for_drag_drop_paths_at(
    paths: &[PathBuf],
    drop_position: Option<(f64, f64)>,
) -> Vec<OpenParams> {
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
            drag_drop: Some(true),
            drop_x: drop_position.map(|(x, _)| x),
            drop_y: drop_position.map(|(_, y)| y),
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
            drag_drop: Some(true),
            drop_x: drop_position.map(|(x, _)| x),
            drop_y: drop_position.map(|(_, y)| y),
            ..OpenParams::default()
        });
    }

    if !params.is_empty() || dirs.len() > 1 {
        params.extend(dirs.into_iter().map(|folder| OpenParams {
            folder: Some(folder),
            drag_drop: Some(true),
            drop_x: drop_position.map(|(x, _)| x),
            drop_y: drop_position.map(|(_, y)| y),
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

/// Frontend ack for a navigation that carried a `request_id`. This is what
/// makes the display tools able to report a real failure: without it they can
/// only observe that Tauri accepted the event, never that the UI acted on it.
#[tauri::command]
pub async fn complete_deep_link_navigation(
    request_id: String,
    ok: bool,
    error: Option<String>,
) -> Result<(), String> {
    crate::services::display::complete_navigation(
        &request_id,
        crate::services::display::NavigationAck { ok, error },
    );
    Ok(())
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
        settings_tab: None,
        collection: None,
        view,
        size,
        zoom,
        fullscreen,
        focus,
        image_id,
        gap,
        drag_drop: None,
        drop_x: None,
        drop_y: None,
        request_id: None,
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
/// Settings links accept the six UI tab IDs; an absent or unknown fragment
/// falls back to `general`, while malformed percent encoding remains an error.
pub fn parse_deep_link(url: &str) -> Result<OpenParams, String> {
    let mut params = OpenParams {
        path: None,
        paths: None,
        folder: None,
        settings_tab: None,
        collection: None,
        view: None,
        size: None,
        zoom: None,
        fullscreen: None,
        focus: None,
        image_id: None,
        gap: None,
        drag_drop: None,
        drop_x: None,
        drop_y: None,
        request_id: None,
    };

    // Extract the action from the URL (e.g., "open", "grid", "loupe")
    // cull://open?path=... or cull://grid?size=280
    let action = if let Some(scheme_end) = url.find("://") {
        let after_scheme = &url[scheme_end + 3..];
        let action_end = after_scheme.find(['?', '#']).unwrap_or(after_scheme.len());
        Some(after_scheme[..action_end].to_string())
    } else {
        None
    };

    // Map action to view mode if not explicitly set
    match action.as_deref() {
        Some("grid") => params.view = Some("grid".to_string()),
        Some("loupe") => params.view = Some("loupe".to_string()),
        Some("compare") => params.view = Some("compare".to_string()),
        Some("settings") => {
            let fragment = url
                .split_once('#')
                .map(|(_, fragment)| fragment)
                .unwrap_or("");
            let decoded = percent_decode(fragment)?;
            params.settings_tab = Some(match decoded.as_str() {
                "general" | "appearance" | "ai" | "agent-access" | "privacy" | "plugins" => decoded,
                _ => "general".to_string(),
            });
        }
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

    // Create a tempdir for test fixtures. Prefers to nest it under $HOME so the
    // production path validator sees the fixture as "under home" (matching the
    // behavior real users get on a developer machine), but falls back to the
    // system temp location when $HOME is not writable — for example, the macOS
    // CI runner image now exposes /Users/runner as read-only.
    fn home_tempdir(prefix: &str) -> tempfile::TempDir {
        if let Some(home) = dirs::home_dir() {
            if let Ok(dir) = tempfile::Builder::new().prefix(prefix).tempdir_in(&home) {
                return dir;
            }
        }
        tempfile::Builder::new()
            .prefix(prefix)
            .tempdir()
            .expect("tempdir creation should not fail on a working CI runner")
    }

    // Return $HOME when it is writable, otherwise None. Tests that exercise the
    // validator's "is this path under $HOME?" logic use this to skip themselves
    // on read-only CI runners instead of failing on `create_dir`.
    fn writable_home() -> Option<std::path::PathBuf> {
        let home = dirs::home_dir()?;
        let probe = home.join(".cull_deeplink_probe");
        if std::fs::create_dir(&probe).is_ok() {
            let _ = std::fs::remove_dir(&probe);
            Some(home)
        } else {
            None
        }
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
        let Some(home) = writable_home() else {
            // CI runner exposes a read-only $HOME; this assertion is
            // developer-machine-only and we cannot meaningfully exercise it.
            return;
        };
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
        let Some(home) = writable_home() else { return };
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
        let Some(home) = writable_home() else { return };
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
    fn settings_deep_links_parse_every_supported_tab() {
        for tab in [
            "general",
            "appearance",
            "ai",
            "agent-access",
            "privacy",
            "plugins",
        ] {
            let params = parse_deep_link(&format!("cull://settings#{tab}")).unwrap();
            assert_eq!(params.settings_tab.as_deref(), Some(tab));
        }
    }

    #[test]
    fn settings_deep_links_fall_back_to_general_for_invalid_fragments() {
        for url in [
            "cull://settings",
            "cull://settings#",
            "cull://settings#unknown",
        ] {
            let params = parse_deep_link(url).unwrap();
            assert_eq!(params.settings_tab.as_deref(), Some("general"));
        }
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
        if writable_home().is_none() {
            return;
        }
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
    fn launch_folder_builds_grid_import_params() {
        if writable_home().is_none() {
            return;
        }
        let dir = home_tempdir("cull_launch_folder_");
        let folder = dir.path().join("Library");
        std::fs::create_dir(&folder).unwrap();

        let params = open_params_for_launch_path(&folder).unwrap();

        assert_eq!(
            params.folder,
            Some(
                folder
                    .canonicalize()
                    .unwrap()
                    .to_string_lossy()
                    .into_owned()
            )
        );
        assert_eq!(params.view.as_deref(), Some("grid"));
        assert_eq!(params.drag_drop, None);
        assert!(params.path.is_none());
    }

    #[test]
    fn launch_path_rejects_unsupported_files() {
        if writable_home().is_none() {
            return;
        }
        let dir = home_tempdir("cull_launch_unsupported_");
        let text = dir.path().join("notes.txt");
        std::fs::write(&text, b"notes").unwrap();

        assert!(open_params_for_launch_path(&text).is_none());
    }

    #[test]
    fn launch_folder_accepts_explicit_path_outside_home() {
        let folder = tempfile::tempdir().unwrap();

        let params = open_params_for_launch_path(folder.path()).unwrap();

        assert_eq!(
            params.folder,
            Some(
                folder
                    .path()
                    .canonicalize()
                    .unwrap()
                    .to_string_lossy()
                    .into_owned()
            )
        );
    }

    #[test]
    fn launch_image_accepts_explicit_path_outside_home() {
        let dir = tempfile::tempdir().unwrap();
        let image = dir.path().join("shot.png");
        std::fs::write(&image, b"image").unwrap();

        let params = open_params_for_launch_path(&image).unwrap();

        assert_eq!(
            params.path,
            Some(image.canonicalize().unwrap().to_string_lossy().into_owned())
        );
        assert_eq!(params.view.as_deref(), Some("loupe"));
    }

    #[test]
    fn drag_drop_single_image_opens_loupe() {
        if writable_home().is_none() {
            return;
        }
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
        if writable_home().is_none() {
            return;
        }
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
        if writable_home().is_none() {
            return;
        }
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
        assert_eq!(params[0].drag_drop, Some(true));
    }

    #[test]
    fn drag_drop_paths_can_include_drop_position() {
        if writable_home().is_none() {
            return;
        }
        let dir = home_tempdir("cull_drag_position_");
        let folder = dir.path().join("Library");
        std::fs::create_dir(&folder).unwrap();

        let params = open_params_for_drag_drop_paths_at(&[folder], Some((120.5, 88.25)));

        assert_eq!(params.len(), 1);
        assert_eq!(params[0].drag_drop, Some(true));
        assert_eq!(params[0].drop_x, Some(120.5));
        assert_eq!(params[0].drop_y, Some(88.25));
    }

    #[test]
    fn drag_drop_mixed_files_and_folders_keeps_both_import_actions() {
        if writable_home().is_none() {
            return;
        }
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
