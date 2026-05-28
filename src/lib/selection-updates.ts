import type { ImageWithFile, Selection } from './api';

export type ImageDecision = 'accept' | 'reject' | 'undecided';

function baseSelection(item: ImageWithFile): Selection {
    return {
        image_id: item.image.id,
        project_id: item.selection?.project_id ?? null,
        star_rating: item.selection?.star_rating ?? null,
        color_label: item.selection?.color_label ?? null,
        decision: item.selection?.decision ?? 'undecided',
    };
}

export function withRating(item: ImageWithFile, rating: number): ImageWithFile {
    return {
        ...item,
        selection: {
            ...baseSelection(item),
            star_rating: rating,
        },
    };
}

export function withDecision(item: ImageWithFile, decision: ImageDecision): ImageWithFile {
    return {
        ...item,
        selection: {
            ...baseSelection(item),
            decision,
        },
    };
}
