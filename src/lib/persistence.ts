import { get } from 'svelte/store';
import {
    viewMode, thumbnailSize, gridPreset, gridGap, gridScrollTop,
    sidebarVisible, zenMode, activeFolder, activeCollection,
    activeSmartCollection, activeDetectedClass, minSizeFilter, loupeScale, loupePanX, loupePanY,
    lineageLayout, showDetectionBoxes, nsfwMode, embeddingViewState,
    focusedIndex, images,
    type ViewMode, type LineageLayout, type NsfwMode, type EmbeddingViewState,
} from './stores';

const STORAGE_KEY = 'cull-app-state';
const SCHEMA_VERSION = 1;

export interface PersistedState {
    _version: number;
    viewMode: ViewMode;
    thumbnailSize: number;
    gridPreset: number;
    gridGap: number;
    focusedIndex?: number;
    gridScrollTop?: number;
    loadedImageCount?: number;
    sidebarVisible: boolean;
    zenMode: boolean;
    activeFolder: string | null;
    activeCollection: string | null;
    activeSmartCollectionId: string | null;
    activeDetectedClass?: string | null;
    minSizeFilter: number;
    loupeScale: number;
    loupePanX: number;
    loupePanY: number;
    lineageLayout: LineageLayout;
    showDetectionBoxes: boolean;
    nsfwMode: NsfwMode;
    embeddingViewState: EmbeddingViewState;
}

export function saveAppState(): void {
    const state: PersistedState = {
        _version: SCHEMA_VERSION,
        viewMode: get(viewMode),
        thumbnailSize: get(thumbnailSize),
        gridPreset: get(gridPreset),
        gridGap: get(gridGap),
        focusedIndex: get(focusedIndex),
        gridScrollTop: get(gridScrollTop),
        loadedImageCount: get(images).length,
        sidebarVisible: get(sidebarVisible),
        zenMode: get(zenMode),
        activeFolder: get(activeFolder),
        activeCollection: get(activeCollection),
        activeSmartCollectionId: get(activeSmartCollection)?.id ?? null,
        activeDetectedClass: get(activeDetectedClass),
        minSizeFilter: get(minSizeFilter),
        loupeScale: get(loupeScale),
        loupePanX: get(loupePanX),
        loupePanY: get(loupePanY),
        lineageLayout: get(lineageLayout),
        showDetectionBoxes: get(showDetectionBoxes),
        nsfwMode: get(nsfwMode),
        embeddingViewState: get(embeddingViewState),
    };
    try {
        localStorage.setItem(STORAGE_KEY, JSON.stringify(state));
    } catch {
        // localStorage full or unavailable — silent fail
    }
}

export function restoreAppStateBeforeImages(): PersistedState | null {
    try {
        const raw = localStorage.getItem(STORAGE_KEY);
        if (!raw) return null;
        const state: PersistedState = JSON.parse(raw);
        if (state._version !== SCHEMA_VERSION) return null;

        thumbnailSize.set(state.thumbnailSize);
        gridPreset.set(state.gridPreset);
        gridGap.set(state.gridGap);
        sidebarVisible.set(state.sidebarVisible);
        zenMode.set(state.zenMode);
        activeFolder.set(state.activeFolder);
        activeCollection.set(state.activeCollection);
        activeDetectedClass.set(state.activeDetectedClass ?? null);
        minSizeFilter.set(state.minSizeFilter);
        focusedIndex.set(state.focusedIndex ?? 0);
        gridScrollTop.set(state.gridScrollTop ?? 0);
        loupeScale.set(state.loupeScale);
        loupePanX.set(state.loupePanX);
        loupePanY.set(state.loupePanY);
        lineageLayout.set(state.lineageLayout);
        showDetectionBoxes.set(state.showDetectionBoxes);
        nsfwMode.set(state.nsfwMode);
        embeddingViewState.set(state.embeddingViewState);
        return state;
    } catch {
        return null;
    }
}

export function applyRestoredViewState(state: PersistedState | null): void {
    if (!state) return;
    viewMode.set(state.viewMode);
    focusedIndex.set(state.focusedIndex ?? 0);
    gridScrollTop.set(state.gridScrollTop ?? 0);
}
