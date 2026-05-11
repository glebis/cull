# Authorship

*Last updated: 2026-05-12*

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

Code implementation was assisted by **Claude** (Anthropic). The AI generated syntax, function bodies, and boilerplate under human architectural direction and iterative review.

The human author provided:
- Architectural specifications before code generation (design docs in `docs/`, `AGENTS.md`)
- Codebase constraints that shaped all output (`CLAUDE.md`, design system rules)
- Iterative review, rejection, and refinement of AI-generated code
- All debugging and integration decisions
- Architecture Decision Records documenting rationale (see `docs/adr/`)

Development session logs (Claude Code transcripts) are retained locally as contemporaneous evidence of the creative direction process.

## Copyright Notice

Architecture and design copyright (c) 2025-present Gleb Kalinin. Implementation assisted by Claude (Anthropic) under Anthropic's Commercial Terms of Service, which assign output ownership to the customer and include copyright indemnification.

## Cloud API Providers

This application optionally connects to third-party cloud APIs. Each provider operates under its own terms, certifications, and data handling policies. See `docs/PRIVACY.md` for a full data flow map, provider compliance details, and jurisdiction information.

## Why This File Exists

Courts in the US (USCO guidance, 2025) and Germany (2026 rulings) recognize copyright in AI-assisted works when the human provides "creative influence on the design of the concrete work itself." This file documents the human creative process to establish copyrightability under both jurisdictions. Architecture Decision Records (`docs/adr/`) provide additional evidence of human creative choices.
