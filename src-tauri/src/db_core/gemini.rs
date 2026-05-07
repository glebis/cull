use base64::Engine;
use reqwest::Client;
use serde::Deserialize;
use std::path::Path;

pub struct GeminiEmbeddingProvider {
    client: Client,
    api_key: String,
}

#[derive(Deserialize)]
struct EmbedResponse {
    embedding: EmbedValues,
}

#[derive(Deserialize)]
struct EmbedValues {
    values: Vec<f32>,
}

impl GeminiEmbeddingProvider {
    pub fn new(api_key: &str) -> Self {
        GeminiEmbeddingProvider {
            client: Client::new(),
            api_key: api_key.to_string(),
        }
    }

    pub async fn generate_embedding(&self, image_path: &Path) -> Result<Vec<f32>, String> {
        let image_bytes = std::fs::read(image_path).map_err(|e| format!("Read: {}", e))?;

        let mime = match image_path.extension().and_then(|e| e.to_str()) {
            Some("png") => "image/png",
            Some("jpg") | Some("jpeg") => "image/jpeg",
            Some("webp") => "image/webp",
            Some("gif") => "image/gif",
            _ => "image/png",
        };

        let b64 = base64::engine::general_purpose::STANDARD.encode(&image_bytes);

        let url = format!(
            "https://generativelanguage.googleapis.com/v1beta/models/gemini-embedding-exp-03-07:embedContent?key={}",
            self.api_key
        );

        let body = serde_json::json!({
            "model": "models/gemini-embedding-exp-03-07",
            "content": {
                "parts": [{
                    "inline_data": {
                        "mime_type": mime,
                        "data": b64
                    }
                }]
            }
        });

        let resp = self.client
            .post(&url)
            .json(&body)
            .send()
            .await
            .map_err(|e| format!("Request: {}", e))?;

        if !resp.status().is_success() {
            let text = resp.text().await.unwrap_or_default();
            return Err(format!("API error: {}", text));
        }

        let result: EmbedResponse = resp.json().await.map_err(|e| format!("Parse: {}", e))?;
        Ok(result.embedding.values)
    }
}
