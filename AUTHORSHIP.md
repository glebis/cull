# Authorship

*Last updated: 2026-06-04*

## Human Author

**Gleb Kalinin** (Berlin, Germany) — Architecture, design, product decisions, and creative direction.
Contact: glebis@gmail.com

All architectural decisions, data models, API interfaces, component hierarchies, UX patterns, design system, and system integration choices are human-authored. This includes:

- Technology stack selection (Tauri 2, Rust, SvelteKit 5, SQLite, ONNX Runtime)
- Application architecture and module boundaries
- MCP server design: 23+ tool definitions, role-based auth hierarchy (admin/curator/viewer), capability scoping, job system with progress/cancellation, dual transport (Unix socket + HTTP), token management, and path redaction — all designed by the human author
- Database schema and query patterns (rusqlite chosen over SQLx for simplicity in embedded context)
- CLIP embedding pipeline design (local ONNX chosen over cloud APIs for privacy)
- BYOK (Bring Your Own Key) architecture for all cloud APIs — chosen to keep users' data relationships directly with providers
- UI/UX design (Tokyo Night theme, keyboard-first interaction, Capture One-inspired workflows)
- Design system tokens and component conventions
- All product and feature decisions

## AI Implementation

Code implementation was assisted by **Claude** (Anthropic) and **Codex** (OpenAI). The AI generated syntax, function bodies, and boilerplate under human architectural direction and iterative review.

The human author provided:
- Architectural specifications before code generation (design docs in `docs/`, `AGENTS.md`)
- Codebase constraints that shaped all output (`CLAUDE.md`, design system rules)
- Iterative review, rejection, and refinement of AI-generated UX design, interactions and code
- All debugging and integration decisions
- Architecture Decision Records documenting rationale (see `docs/adr/`)

Development session logs (Claude Code and Codex transcripts) are retained locally as contemporaneous evidence of the creative direction process.

## Copyright Notice

Architecture and design copyright (c) 2026-present Gleb Kalinin.
Implementation was assisted by Claude (Anthropic) and Codex (OpenAI) under human direction, review, and integration. Provider output terms are not treated as a substitute for source provenance, license compatibility review, or human authorship documentation.

## AI Provider Output Terms

Cull's release posture does not rely on provider terms alone, but the project records the relevant output-rights claims for transparency:

- OpenAI states that, as between the user and OpenAI and to the extent permitted by law, users `"own the Output"`: https://openai.com/policies/row-terms-of-use/
- Anthropic states that its Commercial Terms let customers `"retain ownership rights"` over generated outputs: https://www.anthropic.com/news/expanded-legal-protections-api-improvements

These terms support commercial use of assisted implementation output, but they do not eliminate the need for human authorship, provenance review, dependency-license review, or checks for copied public code.

## Cloud API Providers

This application optionally connects to third-party cloud APIs. Each provider operates under its own terms, certifications, and data handling policies. See `docs/PRIVACY.md` for a full data flow map, provider compliance details, and jurisdiction information.

## Why This File Exists

Copyright protection for AI-assisted works depends on human authorship and jurisdiction-specific originality standards. This file documents the human creative process behind Cull so that the project can distinguish human architecture, selection, arrangement, and review from machine-assisted implementation details. Architecture Decision Records (`docs/adr/`) provide additional evidence of human creative choices.
