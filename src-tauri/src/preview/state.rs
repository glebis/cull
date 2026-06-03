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
#[serde(rename_all = "camelCase")]
pub struct PreviewOverlayConfig {
    pub show_filename: bool,
    pub show_rating: bool,
    pub show_decision: bool,
    pub show_metadata_rail: bool,
}

impl Default for PreviewOverlayConfig {
    fn default() -> Self {
        Self {
            show_filename: false,
            show_rating: false,
            show_decision: false,
            show_metadata_rail: false,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PreviewState {
    pub image_id: Option<String>,
    pub display_mode: PreviewDisplayMode,
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
            display_mode: PreviewDisplayMode::ImageOnly,
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
        display_mode: PreviewDisplayMode,
        overlay: PreviewOverlayConfig,
        frozen: Option<bool>,
        blanked: Option<bool>,
    ) -> PreviewState {
        let mut state = self.state.lock();
        state.image_id = image_id;
        state.display_mode = display_mode;
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
