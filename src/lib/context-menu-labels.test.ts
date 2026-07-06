import { describe, expect, it } from 'vitest';
import { readFileSync } from 'node:fs';
import { join } from 'node:path';

const contextMenuSource = readFileSync(
    join(process.cwd(), 'src/lib/components/ContextMenu.svelte'),
    'utf8',
);

describe('context menu decision labels', () => {
    it('labels the accept decision as Accept, not Select', () => {
        const acceptHandlerIndex = contextMenuSource.indexOf("onclick={() => handleDecision('accept')}");
        const acceptItemStart = contextMenuSource.lastIndexOf('<button', acceptHandlerIndex);
        const acceptItemEnd = contextMenuSource.indexOf('</button>', acceptHandlerIndex);
        const acceptItem = contextMenuSource.slice(acceptItemStart, acceptItemEnd);

        expect(acceptItem).toContain('<span>Accept</span>');
        expect(acceptItem).not.toContain('<span>Select</span>');
    });

    it('keeps the collection submenu filterable and viewport constrained', () => {
        expect(contextMenuSource).toContain('class="submenu collection-submenu"');
        expect(contextMenuSource).toContain('placeholder="Filter collections"');
        expect(contextMenuSource).toContain('filteredCollectionList');
        expect(contextMenuSource).toContain('+ New Collection...');
        expect(contextMenuSource).toContain('class="context-menu-item collection-item"');
        expect(contextMenuSource).toContain('class="collection-name"');
        expect(contextMenuSource).toContain('--submenu-top');
        expect(contextMenuSource).toContain('--submenu-max-height');
    });
});
