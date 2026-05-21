use ort::session::builder::GraphOptimizationLevel;
use ort::session::Session;
use ort::value::Tensor;
use serde::Serialize;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use crate::db_core::remote_embeddings::{OLLAMA_EMBEDDING_MODEL_ID, OPENAI_EMBEDDING_MODEL_ID};

pub const CLIP_MODEL_ID: &str = "clip-vit-b32";
pub const DINO_V2_SMALL_MODEL_ID: &str = "dinov2-vits14";
pub const GEMINI_EMBEDDING_MODEL_ID: &str = "gemini-embedding-2";

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct EmbeddingModelSpec {
    pub model_id: &'static str,
    pub display_name: &'static str,
    pub url: &'static str,
    pub file_name: &'static str,
    pub input_name: &'static str,
    pub output_name: &'static str,
    pub input_size: u32,
    pub output_dims: usize,
    pub mean: [f32; 3],
    pub std: [f32; 3],
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct EmbeddingProviderSpec {
    pub id: &'static str,
    pub label: &'static str,
    pub short_label: &'static str,
    pub model_id: &'static str,
    pub runtime: &'static str,
    pub dimensions: usize,
    pub api_key_provider: Option<&'static str>,
    pub downloadable: bool,
    pub download_label: Option<&'static str>,
}

pub const CLIP_MODEL_SPEC: EmbeddingModelSpec = EmbeddingModelSpec {
    model_id: CLIP_MODEL_ID,
    display_name: "CLIP ViT-B/32",
    url: "https://huggingface.co/Qdrant/clip-ViT-B-32-vision/resolve/main/model.onnx",
    file_name: "clip-vit-b32-vision.onnx",
    input_name: "input",
    output_name: "output",
    input_size: 224,
    output_dims: 512,
    mean: [0.48145466, 0.4578275, 0.40821073],
    std: [0.26862954, 0.26130258, 0.27577711],
};

pub const DINO_V2_SMALL_MODEL_SPEC: EmbeddingModelSpec = EmbeddingModelSpec {
    model_id: DINO_V2_SMALL_MODEL_ID,
    display_name: "DINOv2 ViT-S/14",
    url: "https://huggingface.co/sefaburak/dinov2-small-onnx/resolve/main/dinov2_vits14.onnx",
    file_name: "dinov2-vits14.onnx",
    input_name: "input",
    output_name: "output",
    input_size: 224,
    output_dims: 384,
    mean: [0.485, 0.456, 0.406],
    std: [0.229, 0.224, 0.225],
};

pub const EMBEDDING_PROVIDER_SPECS: [EmbeddingProviderSpec; 5] = [
    EmbeddingProviderSpec {
        id: "clip",
        label: "CLIP ViT-B/32",
        short_label: "CLIP",
        model_id: CLIP_MODEL_ID,
        runtime: "local-onnx",
        dimensions: 512,
        api_key_provider: None,
        downloadable: true,
        download_label: Some("Download CLIP (~350MB)"),
    },
    EmbeddingProviderSpec {
        id: "dinov2",
        label: "DINOv2 ViT-S/14",
        short_label: "DINOv2",
        model_id: DINO_V2_SMALL_MODEL_ID,
        runtime: "local-onnx",
        dimensions: 384,
        api_key_provider: None,
        downloadable: true,
        download_label: Some("Download DINOv2 (~87MB)"),
    },
    EmbeddingProviderSpec {
        id: "gemini",
        label: "Gemini Embedding 2",
        short_label: "Gemini",
        model_id: GEMINI_EMBEDDING_MODEL_ID,
        runtime: "cloud-api",
        dimensions: 3072,
        api_key_provider: Some("google"),
        downloadable: false,
        download_label: None,
    },
    EmbeddingProviderSpec {
        id: "openai",
        label: "OpenAI Text Embedding 3 Large",
        short_label: "OpenAI",
        model_id: OPENAI_EMBEDDING_MODEL_ID,
        runtime: "cloud-api",
        dimensions: 3072,
        api_key_provider: Some("openai"),
        downloadable: false,
        download_label: None,
    },
    EmbeddingProviderSpec {
        id: "ollama",
        label: "Ollama Text Embeddings",
        short_label: "Ollama",
        model_id: OLLAMA_EMBEDDING_MODEL_ID,
        runtime: "local-api",
        dimensions: 0,
        api_key_provider: None,
        downloadable: false,
        download_label: None,
    },
];

pub fn embedding_model_spec(model_id: &str) -> Option<EmbeddingModelSpec> {
    match model_id {
        CLIP_MODEL_ID => Some(CLIP_MODEL_SPEC),
        DINO_V2_SMALL_MODEL_ID => Some(DINO_V2_SMALL_MODEL_SPEC),
        _ => None,
    }
}

pub fn embedding_provider_specs() -> &'static [EmbeddingProviderSpec] {
    &EMBEDDING_PROVIDER_SPECS
}

pub fn embedding_provider_for_model(model_id: &str) -> Option<EmbeddingProviderSpec> {
    if model_id.starts_with("openai:") {
        return EMBEDDING_PROVIDER_SPECS
            .iter()
            .find(|provider| provider.id == "openai")
            .copied();
    }
    if model_id.starts_with("ollama:") {
        return EMBEDDING_PROVIDER_SPECS
            .iter()
            .find(|provider| provider.id == "ollama")
            .copied();
    }
    EMBEDDING_PROVIDER_SPECS
        .iter()
        .find(|provider| provider.model_id == model_id)
        .copied()
}

pub struct EmbeddingEngine {
    pub session: Option<Mutex<Session>>,
    loaded_model_id: Option<String>,
    model_dir: PathBuf,
}

impl EmbeddingEngine {
    pub fn new(model_dir: &Path) -> Self {
        std::fs::create_dir_all(model_dir).ok();
        EmbeddingEngine {
            session: None,
            loaded_model_id: None,
            model_dir: model_dir.to_path_buf(),
        }
    }

    pub fn model_path(&self) -> PathBuf {
        self.model_path_for(CLIP_MODEL_ID)
            .unwrap_or_else(|_| self.model_dir.join(CLIP_MODEL_SPEC.file_name))
    }

    pub fn model_path_for(&self, model_id: &str) -> Result<PathBuf, String> {
        let spec = embedding_model_spec(model_id)
            .ok_or_else(|| format!("Unsupported embedding model '{}'", model_id))?;
        Ok(self.model_dir.join(spec.file_name))
    }

    pub fn is_model_available(&self) -> bool {
        self.model_path().exists()
    }

    pub fn is_model_available_for(&self, model_id: &str) -> Result<bool, String> {
        Ok(self.model_path_for(model_id)?.exists())
    }

    pub fn load_model(&mut self) -> Result<(), String> {
        self.load_model_for(CLIP_MODEL_ID)
    }

    pub fn load_model_for(&mut self, model_id: &str) -> Result<(), String> {
        let spec = embedding_model_spec(model_id)
            .ok_or_else(|| format!("Unsupported embedding model '{}'", model_id))?;
        if self.session.is_some() && self.loaded_model_id.as_deref() == Some(spec.model_id) {
            return Ok(());
        }

        let path = self.model_path_for(spec.model_id)?;
        if !path.exists() {
            return Err(format!(
                "Model '{}' not downloaded. Download it first.",
                spec.model_id
            ));
        }
        let session = Session::builder()
            .map_err(|e| format!("Failed to create session builder: {}", e))?
            .with_optimization_level(GraphOptimizationLevel::Level3)
            .map_err(|e| format!("Failed to set optimization: {}", e))?
            .commit_from_file(&path)
            .map_err(|e| format!("Failed to load model: {}", e))?;
        self.session = Some(Mutex::new(session));
        self.loaded_model_id = Some(spec.model_id.to_string());
        Ok(())
    }

    pub fn generate_embedding(&self, image_path: &Path) -> Result<Vec<f32>, String> {
        self.generate_embedding_for(CLIP_MODEL_ID, image_path)
    }

    pub fn generate_embedding_for(
        &self,
        model_id: &str,
        image_path: &Path,
    ) -> Result<Vec<f32>, String> {
        let spec = embedding_model_spec(model_id)
            .ok_or_else(|| format!("Unsupported embedding model '{}'", model_id))?;
        if self.loaded_model_id.as_deref() != Some(spec.model_id) {
            return Err(format!("Model '{}' not loaded", spec.model_id));
        }
        let session = self.session.as_ref().ok_or("Model not loaded")?;
        let mut session = session.lock().unwrap();

        let img = crate::db_core::image_decode::decode_image(image_path, false)?.image;
        let resized = img.resize_exact(
            spec.input_size,
            spec.input_size,
            image::imageops::FilterType::Lanczos3,
        );
        let rgb = resized.to_rgb8();

        let size = spec.input_size as usize;
        let mut tensor_data = vec![0.0f32; 3 * size * size];
        for (i, pixel) in rgb.pixels().enumerate() {
            for c in 0..3 {
                let val = pixel[c] as f32 / 255.0;
                let normalized = (val - spec.mean[c]) / spec.std[c];
                tensor_data[c * size * size + i] = normalized;
            }
        }

        let input_tensor = Tensor::from_array(([1usize, 3, size, size], tensor_data))
            .map_err(|e| format!("Tensor creation error: {}", e))?;

        let outputs = session
            .run(ort::inputs![input_tensor])
            .map_err(|e| format!("Inference error: {}", e))?;

        // Extract embedding
        let output = outputs.iter().next().ok_or("No output from model")?;
        let (_shape, data) = output
            .1
            .try_extract_tensor::<f32>()
            .map_err(|e| format!("Extract error: {}", e))?;

        let embedding = extract_embedding(data, spec.output_dims)?;

        // L2 normalize
        let norm: f32 = embedding.iter().map(|x| x * x).sum::<f32>().sqrt();
        if norm > 0.0 {
            Ok(embedding.iter().map(|x| x / norm).collect())
        } else {
            Ok(embedding)
        }
    }
}

fn extract_embedding(data: &[f32], dims: usize) -> Result<Vec<f32>, String> {
    if data.len() == dims {
        return Ok(data.to_vec());
    }
    if data.len() > dims && data.len() % dims == 0 {
        return Ok(data[..dims].to_vec());
    }
    Err(format!(
        "Unexpected embedding output length {}; expected {} or a multiple of {}",
        data.len(),
        dims,
        dims
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dinov2_model_spec_points_to_small_onnx_feature_model() {
        let spec = embedding_model_spec("dinov2-vits14").unwrap();

        assert_eq!(spec.model_id, "dinov2-vits14");
        assert_eq!(spec.file_name, "dinov2-vits14.onnx");
        assert_eq!(
            spec.url,
            "https://huggingface.co/sefaburak/dinov2-small-onnx/resolve/main/dinov2_vits14.onnx"
        );
        assert_eq!(spec.input_size, 224);
        assert_eq!(spec.output_dims, 384);
        assert_eq!(spec.input_name, "input");
        assert_eq!(spec.output_name, "output");
        assert_eq!(spec.mean, [0.485, 0.456, 0.406]);
        assert_eq!(spec.std, [0.229, 0.224, 0.225]);
    }

    #[test]
    fn embedding_engine_reports_model_specific_availability() {
        let tmp = tempfile::tempdir().unwrap();
        let engine = EmbeddingEngine::new(tmp.path());

        assert_eq!(
            engine.model_path_for("dinov2-vits14").unwrap(),
            tmp.path().join("dinov2-vits14.onnx")
        );
        assert!(!engine.is_model_available_for("dinov2-vits14").unwrap());
    }

    #[test]
    fn embedding_provider_catalog_lists_supported_runtime_providers() {
        let providers = embedding_provider_specs();
        let ids: Vec<&str> = providers.iter().map(|provider| provider.id).collect();

        assert_eq!(ids, vec!["clip", "dinov2", "gemini", "openai", "ollama"]);
        assert_eq!(providers[0].model_id, CLIP_MODEL_ID);
        assert_eq!(providers[0].runtime, "local-onnx");
        assert_eq!(providers[0].dimensions, 512);
        assert!(providers[0].downloadable);

        assert_eq!(providers[1].model_id, DINO_V2_SMALL_MODEL_ID);
        assert_eq!(providers[1].runtime, "local-onnx");
        assert_eq!(providers[1].dimensions, 384);
        assert!(providers[1].downloadable);

        assert_eq!(providers[2].id, "gemini");
        assert_eq!(providers[2].model_id, "gemini-embedding-2");
        assert_eq!(providers[2].runtime, "cloud-api");
        assert_eq!(providers[2].api_key_provider, Some("google"));
        assert!(!providers[2].downloadable);

        assert_eq!(providers[3].id, "openai");
        assert_eq!(providers[3].model_id, "openai:text-embedding-3-large");
        assert_eq!(providers[3].runtime, "cloud-api");
        assert_eq!(providers[3].api_key_provider, Some("openai"));
        assert_eq!(providers[3].dimensions, 3072);
        assert!(!providers[3].downloadable);

        assert_eq!(providers[4].id, "ollama");
        assert_eq!(providers[4].model_id, "ollama:embeddinggemma");
        assert_eq!(providers[4].runtime, "local-api");
        assert_eq!(providers[4].dimensions, 0);
        assert!(!providers[4].downloadable);
    }

    #[test]
    fn provider_for_model_maps_stored_model_names() {
        assert_eq!(
            embedding_provider_for_model(CLIP_MODEL_ID).unwrap().id,
            "clip"
        );
        assert_eq!(
            embedding_provider_for_model(DINO_V2_SMALL_MODEL_ID)
                .unwrap()
                .id,
            "dinov2"
        );
        assert_eq!(
            embedding_provider_for_model("gemini-embedding-2")
                .unwrap()
                .id,
            "gemini"
        );
        assert_eq!(
            embedding_provider_for_model("openai:text-embedding-3-large")
                .unwrap()
                .id,
            "openai"
        );
        assert_eq!(
            embedding_provider_for_model("ollama:embeddinggemma")
                .unwrap()
                .id,
            "ollama"
        );
        assert!(embedding_provider_for_model("unknown-model").is_none());
    }
}
