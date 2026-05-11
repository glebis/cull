# Authorship

## Human Author

**Gleb Kalinin** — Architecture, design, product decisions, and creative direction.

All architectural decisions, data models, API interfaces, component hierarchies, UX patterns, design system, and system integration choices are human-authored. This includes:

- Technology stack selection (Tauri 2, Rust, SvelteKit 5, SQLite, ONNX Runtime)
- Application architecture and module boundaries
- MCP server design (23+ tools, role-based auth, job system, scope filtering)
- Database schema and query patterns
- CLIP embedding pipeline design
- UI/UX design (Tokyo Night theme, keyboard-first interaction, Capture One-inspired workflows)
- Design system tokens and component conventions
- All product and feature decisions

## AI Implementation

Code implementation was assisted by **Claude** (Anthropic). The AI generated syntax, function bodies, and boilerplate under human architectural direction and iterative review.

The human author provided:
- Architectural specifications before code generation (design docs, AGENTS.md)
- Codebase constraints that shaped all output (CLAUDE.md, design system)
- Iterative review, rejection, and refinement of AI-generated code
- All debugging and integration decisions

## Copyright Notice

Architecture and design copyright (c) 2025-2026 Gleb Kalinin. Implementation assisted by Claude (Anthropic) under Anthropic's Commercial Terms of Service, which assign output ownership to the customer.

## Why This File Exists

Courts in the US (USCO guidance, 2025) and Germany (2026 rulings) recognize copyright in AI-assisted works when the human provides "creative influence on the design of the concrete work itself." This file documents the human creative process to establish copyrightability under both jurisdictions.
