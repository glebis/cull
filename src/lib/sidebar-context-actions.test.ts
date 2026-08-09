import { describe, expect, it, vi } from 'vitest';
import {
    buildCollectionContextActions,
    buildFolderContextActions,
    buildSmartCollectionContextActions,
} from './sidebar-context-actions';

describe('sidebar contextual action policy', () => {
    it('offers the supported folder actions and protects synthetic groups from mutation', async () => {
        const handlers = {
            onOpen: vi.fn(),
            onReveal: vi.fn(),
            onRename: vi.fn(),
            onRescan: vi.fn(),
            onRemove: vi.fn(),
        };
        const items = buildFolderContextActions({
            folder: '/Pictures/Run 1',
            name: 'Run 1',
            renamable: true,
            removable: true,
            ...handlers,
        });

        expect(items.map(item => item.label)).toEqual([
            'Open Folder',
            'Reveal in Finder',
            'Rename…',
            'Rescan Folder',
            'Remove Folder from Library…',
        ]);
        await items[2].action?.();
        expect(handlers.onRename).toHaveBeenCalledWith('/Pictures/Run 1', 'Run 1');
        expect(items.at(-1)).toMatchObject({ danger: true, separatorBefore: true });

        const group = buildFolderContextActions({
            folder: '/Pictures',
            name: 'Pictures',
            renamable: true,
            removable: false,
            ...handlers,
        });
        expect(group.map(item => item.label)).toEqual(['Open Folder', 'Reveal in Finder', 'Rename…']);
    });

    it('preserves collection actions and hides content actions for an empty collection', () => {
        const handlers = {
            onOpen: vi.fn(), onRename: vi.fn(), onDuplicate: vi.fn(), onExport: vi.fn(),
            onPublish: vi.fn(), onCollect: vi.fn(), onTogglePin: vi.fn(), onCopyId: vi.fn(), onDelete: vi.fn(),
        };
        const full = buildCollectionContextActions({
            collectionId: 'c1', name: 'Portfolio', count: 4, pinned: false, ...handlers,
        });
        expect(full.map(item => item.label)).toEqual([
            'Open Collection', 'Rename…', 'Duplicate…', 'Export to Folder…', 'Publish Collection',
            'Use for Collect Mode', 'Pin Collection', 'Copy Collection ID', 'Delete Collection…',
        ]);

        const empty = buildCollectionContextActions({
            collectionId: 'c1', name: 'Portfolio', count: 0, pinned: true, ...handlers,
        });
        expect(empty.map(item => item.label)).not.toContain('Export to Folder…');
        expect(empty.map(item => item.label)).not.toContain('Publish Collection');
        expect(empty.map(item => item.label)).toContain('Unpin Collection');
    });

    it('protects preset smart collections while exposing edit and delete for user collections', () => {
        const handlers = { onOpen: vi.fn(), onEdit: vi.fn(), onDelete: vi.fn() };
        const preset = buildSmartCollectionContextActions({
            id: 'preset', name: 'Recent Imports', isPreset: true, ...handlers,
        });
        expect(preset.map(item => item.label)).toEqual(['Open Smart Collection']);

        const saved = buildSmartCollectionContextActions({
            id: 'saved', name: 'Five Stars', isPreset: false, ...handlers,
        });
        expect(saved.map(item => item.label)).toEqual([
            'Open Smart Collection', 'Edit Rules…', 'Delete Smart Collection…',
        ]);
        expect(saved.at(-1)).toMatchObject({ danger: true, separatorBefore: true });
    });
});
