# Security Policy

## Supported Versions

Security fixes are accepted for the current development line until a stable release policy is published.

| Version | Supported |
|---|---|
| 0.1.x | Yes |

## Reporting A Vulnerability

Please report security issues privately by email to `glebis@gmail.com`.

Do not open a public GitHub issue for vulnerabilities involving:

- unauthorized file access
- path traversal
- leaking local image paths or thumbnails
- API key or MCP token exposure
- unintended network exposure of the MCP HTTP server
- destructive file operations

Include the affected version or commit, operating system, reproduction steps, expected behavior, actual behavior, and any relevant logs with private paths redacted.

## Security Model

Cull is local-first. The highest-risk surfaces are filesystem access, local database integrity, BYOK API key handling, MCP token handling, and optional HTTP MCP exposure. The HTTP MCP server should remain disabled unless explicitly enabled by the user.
