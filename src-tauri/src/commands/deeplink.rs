use serde::Serialize;
use tauri::{AppHandle, Emitter};

#[derive(Serialize, Clone)]
pub struct OpenParams {
    pub path: Option<String>,
    pub paths: Option<Vec<String>>,
    pub folder: Option<String>,
    pub view: Option<String>,
    pub size: Option<u32>,
    pub zoom: Option<u32>,
    pub fullscreen: Option<bool>,
    pub focus: Option<u32>,
    pub gap: Option<u32>,
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
        gap,
    };
    app.emit("open-with-params", params)
        .map_err(|e| e.to_string())
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
