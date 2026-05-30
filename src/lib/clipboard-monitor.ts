import {
    activeCollection,
    activeDetectedClass,
    activeFolder,
    activeSmartCollection,
    navigateTo,
} from './stores';
import { loadImagesForCurrentScope } from './image-loading';

export async function applyClipboardMonitorCollection(collectionId: string) {
    activeCollection.set(collectionId);
    activeFolder.set(null);
    activeSmartCollection.set(null);
    activeDetectedClass.set(null);
    navigateTo('grid');
    await loadImagesForCurrentScope({ force: true, invalidateCache: true });
}
