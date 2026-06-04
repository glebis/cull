import { describe, expect, it } from 'vitest';
import { readFileSync } from 'node:fs';
import { join } from 'node:path';

const root = process.cwd();

function source(path: string): string {
    return readFileSync(join(root, path), 'utf8');
}

describe('Agent view snapshot contract', () => {
    it('registers snapshot commands in Rust, permissions, and the API layer', () => {
        const lib = source('src-tauri/src/lib.rs');
        const permissions = source('src-tauri/permissions/app-ui.toml');
        const api = source('src/lib/api.ts');

        for (const command of [
            'capture_agent_window_snapshot',
            'complete_agent_view_snapshot',
            'get_last_agent_view_snapshot',
            'request_agent_view_snapshot',
        ]) {
            expect(lib).toContain(`commands::agent_snapshots::${command}`);
            expect(permissions).toContain(`"${command}"`);
            expect(api).toContain(`'${command}'`);
        }
    });

    it('surfaces an agent selection event payload in the frontend listener', () => {
        const page = source('src/routes/+page.svelte');

        expect(page).toContain('agent-view-snapshot:select-images');
        expect(page).toContain('applyAgentViewSnapshotSelection');
    });
});
