import { readFileSync } from 'node:fs';
import { join } from 'node:path';
import { describe, expect, it } from 'vitest';

const read = (path: string) => readFileSync(join(process.cwd(), path), 'utf8');

describe('atomic sidebar folder rename contract', () => {
    it('registers one typed backend command and exposes it only from real folder rows', () => {
        const api = read('src/lib/api.ts');
        const rust = read('src-tauri/src/lib.rs');
        const sidebar = read('src/lib/components/Sidebar.svelte');

        expect(api).toContain("invoke<RenameFolderResult>('rename_folder', { folder, newName })");
        expect(rust).toContain('commands::files::rename_folder');
        expect(sidebar).toContain('openFolderContextMenu(e, folder.fullPath, folder.name, folder.isGroup)');
        expect(sidebar).toContain("title: 'Rename Folder'");
        expect(sidebar).toContain('handleRenameFolder(folderContextMenu!.path, folderName(folderContextMenu!.path))');
        expect(sidebar).toContain('await apiRenameFolder(folder, name.trim())');
        expect(sidebar).toContain('renamedFolderPath(path, result.oldPath, result.newPath)');
    });
});
