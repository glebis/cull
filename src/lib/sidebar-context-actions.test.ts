import { describe, expect, it, vi } from 'vitest';
import {
    buildCollectionContextActions,
    buildCanvasContextActions,
    buildFolderContextActions,
    buildSmartCollectionContextActions,
} from './sidebar-context-actions';

describe('sidebar contextual action policy', () => {
    it('keeps canvas menus limited to supported open and delete operations', () => {
        const onOpen = vi.fn();
        const onDelete = vi.fn();
        const items = buildCanvasContextActions({ canvasId: 'canvas-1', name: 'Selects', onOpen, onDelete });
        expect(items.map(item => item.label)).toEqual(['Open Canvas', 'Delete Canvas…']);
        expect(items.at(-1)).toMatchObject({ danger: true, separatorBefore: true });
    });

    it('offers physical-folder actions, targeted rescan, and collection targets', async () => {
        const reveal = vi.fn();
        const rescan = vi.fn();
        const add = vi.fn();
        const create = vi.fn();
        const copy = vi.fn();
        const remove = vi.fn();
        const items = buildFolderContextActions({
            folder: '/Pictures/Run 1',
            removable: true,
            collections: [['c1', 'Portfolio', 12]],
            onReveal: reveal,
            onRescan: rescan,
            onAddToCollection: add,
            onCreateCollection: create,
            onCopyPath: copy,
            onRemove: remove,
        });

        expect(items.map(item => item.label)).toEqual([
            'Reveal in Finder',
            'Rescan Folder',
            'Add Contents to Collection',
            'Copy Path',
            'Remove Folder from Library…',
        ]);
        expect(items.map(item => item.label)).not.toContain('Rename Folder…');
        expect(items[2].children?.map(item => item.label)).toEqual(['New Collection…', 'Portfolio']);
        await items[2].children?.[1].action?.();
        expect(add).toHaveBeenCalledWith('/Pictures/Run 1', 'c1');
        expect(items.at(-1)).toMatchObject({ danger: true, separatorBefore: true });

        const root = buildFolderContextActions({
            folder: '/',
            removable: false,
            collections: [],
            onReveal: reveal,
            onRescan: rescan,
            onAddToCollection: add,
            onCreateCollection: create,
            onCopyPath: copy,
            onRemove: remove,
        });
        expect(root.map(item => item.label)).not.toContain('Rescan Folder');
        expect(root.map(item => item.label)).not.toContain('Remove Folder from Library…');
    });

    it('preserves supported collection actions and hides export for an empty collection', () => {
        const handlers = {
            onOpen: vi.fn(), onRename: vi.fn(), onExport: vi.fn(), onCollect: vi.fn(),
            onDuplicate: vi.fn(), onPublish: vi.fn(), onTogglePin: vi.fn(), onCopyId: vi.fn(), onDelete: vi.fn(),
        };
        const full = buildCollectionContextActions({
            collectionId: 'c1', name: 'Portfolio', count: 4, pinned: false, ...handlers,
        });
        expect(full.map(item => item.label)).toEqual([
            'Open Collection', 'Rename…', 'Duplicate…', 'Export to Folder…', 'Publish Collection', 'Use for Collect Mode',
            'Pin Collection', 'Copy Collection ID', 'Delete Collection…',
        ]);

        const empty = buildCollectionContextActions({
            collectionId: 'c1', name: 'Portfolio', count: 0, pinned: true, ...handlers,
        });
        expect(empty.map(item => item.label)).not.toContain('Export to Folder…');
        expect(empty.map(item => item.label)).toContain('Unpin Collection');
    });

    it('protects preset smart collections while exposing edit and delete for user collections', () => {
        const handlers = { onOpen: vi.fn(), onEdit: vi.fn(), onExport: vi.fn(), onDelete: vi.fn() };
        const preset = buildSmartCollectionContextActions({
            id: 'preset', name: 'Recent Imports', count: 5, isPreset: true, ...handlers,
        });
        expect(preset.map(item => item.label)).toEqual(['Open Smart Collection', 'Export Results…']);

        const saved = buildSmartCollectionContextActions({
            id: 'saved', name: 'Five Stars', count: 3, isPreset: false, ...handlers,
        });
        expect(saved.map(item => item.label)).toEqual([
            'Open Smart Collection', 'Edit Rules…', 'Export Results…', 'Delete Smart Collection…',
        ]);
        expect(saved.at(-1)).toMatchObject({ danger: true, separatorBefore: true });
    });
});
