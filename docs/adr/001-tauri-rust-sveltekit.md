# ADR-001: Tauri 2 + Rust + SvelteKit 5

**Status:** Accepted
**Date:** 2025-04
**Author:** Gleb Kalinin

## Context

Need a desktop image viewer for AI-generated art. Must be fast for large image libraries, support local AI inference, and feel native on macOS.

## Options Considered

1. **Electron + TypeScript** — Large bundle, high RAM, no native perf for image processing
2. **Swift/AppKit** — Native but macOS-only, harder to add web-based UI features
3. **Tauri 2 + Rust + SvelteKit** — Small bundle, Rust for heavy lifting (ONNX, SQLite, image processing), Svelte for reactive UI

## Decision

Tauri 2 + Rust backend + SvelteKit 5 frontend.

## Rationale

- Rust gives native performance for CLIP inference (ONNX Runtime), SQLite operations, and image I/O without GC pauses
- Tauri 2 bundles are ~10MB vs ~200MB for Electron
- SvelteKit 5 runes provide fine-grained reactivity for gallery views with thousands of images
- Tauri's IPC layer (invoke/commands) gives a clean separation between Rust backend and web frontend
- Future-proof: Tauri 2 supports iOS/Android, enabling potential mobile companion app

## Consequences

- Must maintain two languages (Rust + TypeScript)
- Smaller ecosystem of Tauri plugins compared to Electron
- Rust compile times are slow during development
