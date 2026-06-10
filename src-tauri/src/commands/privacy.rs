// Copyright (c) 2026-present Gleb Kalinin. Architecture and design by author.
// Implementation assisted by Claude (Anthropic). See AUTHORSHIP.md.

use crate::services::audit;
use crate::AppState;
use serde::Serialize;
use tauri::State;
/// Check whether a URL points to a local address by extracting the hostname.
/// Returns true only for exact matches: `localhost`, `127.0.0.1`, `::1`.
fn is_local_url(raw: &str) -> bool {
    // Find the authority portion: skip past "://"
    let after_scheme = match raw.find("://") {
        Some(i) => &raw[i + 3..],
        None => return false,
    };

    // The authority ends at the next '/' or at end-of-string
    let authority = match after_scheme.find('/') {
        Some(i) => &after_scheme[..i],
        None => after_scheme,
    };

    // Strip optional userinfo (anything before '@')
    let host_port = match authority.rfind('@') {
        Some(i) => &authority[i + 1..],
        None => authority,
    };

    // Handle bracketed IPv6 addresses like [::1]:port
    let host = if host_port.starts_with('[') {
        match host_port.find(']') {
            Some(i) => &host_port[1..i],
            None => return false,
        }
    } else {
        // Strip port suffix for plain hosts
        match host_port.rfind(':') {
            Some(i) => &host_port[..i],
            None => host_port,
        }
    };

    host == "localhost" || host == "127.0.0.1" || host == "::1"
}

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
    let ollama_is_local = is_local_url(&ollama_url);
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
    let ollama_embedding_is_local = is_local_url(&ollama_embedding_url);
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn localhost_is_local() {
        assert!(is_local_url("http://localhost:11434/api/generate"));
    }

    #[test]
    fn localhost_with_port_is_local() {
        assert!(is_local_url("http://localhost:9847/mcp"));
    }

    #[test]
    fn loopback_ipv4_is_local() {
        assert!(is_local_url("http://127.0.0.1:11434/api/embed"));
    }

    #[test]
    fn loopback_ipv6_is_local() {
        assert!(is_local_url("http://[::1]:11434/api/generate"));
    }

    #[test]
    fn localhost_evil_subdomain_is_not_local() {
        assert!(!is_local_url("https://localhost.evil.com"));
    }

    #[test]
    fn remote_host_is_not_local() {
        assert!(!is_local_url("https://api.example.com/v1/embed"));
    }

    #[test]
    fn garbage_input_is_not_local() {
        assert!(!is_local_url("not-a-url"));
    }
}
