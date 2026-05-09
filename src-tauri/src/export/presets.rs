use crate::export::manifest::ExportTarget;

pub struct PlatformPreset {
    pub id: &'static str,
    pub platform: &'static str,
    pub format: &'static str,
    pub width: u32,
    pub height: u32,
    pub mime: &'static str,
    pub quality: Option<f32>,
}

pub const PRESETS: &[PlatformPreset] = &[
    PlatformPreset { id: "ig_post", platform: "instagram", format: "post", width: 1080, height: 1350, mime: "image/png", quality: None },
    PlatformPreset { id: "ig_story", platform: "instagram", format: "story", width: 1080, height: 1920, mime: "image/png", quality: None },
    PlatformPreset { id: "ig_carousel", platform: "instagram", format: "carousel", width: 1080, height: 1350, mime: "image/png", quality: None },
    PlatformPreset { id: "li_pdf", platform: "linkedin", format: "pdf_carousel", width: 1080, height: 1350, mime: "application/pdf", quality: None },
    PlatformPreset { id: "tw_post", platform: "twitter", format: "post", width: 1600, height: 900, mime: "image/jpeg", quality: Some(0.90) },
    PlatformPreset { id: "tt_story", platform: "tiktok", format: "story", width: 1080, height: 1920, mime: "image/png", quality: None },
    PlatformPreset { id: "pin", platform: "pinterest", format: "pin", width: 1000, height: 1500, mime: "image/png", quality: None },
    PlatformPreset { id: "tg_post", platform: "telegram", format: "post", width: 1280, height: 1280, mime: "image/jpeg", quality: Some(0.85) },
    PlatformPreset { id: "yt_thumb", platform: "youtube", format: "thumbnail", width: 1280, height: 720, mime: "image/png", quality: None },
    PlatformPreset { id: "bsky_post", platform: "bluesky", format: "post", width: 2000, height: 2000, mime: "image/png", quality: None },
    PlatformPreset { id: "threads_post", platform: "threads", format: "post", width: 1080, height: 1350, mime: "image/png", quality: None },
    PlatformPreset { id: "fb_post", platform: "facebook", format: "post", width: 1200, height: 630, mime: "image/jpeg", quality: Some(0.90) },
    PlatformPreset { id: "fb_story", platform: "facebook", format: "story", width: 1080, height: 1920, mime: "image/png", quality: None },
];

pub fn get_preset(id: &str) -> Option<&'static PlatformPreset> {
    PRESETS.iter().find(|p| p.id == id)
}

pub fn list_presets() -> &'static [PlatformPreset] {
    PRESETS
}

impl PlatformPreset {
    pub fn to_target(&self) -> ExportTarget {
        ExportTarget {
            id: self.id.to_string(),
            platform: self.platform.to_string(),
            format: self.format.to_string(),
            width: self.width,
            height: self.height,
            mime: self.mime.to_string(),
            quality: self.quality,
        }
    }
}
