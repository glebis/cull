use regex::Regex;
use serde::{Deserialize, Serialize};
use std::sync::LazyLock;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SourceEvidence {
    pub detector: String,
    pub source_label: Option<String>,
    pub confidence: f64,
    pub details: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SourceDetectionResult {
    pub source_label: Option<String>,
    pub confidence: f64,
    pub is_ai_generated: Option<bool>,
    pub ai_prompt: Option<String>,
    pub evidence: Vec<SourceEvidence>,
}

impl SourceDetectionResult {
    pub fn unknown() -> Self {
        Self {
            source_label: None,
            confidence: 0.0,
            is_ai_generated: None,
            ai_prompt: None,
            evidence: vec![],
        }
    }

    pub fn to_evidence_json(&self) -> String {
        serde_json::to_string(&self.evidence).unwrap_or_else(|_| "[]".to_string())
    }
}

static DALLE_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)DALL[\-·\.]?E[\s_]?\d{4}").unwrap()
});

static COMFYUI_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)^ComfyUI_\d+").unwrap()
});

static SD_SEED_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"^\d{5,10}-\d+\.png$").unwrap()
});

pub fn detect_from_filename(filename: &str) -> SourceDetectionResult {
    let name = std::path::Path::new(filename)
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or(filename);

    if DALLE_RE.is_match(name) {
        return SourceDetectionResult {
            source_label: Some("dalle".to_string()),
            confidence: 0.7,
            is_ai_generated: Some(true),
            ai_prompt: None,
            evidence: vec![SourceEvidence {
                detector: "filename".to_string(),
                source_label: Some("dalle".to_string()),
                confidence: 0.7,
                details: format!("Filename matches DALL-E pattern: {}", name),
            }],
        };
    }

    if COMFYUI_RE.is_match(name) {
        return SourceDetectionResult {
            source_label: Some("comfyui".to_string()),
            confidence: 0.6,
            is_ai_generated: Some(true),
            ai_prompt: None,
            evidence: vec![SourceEvidence {
                detector: "filename".to_string(),
                source_label: Some("comfyui".to_string()),
                confidence: 0.6,
                details: format!("Filename matches ComfyUI pattern: {}", name),
            }],
        };
    }

    if SD_SEED_RE.is_match(name) {
        return SourceDetectionResult {
            source_label: Some("stable_diffusion".to_string()),
            confidence: 0.3,
            is_ai_generated: Some(true),
            ai_prompt: None,
            evidence: vec![SourceEvidence {
                detector: "filename".to_string(),
                source_label: Some("stable_diffusion".to_string()),
                confidence: 0.3,
                details: format!("Filename matches SD seed pattern: {}", name),
            }],
        };
    }

    SourceDetectionResult::unknown()
}

pub fn detect_from_png_text_chunks(chunks: &[(String, String)]) -> SourceDetectionResult {
    for (key, value) in chunks {
        match key.as_str() {
            "parameters" if value.contains("Steps:") && value.contains("Sampler:") => {
                let prompt = value.lines().next().map(|l| l.to_string());
                return SourceDetectionResult {
                    source_label: Some("stable_diffusion".to_string()),
                    confidence: 0.95,
                    is_ai_generated: Some(true),
                    ai_prompt: prompt,
                    evidence: vec![SourceEvidence {
                        detector: "png_text".to_string(),
                        source_label: Some("stable_diffusion".to_string()),
                        confidence: 0.95,
                        details: "Found A1111-style parameters chunk".to_string(),
                    }],
                };
            }
            "prompt" if value.contains("class_type") => {
                return SourceDetectionResult {
                    source_label: Some("comfyui".to_string()),
                    confidence: 0.95,
                    is_ai_generated: Some(true),
                    ai_prompt: None,
                    evidence: vec![SourceEvidence {
                        detector: "png_text".to_string(),
                        source_label: Some("comfyui".to_string()),
                        confidence: 0.95,
                        details: "Found ComfyUI prompt JSON chunk".to_string(),
                    }],
                };
            }
            "Dream" => {
                return SourceDetectionResult {
                    source_label: Some("stable_diffusion".to_string()),
                    confidence: 0.9,
                    is_ai_generated: Some(true),
                    ai_prompt: Some(value.clone()),
                    evidence: vec![SourceEvidence {
                        detector: "png_text".to_string(),
                        source_label: Some("stable_diffusion".to_string()),
                        confidence: 0.9,
                        details: "Found Dream chunk (InvokeAI)".to_string(),
                    }],
                };
            }
            "Software" if value.to_lowercase().contains("nanobanana") => {
                return SourceDetectionResult {
                    source_label: Some("nanobanana".to_string()),
                    confidence: 0.9,
                    is_ai_generated: Some(true),
                    ai_prompt: None,
                    evidence: vec![SourceEvidence {
                        detector: "png_text".to_string(),
                        source_label: Some("nanobanana".to_string()),
                        confidence: 0.9,
                        details: "Software field contains Nanobanana".to_string(),
                    }],
                };
            }
            _ => {}
        }
    }

    SourceDetectionResult::unknown()
}

pub fn combine_detection_results(results: Vec<SourceDetectionResult>) -> SourceDetectionResult {
    let mut all_evidence: Vec<SourceEvidence> = vec![];
    let mut best: Option<SourceDetectionResult> = None;

    for r in results {
        all_evidence.extend(r.evidence.clone());
        match &best {
            None if r.source_label.is_some() => best = Some(r),
            Some(b) if r.confidence > b.confidence && r.source_label.is_some() => {
                best = Some(r);
            }
            _ => {}
        }
    }

    match best {
        Some(mut b) => {
            b.evidence = all_evidence;
            b
        }
        None => {
            let mut unknown = SourceDetectionResult::unknown();
            unknown.evidence = all_evidence;
            unknown
        }
    }
}

pub fn detect_source(filename: &str, png_text_chunks: &[(String, String)]) -> SourceDetectionResult {
    let filename_result = detect_from_filename(filename);
    let png_result = detect_from_png_text_chunks(png_text_chunks);
    combine_detection_results(vec![png_result, filename_result])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_filename_dalle() {
        let result = detect_from_filename("DALL·E 2024-01-15 14.32.05.png");
        assert_eq!(result.source_label, Some("dalle".to_string()));
        assert!(result.confidence > 0.5);
    }

    #[test]
    fn test_filename_comfyui() {
        let result = detect_from_filename("ComfyUI_00042_.png");
        assert_eq!(result.source_label, Some("comfyui".to_string()));
    }

    #[test]
    fn test_filename_midjourney_discord() {
        let result = detect_from_filename("username_a_portrait_of_a_cat_abc123def.png");
        assert!(result.confidence < 0.5 || result.source_label.is_none());
    }

    #[test]
    fn test_filename_generic() {
        let result = detect_from_filename("IMG_2024.jpg");
        assert_eq!(result.source_label, None);
    }

    #[test]
    fn test_png_text_stable_diffusion() {
        let chunks = vec![
            ("parameters".to_string(), "masterpiece, best quality\nNegative prompt: bad\nSteps: 20, Sampler: Euler".to_string()),
        ];
        let result = detect_from_png_text_chunks(&chunks);
        assert_eq!(result.source_label, Some("stable_diffusion".to_string()));
        assert!(result.confidence > 0.8);
        assert!(result.ai_prompt.is_some());
    }

    #[test]
    fn test_png_text_comfyui_workflow() {
        let chunks = vec![
            ("prompt".to_string(), r#"{"3": {"class_type": "KSampler"}}"#.to_string()),
            ("workflow".to_string(), r#"{"nodes": []}"#.to_string()),
        ];
        let result = detect_from_png_text_chunks(&chunks);
        assert_eq!(result.source_label, Some("comfyui".to_string()));
    }

    #[test]
    fn test_combine_evidence() {
        let results = vec![
            SourceDetectionResult {
                source_label: Some("stable_diffusion".to_string()),
                confidence: 0.9,
                is_ai_generated: Some(true),
                ai_prompt: Some("a cat".to_string()),
                evidence: vec![SourceEvidence {
                    detector: "png_text".to_string(),
                    source_label: Some("stable_diffusion".to_string()),
                    confidence: 0.9,
                    details: "Found parameters chunk".to_string(),
                }],
            },
            SourceDetectionResult {
                source_label: None,
                confidence: 0.0,
                is_ai_generated: None,
                ai_prompt: None,
                evidence: vec![],
            },
        ];
        let combined = combine_detection_results(results);
        assert_eq!(combined.source_label, Some("stable_diffusion".to_string()));
        assert!(combined.confidence > 0.8);
    }
}
