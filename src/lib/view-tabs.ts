import type { ViewMode } from './stores';

export interface ViewTab {
    id: ViewMode;
    label: string;
    key?: string;
    icon: ViewTabIconId;
    /** Render as an icon-first tab. The label appears for the active tab and
     * as a hover/focus overlay without changing layout. */
    compact?: boolean;
    /** When true, this tab only appears if a plugin has registered its id
     * (e.g. publish is owned by the bundled cull-publish plugin). Avoids the
     * gate-drift the tab registry exists to prevent. */
    requiresRegisteredTab?: boolean;
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
    { id: 'grid', label: 'Grid', key: '⌘1', icon: 'grid-contact-sheet', compact: true },
    { id: 'loupe', label: 'Loupe', key: '⌘2', icon: 'loupe-focus', compact: true },
    { id: 'compare', label: 'Compare', key: '⌘3', icon: 'compare-split', compact: true },
    { id: 'canvas', label: 'Canvas', key: '⌘4', icon: 'canvas-board', compact: true },
    { id: 'lineage', label: 'Lineage', key: '⌘5', icon: 'lineage-branch', compact: true },
    { id: 'embeddings', label: 'Embeddings', key: '⌘6', icon: 'embedding-map', compact: true },
    { id: 'publish', label: 'Publish', icon: 'publish-launch', compact: true, requiresRegisteredTab: true },
    { id: 'export', label: 'Export', key: '⌘7', icon: 'export-package', compact: true },
];

export function visibleViewTabs(registeredTabIds: Set<string>): ViewTab[] {
    return VIEW_TABS.filter(tab => !tab.requiresRegisteredTab || registeredTabIds.has(tab.id));
}
