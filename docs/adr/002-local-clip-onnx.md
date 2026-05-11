# ADR-002: Local CLIP Embeddings via ONNX Runtime

**Status:** Accepted
**Date:** 2025-05
**Author:** Gleb Kalinin

## Context

Need semantic search across image libraries — "find images similar to this" and "find images matching this text description." Cloud embedding APIs exist (OpenAI, Google) but create privacy and latency concerns.

## Options Considered

1. **Cloud API (OpenAI embeddings)** — Easy integration, but every image sent to US servers, ongoing API cost, requires internet
2. **Local CLIP via Python** — Good models available, but Python dependency is heavy for a desktop app
3. **Local CLIP via ONNX Runtime in Rust** — Run CLIP model locally, no data leaves machine, no API cost

## Decision

ONNX Runtime in Rust with CLIP ViT-B/32 model, bundled with the app.

## Rationale

- Privacy: images never leave the user's machine for core search functionality
- Performance: ONNX Runtime on Apple Silicon is fast enough for real-time embedding
- Cost: zero per-image cost after initial model download
- Offline: works without internet connection
- GDPR: no data processing agreement needed for core functionality
- Cloud embeddings (Gemini) available as opt-in alternative for users who want higher quality

## Consequences

- App bundle is larger (~150MB with model)
- CLIP ViT-B/32 is less capable than latest cloud models
- Must handle model updates/versioning ourselves
