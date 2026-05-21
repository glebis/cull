use crate::db_core::models::{GenerationRun, ImageWithFile};
use reqwest::Client;
use serde::{Deserialize, Serialize};

pub const OPENAI_EMBEDDING_MODEL: &str = "text-embedding-3-large";
pub const OPENAI_EMBEDDING_MODEL_ID: &str = "openai:text-embedding-3-large";
pub const OPENAI_EMBEDDING_ENDPOINT: &str = "https://api.openai.com/v1/embeddings";
pub const OLLAMA_EMBEDDING_MODEL: &str = "embeddinggemma";
pub const OLLAMA_EMBEDDING_MODEL_ID: &str = "ollama:embeddinggemma";
pub const OLLAMA_EMBEDDING_URL: &str = "http://localhost:11434/api/embed";

pub struct OpenAiTextEmbeddingProvider {
    client: Client,
    api_key: String,
    model: String,
}

pub struct OllamaTextEmbeddingProvider {
    client: Client,
    url: String,
    model: String,
}

#[derive(Debug, Serialize)]
struct OpenAiEmbeddingRequest<'a> {
    model: &'a str,
    input: &'a str,
    encoding_format: &'a str,
}

#[derive(Debug, Deserialize)]
struct OpenAiEmbeddingResponse {
    data: Vec<OpenAiEmbeddingData>,
}

#[derive(Debug, Deserialize)]
struct OpenAiEmbeddingData {
    embedding: Vec<f32>,
}

#[derive(Debug, Serialize)]
struct OllamaEmbedRequest<'a> {
    model: &'a str,
    input: &'a str,
    truncate: bool,
}

#[derive(Debug, Deserialize)]
struct OllamaEmbedResponse {
    embeddings: Option<Vec<Vec<f32>>>,
    embedding: Option<Vec<f32>>,
}

impl OpenAiTextEmbeddingProvider {
    pub fn new(api_key: &str, model: &str) -> Self {
        OpenAiTextEmbeddingProvider {
            client: Client::new(),
            api_key: api_key.to_string(),
            model: model.to_string(),
        }
    }

    pub async fn generate_embedding(&self, input: &str) -> Result<Vec<f32>, String> {
        let request = OpenAiEmbeddingRequest {
            model: &self.model,
            input,
            encoding_format: "float",
        };
        let resp = self
            .client
            .post(OPENAI_EMBEDDING_ENDPOINT)
            .bearer_auth(&self.api_key)
            .json(&request)
            .send()
            .await
            .map_err(|e| format!("OpenAI request failed: {}", e))?;

        if !resp.status().is_success() {
            let status = resp.status();
            let text = resp.text().await.unwrap_or_default();
            return Err(format!("OpenAI returned {}: {}", status, text));
        }

        let body = resp
            .text()
            .await
            .map_err(|e| format!("OpenAI body: {}", e))?;
        parse_openai_embedding_response(&body)
    }
}

impl OllamaTextEmbeddingProvider {
    pub fn new(url: &str, model: &str) -> Self {
        OllamaTextEmbeddingProvider {
            client: Client::new(),
            url: normalize_ollama_embed_url(url),
            model: model.to_string(),
        }
    }

    pub async fn generate_embedding(&self, input: &str) -> Result<Vec<f32>, String> {
        let request = OllamaEmbedRequest {
            model: &self.model,
            input,
            truncate: true,
        };
        let resp = self
            .client
            .post(&self.url)
            .json(&request)
            .send()
            .await
            .map_err(|e| format!("Ollama embedding request failed: {}", e))?;

        if !resp.status().is_success() {
            let status = resp.status();
            let text = resp.text().await.unwrap_or_default();
            return Err(format!("Ollama returned {}: {}", status, text));
        }

        let body = resp
            .text()
            .await
            .map_err(|e| format!("Ollama body: {}", e))?;
        parse_ollama_embedding_response(&body)
    }
}

pub async fn check_ollama_embedding_available(url: &str) -> Result<Vec<String>, String> {
    let tags_url = ollama_tags_url(url);
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

pub fn openai_storage_model_id(model: &str) -> String {
    storage_model_id("openai", model)
}

pub fn ollama_storage_model_id(model: &str) -> String {
    storage_model_id("ollama", model)
}

pub fn normalize_embedding(vector: Vec<f32>) -> Vec<f32> {
    let norm = vector.iter().map(|x| x * x).sum::<f32>().sqrt();
    if norm > 0.0 {
        vector.into_iter().map(|x| x / norm).collect()
    } else {
        vector
    }
}

pub fn build_text_embedding_input(
    img: &ImageWithFile,
    generation_run: Option<&GenerationRun>,
    vision_metadata: &[(String, String, String)],
) -> String {
    let mut parts = Vec::new();

    if let Some(run) = generation_run {
        if let Some(prompt) = &run.prompt {
            push_labeled(&mut parts, "prompt", prompt);
        }
        if let Some(negative) = &run.negative_prompt {
            push_labeled(&mut parts, "negative prompt", negative);
        }
        if let Some(provider) = &run.provider {
            push_labeled(&mut parts, "provider", provider);
        }
        if let Some(model) = &run.model {
            push_labeled(&mut parts, "model", model);
        }
    }

    if generation_run.is_none() {
        if let Some(prompt) = &img.image.ai_prompt {
            push_labeled(&mut parts, "prompt", prompt);
        }
    }

    if let Some(source) = &img.source_label {
        push_labeled(&mut parts, "source", source);
    }

    if let Some(file_name) = std::path::Path::new(&img.path)
        .file_stem()
        .and_then(|value| value.to_str())
    {
        push_labeled(&mut parts, "filename", file_name);
    }
    push_labeled(&mut parts, "format", &img.image.format);
    push_labeled(
        &mut parts,
        "dimensions",
        &format!("{}x{}", img.image.width, img.image.height),
    );

    let mut metadata = vision_metadata.to_vec();
    metadata.sort_by(|a, b| a.0.cmp(&b.0).then(a.2.cmp(&b.2)));
    for (key, value, source) in metadata {
        let normalized_key = key.replace('_', " ");
        push_labeled(
            &mut parts,
            &format!("{} ({})", normalized_key, source),
            &value,
        );
    }

    parts.join("\n")
}

pub fn normalize_ollama_embed_url(url: &str) -> String {
    let trimmed = url.trim().trim_end_matches('/');
    if trimmed.is_empty() {
        return OLLAMA_EMBEDDING_URL.to_string();
    }
    if trimmed.ends_with("/api/embed") || trimmed.ends_with("/api/embeddings") {
        return trimmed.to_string();
    }
    if let Some(base) = trimmed.strip_suffix("/api/generate") {
        return format!("{}/api/embed", base.trim_end_matches('/'));
    }
    format!("{}/api/embed", trimmed)
}

pub fn ollama_tags_url(embed_url: &str) -> String {
    let normalized = normalize_ollama_embed_url(embed_url);
    let base = normalized
        .strip_suffix("/api/embed")
        .or_else(|| normalized.strip_suffix("/api/embeddings"))
        .unwrap_or(normalized.as_str())
        .trim_end_matches('/');
    format!("{}/api/tags", base)
}

pub fn parse_openai_embedding_response(body: &str) -> Result<Vec<f32>, String> {
    let response: OpenAiEmbeddingResponse =
        serde_json::from_str(body).map_err(|e| format!("OpenAI parse error: {}", e))?;
    response
        .data
        .into_iter()
        .next()
        .map(|data| data.embedding)
        .filter(|embedding| !embedding.is_empty())
        .ok_or_else(|| "OpenAI response did not include an embedding".to_string())
}

pub fn parse_ollama_embedding_response(body: &str) -> Result<Vec<f32>, String> {
    let response: OllamaEmbedResponse =
        serde_json::from_str(body).map_err(|e| format!("Ollama parse error: {}", e))?;
    if let Some(embedding) = response.embedding {
        if !embedding.is_empty() {
            return Ok(embedding);
        }
    }
    response
        .embeddings
        .and_then(|mut embeddings| embeddings.drain(..).next())
        .filter(|embedding| !embedding.is_empty())
        .ok_or_else(|| "Ollama response did not include an embedding".to_string())
}

fn storage_model_id(provider: &str, model: &str) -> String {
    let trimmed = model.trim();
    if trimmed.starts_with(&format!("{}:", provider)) {
        trimmed.to_string()
    } else {
        format!("{}:{}", provider, trimmed)
    }
}

fn push_labeled(parts: &mut Vec<String>, label: &str, value: &str) {
    let trimmed = value.trim();
    if !trimmed.is_empty() {
        parts.push(format!("{}: {}", label, trimmed));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db_core::models::{Image, Selection};

    fn sample_image() -> ImageWithFile {
        ImageWithFile {
            image: Image {
                id: "img-1".to_string(),
                sha256_hash: "hash".to_string(),
                width: 1024,
                height: 768,
                format: "png".to_string(),
                file_size: 42,
                created_at: "2026-01-01T00:00:00Z".to_string(),
                imported_at: "2026-01-01T00:00:00Z".to_string(),
                ai_prompt: Some("fallback prompt".to_string()),
                raw_metadata: None,
            },
            path: "/tmp/neon-city.png".to_string(),
            thumbnail_path: None,
            selection: Some(Selection {
                image_id: "img-1".to_string(),
                project_id: None,
                star_rating: Some(5),
                color_label: None,
                decision: "accept".to_string(),
            }),
            source_label: Some("midjourney".to_string()),
            missing_at: None,
        }
    }

    #[test]
    fn storage_model_ids_are_namespaced() {
        assert_eq!(
            openai_storage_model_id("text-embedding-3-small"),
            "openai:text-embedding-3-small"
        );
        assert_eq!(
            openai_storage_model_id("openai:text-embedding-3-large"),
            "openai:text-embedding-3-large"
        );
        assert_eq!(
            ollama_storage_model_id("nomic-embed-text"),
            "ollama:nomic-embed-text"
        );
    }

    #[test]
    fn text_embedding_input_prefers_generation_prompt_and_metadata() {
        let img = sample_image();
        let run = GenerationRun {
            id: "run-1".to_string(),
            prompt: Some("cinematic neon city".to_string()),
            negative_prompt: Some("low detail".to_string()),
            provider: Some("openai".to_string()),
            model: Some("gpt-image-2".to_string()),
            settings_json: "{}".to_string(),
            seed: None,
            parent_run_id: None,
            source_type: "sidecar".to_string(),
            source_path: None,
            raw_metadata_json: None,
            created_at: Some("2026-01-01T00:00:00Z".to_string()),
            imported_at: "2026-01-01T00:00:00Z".to_string(),
        };
        let metadata = vec![
            (
                "tags".to_string(),
                "city, night, neon".to_string(),
                "minicpm-v".to_string(),
            ),
            (
                "description".to_string(),
                "A bright city street".to_string(),
                "minicpm-v".to_string(),
            ),
        ];

        let text = build_text_embedding_input(&img, Some(&run), &metadata);

        assert!(text.contains("prompt: cinematic neon city"));
        assert!(text.contains("negative prompt: low detail"));
        assert!(text.contains("source: midjourney"));
        assert!(text.contains("filename: neon-city"));
        assert!(text.contains("description (minicpm-v): A bright city street"));
        assert!(!text.contains("fallback prompt"));
    }

    #[test]
    fn parses_openai_embedding_response() {
        let body =
            r#"{"data":[{"embedding":[0.1,0.2,0.3],"index":0}],"model":"text-embedding-3-large"}"#;
        assert_eq!(
            parse_openai_embedding_response(body).unwrap(),
            vec![0.1, 0.2, 0.3]
        );
    }

    #[test]
    fn parses_current_and_legacy_ollama_embedding_responses() {
        let current = r#"{"model":"embeddinggemma","embeddings":[[0.1,0.2,0.3]]}"#;
        let legacy = r#"{"embedding":[0.4,0.5]}"#;

        assert_eq!(
            parse_ollama_embedding_response(current).unwrap(),
            vec![0.1, 0.2, 0.3]
        );
        assert_eq!(
            parse_ollama_embedding_response(legacy).unwrap(),
            vec![0.4, 0.5]
        );
    }

    #[test]
    fn ollama_urls_are_normalized_to_embed_and_tags() {
        assert_eq!(
            normalize_ollama_embed_url("http://localhost:11434/api/generate"),
            "http://localhost:11434/api/embed"
        );
        assert_eq!(
            normalize_ollama_embed_url("http://localhost:11434"),
            "http://localhost:11434/api/embed"
        );
        assert_eq!(
            ollama_tags_url("http://localhost:11434/api/embed"),
            "http://localhost:11434/api/tags"
        );
    }

    #[test]
    fn normalizes_nonzero_embedding_vectors() {
        let normalized = normalize_embedding(vec![3.0, 4.0]);
        assert!((normalized[0] - 0.6).abs() < 0.0001);
        assert!((normalized[1] - 0.8).abs() < 0.0001);
        assert_eq!(normalize_embedding(vec![0.0, 0.0]), vec![0.0, 0.0]);
    }
}
