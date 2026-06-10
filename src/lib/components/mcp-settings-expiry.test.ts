import { describe, expect, it } from 'vitest';
import { readFileSync } from 'node:fs';
import { join } from 'node:path';

function source(path: string): string {
    return readFileSync(join(process.cwd(), path), 'utf8');
}

const settings = source('src/lib/components/McpSettings.svelte');

describe('McpSettings token-expiry create flow', () => {
    it('exposes an expiry control in the create-token form', () => {
        // A bindable expiry state and an explicit expiry input in the form.
        expect(settings).toContain('newExpiryDays');
        expect(settings).toContain('class="expiry-select"');
    });

    it('passes expires_at to createMcpToken when an expiry is chosen', () => {
        // handleCreate must compute an ISO expiry and forward it as the 4th arg.
        expect(settings).toMatch(/createMcpToken\(\s*newName\.trim\(\),\s*newRole,[^)]*expir/s);
    });

    it('renders relative expiry status via the shared helper', () => {
        expect(settings).toContain("from '$lib/token-expiry'");
        expect(settings).toContain('relativeExpiry(');
        expect(settings).toContain('expiryState(');
    });

    it('colors the expiry-soon and expired states with design tokens', () => {
        // warn -> orange, expired -> red, per design system.
        expect(settings).toContain('.token-expiry.warn');
        expect(settings).toContain('color: var(--orange)');
        expect(settings).toContain('.token-expiry.expired');
        expect(settings).toContain('color: var(--red)');
    });
});
