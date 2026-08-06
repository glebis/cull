import { describe, expect, it } from 'vitest';
import { readFileSync } from 'node:fs';
import { join } from 'node:path';

const source = readFileSync(join(process.cwd(), 'src/lib/components/SessionSwitcher.svelte'), 'utf8');

describe('session switcher context action policy', () => {
    it('offers only the supported session actions, with delete last and safely confirmed', () => {
        expect(source).toContain("label: 'Open Session'");
        expect(source).toContain("label: 'Reveal Session Folder in Finder'");
        expect(source).toContain("label: 'Convert to Collection'");
        expect(source).toContain("label: 'Delete Session…'");
        expect(source.indexOf("label: 'Delete Session…'")).toBeGreaterThan(source.indexOf("label: 'Convert to Collection'"));
        expect(source).toContain('danger: true');
        expect(source).toContain('separatorBefore: true');
        expect(source).toContain("deleteSession(session.id, false)");
        expect(source).toContain('requestConfirm({');
        expect(source).toContain('Original files stay on disk.');
        expect(source).not.toContain('window.confirm');
    });

    it('makes session actions available through pointer, keyboard, and an overflow control', () => {
        expect(source).toContain("event.key === 'ContextMenu' || (event.shiftKey && event.key === 'F10')");
        expect(source).toContain('oncontextmenu={(event) => openSessionContextMenu(event, session)}');
        expect(source).toContain('onkeydown={(event) => { if (isContextMenuKey(event)) openSessionContextMenu(event, session); }}');
        expect(source).toContain('class="session-menu-button"');
        expect(source).toContain('<ActionMenu');
        expect(source).toContain('revealItemInDir(session.folder_path)');
    });
});
