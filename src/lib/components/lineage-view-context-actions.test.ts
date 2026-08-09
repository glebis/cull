import { describe, expect, it } from 'vitest';
import { readFileSync } from 'node:fs';
import { join } from 'node:path';

const source = readFileSync(join(process.cwd(), 'src/lib/components/LineageView.svelte'), 'utf8');

describe('lineage group context action policy', () => {
    it('offers rename and a confirmed destructive dissolve action', () => {
        expect(source).toContain("label: 'Rename…'");
        expect(source).toContain("label: 'Dissolve Group…'");
        expect(source.indexOf("label: 'Dissolve Group…'")).toBeGreaterThan(source.indexOf("label: 'Rename…'"));
        expect(source).toContain('danger: true');
        expect(source).toContain('separatorBefore: true');
        expect(source).toContain('requestTextInput({');
        expect(source).toContain('requestConfirm({');
        expect(source).not.toContain('window.confirm');
    });

    it('makes group actions available from headers and tabs by pointer, keyboard, and overflow controls', () => {
        expect(source).toContain("event.key === 'ContextMenu' || (event.shiftKey && event.key === 'F10')");
        expect(source).toContain('oncontextmenu={(event) => openGroupContextMenu(event, group)}');
        expect(source).toContain('class="group-menu-button"');
        expect(source).toContain('class="group-tab-menu-button"');
        expect(source).toContain('opener: contextOpener(event)');
        expect(source).toContain("event.target.closest<HTMLElement>('button, [href], input, select, textarea, [tabindex]')");
        expect(source).toContain('{#key groupContextMenu.group.id}');
        expect(source).toContain('opener={groupContextMenu.opener}');
        expect(source).toContain('<ActionMenu');
    });
});
