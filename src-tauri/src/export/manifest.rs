use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExportManifest {
    pub kind: String,
    pub schema_version: u32,
    pub id: String,
    pub title: String,
    pub locale: String,
    pub created_at: String,
    pub updated_at: String,
    pub source: ManifestSource,
    pub defaults: ManifestDefaults,
    pub targets: Vec<ExportTarget>,
    pub slides: Vec<Slide>,
    pub assets: Vec<Asset>,
    pub agent_tasks: Vec<AgentTask>,
    pub agent_hints: AgentHints,
    pub agent_contract: AgentContract,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ManifestSource {
    pub app: String,
    pub collection_id: Option<String>,
    pub image_ids: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ManifestDefaults {
    pub template: String,
    pub fonts: ManifestFonts,
    pub colors: ManifestColors,
    pub safe_area: SafeArea,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ManifestFonts {
    pub serif: String,
    pub mono: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ManifestColors {
    pub preset: String,
    pub background: String,
    pub foreground: String,
    pub accent: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SafeArea {
    pub top: u32,
    pub right: u32,
    pub bottom: u32,
    pub left: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExportTarget {
    pub id: String,
    pub platform: String,
    pub format: String,
    pub width: u32,
    pub height: u32,
    pub mime: String,
    pub quality: Option<f32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Slide {
    pub id: String,
    pub template: Option<String>,
    pub targets: Option<Vec<String>>,
    pub image: SlideImage,
    pub text: SlideText,
    pub overlay: SlideOverlay,
    pub metadata: SlideMetadata,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SlideImage {
    pub asset_id: String,
    pub fit: String,
    pub focal_point: Option<FocalPoint>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FocalPoint {
    pub x: f32,
    pub y: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SlideText {
    pub headline: String,
    pub body: String,
    pub caption: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SlideOverlay {
    pub position: String,
    pub scrim: Scrim,
    pub text_color: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Scrim {
    #[serde(rename = "type")]
    pub scrim_type: String,
    pub direction: String,
    pub from: String,
    pub to: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SlideMetadata {
    pub rating: Option<u8>,
    pub tags: Vec<String>,
    pub alt: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Asset {
    pub id: String,
    pub kind: String,
    pub uri: String,
    pub mime: String,
    pub width: u32,
    pub height: u32,
    pub provenance: Option<AssetProvenance>,
    /// Simple source tag for non-AI provenance (e.g. "raw_preview" when a RAW
    /// thumbnail is used in place of the original RAW file).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source_kind: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AssetProvenance {
    pub provider: String,
    pub model: String,
    pub prompt: String,
    pub thinking: Option<bool>,
    pub reference_assets: Vec<String>,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentTask {
    pub slide_id: String,
    pub field: String,
    pub task: String,
    pub required: bool,
    pub max_chars: Option<u32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentHints {
    pub tone: String,
    pub allow_generated_images: bool,
    pub language: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentContract {
    pub mutable_paths: Vec<String>,
    pub append_only: Vec<String>,
    pub immutable_paths: Vec<String>,
}

impl ExportManifest {
    pub fn new(id: String, title: String) -> Self {
        let now = chrono::Utc::now().to_rfc3339();
        Self {
            kind: "cull-story/v1".to_string(),
            schema_version: 1,
            id,
            title,
            locale: "en".to_string(),
            created_at: now.clone(),
            updated_at: now,
            source: ManifestSource {
                app: "cull".to_string(),
                collection_id: None,
                image_ids: vec![],
            },
            defaults: ManifestDefaults {
                template: "editorial".to_string(),
                fonts: ManifestFonts {
                    serif: "EB Garamond".to_string(),
                    mono: "JetBrains Mono".to_string(),
                },
                colors: ManifestColors {
                    preset: "light".to_string(),
                    background: "#f7f3ea".to_string(),
                    foreground: "#171717".to_string(),
                    accent: "#c6422b".to_string(),
                },
                safe_area: SafeArea {
                    top: 96,
                    right: 72,
                    bottom: 96,
                    left: 72,
                },
            },
            targets: vec![],
            slides: vec![],
            assets: vec![],
            agent_tasks: vec![],
            agent_hints: AgentHints {
                tone: "quiet editorial".to_string(),
                allow_generated_images: true,
                language: "en".to_string(),
            },
            agent_contract: AgentContract {
                mutable_paths: vec![
                    "/slides/*/text/*".to_string(),
                    "/slides/*/metadata/alt".to_string(),
                    "/slides/*/overlay".to_string(),
                ],
                append_only: vec!["/assets".to_string()],
                immutable_paths: vec![
                    "/kind".to_string(),
                    "/source".to_string(),
                    "/targets".to_string(),
                    "/slides/*/image/asset_id".to_string(),
                ],
            },
        }
    }
}
