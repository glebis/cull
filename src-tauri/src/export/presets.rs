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
    PlatformPreset {
        id: "web_responsive",
        platform: "web",
        format: "responsive",
        width: 1600,
        height: 1000,
        mime: "image/jpeg",
        quality: Some(0.90),
    },
    PlatformPreset {
        id: "ig_post",
        platform: "instagram",
        format: "post",
        width: 1080,
        height: 1350,
        mime: "image/png",
        quality: None,
    },
    PlatformPreset {
        id: "ig_story",
        platform: "instagram",
        format: "story",
        width: 1080,
        height: 1920,
        mime: "image/png",
        quality: None,
    },
    PlatformPreset {
        id: "ig_square",
        platform: "instagram",
        format: "square",
        width: 1080,
        height: 1080,
        mime: "image/png",
        quality: None,
    },
    PlatformPreset {
        id: "ig_carousel",
        platform: "instagram",
        format: "carousel",
        width: 1080,
        height: 1350,
        mime: "image/png",
        quality: None,
    },
    PlatformPreset {
        id: "li_pdf",
        platform: "linkedin",
        format: "pdf_carousel",
        width: 1080,
        height: 1350,
        mime: "application/pdf",
        quality: None,
    },
    PlatformPreset {
        id: "li_post",
        platform: "linkedin",
        format: "post",
        width: 1200,
        height: 628,
        mime: "image/jpeg",
        quality: Some(0.90),
    },
    PlatformPreset {
        id: "li_square",
        platform: "linkedin",
        format: "square",
        width: 1200,
        height: 1200,
        mime: "image/png",
        quality: None,
    },
    PlatformPreset {
        id: "tw_post",
        platform: "twitter",
        format: "post",
        width: 1600,
        height: 900,
        mime: "image/jpeg",
        quality: Some(0.90),
    },
    PlatformPreset {
        id: "tt_story",
        platform: "tiktok",
        format: "story",
        width: 1080,
        height: 1920,
        mime: "image/png",
        quality: None,
    },
    PlatformPreset {
        id: "pin",
        platform: "pinterest",
        format: "pin",
        width: 1000,
        height: 1500,
        mime: "image/png",
        quality: None,
    },
    PlatformPreset {
        id: "tg_post",
        platform: "telegram",
        format: "post",
        width: 1280,
        height: 1280,
        mime: "image/jpeg",
        quality: Some(0.85),
    },
    PlatformPreset {
        id: "yt_thumb",
        platform: "youtube",
        format: "thumbnail",
        width: 1280,
        height: 720,
        mime: "image/png",
        quality: None,
    },
    PlatformPreset {
        id: "bsky_post",
        platform: "bluesky",
        format: "post",
        width: 2000,
        height: 2000,
        mime: "image/png",
        quality: None,
    },
    PlatformPreset {
        id: "threads_post",
        platform: "threads",
        format: "post",
        width: 1080,
        height: 1350,
        mime: "image/png",
        quality: None,
    },
    PlatformPreset {
        id: "fb_feed",
        platform: "facebook",
        format: "feed",
        width: 1080,
        height: 1350,
        mime: "image/jpeg",
        quality: Some(0.90),
    },
    PlatformPreset {
        id: "fb_link",
        platform: "facebook",
        format: "link_preview",
        width: 1200,
        height: 630,
        mime: "image/jpeg",
        quality: Some(0.90),
    },
    PlatformPreset {
        id: "fb_post",
        platform: "facebook",
        format: "post",
        width: 1200,
        height: 630,
        mime: "image/jpeg",
        quality: Some(0.90),
    },
    PlatformPreset {
        id: "fb_story",
        platform: "facebook",
        format: "story",
        width: 1080,
        height: 1920,
        mime: "image/png",
        quality: None,
    },
    PlatformPreset {
        id: "portfolio_pdf",
        platform: "pdf",
        format: "portfolio",
        width: 2480,
        height: 3508,
        mime: "application/pdf",
        quality: None,
    },
];

pub fn get_preset(id: &str) -> Option<&'static PlatformPreset> {
    PRESETS.iter().find(|p| p.id == id)
}

#[allow(dead_code)]
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn final_export_master_presets_include_linkedin_and_portfolio_pdf_targets() {
        let linkedin_landscape = get_preset("li_post").expect("linkedin landscape preset");
        assert_eq!(linkedin_landscape.platform, "linkedin");
        assert_eq!(linkedin_landscape.width, 1200);
        assert_eq!(linkedin_landscape.height, 628);

        let linkedin_square = get_preset("li_square").expect("linkedin square preset");
        assert_eq!(linkedin_square.platform, "linkedin");
        assert_eq!(linkedin_square.width, 1200);
        assert_eq!(linkedin_square.height, 1200);

        let portfolio_pdf = get_preset("portfolio_pdf").expect("portfolio pdf preset");
        assert_eq!(portfolio_pdf.platform, "pdf");
        assert_eq!(portfolio_pdf.format, "portfolio");
        assert_eq!(portfolio_pdf.mime, "application/pdf");
        assert_eq!(portfolio_pdf.width, 2480);
        assert_eq!(portfolio_pdf.height, 3508);
    }
}
