import { describe, expect, it } from 'vitest';
import { readFileSync } from 'node:fs';
import { join } from 'node:path';

const root = process.cwd();

function readProjectFile(path: string): string {
    return readFileSync(join(root, path), 'utf8');
}

function menuSection(source: string, startMarker: string, endMarker: string): string {
    const start = source.indexOf(startMarker);
    const end = source.indexOf(endMarker, start);
    expect(start).toBeGreaterThanOrEqual(0);
    expect(end).toBeGreaterThan(start);
    return source.slice(start, end);
}

describe('File menu contract', () => {
    it('exposes Import Folder from the standard File menu', () => {
        const menuSource = readProjectFile('src-tauri/src/menu.rs');
        const fileMenu = menuSection(menuSource, '// File menu', '// Edit menu');

        expect(fileMenu).toContain('Submenu::new(app, "File", true)');
        expect(fileMenu).toContain('"import_folder"');
        expect(fileMenu).toContain('"Import Folder..."');
        expect(fileMenu.indexOf('"open_file"')).toBeLessThan(fileMenu.indexOf('"import_folder"'));
        expect(menuSource).toContain('"import_folder"');
    });

    it('exposes Apple Photos discovery only in the macOS File menu', () => {
        const menuSource = readProjectFile('src-tauri/src/menu.rs');
        const fileMenu = menuSection(menuSource, '// File menu', '// Edit menu');

        expect(fileMenu).toContain('#[cfg(target_os = "macos")]');
        expect(fileMenu).toContain('"import_apple_photos"');
        expect(fileMenu).toContain('"Import from Apple Photos..."');
        expect(fileMenu.indexOf('"import_folder"')).toBeLessThan(fileMenu.indexOf('"import_apple_photos"'));
    });
});
