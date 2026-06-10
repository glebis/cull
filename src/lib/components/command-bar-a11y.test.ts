import { describe, expect, it } from 'vitest';
import { readFileSync } from 'node:fs';
import { join } from 'node:path';

// Source contract: the search bar renders two adjacent '×' buttons (clear
// vs close) plus an icon-only pill close. Each needs a distinct accessible
// name so screen-reader users do not have to trial-and-error them.
const bar = readFileSync(join(process.cwd(), 'src/lib/components/CommandBar.svelte'), 'utf8');

function attrsForButton(className: string): string {
    // Inline arrow-function handlers contain '>', so capture the whole
    // element instead of trying to terminate at the opening tag.
    const match = bar.match(new RegExp(`<button[^<]*class="${className}"[\\s\\S]*?</button>`));
    expect(match, `expected a <button class="${className}">`).not.toBeNull();
    return match![0];
}

describe('command bar button labeling contract (UX-06)', () => {
    it('labels the clear button distinctly', () => {
        const clear = attrsForButton('clear-btn');
        expect(clear).toContain('aria-label="Clear query"');
        expect(clear).toContain('title="Clear query"');
    });

    it('labels the close button distinctly', () => {
        const close = attrsForButton('close-btn');
        expect(close).toContain('aria-label="Close search"');
        expect(close).toContain('title="Close search"');
    });

    it('labels the collapsed-pill close button', () => {
        const pill = attrsForButton('pill-close');
        expect(pill).toContain('aria-label="Remove filter"');
        expect(pill).toContain('title="Remove filter"');
    });

    it('visually differentiates clear from close', () => {
        // Clear uses the backspace-style glyph; close keeps the ×.
        const clearBlock = bar.match(/<button[^>]*class="clear-btn"[\s\S]*?<\/button>/)![0];
        const closeBlock = bar.match(/<button[^>]*class="close-btn"[\s\S]*?<\/button>/)![0];
        expect(clearBlock).not.toBe(closeBlock);
        expect(clearBlock.includes('⌫')).toBe(true);
        expect(closeBlock.includes('×')).toBe(true);
    });

    it('gives every icon-only button in the bar an aria-label', () => {
        for (const cls of ['clear-btn', 'close-btn', 'pill-close', 'mic-btn', 'locale-btn']) {
            expect(attrsForButton(cls), `${cls} needs aria-label`).toContain('aria-label=');
        }
    });
});
