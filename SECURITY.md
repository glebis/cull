# Security Policy

## Reporting a Vulnerability

Report security issues privately to `glebis@gmail.com`. Do not open public issues for vulnerabilities involving file access, path traversal, API key exposure, MCP token leaks, or unintended network exposure.

Include: affected version/commit, OS, reproduction steps, and relevant logs with private paths redacted.

## Threat Model

Cull is a local-first Tauri 2 desktop app. It never phones home. All data stays on disk and in macOS Keychain. The attack surface is the local machine and optional network listeners.

### MCP Unix Socket (`mcp.sock`)

- Located at `$APPDATA/mcp.sock` with `0600` permissions (owner-only read/write).
- Connections authenticate as `AuthContext::Local` and bypass all capability checks — full admin access.
- **This is intentional.** The socket is the local control plane. File permissions are the sole access control boundary.
- Threat: a local process running as the same user can connect and issue any command. This matches the macOS security model — same-user processes already have equivalent filesystem access.

### MCP HTTP Server

- **Off by default.** Must be explicitly enabled by the user.
- Requires a Bearer token for every request. Tokens are SHA-256 hashed with a per-install pepper stored in macOS Keychain.
- Tokens carry a role: Viewer, Curator, Operator, or Admin. Each role maps to a set of capabilities; requests outside the role's scope are rejected.
- Cross-origin requests blocked via `Origin` header validation.
- Threat: token leakage grants remote access scoped to the token's role. Mitigation: tokens never leave Keychain, pepper is per-install, and the server binds to `127.0.0.1` by default.

### Deep Links (`cull://`)

- URL-based deep links validate all paths against the user's home directory.
- Sensitive directories are blocked: `.ssh`, `.gnupg`, `.config`, `.aws`, `.kube`, and similar.
- File-based invocations (drag-drop, Finder "Open With") are treated as user-initiated and trusted.
- Threat: a malicious `cull://` URL could attempt path traversal. Mitigation: allowlist validation rejects paths outside `$HOME` and blocks known sensitive subdirectories.

### API Key Storage

- All third-party API keys (OpenAI, Google, Cohere, OpenRouter) are stored in macOS Keychain via the `keyring` crate.
- Keys are never written to SQLite, config files, or logs.
- The frontend queries key-presence via boolean flags from the database — actual secrets never cross the IPC boundary for display.

### Static Publishing

- The local preview server binds to `127.0.0.1` only — not exposed to the network.
- Export paths are validated against the user's home directory using the same rules as deep links.

### Asset Protocol

- Tauri's `asset:` protocol is scoped to three directories only:
  - `$APPDATA/thumbnails/**`
  - `$APPDATA/generated/**`
  - `$HOME/.codex/generated_images/**`
- Cull does not add imported library roots, imported original files, or user-selected clipboard capture folders to that scope at runtime.
- All other filesystem paths are inaccessible via the asset protocol.

## Supported Versions

| Version | Supported |
|---------|-----------|
| 0.1.x   | Yes       |
