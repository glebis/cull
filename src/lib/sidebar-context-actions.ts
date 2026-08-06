import type { ActionMenuItem } from './components/ActionMenu.svelte';

type CollectionRow = [id: string, name: string, count: number];

export interface CanvasContextActionOptions {
    canvasId: string;
    name: string;
    onOpen: (canvasId: string) => void | Promise<void>;
    onDelete: (canvasId: string, name: string) => void | Promise<void>;
}

export function buildCanvasContextActions(options: CanvasContextActionOptions): ActionMenuItem[] {
    return [
        { id: 'canvas-open', label: 'Open Canvas', action: () => options.onOpen(options.canvasId) },
        {
            id: 'canvas-delete',
            label: 'Delete Canvas…',
            action: () => options.onDelete(options.canvasId, options.name),
            danger: true,
            separatorBefore: true,
        },
    ];
}

export interface FolderContextActionOptions {
    folder: string;
    removable: boolean;
    collections: CollectionRow[];
    onReveal: (folder: string) => void | Promise<void>;
    onRescan: (folder: string) => void | Promise<void>;
    onAddToCollection: (folder: string, collectionId: string) => void | Promise<void>;
    onCreateCollection: (folder: string) => void | Promise<void>;
    onCopyPath: (folder: string) => void | Promise<void>;
    onRemove: (folder: string) => void | Promise<void>;
}

export function buildFolderContextActions(options: FolderContextActionOptions): ActionMenuItem[] {
    const collectionChildren: ActionMenuItem[] = [
        {
            id: 'folder-collection-new',
            label: 'New Collection…',
            action: () => options.onCreateCollection(options.folder),
        },
        ...options.collections.map(([id, name]) => ({
            id: `folder-collection-${id}`,
            label: name,
            action: () => options.onAddToCollection(options.folder, id),
        })),
    ];

    return [
        {
            id: 'folder-reveal',
            label: 'Reveal in Finder',
            action: () => options.onReveal(options.folder),
        },
        {
            id: 'folder-rescan',
            label: 'Rescan Folder',
            action: () => options.onRescan(options.folder),
        },
        {
            id: 'folder-add-to-collection',
            label: 'Add Contents to Collection',
            children: collectionChildren,
        },
        {
            id: 'folder-copy-path',
            label: 'Copy Path',
            action: () => options.onCopyPath(options.folder),
        },
        {
            id: 'folder-remove',
            label: 'Remove Folder from Library…',
            action: () => options.onRemove(options.folder),
            danger: true,
            separatorBefore: true,
            hidden: !options.removable,
        },
    ].filter(item => !item.hidden);
}

export interface CollectionContextActionOptions {
    collectionId: string;
    name: string;
    count: number;
    pinned: boolean;
    onOpen: (collectionId: string) => void | Promise<void>;
    onRename: (collectionId: string, name: string) => void | Promise<void>;
    onDuplicate: (collectionId: string, name: string) => void | Promise<void>;
    onExport: (collectionId: string) => void | Promise<void>;
    onPublish: (collectionId: string) => void | Promise<void>;
    onCollect: (collectionId: string, name: string) => void | Promise<void>;
    onTogglePin: (collectionId: string) => void | Promise<void>;
    onCopyId: (collectionId: string) => void | Promise<void>;
    onDelete: (collectionId: string, name: string) => void | Promise<void>;
}

export function buildCollectionContextActions(options: CollectionContextActionOptions): ActionMenuItem[] {
    return [
        { id: 'collection-open', label: 'Open Collection', action: () => options.onOpen(options.collectionId) },
        { id: 'collection-rename', label: 'Rename…', action: () => options.onRename(options.collectionId, options.name) },
        { id: 'collection-duplicate', label: 'Duplicate…', action: () => options.onDuplicate(options.collectionId, options.name) },
        {
            id: 'collection-export',
            label: 'Export to Folder…',
            action: () => options.onExport(options.collectionId),
            hidden: options.count === 0,
        },
        {
            id: 'collection-publish',
            label: 'Publish Collection',
            action: () => options.onPublish(options.collectionId),
            hidden: options.count === 0,
        },
        {
            id: 'collection-collect',
            label: 'Use for Collect Mode',
            action: () => options.onCollect(options.collectionId, options.name),
        },
        {
            id: 'collection-pin',
            label: options.pinned ? 'Unpin Collection' : 'Pin Collection',
            action: () => options.onTogglePin(options.collectionId),
        },
        {
            id: 'collection-copy-id',
            label: 'Copy Collection ID',
            action: () => options.onCopyId(options.collectionId),
        },
        {
            id: 'collection-delete',
            label: 'Delete Collection…',
            action: () => options.onDelete(options.collectionId, options.name),
            danger: true,
            separatorBefore: true,
        },
    ].filter(item => !item.hidden);
}

export interface SmartCollectionContextActionOptions {
    id: string;
    name: string;
    count: number;
    isPreset: boolean;
    onOpen: (id: string) => void | Promise<void>;
    onEdit: (id: string) => void | Promise<void>;
    onExport: (id: string) => void | Promise<void>;
    onDelete: (id: string, name: string) => void | Promise<void>;
}

export function buildSmartCollectionContextActions(options: SmartCollectionContextActionOptions): ActionMenuItem[] {
    return [
        { id: 'smart-open', label: 'Open Smart Collection', action: () => options.onOpen(options.id) },
        {
            id: 'smart-edit',
            label: 'Edit Rules…',
            action: () => options.onEdit(options.id),
            hidden: options.isPreset,
        },
        {
            id: 'smart-export',
            label: 'Export Results…',
            action: () => options.onExport(options.id),
            hidden: options.count === 0,
        },
        {
            id: 'smart-delete',
            label: 'Delete Smart Collection…',
            action: () => options.onDelete(options.id, options.name),
            danger: true,
            separatorBefore: true,
            hidden: options.isPreset,
        },
    ].filter(item => !item.hidden);
}
