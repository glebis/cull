import type { Canvas, StaticPublishRequest } from './api';
import { parseCanvasDocumentLayout, serializeCanvasDocumentLayout } from './canvas-document';

export interface SavedCanvasPublishOptions {
    canvas: Canvas;
    canvasName?: string;
    outputDir: string;
    shareUrl: string;
    includeThumbnails: boolean;
    includeWeb: boolean;
    includeFull: boolean;
}

export function buildStaticPublishRequestFromSavedCanvas(options: SavedCanvasPublishOptions): StaticPublishRequest {
    const document = parseCanvasDocumentLayout(options.canvas.layout_json);

    return {
        canvas_name: options.canvasName?.trim() || options.canvas.name.trim() || 'Current Canvas',
        items: document.items.map(item => ({
            image_id: item.imageId,
            x: item.x,
            y: item.y,
            width: item.width,
            height: item.height,
            hidden: item.hidden,
        })),
        layout_json: serializeCanvasDocumentLayout(document),
        output_dir: trimmedOrNull(options.outputDir),
        share_url: trimmedOrNull(options.shareUrl),
        include_thumbnails: options.includeThumbnails,
        include_web: options.includeWeb,
        include_full: options.includeFull,
    };
}

export function countSavedCanvasItems(canvas: Canvas | null): number {
    if (!canvas) return 0;
    try {
        return parseCanvasDocumentLayout(canvas.layout_json).items.length;
    } catch (_) {
        return 0;
    }
}

function trimmedOrNull(value: string): string | null {
    const trimmed = value.trim();
    return trimmed.length > 0 ? trimmed : null;
}
