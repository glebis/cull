use ort::session::builder::GraphOptimizationLevel;
use ort::session::Session;
use ort::value::Tensor;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::sync::Mutex;

const YOLO_INPUT_SIZE: u32 = 640;
const YOLO_CONF_THRESHOLD: f32 = 0.25;
const YOLO_IOU_THRESHOLD: f32 = 0.45;

const NUDENET_INPUT_SIZE: u32 = 320;
const NUDENET_CONF_THRESHOLD: f32 = 0.3;
const NUDENET_IOU_THRESHOLD: f32 = 0.45;

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
pub enum YoloVariant {
    Nano,
    Small,
    Medium,
}

impl YoloVariant {
    pub fn filename(&self) -> &str {
        match self {
            YoloVariant::Nano => "yolo11n.onnx",
            YoloVariant::Small => "yolo11s.onnx",
            YoloVariant::Medium => "yolo11m.onnx",
        }
    }

    pub fn model_name(&self) -> &str {
        match self {
            YoloVariant::Nano => "yolo11n",
            YoloVariant::Small => "yolo11s",
            YoloVariant::Medium => "yolo11m",
        }
    }

    pub fn from_str(s: &str) -> Option<Self> {
        match s {
            "nano" | "n" => Some(YoloVariant::Nano),
            "small" | "s" => Some(YoloVariant::Small),
            "medium" | "m" => Some(YoloVariant::Medium),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Detection {
    pub class_name: String,
    pub confidence: f32,
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
}

pub struct DetectionEngine {
    pub session: Option<Mutex<Session>>,
    model_dir: PathBuf,
    pub loaded_variant: Option<YoloVariant>,
    input_size: u32,
    num_classes: usize,
    class_names: Vec<&'static str>,
    conf_threshold: f32,
    iou_threshold: f32,
}

impl DetectionEngine {
    pub fn new_yolo(model_dir: &Path) -> Self {
        std::fs::create_dir_all(model_dir).ok();
        DetectionEngine {
            session: None,
            model_dir: model_dir.to_path_buf(),
            loaded_variant: None,
            input_size: YOLO_INPUT_SIZE,
            num_classes: 80,
            class_names: COCO_CLASSES.to_vec(),
            conf_threshold: YOLO_CONF_THRESHOLD,
            iou_threshold: YOLO_IOU_THRESHOLD,
        }
    }

    pub fn new_nudenet(model_dir: &Path) -> Self {
        std::fs::create_dir_all(model_dir).ok();
        DetectionEngine {
            session: None,
            model_dir: model_dir.to_path_buf(),
            loaded_variant: None,
            input_size: NUDENET_INPUT_SIZE,
            num_classes: 18,
            class_names: NUDENET_CLASSES.to_vec(),
            conf_threshold: NUDENET_CONF_THRESHOLD,
            iou_threshold: NUDENET_IOU_THRESHOLD,
        }
    }

    pub fn model_path_for_variant(&self, variant: YoloVariant) -> PathBuf {
        self.model_dir.join(variant.filename())
    }

    pub fn nudenet_model_path(&self) -> PathBuf {
        self.model_dir.join("nudenet.onnx")
    }

    pub fn is_variant_available(&self, variant: YoloVariant) -> bool {
        self.model_path_for_variant(variant).exists()
    }

    pub fn is_nudenet_available(&self) -> bool {
        self.nudenet_model_path().exists()
    }

    pub fn load_yolo(&mut self, variant: YoloVariant) -> Result<(), String> {
        let path = self.model_path_for_variant(variant);
        if !path.exists() {
            return Err(format!("Model {} not downloaded", variant.filename()));
        }
        let session = Session::builder()
            .map_err(|e| format!("Session builder error: {}", e))?
            .with_optimization_level(GraphOptimizationLevel::Level3)
            .map_err(|e| format!("Optimization error: {}", e))?
            .commit_from_file(&path)
            .map_err(|e| format!("Model load error: {}", e))?;
        self.session = Some(Mutex::new(session));
        self.loaded_variant = Some(variant);
        self.input_size = YOLO_INPUT_SIZE;
        self.num_classes = 80;
        self.class_names = COCO_CLASSES.to_vec();
        self.conf_threshold = YOLO_CONF_THRESHOLD;
        self.iou_threshold = YOLO_IOU_THRESHOLD;
        Ok(())
    }

    pub fn load_nudenet(&mut self) -> Result<(), String> {
        let path = self.nudenet_model_path();
        if !path.exists() {
            return Err("NudeNet model not downloaded".to_string());
        }
        let session = Session::builder()
            .map_err(|e| format!("Session builder error: {}", e))?
            .with_optimization_level(GraphOptimizationLevel::Level3)
            .map_err(|e| format!("Optimization error: {}", e))?
            .commit_from_file(&path)
            .map_err(|e| format!("Model load error: {}", e))?;
        self.session = Some(Mutex::new(session));
        self.loaded_variant = None;
        self.input_size = NUDENET_INPUT_SIZE;
        self.num_classes = 18;
        self.class_names = NUDENET_CLASSES.to_vec();
        self.conf_threshold = NUDENET_CONF_THRESHOLD;
        self.iou_threshold = NUDENET_IOU_THRESHOLD;
        Ok(())
    }

    pub fn detect(&self, image_path: &Path) -> Result<Vec<Detection>, String> {
        let session_mutex = self.session.as_ref().ok_or("Model not loaded")?;
        let mut session = session_mutex.lock().unwrap();

        let img = crate::db_core::image_decode::decode_image(image_path, false)?.image;
        let (orig_w, orig_h) = (img.width(), img.height());

        // Letterbox resize preserving aspect ratio
        let scale = f32::min(
            self.input_size as f32 / orig_w as f32,
            self.input_size as f32 / orig_h as f32,
        );
        let new_w = (orig_w as f32 * scale) as u32;
        let new_h = (orig_h as f32 * scale) as u32;
        let pad_x = (self.input_size - new_w) / 2;
        let pad_y = (self.input_size - new_h) / 2;

        let resized = img.resize_exact(new_w, new_h, image::imageops::FilterType::Lanczos3);

        // Create padded image (gray padding = 114/255)
        let mut padded = image::RgbImage::from_pixel(
            self.input_size,
            self.input_size,
            image::Rgb([114, 114, 114]),
        );
        image::imageops::overlay(&mut padded, &resized.to_rgb8(), pad_x as i64, pad_y as i64);

        // NCHW tensor, normalized to [0, 1]
        let sz = self.input_size as usize;
        let mut tensor_data = vec![0.0f32; 3 * sz * sz];
        for (i, pixel) in padded.pixels().enumerate() {
            for c in 0..3 {
                tensor_data[c * sz * sz + i] = pixel[c] as f32 / 255.0;
            }
        }

        let input_tensor = Tensor::from_array(([1usize, 3, sz, sz], tensor_data))
            .map_err(|e| format!("Tensor error: {}", e))?;

        let outputs = session
            .run(ort::inputs![input_tensor])
            .map_err(|e| format!("Inference error: {}", e))?;

        let output = outputs.iter().next().ok_or("No output")?;
        let (shape, data) = output
            .1
            .try_extract_tensor::<f32>()
            .map_err(|e| format!("Extract error: {}", e))?;

        // YOLOv8 output: [1, 4+num_classes, num_boxes] — transposed compared to v5
        let num_fields = shape[1] as usize; // 4 + num_classes
        let num_boxes = shape[2] as usize;
        let stride_1 = num_boxes; // stride for dimension 1

        let _ = num_fields; // validate shape if needed
        let mut raw_detections: Vec<Detection> = Vec::new();
        let mut confidences: Vec<f32> = Vec::new();

        for i in 0..num_boxes {
            // Extract bbox (cx, cy, w, h) — flat layout: [batch][field][box]
            let cx = data[i];
            let cy = data[stride_1 + i];
            let w = data[2 * stride_1 + i];
            let h = data[3 * stride_1 + i];

            // Find best class
            let mut best_conf = 0.0f32;
            let mut best_class = 0usize;
            for c in 0..self.num_classes {
                let conf = data[(4 + c) * stride_1 + i];
                if conf > best_conf {
                    best_conf = conf;
                    best_class = c;
                }
            }

            if best_conf < self.conf_threshold {
                continue;
            }

            // Convert from letterboxed coords back to normalized [0,1] relative to original image
            let x1: f32 = (cx - w / 2.0 - pad_x as f32) / (new_w as f32);
            let y1: f32 = (cy - h / 2.0 - pad_y as f32) / (new_h as f32);
            let bw: f32 = w / new_w as f32;
            let bh: f32 = h / new_h as f32;

            // Clamp to [0, 1]
            let x1 = x1.clamp(0.0, 1.0);
            let y1 = y1.clamp(0.0, 1.0);
            let bw = bw.clamp(0.0, 1.0 - x1);
            let bh = bh.clamp(0.0, 1.0 - y1);

            let class_name = if best_class < self.class_names.len() {
                self.class_names[best_class].to_string()
            } else {
                format!("class_{}", best_class)
            };

            raw_detections.push(Detection {
                class_name,
                confidence: best_conf,
                x: x1,
                y: y1,
                width: bw,
                height: bh,
            });
            confidences.push(best_conf);
        }

        Ok(nms(raw_detections, confidences, self.iou_threshold))
    }
}

fn iou(a: &Detection, b: &Detection) -> f32 {
    let x1 = f32::max(a.x, b.x);
    let y1 = f32::max(a.y, b.y);
    let x2 = f32::min(a.x + a.width, b.x + b.width);
    let y2 = f32::min(a.y + a.height, b.y + b.height);

    let intersection = f32::max(0.0, x2 - x1) * f32::max(0.0, y2 - y1);
    let area_a = a.width * a.height;
    let area_b = b.width * b.height;
    let union = area_a + area_b - intersection;

    if union <= 0.0 {
        0.0
    } else {
        intersection / union
    }
}

fn nms(detections: Vec<Detection>, confidences: Vec<f32>, iou_threshold: f32) -> Vec<Detection> {
    if detections.is_empty() {
        return detections;
    }

    let mut indices: Vec<usize> = (0..detections.len()).collect();
    indices.sort_by(|&a, &b| confidences[b].partial_cmp(&confidences[a]).unwrap());

    let mut keep: Vec<Detection> = Vec::new();
    let mut suppressed = vec![false; detections.len()];

    for &i in &indices {
        if suppressed[i] {
            continue;
        }
        keep.push(detections[i].clone());
        for &j in &indices {
            if j <= i || suppressed[j] {
                continue;
            }
            if detections[i].class_name == detections[j].class_name
                && iou(&detections[i], &detections[j]) > iou_threshold
            {
                suppressed[j] = true;
            }
        }
    }

    keep
}

const COCO_CLASSES: &[&str] = &[
    "person",
    "bicycle",
    "car",
    "motorcycle",
    "airplane",
    "bus",
    "train",
    "truck",
    "boat",
    "traffic light",
    "fire hydrant",
    "stop sign",
    "parking meter",
    "bench",
    "bird",
    "cat",
    "dog",
    "horse",
    "sheep",
    "cow",
    "elephant",
    "bear",
    "zebra",
    "giraffe",
    "backpack",
    "umbrella",
    "handbag",
    "tie",
    "suitcase",
    "frisbee",
    "skis",
    "snowboard",
    "sports ball",
    "kite",
    "baseball bat",
    "baseball glove",
    "skateboard",
    "surfboard",
    "tennis racket",
    "bottle",
    "wine glass",
    "cup",
    "fork",
    "knife",
    "spoon",
    "bowl",
    "banana",
    "apple",
    "sandwich",
    "orange",
    "broccoli",
    "carrot",
    "hot dog",
    "pizza",
    "donut",
    "cake",
    "chair",
    "couch",
    "potted plant",
    "bed",
    "dining table",
    "toilet",
    "tv",
    "laptop",
    "mouse",
    "remote",
    "keyboard",
    "cell phone",
    "microwave",
    "oven",
    "toaster",
    "sink",
    "refrigerator",
    "book",
    "clock",
    "vase",
    "scissors",
    "teddy bear",
    "hair drier",
    "toothbrush",
];

const NUDENET_CLASSES: &[&str] = &[
    "FEMALE_GENITALIA_COVERED",
    "FACE_FEMALE",
    "BUTTOCKS_EXPOSED",
    "FEMALE_BREAST_EXPOSED",
    "FEMALE_GENITALIA_EXPOSED",
    "MALE_BREAST_EXPOSED",
    "ANUS_EXPOSED",
    "FEET_EXPOSED",
    "BELLY_COVERED",
    "FEET_COVERED",
    "ARMPITS_COVERED",
    "ARMPITS_EXPOSED",
    "FACE_MALE",
    "BELLY_EXPOSED",
    "MALE_GENITALIA_EXPOSED",
    "ANUS_COVERED",
    "FEMALE_BREAST_COVERED",
    "BUTTOCKS_COVERED",
];
