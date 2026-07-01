import type { ViewMode } from './stores';

export interface AgentSnapshotRect {
    left: number;
    top: number;
    width: number;
    height: number;
}

export interface AgentSnapshotRectItem {
    imageId: string;
    filename: string;
    path: string;
    thumbnailPath: string | null;
    aiPrompt: string | null;
    generationPrompt: string | null;
    generationProvider: string | null;
    generationModel: string | null;
    generationSeed: string | null;
    generationSettingsJson: string | null;
    bounds: AgentSnapshotRect;
    rating: number | null;
    decision: string | null;
    viewRole: string;
    selected?: boolean;
    focused?: boolean;
}

export interface AgentSnapshotVisibleImage {
    label: string;
    image_id: string;
    filename: string;
    path: string;
    thumbnail_path: string | null;
    ai_prompt: string | null;
    generation_prompt: string | null;
    generation_provider: string | null;
    generation_model: string | null;
    generation_seed: string | null;
    generation_settings_json: string | null;
    bounds_css: AgentSnapshotRect;
    bounds_px: AgentSnapshotRect;
    visible_ratio: number;
    focused: boolean;
    selected: boolean;
    rating: number | null;
    decision: string | null;
    view_role: string;
}

export interface AgentSnapshotManifest {
    schema_version: 1;
    snapshot_id: string;
    created_at: string;
    view_mode: ViewMode;
    capture_reason: string;
    destination: AgentSnapshotDestination;
    files: AgentSnapshotFiles;
    window: AgentSnapshotWindow;
    scope: AgentSnapshotScope;
    visible_images: AgentSnapshotVisibleImage[];
}

export interface AgentSnapshotFiles {
    raw_png: string;
    annotated_png: string;
    manifest_json: string;
}

export interface AgentSnapshotDestination {
    kind: 'local' | 'clipboard';
    detail: string | null;
}

export interface AgentSnapshotWindow {
    label: string;
    title: string;
    width_css: number;
    height_css: number;
    device_pixel_ratio: number;
}

export interface AgentSnapshotScope {
    kind: string;
    id: string | null;
    label: string;
    path: string | null;
}

export interface CollectVisibleImageTargetsOptions {
    viewMode: ViewMode;
    viewport: AgentSnapshotRect;
    devicePixelRatio: number;
    items: AgentSnapshotRectItem[];
    selectedIds: Set<string>;
    focusedImageId: string | null;
    visibleThreshold?: number;
}

export interface DomCollectVisibleImageTargetsOptions {
    viewMode: ViewMode;
    root?: ParentNode;
    viewport?: AgentSnapshotRect;
    devicePixelRatio?: number;
    selectedIds?: Set<string>;
    focusedImageId?: string | null;
    visibleThreshold?: number;
}

export interface BuildAgentSnapshotManifestOptions {
    snapshotId: string;
    createdAt: string;
    viewMode: ViewMode;
    captureReason: string;
    destination: AgentSnapshotDestination;
    files: AgentSnapshotFiles;
    window: AgentSnapshotWindow;
    scope: AgentSnapshotScope;
    visibleImages: AgentSnapshotVisibleImage[];
}

const DEFAULT_VISIBLE_THRESHOLD = 0.2;

function round(value: number): number {
    return Math.round(value * 10000) / 10000;
}

function rectArea(rect: AgentSnapshotRect): number {
    return Math.max(0, rect.width) * Math.max(0, rect.height);
}

function intersectRect(a: AgentSnapshotRect, b: AgentSnapshotRect): AgentSnapshotRect | null {
    const left = Math.max(a.left, b.left);
    const top = Math.max(a.top, b.top);
    const right = Math.min(a.left + a.width, b.left + b.width);
    const bottom = Math.min(a.top + a.height, b.top + b.height);
    const width = right - left;
    const height = bottom - top;
    if (width <= 0 || height <= 0) return null;
    return { left, top, width, height };
}

function scaleRect(rect: AgentSnapshotRect, scale: number): AgentSnapshotRect {
    return {
        left: Math.round(rect.left * scale),
        top: Math.round(rect.top * scale),
        width: Math.round(rect.width * scale),
        height: Math.round(rect.height * scale),
    };
}

function fileNameFromPath(path: string): string {
    return path.split('/').filter(Boolean).pop() ?? path;
}

function selectedSetContains(selectedIds: Set<string>, imageId: string): boolean {
    return selectedIds.has(imageId);
}

export function collectVisibleImageTargetsFromRects(
    options: CollectVisibleImageTargetsOptions,
): AgentSnapshotVisibleImage[] {
    const threshold = options.visibleThreshold ?? DEFAULT_VISIBLE_THRESHOLD;
    const targets: AgentSnapshotVisibleImage[] = [];

    for (const item of options.items) {
        const area = rectArea(item.bounds);
        if (area <= 0) continue;
        const intersection = intersectRect(item.bounds, options.viewport);
        if (!intersection) continue;
        const visibleRatio = round(rectArea(intersection) / area);
        if (visibleRatio < threshold) continue;

        const selected = item.selected ?? selectedSetContains(options.selectedIds, item.imageId);
        const focused = item.focused ?? options.focusedImageId === item.imageId;
        targets.push({
            label: String(targets.length + 1),
            image_id: item.imageId,
            filename: item.filename || fileNameFromPath(item.path),
            path: item.path,
            thumbnail_path: item.thumbnailPath,
            ai_prompt: item.aiPrompt,
            generation_prompt: item.generationPrompt,
            generation_provider: item.generationProvider,
            generation_model: item.generationModel,
            generation_seed: item.generationSeed,
            generation_settings_json: item.generationSettingsJson,
            bounds_css: {
                left: round(item.bounds.left),
                top: round(item.bounds.top),
                width: round(item.bounds.width),
                height: round(item.bounds.height),
            },
            bounds_px: scaleRect(item.bounds, options.devicePixelRatio),
            visible_ratio: visibleRatio,
            focused,
            selected,
            rating: item.rating,
            decision: item.decision,
            view_role: item.viewRole,
        });
    }

    return targets;
}

export function collectVisibleImageTargets(
    options: DomCollectVisibleImageTargetsOptions,
): AgentSnapshotVisibleImage[] {
    const root = options.root ?? document;
    const viewport = options.viewport ?? {
        left: 0,
        top: 0,
        width: window.innerWidth,
        height: window.innerHeight,
    };
    const devicePixelRatio = options.devicePixelRatio ?? window.devicePixelRatio ?? 1;
    const selectedIds = options.selectedIds ?? new Set<string>();
    const elements = Array.from(root.querySelectorAll<HTMLElement>('[data-agent-image-id]'));
    const items: AgentSnapshotRectItem[] = elements.map(element => {
        const rect = element.getBoundingClientRect();
        const imageId = element.dataset.agentImageId ?? '';
        const path = element.dataset.agentPath ?? '';
        const ratingRaw = element.dataset.agentRating;
        return {
            imageId,
            filename: element.dataset.agentFilename ?? fileNameFromPath(path),
            path,
            thumbnailPath: element.dataset.agentThumbnailPath || null,
            aiPrompt: element.dataset.agentAiPrompt || null,
            generationPrompt: element.dataset.agentGenerationPrompt || null,
            generationProvider: element.dataset.agentGenerationProvider || null,
            generationModel: element.dataset.agentGenerationModel || null,
            generationSeed: element.dataset.agentGenerationSeed || null,
            generationSettingsJson: element.dataset.agentGenerationSettingsJson || null,
            bounds: {
                left: rect.left,
                top: rect.top,
                width: rect.width,
                height: rect.height,
            },
            rating: ratingRaw === undefined || ratingRaw === '' ? null : Number(ratingRaw),
            decision: element.dataset.agentDecision || null,
            viewRole: element.dataset.agentViewRole ?? 'image',
            selected: element.dataset.agentSelected === 'true' ? true : undefined,
            focused: element.dataset.agentFocused === 'true' ? true : undefined,
        };
    }).filter(item => item.imageId && item.path);

    return collectVisibleImageTargetsFromRects({
        viewMode: options.viewMode,
        viewport,
        devicePixelRatio,
        items,
        selectedIds,
        focusedImageId: options.focusedImageId ?? null,
        visibleThreshold: options.visibleThreshold,
    });
}

export function buildAgentSnapshotManifest(
    options: BuildAgentSnapshotManifestOptions,
): AgentSnapshotManifest {
    return {
        schema_version: 1,
        snapshot_id: options.snapshotId,
        created_at: options.createdAt,
        view_mode: options.viewMode,
        capture_reason: options.captureReason,
        destination: options.destination,
        files: options.files,
        window: options.window,
        scope: options.scope,
        visible_images: options.visibleImages,
    };
}

export function imageIdsForSnapshotLabels(
    manifest: Pick<AgentSnapshotManifest, 'visible_images'>,
    labels: string[],
): string[] {
    const byLabel = new Map(manifest.visible_images.map(image => [image.label, image.image_id]));
    return labels.map(label => {
        const imageId = byLabel.get(label);
        if (!imageId) throw new Error(`Unknown snapshot label: ${label}`);
        return imageId;
    });
}

export async function drawAnnotatedSnapshot(
    rawPngBase64: string,
    targets: AgentSnapshotVisibleImage[],
): Promise<string> {
    if (typeof document === 'undefined') {
        throw new Error('Annotated snapshots require a browser canvas');
    }

    const image = await loadPng(rawPngBase64);
    const canvas = document.createElement('canvas');
    canvas.width = image.naturalWidth || image.width;
    canvas.height = image.naturalHeight || image.height;
    const ctx = canvas.getContext('2d');
    if (!ctx) throw new Error('Canvas 2D context unavailable');

    ctx.drawImage(image, 0, 0);
    ctx.lineWidth = Math.max(2, Math.round(canvas.width / 800));
    ctx.font = `${Math.max(18, Math.round(canvas.width / 60))}px JetBrains Mono, monospace`;
    ctx.textBaseline = 'top';

    for (const target of targets) {
        const rect = target.bounds_px;
        const label = target.label;
        const pad = 6;
        const metrics = ctx.measureText(label);
        const labelWidth = Math.ceil(metrics.width + pad * 2);
        const labelHeight = Math.ceil(parseInt(ctx.font, 10) + pad * 2);
        const x = Math.max(0, Math.min(rect.left, canvas.width - labelWidth));
        const y = Math.max(0, Math.min(rect.top, canvas.height - labelHeight));

        ctx.strokeStyle = target.focused ? '#9ece6a' : '#7aa2f7';
        ctx.fillStyle = 'rgba(8, 8, 12, 0.74)';
        ctx.strokeRect(rect.left, rect.top, rect.width, rect.height);
        ctx.fillRect(x, y, labelWidth, labelHeight);
        ctx.strokeRect(x, y, labelWidth, labelHeight);
        ctx.fillStyle = '#e0e0e0';
        ctx.fillText(label, x + pad, y + pad);
    }

    return canvas.toDataURL('image/png').replace(/^data:image\/png;base64,/, '');
}

function loadPng(base64OrDataUrl: string): Promise<HTMLImageElement> {
    const src = base64OrDataUrl.startsWith('data:')
        ? base64OrDataUrl
        : `data:image/png;base64,${base64OrDataUrl}`;
    return new Promise((resolve, reject) => {
        const image = new Image();
        image.onload = () => resolve(image);
        image.onerror = () => reject(new Error('Failed to load snapshot PNG'));
        image.src = src;
    });
}
