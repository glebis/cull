use parking_lot::Mutex;
use serde::{Deserialize, Serialize};
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PreviewDisplayMode {
    ImageOnly,
    ClientReview,
    MetadataReview,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PreviewDisplayLayout {
    Single,
    Compare,
    Grid,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PreviewRailSide {
    Left,
    Right,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PreviewRailWidth {
    Narrow,
    Medium,
    Wide,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PreviewRailTextSize {
    Small,
    Medium,
    Large,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PreviewOverlayConfig {
    pub show_filename: bool,
    pub show_rating: bool,
    pub show_decision: bool,
    pub show_metadata_rail: bool,
    pub show_dimensions: bool,
    pub show_format: bool,
    pub show_source: bool,
    pub show_prompt: bool,
    pub show_tags: bool,
    pub show_histogram: bool,
    pub rail_side: PreviewRailSide,
    pub rail_width: PreviewRailWidth,
    pub rail_text_size: PreviewRailTextSize,
}

impl Default for PreviewOverlayConfig {
    fn default() -> Self {
        Self {
            show_filename: false,
            show_rating: false,
            show_decision: false,
            show_metadata_rail: false,
            show_dimensions: false,
            show_format: false,
            show_source: false,
            show_prompt: false,
            show_tags: false,
            show_histogram: false,
            rail_side: PreviewRailSide::Right,
            rail_width: PreviewRailWidth::Medium,
            rail_text_size: PreviewRailTextSize::Medium,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PreviewState {
    pub image_id: Option<String>,
    pub image_ids: Vec<String>,
    pub display_mode: PreviewDisplayMode,
    pub layout: PreviewDisplayLayout,
    pub overlay: PreviewOverlayConfig,
    pub frozen: bool,
    pub blanked: bool,
    pub version: u64,
    pub updated_at_ms: u64,
}

impl Default for PreviewState {
    fn default() -> Self {
        Self {
            image_id: None,
            image_ids: Vec::new(),
            display_mode: PreviewDisplayMode::ImageOnly,
            layout: PreviewDisplayLayout::Single,
            overlay: PreviewOverlayConfig::default(),
            frozen: false,
            blanked: false,
            version: 0,
            updated_at_ms: 0,
        }
    }
}

#[derive(Default)]
pub struct PreviewStateStore {
    state: Mutex<PreviewState>,
}

impl PreviewStateStore {
    pub fn get(&self) -> PreviewState {
        self.state.lock().clone()
    }

    pub fn update(
        &self,
        image_id: Option<String>,
        image_ids: Option<Vec<String>>,
        display_mode: PreviewDisplayMode,
        layout: Option<PreviewDisplayLayout>,
        overlay: PreviewOverlayConfig,
        frozen: Option<bool>,
        blanked: Option<bool>,
    ) -> PreviewState {
        let mut state = self.state.lock();
        state.image_ids = image_ids.unwrap_or_else(|| image_id.iter().cloned().collect());
        state.image_id = image_id.or_else(|| state.image_ids.first().cloned());
        state.display_mode = display_mode;
        if let Some(layout) = layout {
            state.layout = layout;
        }
        state.overlay = overlay;
        if let Some(frozen) = frozen {
            state.frozen = frozen;
        }
        if let Some(blanked) = blanked {
            state.blanked = blanked;
        }
        state.version += 1;
        state.updated_at_ms = current_time_ms();
        state.clone()
    }
}

fn current_time_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis().min(u128::from(u64::MAX)) as u64)
        .unwrap_or(0)
}
