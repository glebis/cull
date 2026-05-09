use crate::commands::export::PresetInfo;
use crate::export::presets;

pub fn list_presets() -> Vec<PresetInfo> {
    presets::PRESETS
        .iter()
        .map(|p| PresetInfo {
            id: p.id.to_string(),
            platform: p.platform.to_string(),
            format: p.format.to_string(),
            width: p.width,
            height: p.height,
            mime: p.mime.to_string(),
        })
        .collect()
}
