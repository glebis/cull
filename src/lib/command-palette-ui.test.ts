import { describe, expect, it } from 'vitest';
import { readFileSync } from 'node:fs';
import { join } from 'node:path';

const componentSource = readFileSync(
    join(process.cwd(), 'src/lib/components/CommandPalette.svelte'),
    'utf8',
);
const keySource = readFileSync(join(process.cwd(), 'src/lib/keys.ts'), 'utf8');
const menuSource = readFileSync(join(process.cwd(), 'src-tauri/src/menu.rs'), 'utf8');

describe('command palette UI contract', () => {
    it('wires the query input as an accessible combobox controlling the result list', () => {
        expect(componentSource).toContain('role="combobox"');
        expect(componentSource).toContain('aria-controls={COMMAND_PALETTE_RESULTS_ID}');
        expect(componentSource).toContain('aria-activedescendant={selectedItem ? commandOptionId(selectedItem.id) : undefined}');
        expect(componentSource).toContain('id={COMMAND_PALETTE_RESULTS_ID}');
        expect(componentSource).toContain('id={commandOptionId(item.id)}');
    });

    it('keeps global palette shortcuts in the app keyboard pipeline', () => {
        expect(keySource).toContain("e.key.toLowerCase() === 'p' && e.metaKey && !e.shiftKey");
        expect(keySource).toContain("openCommandPalette('commands')");
        expect(keySource).toContain("openCommandPalette('all')");
        expect(keySource).toContain("openCommandPalette('commands')");
    });

    it('exposes the command palette from the native View menu', () => {
        expect(menuSource).toContain('"command_palette"');
        expect(menuSource).toContain('"Command Palette..."');
        expect(menuSource).toContain('Some::<&str>("CmdOrCtrl+P")');
        expect(menuSource).toMatch(/"command_palette"[\s\S]*app\.emit\("menu-action", id\)/);
    });

    it('does not hardcode colors in the palette styles', () => {
        const style = componentSource.slice(
            componentSource.indexOf('<style>'),
            componentSource.indexOf('</style>'),
        );
        expect(style).not.toMatch(/#[0-9a-fA-F]{3,8}\b/);
        expect(style).not.toMatch(/rgba?\(/);
        expect(style).not.toMatch(/hsla?\(/);
    });
});
