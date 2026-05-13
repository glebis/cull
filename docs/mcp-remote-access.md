# MCP Remote Access

Cull exposes an MCP server that lets AI agents browse, curate, search, and export your image library. Three connection methods: local stdio, LAN HTTP, or tunneled HTTP.

## Local Access (Claude Code)

Add to your Claude Code MCP config (`~/.claude/settings.json` or project `.claude/settings.json`):

```json
{
  "mcpServers": {
    "cull": {
      "command": "/Applications/Cull.app/Contents/MacOS/cull",
      "args": ["--mcp-stdio"]
    }
  }
}
```

The stdio bridge auto-launches Cull in tray mode if it isn't already running. Local connections get full admin access with no token required — the Unix socket uses filesystem permissions (`0600`).

## HTTP Access

### Enable

Via CLI:

```bash
cull --mcp-http          # default port 9847
cull --mcp-http 8080     # custom port
```

Or toggle in **Settings > MCP Server > HTTP Server**.

Default bind: `127.0.0.1:9847` (localhost only). Change the listen address in settings to bind to LAN.

### Create a Token

Open **Settings > MCP Server > Access Tokens > Create Token**. Pick a name and role. The secret is shown once — copy it immediately.

### Connect

Any MCP client that supports Streamable HTTP can connect with a bearer token:

```bash
curl -H "Authorization: Bearer <token>" \
  -H "Content-Type: application/json" \
  http://localhost:9847/mcp \
  -d '{"jsonrpc":"2.0","method":"tools/list","id":1}'
```

## Tunnel Recipes

### Tailscale (recommended)

Install Tailscale on both machines. Access via Tailscale IP:

```
http://<tailscale-ip>:9847
```

Already encrypted end-to-end. No extra TLS needed. Best for personal use across your own devices.

### Cloudflare Tunnel

```bash
cloudflared tunnel --url http://localhost:9847
```

Handles TLS automatically. Gives you a public `*.trycloudflare.com` URL. Good for sharing with collaborators temporarily.

### ngrok

```bash
ngrok http 9847
```

Quick way to get a public URL for testing. Free tier has session limits.

## Roles

| Role | Can do | Use case |
|---|---|---|
| **viewer** | Read images, folders, collections. Search by similarity and detected objects. | Telegram bot browsing a gallery |
| **curator** | Viewer + set ratings, decisions. Create/manage collections. Export. | Friend curating a shared project |
| **operator** | Curator + import folders/files. Run AI engines (embeddings, detection, vision). | Automation pipeline |
| **admin** | Everything + control app display, manage tokens, change settings. | Your own Claude Code session |

### Scope Restrictions

Tokens can optionally restrict access to specific content:

```json
{
  "collections": ["col_abc"],
  "folders": ["/art/midjourney"],
  "tags": ["public"]
}
```

Union semantics: an image is accessible if it matches any filter. Null scope means unrestricted.

## Security

- HTTP binds to `127.0.0.1` by default. Explicit setting required for LAN/public binding.
- Every HTTP request requires a valid `Authorization: Bearer <token>` header.
- Filesystem paths are redacted for remote clients — they see filenames only, not full paths.
- Create scoped tokens with minimal roles for remote access. Never share admin tokens over tunnels.
- Tokens can be rotated (new secret, old one invalidated) or revoked in Settings.
- All MCP tool invocations are logged in the audit log (Settings > MCP Server).

## Troubleshooting

| Problem | Fix |
|---|---|
| "Connection refused" | HTTP server not enabled. Start with `--mcp-http` or toggle in Settings. |
| "401 Unauthorized" | Token invalid, expired, or revoked. Create a new one in Settings. |
| "Socket not found" (stdio) | App not running. Start with `cull` or `cull --tray`. |
| Tools return empty results | Token scope may restrict visible content. Check scope filters. |
| "Permission denied" on a tool | Token role lacks the required capability. See role table above. |
