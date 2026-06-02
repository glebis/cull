export interface PresentationFlags {
    zen: boolean;
    imageOnly: boolean;
}

export function nextThreeStagePresentationState(current: PresentationFlags): PresentationFlags {
    if (!current.zen) {
        return { zen: true, imageOnly: false };
    }
    if (!current.imageOnly) {
        return { zen: true, imageOnly: true };
    }
    return { zen: false, imageOnly: false };
}

export const nextExportPresentationState = nextThreeStagePresentationState;
