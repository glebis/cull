use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::Path;

const DEFAULT_OLLAMA_URL: &str = "http://localhost:11434/api/generate";

const VISION_PROMPT: &str = r#"Analyze this image comprehensively. Be precise and concise.

1. DESCRIPTION: What is this image showing? (1-2 sentences)
2. SCENE_TYPE: screenshot/photo/document/meme/artwork/chart/map/other
3. OBJECTS: Key objects visible (comma-separated)
4. MOOD: Overall mood/atmosphere (1-2 words)
5. TAGS: 5 searchable keywords (comma-separated)
6. PEOPLE_COUNT: Number of people visible (0 if none)
7. DOMINANT_COLORS: Top 3 colors (comma-separated)
8. IMAGE_QUALITY: high/medium/low/blurry
9. INDOOR_OUTDOOR: indoor/outdoor/unknown
10. TIME_OF_DAY: day/night/dawn/dusk/unknown
11. ACTIVITY: What activity is happening (1-3 words, or "none")
12. NSFW_SCORE: 0-100 (0=safe, 100=explicit)
13. AESTHETIC_SCORE: 0-100 (visual quality/composition)
14. FACES_COUNT: Number of faces visible

Format EXACTLY as:
DESCRIPTION: [text]
SCENE_TYPE: [type]
OBJECTS: [list]
MOOD: [mood]
TAGS: [tags]
PEOPLE_COUNT: [n]
DOMINANT_COLORS: [colors]
IMAGE_QUALITY: [quality]
INDOOR_OUTDOOR: [value]
TIME_OF_DAY: [value]
ACTIVITY: [value]
NSFW_SCORE: [0-100]
AESTHETIC_SCORE: [0-100]
FACES_COUNT: [n]"#;

#[derive(Debug, Serialize, Deserialize)]
struct OllamaRequest {
    model: String,
    prompt: String,
    images: Vec<String>,
    stream: bool,
}

#[derive(Debug, Deserialize)]
struct OllamaResponse {
    response: String,
}

pub fn parse_vision_response(text: &str) -> HashMap<String, String> {
    let mut fields = HashMap::new();
    for line in text.lines() {
        if let Some((key, value)) = line.split_once(':') {
            let key = key.trim().to_lowercase().replace(' ', "_");
            let value = value.trim().to_string();
            if !value.is_empty() {
                fields.insert(key, value);
            }
        }
    }
    fields
}

pub async fn analyze_image(
    image_path: &Path,
    ollama_url: &str,
    model: &str,
) -> Result<HashMap<String, String>, String> {
    let image_data = std::fs::read(image_path).map_err(|e| format!("Read error: {}", e))?;
    let b64 = base64::Engine::encode(&base64::engine::general_purpose::STANDARD, &image_data);

    let request = OllamaRequest {
        model: model.to_string(),
        prompt: VISION_PROMPT.to_string(),
        images: vec![b64],
        stream: false,
    };

    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(120))
        .build()
        .map_err(|e| format!("Client error: {}", e))?;

    let resp = client
        .post(ollama_url)
        .json(&request)
        .send()
        .await
        .map_err(|e| format!("Ollama request failed: {}", e))?;

    if !resp.status().is_success() {
        return Err(format!("Ollama returned {}", resp.status()));
    }

    let body: OllamaResponse = resp
        .json()
        .await
        .map_err(|e| format!("Parse error: {}", e))?;

    Ok(parse_vision_response(&body.response))
}

pub async fn check_ollama_available(ollama_url: &str) -> Result<Vec<String>, String> {
    let base = ollama_url
        .strip_suffix("/api/generate")
        .unwrap_or(ollama_url)
        .trim_end_matches('/');
    let tags_url = format!("{}/api/tags", base);

    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(5))
        .build()
        .map_err(|e| format!("Client error: {}", e))?;

    let resp = client
        .get(&tags_url)
        .send()
        .await
        .map_err(|e| format!("Ollama not reachable: {}", e))?;

    if !resp.status().is_success() {
        return Err(format!("Ollama returned {}", resp.status()));
    }

    #[derive(Deserialize)]
    struct TagsResponse {
        models: Vec<ModelInfo>,
    }
    #[derive(Deserialize)]
    struct ModelInfo {
        name: String,
    }

    let body: TagsResponse = resp
        .json()
        .await
        .map_err(|e| format!("Parse error: {}", e))?;

    Ok(body.models.into_iter().map(|m| m.name).collect())
}

pub fn default_ollama_url() -> &'static str {
    DEFAULT_OLLAMA_URL
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_vision_response() {
        let text = "DESCRIPTION: A dog sitting on grass\nSCENE_TYPE: photo\nOBJECTS: dog, grass, ball\nMOOD: peaceful\nTAGS: dog, outdoor, grass, sunny, pet\nPEOPLE_COUNT: 0\nDOMINANT_COLORS: green, brown, white\nIMAGE_QUALITY: high\nINDOOR_OUTDOOR: outdoor\nTIME_OF_DAY: day\nACTIVITY: resting\nNSFW_SCORE: 0\nAESTHETIC_SCORE: 72\nFACES_COUNT: 0";
        let fields = parse_vision_response(text);
        assert_eq!(
            fields.get("description"),
            Some(&"A dog sitting on grass".to_string())
        );
        assert_eq!(fields.get("scene_type"), Some(&"photo".to_string()));
        assert_eq!(fields.get("nsfw_score"), Some(&"0".to_string()));
        assert_eq!(fields.len(), 14);
    }
}
