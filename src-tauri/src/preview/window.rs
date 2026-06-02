pub const PREVIEW_DISPLAY_LABEL: &str = "preview-display";
pub const PREVIEW_DISPLAY_TITLE: &str = "Cull Preview Display";
pub const PREVIEW_DISPLAY_URL: &str = "?previewDisplay=1";

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PreviewDisplayWindowSpec {
    pub label: &'static str,
    pub title: &'static str,
    pub url: &'static str,
    pub width: f64,
    pub height: f64,
    pub min_width: f64,
    pub min_height: f64,
}

pub fn preview_display_window_spec() -> PreviewDisplayWindowSpec {
    PreviewDisplayWindowSpec {
        label: PREVIEW_DISPLAY_LABEL,
        title: PREVIEW_DISPLAY_TITLE,
        url: PREVIEW_DISPLAY_URL,
        width: 1440.0,
        height: 960.0,
        min_width: 800.0,
        min_height: 600.0,
    }
}

pub fn preview_monitor_key(
    index: usize,
    name: Option<&str>,
    x: i32,
    y: i32,
    width: u32,
    height: u32,
) -> String {
    let name = name
        .map(slugify_monitor_name)
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| format!("display-{index}"));
    format!("{name}-{x}x{y}-{width}x{height}")
}

fn slugify_monitor_name(name: &str) -> String {
    let mut slug = String::new();
    let mut previous_dash = false;
    for ch in name.chars() {
        if ch.is_ascii_alphanumeric() {
            slug.push(ch.to_ascii_lowercase());
            previous_dash = false;
        } else if !previous_dash {
            slug.push('-');
            previous_dash = true;
        }
    }
    slug.trim_matches('-').to_string()
}
