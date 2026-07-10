import { describe, expect, it } from 'vitest';
import { readFileSync } from 'node:fs';
import { join } from 'node:path';

const root = process.cwd();

function readProjectFile(path: string): string {
    return readFileSync(join(root, path), 'utf8');
}

describe('raw support default contract (module_raw un-gated in v1)', () => {
    it('defaults the Settings toggle to enabled unless explicitly disabled', () => {
        const settings = readProjectFile('src/lib/components/GeneralSettings.svelte');

        expect(settings).toContain("moduleRaw = raw !== 'false'");
        expect(settings).not.toContain("moduleRaw = raw === 'true'");
    });

    it('defaults module_raw to enabled in the Rust backend via a single shared helper', () => {
        const importRs = readProjectFile('src-tauri/src/db_core/import.rs');
        const libRs = readProjectFile('src-tauri/src/lib.rs');
        const commandsImportRs = readProjectFile('src-tauri/src/commands/import.rs');
        const servicesImportRs = readProjectFile('src-tauri/src/services/import.rs');

        expect(importRs).toContain('pub fn is_module_raw_enabled');
        expect(importRs).toContain('.unwrap_or(true)');
        for (const consumer of [libRs, commandsImportRs, servicesImportRs]) {
            expect(consumer).toContain('is_module_raw_enabled');
            expect(consumer).not.toMatch(/get_setting\("module_raw"\)/);
        }
    });

    it('does not label RAW support as experimental in settings', () => {
        const settings = readProjectFile('src/lib/components/GeneralSettings.svelte');

        expect(settings).toContain('RAW File Support');
        expect(settings.toLowerCase()).not.toContain('experimental');
    });
});
