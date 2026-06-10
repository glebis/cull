import { describe, expect, it } from 'vitest';
import { readFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import config from '../../src-tauri/tauri.conf.json';

describe('Tauri asset protocol config', () => {
    it('uses a CSP instead of disabling browser protections', () => {
        const csp = config.app.security.csp;

        expect(csp).not.toBeNull();
        expect(csp['default-src']).toBe("'self'");
        expect(csp['img-src']).toContain('asset:');
        expect(csp['connect-src']).not.toContain('http://localhost:*');
        expect(csp['connect-src']).not.toContain('http://127.0.0.1:*');
        // The plugin runtime (bd imageview-dkz.23) widens script-src by exactly
        // blob: for hash-verified plugin imports — nothing else.
        expect(csp['script-src']).toBe("'self' blob:");
    });

    it('limits static asset protocol scope to app-owned/generated image folders', () => {
        const scope = config.app.security.assetProtocol.scope;
        const allow = Array.isArray(scope) ? scope : scope.allow;

        expect(allow).not.toContain('**');
        expect(allow).not.toContain('**/*');
        expect(allow).toEqual([
            '$APPDATA/thumbnails/**/*',
            '$APPDATA/generated/**/*',
        ]);
        // Never ship developer-personal paths (e.g. ~/.codex) in the scope.
        for (const entry of allow) {
            expect(entry).not.toMatch(/\.codex/);
            expect(entry.startsWith('$HOME')).toBe(false);
        }
    });

    it('does not expand the asset protocol scope at runtime for imported originals', () => {
        const runtimeSources = [
            '../../src-tauri/src/lib.rs',
            '../../src-tauri/src/commands/import.rs',
            '../../src-tauri/src/commands/clipboard_monitor.rs',
        ].map(path => readFileSync(fileURLToPath(new URL(path, import.meta.url)), 'utf8'));

        for (const source of runtimeSources) {
            expect(source).not.toContain('.asset_protocol_scope()');
            expect(source).not.toContain('allow_directory(');
            expect(source).not.toContain('allow_file(');
        }
    });
});
