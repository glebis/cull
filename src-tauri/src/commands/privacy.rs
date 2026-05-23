// Copyright (c) 2025-present Gleb Kalinin. Architecture and design by author.
// Implementation assisted by Claude (Anthropic). See AUTHORSHIP.md.

use crate::services::audit;
use crate::AppState;
use serde::Serialize;
use tauri::State;

#[derive(Debug, Serialize)]
pub struct DataFlowEntry {
    pub feature: String,
    pub status: String,
    pub server: String,
    pub data_sent: String,
}

#[tauri::command]
pub async fn get_data_flow_status(
    state: State<'_, AppState>,
) -> Result<Vec<DataFlowEntry>, String> {
    let mut entries = Vec::new();

    let clip_available = {
        let engine = state.embedding_engine.lock();
        engine.is_model_available()
    };
    entries.push(DataFlowEntry {
        feature: "CLIP embeddings".into(),
        status: if clip_available { "local" } else { "off" }.into(),
        server: "—".into(),
        data_sent: "Nothing".into(),
    });

    entries.push(DataFlowEntry {
        feature: "Object detection".into(),
        status: "local".into(),
        server: "—".into(),
        data_sent: "Nothing".into(),
    });

    let ollama_url = state
        .db
        .get_setting("ollama_url")
        .ok()
        .flatten()
        .unwrap_or_else(|| "http://localhost:11434/api/generate".to_string());
    let ollama_is_local = ollama_url.contains("localhost") || ollama_url.contains("127.0.0.1");
    entries.push(DataFlowEntry {
        feature: "Ollama vision".into(),
        status: if ollama_is_local { "local" } else { "active" }.into(),
        server: if ollama_is_local {
            "localhost".into()
        } else {
            "remote".into()
        },
        data_sent: "Images".into(),
    });

    let gemini_key = state
        .db
        .get_setting("api_key_exists_google")
        .ok()
        .flatten()
        .map(|v| v == "true")
        .unwrap_or(false);
    entries.push(DataFlowEntry {
        feature: "Gemini embeddings".into(),
        status: if gemini_key { "active" } else { "off" }.into(),
        server: if gemini_key {
            "US 🇺🇸".into()
        } else {
            "—".into()
        },
        data_sent: if gemini_key {
            "Images + API key".into()
        } else {
            "Nothing".into()
        },
    });

    let cohere_key = state
        .db
        .get_setting("api_key_exists_cohere")
        .ok()
        .flatten()
        .map(|v| v == "true")
        .unwrap_or(false);
    entries.push(DataFlowEntry {
        feature: "Cohere multimodal embeddings".into(),
        status: if cohere_key { "active" } else { "off" }.into(),
        server: if cohere_key {
            "CA/US".into()
        } else {
            "—".into()
        },
        data_sent: if cohere_key {
            "Images + API key".into()
        } else {
            "Nothing".into()
        },
    });

    let ollama_embedding_url = state
        .db
        .get_setting("ollama_embedding_url")
        .ok()
        .flatten()
        .unwrap_or_else(|| "http://localhost:11434/api/embed".to_string());
    let ollama_embedding_is_local =
        ollama_embedding_url.contains("localhost") || ollama_embedding_url.contains("127.0.0.1");
    entries.push(DataFlowEntry {
        feature: "Ollama embeddings".into(),
        status: if ollama_embedding_is_local {
            "local"
        } else {
            "active"
        }
        .into(),
        server: if ollama_embedding_is_local {
            "localhost".into()
        } else {
            "remote".into()
        },
        data_sent: "Metadata text".into(),
    });

    let openai_key = state
        .db
        .get_setting("api_key_exists_openai")
        .ok()
        .flatten()
        .map(|v| v == "true")
        .unwrap_or(false);
    entries.push(DataFlowEntry {
        feature: "OpenAI generation".into(),
        status: if openai_key { "configured" } else { "off" }.into(),
        server: if openai_key {
            "US 🇺🇸".into()
        } else {
            "—".into()
        },
        data_sent: if openai_key {
            "Prompts + images".into()
        } else {
            "Nothing".into()
        },
    });
    entries.push(DataFlowEntry {
        feature: "OpenAI embeddings".into(),
        status: if openai_key { "configured" } else { "off" }.into(),
        server: if openai_key {
            "US 🇺🇸".into()
        } else {
            "—".into()
        },
        data_sent: if openai_key {
            "Metadata text + API key".into()
        } else {
            "Nothing".into()
        },
    });

    let openrouter_key = state
        .db
        .get_setting("api_key_exists_openrouter")
        .ok()
        .flatten()
        .map(|v| v == "true")
        .unwrap_or(false);
    entries.push(DataFlowEntry {
        feature: "OpenRouter generation".into(),
        status: if openrouter_key { "configured" } else { "off" }.into(),
        server: if openrouter_key {
            "US 🇺🇸".into()
        } else {
            "—".into()
        },
        data_sent: if openrouter_key {
            "Prompts + images".into()
        } else {
            "Nothing".into()
        },
    });

    let mcp_http = state
        .db
        .get_setting("mcp_http_enabled")
        .ok()
        .flatten()
        .map(|v| v == "true")
        .unwrap_or(false);
    entries.push(DataFlowEntry {
        feature: "MCP (local)".into(),
        status: "local".into(),
        server: "localhost".into(),
        data_sent: "Metadata".into(),
    });
    entries.push(DataFlowEntry {
        feature: "MCP (HTTP)".into(),
        status: if mcp_http { "active" } else { "off" }.into(),
        server: if mcp_http {
            "network".into()
        } else {
            "—".into()
        },
        data_sent: "Metadata".into(),
    });

    Ok(entries)
}

#[tauri::command]
pub async fn get_api_audit_log(
    state: State<'_, AppState>,
    limit: u32,
) -> Result<Vec<audit::AuditLogEntry>, String> {
    let clamped = limit.min(100).max(1);
    audit::get_audit_log(&state.db, clamped).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn export_audit_log(state: State<'_, AppState>) -> Result<String, String> {
    audit::export_audit_log_json(&state.db).map_err(|e| e.to_string())
}
