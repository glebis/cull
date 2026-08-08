export const TRASH_IMAGES_REQUESTED_EVENT = 'trash-images-requested';

export interface TrashImagesRequestDetail {
    imageIds: string[];
}

export function requestTrashImages(imageIds: Iterable<string> = []): void {
    const canonicalIds = [...new Set(imageIds)].filter(Boolean);
    window.dispatchEvent(new CustomEvent<TrashImagesRequestDetail>(
        TRASH_IMAGES_REQUESTED_EVENT,
        { detail: { imageIds: canonicalIds } },
    ));
}
