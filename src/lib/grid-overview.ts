/** Switch to a single canvas before a 4K viewport would mount thousands of image nodes. */
export const GRID_OVERVIEW_MAX_SIZE = 64;

/** At this density the full library can fit on screen, so page through the whole scope. */
export const GRID_FULL_SCOPE_OVERVIEW_MAX_SIZE = 12;

/** Extreme density is a metadata map; actual thumbnails are decoded by the hover lens. */
export function shouldDecodeGridOverviewThumbnails(thumbnailSize: number): boolean {
    return thumbnailSize > GRID_FULL_SCOPE_OVERVIEW_MAX_SIZE;
}
