import { describe, expect, it } from 'vitest';
import { visibleViewTabs } from './view-tabs';

describe('view tabs', () => {
    it('hides Publish while the Static Publishing module is disabled', () => {
        const ids = visibleViewTabs(false).map(tab => tab.id);

        expect(ids).not.toContain('publish');
        expect(ids).toContain('export');
    });

    it('shows Publish immediately before Export when Static Publishing is enabled', () => {
        const ids = visibleViewTabs(true).map(tab => tab.id);

        expect(ids).toContain('publish');
        expect(ids.indexOf('publish')).toBe(ids.indexOf('export') - 1);
    });
});
