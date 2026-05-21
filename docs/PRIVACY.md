# Privacy & Data Flow

*Last updated: 2026-05-21*

This document describes what data Cull sends to external services, where it goes, and under what legal framework each provider operates.

## Core Principle

Cull is local-first. All core features (viewing, rating, collections, CLIP search, object detection) run entirely on your machine. Cloud features are strictly opt-in via BYOK (Bring Your Own Key).

## Data Flow Summary

| Feature | Destination | Data Sent | Leaves Machine? |
|---|---|---|---|
| CLIP embeddings | Local (ONNX Runtime) | Nothing | No |
| Object detection | Local (ONNX Runtime) | Nothing | No |
| Ollama vision | User-configured (default: localhost) | Image bytes (base64) | Only if remote Ollama |
| Gemini embeddings | Google servers (US) | Image bytes + API key | Yes |
| OpenAI generation | OpenAI servers (US) | Prompt + optional image + API key | Yes |
| OpenRouter generation | OpenRouter servers (US) | Prompt + optional image + API key | Yes |
| MCP server (local) | Unix socket (localhost) | Metadata, paths | No |
| MCP server (HTTP) | Network-exposed | Metadata, paths, thumbnails | If enabled |
| SQLite database | Local file | Nothing | No |

## Provider Compliance Details

### OpenAI (`api.openai.com`)

| | |
|---|---|
| **Company** | OpenAI, Inc. (San Francisco, CA, USA) |
| **SOC 2 Type II** | See OpenAI trust portal for current report scope |
| **GDPR** | DPA and transfer terms are available from OpenAI; check current terms before processing regulated data |
| **EU data residency** | Availability depends on account, product, and current OpenAI offering |
| **Data retention** | Check current API data retention settings and account eligibility |
| **Training on API inputs** | API training defaults and opt-in settings are governed by current OpenAI policy |
| **DPA** | openai.com/policies/data-processing-addendum/ |
| **ToS** | openai.com/policies/terms-of-use/ |
| **Trust portal** | trust.openai.com |

**What Cull sends:** Prompts (text) and optionally source images (base64) for image generation. API key is sent as a bearer token.

### Google Gemini API (`generativelanguage.googleapis.com`)

| | |
|---|---|
| **Company** | Google LLC (Mountain View, CA, USA) |
| **Certifications** | SOC 1/2/3, ISO 9001, ISO/IEC 27001, 27017, 27018, 27701, 42001 |
| **GDPR** | DPA available via Google Cloud terms |
| **EU data residency** | Available via Vertex AI / Enterprise |
| **Data retention** | Check current Google AI / Google Cloud terms |
| **Training on API inputs** | Free and paid tiers can differ; check current Google AI / Google Cloud terms before use |
| **DPA** | Via Google Cloud terms |
| **ToS** | ai.google.dev/gemini-api/terms |

**What Cull sends:** Full images (base64-encoded) for embedding generation. API key is sent as a URL parameter.

**IMPORTANT:** Gemini privacy behavior depends on the API tier and account terms. Verify the current Google terms before sending private or sensitive images.

### OpenRouter (`openrouter.ai`)

| | |
|---|---|
| **Company** | OpenRouter (US) |
| **SOC 2** | Advertised for Enterprise tier. No public report available |
| **GDPR** | Claims compliance. Relies on SCCs and adequacy decisions |
| **EU data residency** | Enterprise tier only |
| **Data retention** | Depends on selected routing and downstream providers |
| **Training on API inputs** | Downstream providers' policies apply |
| **DPA** | Not publicly listed (Enterprise agreements only) |
| **ToS** | openrouter.ai/terms |
| **Privacy** | openrouter.ai/privacy |

**What Cull sends:** Prompts and optionally source images for generation. Note: OpenRouter is a proxy — data ultimately reaches downstream providers (OpenAI, Anthropic, Google, etc.). Privacy posture is only as strong as the weakest provider in the chain.

### Ollama (localhost)

| | |
|---|---|
| **Software** | Ollama (open-source, local inference) |
| **Certifications** | None — local software, no third-party audit needed |
| **Data residency** | Your machine |
| **Data retention** | Local only |
| **Training** | No data collection. Limited opt-out telemetry (version, request counts — not prompts) |
| **Privacy** | ollama.com/privacy |

**What Cull sends:** Images (base64) to the Ollama API endpoint. By default this is `localhost:11434` — data never leaves your machine unless you configure a remote Ollama instance.

## GDPR Risk Assessment

| Provider | Risk | Notes |
|---|---|---|
| CLIP (local) | **None** | No data processing outside your machine |
| Ollama (local) | **None** | Same as above |
| OpenAI API | **Review required** | Check current account data controls, DPA, residency, and retention settings |
| Google Gemini (paid) | **Review required** | Check current Google AI or Vertex AI terms, DPA, residency, and training policy |
| Google Gemini (free) | **Review required** | Free tier privacy and training terms can differ from paid/enterprise terms |
| OpenRouter | **Medium** | Proxy model means data reaches downstream providers. Enterprise tier recommended |

## Recommendations for Users

1. **For maximum privacy:** Use only local features (CLIP, object detection, Ollama with localhost). No data leaves your machine.
2. **For cloud features with privacy:** Use provider accounts with documented retention, training, residency, and DPA settings.
3. **Avoid for sensitive images:** Any free or proxy tier whose current terms you have not reviewed.
4. **EU/GDPR compliance:** Confirm a current DPA, transfer mechanism, data residency setting, and retention setting before processing images of identifiable people.

## For Developers / Self-hosters

All external API calls are made from:
- `src-tauri/src/db_core/gemini.rs` — Gemini embedding requests
- `src-tauri/src/services/generation.rs` — Image generation (OpenAI, OpenRouter, Google)
- `src-tauri/src/db_core/vision.rs` — Ollama vision requests

No other code in the application makes external network requests.
