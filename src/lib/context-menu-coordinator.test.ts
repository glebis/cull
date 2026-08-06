import { describe, expect, it, vi } from 'vitest';
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
});
