use ort::session::Session;
use ort::session::builder::GraphOptimizationLevel;
use ort::value::Tensor;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

const CLIP_INPUT_SIZE: u32 = 224;
const CLIP_MEAN: [f32; 3] = [0.48145466, 0.4578275, 0.40821073];
const CLIP_STD: [f32; 3] = [0.26862954, 0.26130258, 0.27577711];

pub struct EmbeddingEngine {
    pub session: Option<Mutex<Session>>,
    model_dir: PathBuf,
}

impl EmbeddingEngine {
    pub fn new(model_dir: &Path) -> Self {
        std::fs::create_dir_all(model_dir).ok();
        EmbeddingEngine {
            session: None,
            model_dir: model_dir.to_path_buf(),
        }
    }

    pub fn model_path(&self) -> PathBuf {
        self.model_dir.join("clip-vit-b32-vision.onnx")
    }

    pub fn is_model_available(&self) -> bool {
        self.model_path().exists()
    }

    pub fn load_model(&mut self) -> Result<(), String> {
        let path = self.model_path();
        if !path.exists() {
            return Err("Model not downloaded. Call download_model first.".to_string());
        }
        let session = Session::builder()
            .map_err(|e| format!("Failed to create session builder: {}", e))?
            .with_optimization_level(GraphOptimizationLevel::Level3)
            .map_err(|e| format!("Failed to set optimization: {}", e))?
            .commit_from_file(&path)
            .map_err(|e| format!("Failed to load model: {}", e))?;
        self.session = Some(Mutex::new(session));
        Ok(())
    }

    pub fn generate_embedding(&self, image_path: &Path) -> Result<Vec<f32>, String> {
        let session = self.session.as_ref()
            .ok_or("Model not loaded")?;
        let mut session = session.lock().unwrap();

        // Load and preprocess
        let img = image::open(image_path).map_err(|e| format!("Image open error: {}", e))?;
        let resized = img.resize_exact(CLIP_INPUT_SIZE, CLIP_INPUT_SIZE, image::imageops::FilterType::Lanczos3);
        let rgb = resized.to_rgb8();

        // Convert to NCHW tensor with normalization
        let mut tensor_data = vec![0.0f32; 3 * 224 * 224];
        for (i, pixel) in rgb.pixels().enumerate() {
            for c in 0..3 {
                let val = pixel[c] as f32 / 255.0;
                let normalized = (val - CLIP_MEAN[c]) / CLIP_STD[c];
                tensor_data[c * 224 * 224 + i] = normalized;
            }
        }

        let input_tensor = Tensor::from_array(([1usize, 3, 224, 224], tensor_data))
            .map_err(|e| format!("Tensor creation error: {}", e))?;

        let outputs = session.run(ort::inputs![input_tensor])
            .map_err(|e| format!("Inference error: {}", e))?;

        // Extract embedding
        let output = outputs.iter().next()
            .ok_or("No output from model")?;
        let (_shape, data) = output.1.try_extract_tensor::<f32>()
            .map_err(|e| format!("Extract error: {}", e))?;

        let embedding: Vec<f32> = data.to_vec();

        // L2 normalize
        let norm: f32 = embedding.iter().map(|x| x * x).sum::<f32>().sqrt();
        if norm > 0.0 {
            Ok(embedding.iter().map(|x| x / norm).collect())
        } else {
            Ok(embedding)
        }
    }
}
