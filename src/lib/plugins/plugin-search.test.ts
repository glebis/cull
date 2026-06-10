import { describe, it, expect } from 'vitest';
import { filterPlugins } from './plugin-search';

const items = [
    { id: 'cull-publish', name: 'Publish View (Static Site)', description: 'Build a static site package', permissions: ['export:read', 'library:read'] },
    { id: 'foo', name: 'Foo Tool', description: 'Does foo things', permissions: ['library:read'] },
];

describe('filterPlugins', () => {
    it('returns all on empty query', () => { expect(filterPlugins(items, '')).toHaveLength(2); });
    it('returns all on whitespace-only query', () => { expect(filterPlugins(items, '   ')).toHaveLength(2); });
    it('matches name case-insensitively', () => { expect(filterPlugins(items, 'publish').map(i => i.id)).toEqual(['cull-publish']); });
    it('matches description', () => { expect(filterPlugins(items, 'foo things').map(i => i.id)).toEqual(['foo']); });
    it('matches a permission', () => { expect(filterPlugins(items, 'export:read').map(i => i.id)).toEqual(['cull-publish']); });
    it('returns empty on no match', () => { expect(filterPlugins(items, 'zzz')).toEqual([]); });
});
