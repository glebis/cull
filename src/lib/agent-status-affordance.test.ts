import { readFileSync } from 'node:fs';
import { join } from 'node:path';
import { describe, expect, it } from 'vitest';

function source(path: string): string {
    return readFileSync(join(process.cwd(), path), 'utf8');
}

describe('agent status affordance', () => {
    it('exposes MCP connection status through the Tauri API wrapper', () => {
        const api = source('src/lib/api.ts');
        const backend = source('src-tauri/src/lib.rs');
        const readPermission = source('src-tauri/permissions/app-read.toml');

        expect(api).toContain('export interface McpStatus');
        expect(api).toContain("invoke('get_mcp_status')");
        expect(backend).toContain('commands::mcp::get_mcp_status');
        expect(readPermission).toContain('"get_mcp_status"');
    });

    it('renders a persistent Agent control with idle, connected, and active states', () => {
        const statusBar = source('src/lib/components/StatusBar.svelte');
        const page = source('src/routes/+page.svelte');

        expect(statusBar).toContain('getMcpStatus');
        expect(statusBar).toContain('agentConnectionState');
        expect(statusBar).toContain('Idle');
        expect(statusBar).toContain('connected');
        expect(statusBar).toContain('Active');
        expect(statusBar).toContain('agent-status-dot');
        expect(statusBar).toContain('aria-label={agentButtonAriaLabel}');
        expect(page).toContain('<StatusBar agentBusy={agentChatBusy} />');
    });
});
