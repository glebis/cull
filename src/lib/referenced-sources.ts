import { listen, type UnlistenFn } from '@tauri-apps/api/event';
import { get, writable } from 'svelte/store';
import { cancelReferencedSourceJob, listReferencedSources, openReferencedFolder as openFolder, type ReferencedFolderPage, type ReferencedFolderUpdate, type ReferencedSource } from './api';
import { activeCollection, activeDetectedClass, activeFolder, activeReferencedFolder, activeSmartCollection, showToast } from './stores';
import { loadImagesForCurrentScope } from './image-loading';

export const referencedSources = writable<ReferencedSource[]>([]);
export const referencedFolderPage = writable<ReferencedFolderPage | null>(null);
export const referencedSourceIndexing = writable(false);
let initialized: Promise<UnlistenFn> | null = null;
let readGeneration = 0;
let activeJobId: string | null = null;

export async function refreshReferencedSources() {
    referencedSources.set(await listReferencedSources());
}

export function initializeReferencedSources(): Promise<UnlistenFn> {
    if (initialized) return initialized;
    initialized = (async () => {
        await refreshReferencedSources();
        const stopSources = await listen('sources:changed', () => void refreshReferencedSources());
        const stopPage = await listen<ReferencedFolderUpdate>('referenced-source:page-updated', async ({ payload }) => {
            const active = get(activeReferencedFolder);
            const page = get(referencedFolderPage);
            if (!active || !page || payload.job_id !== page.job_id || payload.job_id !== activeJobId) return;
            activeJobId = null;
            referencedSourceIndexing.set(false);
            if (payload.error) showToast('Could not finish reading the device', { detail: payload.error, type: 'error' });
            await loadImagesForCurrentScope({ resetFocus: false, force: true, invalidateCache: true });
        });
        return () => { stopSources(); stopPage(); initialized = null; };
    })();
    return initialized;
}

export async function openReferencedSourceFolder(source: ReferencedSource, relativePath = '', recursive = source.recursive_default, cursor: string | null = null) {
    const generation = ++readGeneration;
    const supersededJobId = activeJobId;
    activeJobId = null;
    if (supersededJobId) await cancelReferencedSourceJob(supersededJobId);
    if (generation !== readGeneration) return;
    activeCollection.set(null);
    activeSmartCollection.set(null);
    activeDetectedClass.set(null);
    activeFolder.set(null);
    activeReferencedFolder.set({ source_id: source.id, source_name: source.display_name, relative_path: relativePath, recursive });
    if (source.offline_at) {
        referencedFolderPage.set(null);
        referencedSourceIndexing.set(false);
        await loadImagesForCurrentScope({ force: true, invalidateCache: true });
        return;
    }
    referencedSourceIndexing.set(true);
    try {
        const page = await openFolder({ source_id: source.id, relative_path: relativePath, recursive, cursor, limit: 100 });
        if (generation !== readGeneration) {
            await cancelReferencedSourceJob(page.job_id);
            return;
        }
        activeJobId = page.job_id;
        referencedFolderPage.set(page);
        await loadImagesForCurrentScope({ force: true, invalidateCache: true });
    } catch (error) {
        if (generation !== readGeneration) return;
        referencedSourceIndexing.set(false);
        showToast('Could not browse the device', { detail: error instanceof Error ? error.message : String(error), type: 'error' });
        throw error;
    }
}

export async function loadNextReferencedPage() {
    const scope = get(activeReferencedFolder);
    const page = get(referencedFolderPage);
    const source = get(referencedSources).find(item => item.id === scope?.source_id);
    if (!scope || !source || !page?.next_cursor) return;
    await openReferencedSourceFolder(source, scope.relative_path, scope.recursive, page.next_cursor);
}
