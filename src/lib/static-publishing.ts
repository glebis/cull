import type { Canvas, StaticPublishLink, StaticPublishRequest } from './api';
import { parseCanvasDocumentLayout, serializeCanvasDocumentLayout } from './canvas-document';

export interface SavedCanvasPublishOptions {
    canvas: Canvas;
    canvasName?: string;
    outputDir: string;
    shareUrl: string;
    siteTitle?: string;
    siteDescription?: string;
    indexable: boolean;
    links: StaticPublishLink[];
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
        site_title: trimmedOrNull(options.siteTitle ?? ''),
        site_description: trimmedOrNull(options.siteDescription ?? ''),
        indexable: options.indexable,
        links: options.links,
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

export function parseStaticPublishLinks(value: string): StaticPublishLink[] {
    return value
        .split(/\r?\n/)
        .map(line => line.trim())
        .filter(Boolean)
        .flatMap(line => {
            const separator = line.includes('|') ? '|' : ': ';
            const index = line.indexOf(separator);
            if (index <= 0) return [];
            const label = line.slice(0, index).trim();
            const url = line.slice(index + separator.length).trim();
            if (!label || !/^https?:\/\//i.test(url)) return [];
            return [{ label, url }];
        });
}

export function formatStaticPublishLinks(links: StaticPublishLink[]): string {
    return links
        .filter(link => link.label.trim() && link.url.trim())
        .map(link => `${link.label.trim()} | ${link.url.trim()}`)
        .join('\n');
}
