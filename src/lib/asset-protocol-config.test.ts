import { describe, expect, it } from 'vitest';
import config from '../../src-tauri/tauri.conf.json';

describe('Tauri asset protocol config', () => {
    it('uses a CSP instead of disabling browser protections', () => {
        const csp = config.app.security.csp;

        expect(csp).not.toBeNull();
        expect(csp['default-src']).toBe("'self'");
        expect(csp['img-src']).toContain('asset:');
        expect(Object.prototype.hasOwnProperty.call(csp, 'script-src')).toBe(false);
    });

    it('limits static asset protocol scope to app-owned/generated image folders', () => {
        const scope = config.app.security.assetProtocol.scope;
        const allow = Array.isArray(scope) ? scope : scope.allow;

        expect(allow).not.toContain('**');
        expect(allow).not.toContain('**/*');
        expect(allow).toEqual([
            '$APPDATA/thumbnails/**/*',
            '$APPDATA/generated/**/*',
            '$HOME/.codex/generated_images/**/*',
        ]);
    });
});
