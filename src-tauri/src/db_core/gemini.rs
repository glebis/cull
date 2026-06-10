// Copyright (c) 2026-present Gleb Kalinin. Architecture and design by author.
// Implementation assisted by Claude (Anthropic). See AUTHORSHIP.md.

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

/// Gemini embedContent endpoint. The API key is sent via the `x-goog-api-key`
/// header (see `generate_embedding`), never as a `key=` query parameter, so the
/// secret cannot leak through request/proxy logs, crash strings, traces, or history.
pub(crate) const GEMINI_EMBED_CONTENT_URL: &str =
    "https://generativelanguage.googleapis.com/v1beta/models/gemini-embedding-exp-03-07:embedContent";

impl GeminiEmbeddingProvider {
    pub fn new(api_key: &str) -> Self {
        GeminiEmbeddingProvider {
            client: Client::new(),
            api_key: api_key.to_string(),
        }
    }

    /// Build the embedContent request with the API key in the `x-goog-api-key`
    /// header. Centralised so the auth transport is unit-testable and can never
    /// drift back to a `key=` query parameter.
    fn embed_request(&self, body: &serde_json::Value) -> reqwest::RequestBuilder {
        self.client
            .post(GEMINI_EMBED_CONTENT_URL)
            .header("x-goog-api-key", &self.api_key)
            .json(body)
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

        let resp = self
            .embed_request(&body)
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn embed_content_url_carries_no_api_key_query_param() {
        // The endpoint URL must never embed the API key, so the secret cannot
        // leak through request/proxy logs, crash strings, traces, or history.
        assert!(
            !GEMINI_EMBED_CONTENT_URL.contains("key="),
            "Gemini embedContent URL must not contain a key= query param: {}",
            GEMINI_EMBED_CONTENT_URL
        );
    }

    #[test]
    fn embed_request_sends_key_in_header_not_url() {
        let sentinel = "AIzaSySentinelKeyShouldNeverAppear";
        let provider = GeminiEmbeddingProvider::new(sentinel);
        let req = provider
            .embed_request(&serde_json::json!({"content": {}}))
            .build()
            .expect("request must build");

        // Key travels in the header...
        assert_eq!(
            req.headers()
                .get("x-goog-api-key")
                .map(|v| v.to_str().unwrap()),
            Some(sentinel)
        );
        // ...and never in the URL (no query string, no sentinel anywhere).
        assert_eq!(req.url().query(), None);
        let url = req.url().as_str();
        assert!(!url.contains("key="), "URL leaked a key= param: {}", url);
        assert!(!url.contains(sentinel), "URL leaked the API key: {}", url);
    }
}
