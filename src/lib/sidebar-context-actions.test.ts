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

    it('offers the supported folder actions, collection targets, and protects synthetic groups from mutation', async () => {
        const handlers = {
            onOpen: vi.fn(),
            onReveal: vi.fn(),
            onRename: vi.fn(),
            onRescan: vi.fn(),
            onAddToCollection: vi.fn(),
            onCreateCollection: vi.fn(),
            onCopyPath: vi.fn(),
            onRemove: vi.fn(),
        };
        const items = buildFolderContextActions({
            folder: '/Pictures/Run 1',
            name: 'Run 1',
            renamable: true,
            removable: true,
            collections: [['c1', 'Portfolio', 12]],
            ...handlers,
        });

        expect(items.map(item => item.label)).toEqual([
            'Open Folder',
            'Reveal in Finder',
            'Rename…',
            'Rescan Folder',
            'Add Contents to Collection',
            'Copy Path',
            'Remove Folder from Library…',
        ]);
        await items[2].action?.();
        expect(handlers.onRename).toHaveBeenCalledWith('/Pictures/Run 1', 'Run 1');
        const collectionItem = items.find(item => item.id === 'folder-add-to-collection');
        expect(collectionItem?.children?.map(item => item.label)).toEqual(['New Collection…', 'Portfolio']);
        await collectionItem?.children?.[1].action?.();
        expect(handlers.onAddToCollection).toHaveBeenCalledWith('/Pictures/Run 1', 'c1');
        expect(items.at(-1)).toMatchObject({ danger: true, separatorBefore: true });

        const group = buildFolderContextActions({
            folder: '/Pictures',
            name: 'Pictures',
            renamable: true,
            removable: false,
            collections: [],
            ...handlers,
        });
        expect(group.map(item => item.label)).not.toContain('Rescan Folder');
        expect(group.map(item => item.label)).not.toContain('Remove Folder from Library…');
        expect(group.map(item => item.label)).toEqual([
            'Open Folder', 'Reveal in Finder', 'Rename…', 'Add Contents to Collection', 'Copy Path',
        ]);
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

    it('protects preset smart collections while exposing edit, export, and delete for user collections', () => {
        const handlers = { onOpen: vi.fn(), onEdit: vi.fn(), onExport: vi.fn(), onDelete: vi.fn() };
        const preset = buildSmartCollectionContextActions({
            id: 'preset', name: 'Recent Imports', count: 5, isPreset: true, ...handlers,
        });
        expect(preset.map(item => item.label)).toEqual(['Open Smart Collection', 'Export Results…']);

        const empty = buildSmartCollectionContextActions({
            id: 'empty', name: 'No Matches', count: 0, isPreset: false, ...handlers,
        });
        expect(empty.map(item => item.label)).not.toContain('Export Results…');

        const saved = buildSmartCollectionContextActions({
            id: 'saved', name: 'Five Stars', count: 3, isPreset: false, ...handlers,
        });
        expect(saved.map(item => item.label)).toEqual([
            'Open Smart Collection', 'Edit Rules…', 'Export Results…', 'Delete Smart Collection…',
        ]);
        expect(saved.at(-1)).toMatchObject({ danger: true, separatorBefore: true });
    });
});
