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
});
