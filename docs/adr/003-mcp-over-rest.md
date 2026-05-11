# ADR-003: MCP Server Instead of REST API

**Status:** Accepted
**Date:** 2026-05
**Author:** Gleb Kalinin

## Context

Need to expose Cull's library to AI agents (Claude Code, Cursor, automation scripts). Traditional approach would be a REST API. MCP (Model Context Protocol) is Anthropic's emerging standard for tool-agent communication.

## Options Considered

1. **REST API** — Well-understood, broad compatibility, but agents need custom integration code per app
2. **GraphQL** — Flexible queries, but overkill for tool-style interactions
3. **MCP Server** — Native agent integration, tool discovery, structured I/O, emerging ecosystem standard

## Decision

Full MCP server with 23+ tools, role-based auth, HTTP + Unix socket transport. Every aspect of the MCP architecture — tool taxonomy, auth hierarchy, scope filtering, job lifecycle, transport strategy — was designed by the human author.

## Rationale

- MCP is the native protocol for Claude Code, Cursor, and growing agent ecosystem
- Tool discovery means agents can explore capabilities without documentation
- Structured tool inputs/outputs are more reliable than REST for agent consumption
- Role-based auth (admin/curator/viewer) maps naturally to MCP capability scoping — the three-tier hierarchy was designed to match real photography workflow roles
- Unix socket for local agents (zero auth needed), HTTP for remote access (token auth) — dual transport was a deliberate architectural choice to serve both CLI tools and remote agents
- Job system for long-running operations (batch embedding, import) with progress tracking and cancellation — designed around the constraint that image processing can take minutes
- Scope filtering (`check_image_id_scope`, `image_in_scope`, `maybe_redact_path`) was designed to allow partial library access for untrusted agents
- Being an early, comprehensive MCP implementation creates ecosystem positioning

**Rejected alternatives within MCP:**
- Single flat tool list (no auth) — too dangerous for remote access to image libraries
- REST gateway in front of MCP — unnecessary complexity, MCP HTTP transport is sufficient
- Per-tool auth — too granular; capability-based grouping is more maintainable

## Consequences

- MCP is younger than REST — fewer client libraries, less documentation
- Must maintain both transport layers (socket + HTTP)
- Auth and scope filtering add complexity
- Coupling to Anthropic's protocol evolution
