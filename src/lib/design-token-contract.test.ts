import { describe, expect, it } from 'vitest';
import { readFileSync, readdirSync, statSync } from 'node:fs';
import { join } from 'node:path';

const root = process.cwd();

function source(path: string): string {
    return readFileSync(join(root, path), 'utf8');
}

function svelteFiles(directory: string): string[] {
    return readdirSync(join(root, directory)).flatMap(name => {
        const path = join(directory, name);
        const stat = statSync(join(root, path));
        if (stat.isDirectory()) return svelteFiles(path);
        return name.endsWith('.svelte') ? [path] : [];
    });
}

describe('global design-token contract', () => {
    it('does not hide component token drift behind hex fallbacks', () => {
        const offenders = svelteFiles('src').flatMap(path => {
            const matches = source(path).match(/var\(--[^,)]+,\s*#[0-9a-f]{3,8}\)/gi) ?? [];
            return matches.map(match => `${path}: ${match}`);
        });

        expect(offenders).toEqual([]);
    });

    it('defines legacy aliases while product chrome uses canonical names', () => {
        const appCss = source('src/app.css');
        const loupe = source('src/lib/components/Loupe.svelte');

        expect(appCss).toContain('--text-primary: var(--text);');
        expect(appCss).toContain('--bg-elevated: var(--surface);');
        expect(appCss).toContain('--font-mono: var(--font);');
        expect(loupe).not.toMatch(/var\(--(?:text-primary|bg-elevated|font-mono)\)/);
    });

    it('keeps missing-image and export-terminal chrome on canonical tokens', () => {
        const thumbnail = source('src/lib/components/Thumbnail.svelte');
        const terminal = source('src/lib/components/ExportSlideTerminal.svelte');

        expect(thumbnail).not.toContain('#f87171');
        expect(thumbnail).not.toContain('rgba(127, 29, 29');
        expect(thumbnail).toContain('color: var(--red);');
        expect(terminal).not.toContain('color: #7a7fa0;');
        expect(terminal).not.toContain('border: 1px solid #1a1a2e;');
        expect(terminal).toContain('color: var(--text-secondary);');
        expect(terminal).toContain('border: 1px solid var(--border);');
    });
});
