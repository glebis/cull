import { describe, expect, it } from 'vitest';
import config from '../../src-tauri/tauri.conf.json';

describe('Tauri asset protocol config', () => {
    it('allows generated images stored under the hidden .codex directory', () => {
        const scope = config.app.security.assetProtocol.scope;
        const allow = Array.isArray(scope) ? scope : scope.allow;

        expect(allow).toContain('**');
        expect(allow).toContain('$HOME/.codex/generated_images/**');
    });
});
