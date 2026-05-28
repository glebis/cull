import type { ViewMode } from './stores';

export interface ViewTab {
    id: ViewMode;
    label: string;
    key?: string;
    icon: ViewTabIconId;
    requiresStaticPublishing?: boolean;
}

export type ViewTabIconId =
    | 'grid-contact-sheet'
    | 'loupe-focus'
    | 'compare-split'
    | 'canvas-board'
    | 'lineage-branch'
    | 'embedding-map'
    | 'publish-launch'
    | 'export-package';

export const VIEW_TABS: ViewTab[] = [
    { id: 'grid', label: 'Grid', key: '⌘1', icon: 'grid-contact-sheet' },
    { id: 'loupe', label: 'Loupe', key: '⌘2', icon: 'loupe-focus' },
    { id: 'compare', label: 'Compare', key: '⌘3', icon: 'compare-split' },
    { id: 'canvas', label: 'Canvas', key: '⌘4', icon: 'canvas-board' },
    { id: 'lineage', label: 'Lineage', key: '⌘5', icon: 'lineage-branch' },
    { id: 'embeddings', label: 'Embeddings', key: '⌘6', icon: 'embedding-map' },
    { id: 'publish', label: 'Publish', icon: 'publish-launch', requiresStaticPublishing: true },
    { id: 'export', label: 'Export', key: '⌘7', icon: 'export-package' },
];

export function visibleViewTabs(staticPublishingEnabled: boolean): ViewTab[] {
    return VIEW_TABS.filter(tab => !tab.requiresStaticPublishing || staticPublishingEnabled);
}
