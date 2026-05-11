# ADR-003: MCP Server Instead of REST API

**Status:** Accepted
**Date:** 2026-05
**Author:** Gleb Kalinin

## Context

Need to expose ImageView's library to AI agents (Claude Code, Cursor, automation scripts). Traditional approach would be a REST API. MCP (Model Context Protocol) is Anthropic's emerging standard for tool-agent communication.

## Options Considered

1. **REST API** — Well-understood, broad compatibility, but agents need custom integration code per app
2. **GraphQL** — Flexible queries, but overkill for tool-style interactions
3. **MCP Server** — Native agent integration, tool discovery, structured I/O, emerging ecosystem standard

## Decision

Full MCP server with 23+ tools, role-based auth, HTTP + Unix socket transport.

## Rationale

- MCP is the native protocol for Claude Code, Cursor, and growing agent ecosystem
- Tool discovery means agents can explore capabilities without documentation
- Structured tool inputs/outputs are more reliable than REST for agent consumption
- Role-based auth (admin/curator/viewer) maps naturally to MCP capability scoping
- Unix socket for local agents (zero auth needed), HTTP for remote access (token auth)
- Job system for long-running operations (batch embedding, import) with progress tracking
- Being an early, comprehensive MCP implementation creates ecosystem positioning

## Consequences

- MCP is younger than REST — fewer client libraries, less documentation
- Must maintain both transport layers (socket + HTTP)
- Auth and scope filtering add complexity
- Coupling to Anthropic's protocol evolution
