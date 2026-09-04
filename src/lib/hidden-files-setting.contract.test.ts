import { describe, expect, it } from 'vitest';
import { readFileSync } from 'node:fs';
import { join } from 'node:path';

describe('hidden files preference contract', () => {
    it('exposes a default-off General setting backed by the shared app setting', () => {
        const settings = readFileSync(join(process.cwd(), 'src/lib/components/GeneralSettings.svelte'), 'utf8');

        expect(settings).toContain("getAppSetting('show_hidden_files')");
        expect(settings).toContain("setAppSetting('show_hidden_files'");
        expect(settings).toContain('Show hidden files');
        expect(settings).toContain("showHiddenFiles = hidden === 'true'");
    });
});
