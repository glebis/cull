import { describe, expect, it } from 'vitest';
import { readFileSync } from 'node:fs';
import { join } from 'node:path';

function source(path: string): string {
    return readFileSync(join(process.cwd(), path), 'utf8');
}

const dashboard = source('src/lib/components/PrivacyDashboard.svelte');
const api = source('src/lib/api.ts');

describe('PrivacyDashboard MCP audit visibility', () => {
    it('loads the MCP token audit log alongside the API call log', () => {
        expect(api).toContain('getMcpAuditLog');
        expect(api).toContain('get_mcp_audit_log');
        expect(dashboard).toContain('getMcpAuditLog');
        expect(dashboard).toContain('mcpAudit');
    });

    it('renders an MCP audit section listing tool, status, and time', () => {
        expect(dashboard).toContain('Agent Access Log');
        // Each row binds the audited tool name and result status.
        expect(dashboard).toContain('entry.tool_name');
        expect(dashboard).toContain('entry.result_status');
    });

    it('highlights _auth_failed and other non-ok rows with the red token', () => {
        // A denied/unauthorized/error row must be visually distinct.
        expect(dashboard).toContain("entry.result_status === 'ok'");
        expect(dashboard).toContain('var(--red)');
    });

    it('surfaces a count of recent failed-auth events', () => {
        expect(dashboard).toContain("_auth_failed");
        expect(dashboard).toContain('authFailedCount');
    });
});
