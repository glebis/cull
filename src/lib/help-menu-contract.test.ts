import { describe, expect, it } from 'vitest';
import { readFileSync } from 'node:fs';
import { join } from 'node:path';

const root = process.cwd();
const userGuideUrl = 'https://github.com/glebis/cull/blob/main/docs/USER_GUIDE.md';

function readProjectFile(path: string): string {
    return readFileSync(join(root, path), 'utf8');
}

describe('Help menu contract', () => {
    it('labels the Help menu action as the Cull User Guide', () => {
        const menuSource = readProjectFile('src-tauri/src/menu.rs');

        expect(menuSource).toContain('"help"');
        expect(menuSource).toContain('"Cull User Guide"');
        expect(menuSource).not.toContain('"Cull Help"');
    });

    it('opens the user guide instead of a repository README', () => {
        const menuSource = readProjectFile('src/lib/menu.ts');

        expect(menuSource).toContain(userGuideUrl);
        expect(menuSource).not.toMatch(/imageview#readme|cull#readme/i);
    });

    it('keeps the user guide covering the core user workflows', () => {
        const guide = readProjectFile('docs/USER_GUIDE.md');

        for (const heading of [
            '## Install And Run From Source',
            '## Import Images',
            '## Navigate Views',
            '## Review And Curate',
            '## Collections',
            '## Embeddings And Search',
            '## Export Images',
            '## Privacy Defaults',
            '## CLI',
        ]) {
            expect(guide).toContain(heading);
        }
    });
});
