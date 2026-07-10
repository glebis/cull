import { readFileSync, readdirSync } from 'node:fs';
import { join } from 'node:path';
import { describe, expect, it } from 'vitest';

const ROOT = join(process.cwd(), 'src');

function readCss(): string {
    return readFileSync(join(ROOT, 'app.css'), 'utf8');
}

function tokenValue(css: string, name: string): number {
    const m = css.match(new RegExp(`${name}:\\s*(\\d+)`));
    expect(m, `${name} defined in app.css`).not.toBeNull();
    return Number(m![1]);
}

describe('z-index scale', () => {
    it('defines a layered scale with toasts above every overlay', () => {
        const css = readCss();
        const panel = tokenValue(css, '--z-panel');
        const modal = tokenValue(css, '--z-modal');
        const contextMenu = tokenValue(css, '--z-context-menu');
        const palette = tokenValue(css, '--z-command-palette');
        const toast = tokenValue(css, '--z-toast');
        expect(panel).toBeLessThan(modal);
        expect(modal).toBeLessThan(contextMenu);
        expect(contextMenu).toBeLessThan(palette);
        expect(palette).toBeLessThan(toast);
    });

    it('components use scale tokens, not raw overlay z-index values', () => {
        // Any literal z-index at overlay altitude (>=900) must come from the
        // scale. Within-component stacking (small values) is fine.
        const dirs = [
            join(ROOT, 'lib/components'),
            join(ROOT, 'routes'),
            join(ROOT, 'lib/plugins/cull-publish'),
        ];
        const offenders: string[] = [];
        for (const dir of dirs) {
            for (const file of readdirSync(dir)) {
                if (!file.endsWith('.svelte')) continue;
                const src = readFileSync(join(dir, file), 'utf8');
                for (const m of src.matchAll(/z-index:\s*(\d+)/g)) {
                    if (Number(m[1]) >= 900) offenders.push(`${file}: z-index ${m[1]}`);
                }
            }
        }
        expect(offenders).toEqual([]);
    });
});
