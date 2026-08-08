export const TRASH_IMAGES_REQUESTED_EVENT = 'trash-images-requested';

export interface TrashImagesRequestDetail {
    imageIds: string[];
}

export function resolveTrashRequestIds(
    requestedIds: Iterable<string>,
    defaultIds: Iterable<string>,
): string[] {
    const requested = [...new Set(requestedIds)].filter(Boolean);
    return requested.length > 0
        ? requested
        : [...new Set(defaultIds)].filter(Boolean);
}

export function requestTrashImages(imageIds: Iterable<string> = []): void {
    const canonicalIds = [...new Set(imageIds)].filter(Boolean);
    window.dispatchEvent(new CustomEvent<TrashImagesRequestDetail>(
        TRASH_IMAGES_REQUESTED_EVENT,
        { detail: { imageIds: canonicalIds } },
    ));
}
