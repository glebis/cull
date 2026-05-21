use serde::Serialize;
use std::path::PathBuf;
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Mutex, OnceLock,
};
use tauri::{AppHandle, Emitter};

#[derive(Serialize, Clone, Default)]
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

    Some(OpenParams {
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
    })
}

pub fn open_params_for_drag_drop_paths(paths: &[PathBuf]) -> Vec<OpenParams> {
    let dirs: Vec<String> = paths
        .iter()
        .filter(|p| p.is_dir())
        .map(|p| p.to_string_lossy().into_owned())
        .collect();
    let files: Vec<String> = paths
        .iter()
        .filter(|p| !p.is_dir() && crate::extensions::is_image_path(p, false))
        .map(|p| p.to_string_lossy().into_owned())
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
    Some(percent_decode(without_host))
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
    emit_open_params(&app, params).map_err(|e| e.to_string())
}

/// Parse a deep link URL into OpenParams.
pub fn parse_deep_link(url: &str) -> OpenParams {
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
            let decoded = percent_decode(value);
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

    params
}

fn percent_decode(s: &str) -> String {
    let mut result = String::with_capacity(s.len());
    let mut chars = s.chars();
    while let Some(c) = chars.next() {
        if c == '%' {
            let hex: String = chars.by_ref().take(2).collect();
            if let Ok(byte) = u8::from_str_radix(&hex, 16) {
                result.push(byte as char);
            }
        } else if c == '+' {
            result.push(' ');
        } else {
            result.push(c);
        }
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;

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
    fn builds_loupe_params_for_opened_file() {
        let params = open_params_for_file_paths(vec!["/tmp/image.png".to_string()]).unwrap();
        assert_eq!(params.path.as_deref(), Some("/tmp/image.png"));
        assert_eq!(params.view.as_deref(), Some("loupe"));
        assert!(params.paths.is_none());
    }

    #[test]
    fn drag_drop_single_image_opens_loupe() {
        let dir = tempfile::tempdir().unwrap();
        let image = dir.path().join("image.jpg");
        std::fs::write(&image, b"not a real jpeg").unwrap();

        let params = open_params_for_drag_drop_paths(&[image.clone()]);

        assert_eq!(params.len(), 1);
        assert_eq!(params[0].path, Some(image.to_string_lossy().into_owned()));
        assert_eq!(params[0].paths, None);
        assert_eq!(params[0].folder, None);
        assert_eq!(params[0].view.as_deref(), Some("loupe"));
    }

    #[test]
    fn drag_drop_multiple_images_opens_grid_batch() {
        let dir = tempfile::tempdir().unwrap();
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
        let dir = tempfile::tempdir().unwrap();
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
        let dir = tempfile::tempdir().unwrap();
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
