import { describe, expect, it } from 'vitest';
import { readFileSync, readdirSync, statSync } from 'node:fs';
import { join } from 'node:path';

const root = process.cwd();

function readProjectFile(path: string): string {
    return readFileSync(join(root, path), 'utf8');
}

function walk(dir: string, files: string[] = []): string[] {
    for (const entry of readdirSync(join(root, dir))) {
        const rel = join(dir, entry);
        if (statSync(join(root, rel)).isDirectory()) {
            walk(rel, files);
        } else {
            files.push(rel);
        }
    }
    return files;
}

describe('plugin runtime contract (default off, no ungated surface)', () => {
    it('keeps the CSP script-src widened to blob: only', () => {
        const conf = JSON.parse(readProjectFile('src-tauri/tauri.conf.json'));
        expect(conf.app.security.csp['script-src']).toBe("'self' blob:");
        // The rest of the CSP stays tight.
        expect(conf.app.security.csp['default-src']).toBe("'self'");
        expect(conf.app.security.csp['object-src']).toBe("'none'");
    });

    it('gates plugin loading in the page behind the module_plugins flag', () => {
        const page = readProjectFile('src/routes/+page.svelte');
        expect(page).toContain("getAppSetting('module_plugins')");
        expect(page).toContain('pluginsEnabled');
        // The loading call site (not the import) comes after the flag check.
        const flagIdx = page.indexOf("getAppSetting('module_plugins')");
        const loadIdx = page.lastIndexOf('loadInstalledPlugins()');
        expect(loadIdx).toBeGreaterThan(flagIdx);
        expect(page).toMatch(/if\s*\(\s*get\(pluginsEnabled\)|if\s*\(\s*\$pluginsEnabled|if\s*\(pluginsRuntimeEnabled\)/);
    });

    it('palette only exposes plugin commands through the flag-gated registry', () => {
        const palette = readProjectFile('src/lib/command-palette.ts');
        expect(palette).toContain('getPluginPaletteCommands');

        const loader = readProjectFile('src/lib/plugins/loader.ts');
        expect(loader).toContain('get(pluginsEnabled)');
    });

    it('no plugin entry point outside the plugins module and the gated page wiring', () => {
        const offenders: string[] = [];
        for (const file of walk('src')) {
            if (!/\.(ts|svelte)$/.test(file) || file.endsWith('.test.ts')) continue;
            if (file.includes(join('src', 'lib', 'plugins'))) continue;
            const content = readFileSync(join(root, file), 'utf8');
            if (content.includes('plugin_invoke') || content.includes('loadInstalledPlugins') || content.includes('getPluginPaletteCommands')) {
                offenders.push(file);
            }
        }
        offenders.sort();
        expect(offenders).toEqual([
            join('src', 'lib', 'command-palette.ts'),
            join('src', 'routes', '+page.svelte'),
        ]);
    });

    it('the plugins flag store defaults to off', () => {
        const stores = readProjectFile('src/lib/stores.ts');
        expect(stores).toContain('export const pluginsEnabled = writable<boolean>(false);');
    });
});
