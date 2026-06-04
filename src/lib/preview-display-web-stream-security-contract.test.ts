import { describe, expect, it } from 'vitest';
import { readFileSync } from 'node:fs';
import { join } from 'node:path';

const root = process.cwd();

describe('Preview Display web stream security contract', () => {
    it('escapes metadata before writing the web stream rail HTML', () => {
        const source = readFileSync(join(root, 'src-tauri/src/preview/web_stream.rs'), 'utf8');

        expect(source).toContain('function escapeHtml');
        expect(source).toContain('escapeHtml(item.filename)');
        expect(source).toContain('escapeHtml(item.decision)');
        expect(source).toContain('escapeHtml(source)');
        expect(source).toContain('escapeHtml(item.prompt)');
        expect(source).toContain("escapeHtml(item.tags.join(', '))");
    });
});
