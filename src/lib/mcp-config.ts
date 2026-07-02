/**
 * Canonical MCP client config for connecting an agent to Cull over stdio.
 * Single source of truth: rendered snippets and clipboard copies must both
 * use this string so users never paste something different from what they
 * read on screen. Assumes the `cull` CLI is on PATH (see docs/agents.md).
 */
export const MCP_CONFIG_SNIPPET = `{
  "mcpServers": {
    "cull": {
      "command": "cull",
      "args": ["--mcp-stdio"]
    }
  }
}`;
