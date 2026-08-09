import { describe, expect, it, vi } from 'vitest';
import { readFileSync } from 'node:fs';
import { join } from 'node:path';
import { claimContextMenu } from './context-menu-coordinator';

describe('context menu coordinator', () => {
    it('closes the previous owner and does not let stale cleanup release the current owner', () => {
        const first = vi.fn();
        const second = vi.fn();
        const releaseFirst = claimContextMenu(first);
        const releaseSecond = claimContextMenu(second);
        expect(first).toHaveBeenCalledOnce();

        releaseFirst();
        const third = vi.fn();
        const releaseThird = claimContextMenu(third);
        expect(second).toHaveBeenCalledOnce();
        releaseSecond();
        expect(third).not.toHaveBeenCalled();
        releaseThird();
    });

    it('is claimed by both shared sidebar menus and the existing image menu', () => {
        const actionMenu = readFileSync(join(process.cwd(), 'src/lib/components/ActionMenu.svelte'), 'utf8');
        const imageMenu = readFileSync(join(process.cwd(), 'src/lib/components/ContextMenu.svelte'), 'utf8');

        for (const source of [actionMenu, imageMenu]) {
            expect(source).toContain("import { claimContextMenu } from '$lib/context-menu-coordinator'");
            expect(source).toContain('claimContextMenu(onclose)');
        }
    });
});
