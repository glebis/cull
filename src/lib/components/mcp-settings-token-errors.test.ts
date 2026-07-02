import { readFileSync } from 'node:fs';
import { join } from 'node:path';
import { describe, expect, it } from 'vitest';

// Token create/revoke/rotate are security-relevant: a failed revoke that only
// hits console.error leaves the user believing access was removed. Each
// handler's catch block must surface a visible error toast.
describe('McpSettings token operation error surfacing', () => {
    const src = readFileSync(join(process.cwd(), 'src/lib/components/McpSettings.svelte'), 'utf8');

    function catchBlockOf(handler: string): string {
        const start = src.indexOf(`async function ${handler}`);
        expect(start, `${handler} exists`).toBeGreaterThan(-1);
        const end = src.indexOf('async function', start + 1);
        return src.slice(start, end === -1 ? undefined : end);
    }

    for (const [handler, message] of [
        ['handleCreate', 'Could not create token'],
        ['handleRevoke', 'Could not revoke token'],
        ['handleRotate', 'Could not rotate token'],
    ] as const) {
        it(`${handler} shows an error toast on failure`, () => {
            const block = catchBlockOf(handler);
            expect(block).toContain(`showToast('${message}'`);
            expect(block).toContain("type: 'error'");
        });
    }
});
