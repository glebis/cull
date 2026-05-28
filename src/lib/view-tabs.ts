import type { ViewMode } from './stores';

export interface ViewTab {
    id: ViewMode;
    label: string;
    key?: string;
    icon: string;
    requiresStaticPublishing?: boolean;
}

export const VIEW_TABS: ViewTab[] = [
    { id: 'grid', label: 'Grid', key: '⌘1', icon: '⊞' },
    { id: 'loupe', label: 'Loupe', key: '⌘2', icon: '◎' },
    { id: 'compare', label: 'Compare', key: '⌘3', icon: '◨' },
    { id: 'canvas', label: 'Canvas', key: '⌘4', icon: '▦' },
    { id: 'lineage', label: 'Lineage', key: '⌘5', icon: '⎇' },
    { id: 'embeddings', label: 'Embeddings', key: '⌘6', icon: '⁘' },
    { id: 'publish', label: 'Publish', icon: '↗', requiresStaticPublishing: true },
    { id: 'export', label: 'Export', key: '⌘7', icon: '⤓' },
];

export function visibleViewTabs(staticPublishingEnabled: boolean): ViewTab[] {
    return VIEW_TABS.filter(tab => !tab.requiresStaticPublishing || staticPublishingEnabled);
}
