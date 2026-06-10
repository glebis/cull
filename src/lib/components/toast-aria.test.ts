import { describe, expect, it } from 'vitest';
import { readFileSync } from 'node:fs';
import { join } from 'node:path';

// Source contract: toasts carry all success/error feedback, so screen
// readers must be told about them. The container is a polite live region;
// error toasts escalate to role="alert" (implicit assertive).
const toast = readFileSync(join(process.cwd(), 'src/lib/components/Toast.svelte'), 'utf8');

describe('toast accessibility contract (UX-05)', () => {
    it('exposes the toast container as a polite live region', () => {
        expect(toast).toContain('role="status"');
        expect(toast).toContain('aria-live="polite"');
    });

    it('escalates error toasts to role="alert"', () => {
        expect(toast).toMatch(/toast\.type === 'error' \? 'alert'/);
    });

    it('keeps the container live region on the always-rendered element, not behind the length gate', () => {
        // The live region must exist before the first toast arrives, or
        // screen readers may miss the initial announcement.
        const gateIndex = toast.indexOf('$toasts.length > 0');
        const statusIndex = toast.indexOf('role="status"');
        expect(statusIndex).toBeGreaterThan(-1);
        expect(gateIndex === -1 || statusIndex < gateIndex).toBe(true);
    });
});
