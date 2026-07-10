import type { ImageWithFile } from './api';
import {
    createEmptyCanvasDocument,
    parseCanvasDocumentLayout,
    type CanvasAnnotation,
    type CanvasCrop,
    type CanvasDocument,
    type CanvasGroup,
    type CanvasItem,
    type CanvasViewport,
} from './canvas-document';

const ITEM_GAP = 20;
const ITEM_HEIGHT = 200;

export interface CanvasViewItem {
    id: string;
    imageId: string;
    image: ImageWithFile;
    x: number;
    y: number;
    width: number;
    height: number;
    z: number;
    hidden: boolean;
    label: string | null;
    groupId: string | null;
    rotationDegrees: number;
    crop: CanvasCrop | null;
}

export interface AddImagesToCanvasDocumentResult {
    document: CanvasDocument;
    addedImageIds: string[];
    skippedImageIds: string[];
}

export function createCanvasDocumentForImages(
    images: ImageWithFile[],
    baseDocument: CanvasDocument = createEmptyCanvasDocument(),
): CanvasDocument {
    const layout = gridLayout(images);
    const existingItems = new Map(baseDocument.items.map(item => [item.imageId, item]));
    const visibleImages = new Map(images.map(image => [image.image.id, image]));
    const items = baseDocument.items.map(item => {
        const visibleImage = visibleImages.get(item.imageId);
        if (visibleImage) {
            return refreshCanvasItemSource(item, visibleImage);
        }
        return item;
    });
    const maxZ = items.reduce((highest, item) => Math.max(highest, item.z), -1);

    for (const [index, image] of images.entries()) {
        if (existingItems.has(image.image.id)) continue;
        items.push(createCanvasItem(image, layout[index], maxZ + index + 1));
    }

    return sanitizeCanvasDocumentReferences({
        ...baseDocument,
        items,
    });
}

export function addImagesToCanvasDocument(
    document: CanvasDocument,
    images: ImageWithFile[],
    origin: { x: number; y: number },
): AddImagesToCanvasDocumentResult {
    const existingImageIds = new Set(document.items.map(item => item.imageId));
    const uniqueImages = uniqueImagesById(images);
    const imagesToAdd = uniqueImages.filter(image => !existingImageIds.has(image.image.id));
    const skippedImageIds = uniqueImages
        .filter(image => existingImageIds.has(image.image.id))
        .map(image => image.image.id);

    if (imagesToAdd.length === 0) {
        return { document, addedImageIds: [], skippedImageIds };
    }

    const layout = compactGridLayout(imagesToAdd, origin);
    const maxZ = document.items.reduce((highest, item) => Math.max(highest, item.z), -1);
    const additions = imagesToAdd.map((image, index) =>
        createCanvasItem(image, layout[index], maxZ + index + 1)
    );

    return {
        document: sanitizeCanvasDocumentReferences({
            ...document,
            items: [...document.items, ...additions],
        }),
        addedImageIds: additions.map(item => item.imageId),
        skippedImageIds,
    };
}

export function createCanvasDocumentFromLayoutJson(layoutJson: string, images: ImageWithFile[]): CanvasDocument {
    return createCanvasDocumentForImages(images, parseCanvasLayoutOrEmpty(layoutJson));
}

export function createCanvasViewItems(document: CanvasDocument, images: ImageWithFile[]): CanvasViewItem[] {
    const imagesById = new Map(images.map(image => [image.image.id, image]));
    return document.items
        .map((item): CanvasViewItem | null => {
            const image = imagesById.get(item.imageId);
            if (!image) return null;
            return {
                id: item.id,
                imageId: item.imageId,
                image,
                x: item.x,
                y: item.y,
                width: item.width,
                height: item.height,
                z: item.z,
                hidden: item.hidden,
                label: item.label ?? null,
                groupId: item.groupId ?? null,
                rotationDegrees: normalizeRotation(item.transform.rotationDegrees),
                crop: normalizeCrop(item.transform.crop),
            };
        })
        .filter((item): item is CanvasViewItem => item !== null);
}

export function updateCanvasDocumentFromViewItems(
    document: CanvasDocument,
    viewItems: CanvasViewItem[],
    viewport: CanvasViewport,
): CanvasDocument {
    const viewItemsById = new Map(viewItems.map(item => [item.id, item]));
    const updatedItemIds = new Set<string>();
    const items = document.items.map((item) => {
        const viewItem = viewItemsById.get(item.id);
        if (!viewItem) return item;
        updatedItemIds.add(viewItem.id);
        return canvasItemFromViewItem(viewItem, item);
    });

    for (const viewItem of viewItems) {
        if (updatedItemIds.has(viewItem.id)) continue;
        items.push(canvasItemFromViewItem(viewItem));
    }

    return sanitizeCanvasDocumentReferences({
        ...document,
        viewport,
        items,
    });
}

function canvasItemFromViewItem(viewItem: CanvasViewItem, existing?: CanvasItem): CanvasItem {
    const baseItem = existing ?? createCanvasItem(viewItem.image, {
        x: viewItem.x,
        y: viewItem.y,
        width: viewItem.width,
        height: viewItem.height,
    }, viewItem.z);

    return {
        ...baseItem,
        id: viewItem.id,
        imageId: viewItem.imageId,
        x: viewItem.x,
        y: viewItem.y,
        width: viewItem.width,
        height: viewItem.height,
        z: viewItem.z,
        hidden: viewItem.hidden,
        label: viewItem.label,
        groupId: viewItem.groupId,
        transform: {
            ...baseItem.transform,
            crop: normalizeCrop(viewItem.crop),
            rotationDegrees: normalizeRotation(viewItem.rotationDegrees),
        },
        source: {
            contentHash: viewItem.image.image.sha256_hash,
            lastKnownPath: viewItem.image.path,
        },
    };
}

function createCanvasItem(
    image: ImageWithFile,
    rect: { x: number; y: number; width: number; height: number },
    z: number,
): CanvasItem {
    return {
        id: image.image.id,
        imageId: image.image.id,
        x: rect.x,
        y: rect.y,
        width: rect.width,
        height: rect.height,
        z,
        hidden: false,
        label: null,
        groupId: null,
        transform: {
            crop: null,
            rotationDegrees: 0,
            fit: 'contain',
        },
        source: {
            contentHash: image.image.sha256_hash,
            lastKnownPath: image.path,
        },
    };
}

function refreshCanvasItemSource(item: CanvasItem, image: ImageWithFile): CanvasItem {
    return {
        ...item,
        source: {
            contentHash: image.image.sha256_hash,
            lastKnownPath: image.path,
        },
    };
}

function uniqueImagesById(images: ImageWithFile[]): ImageWithFile[] {
    const seen = new Set<string>();
    const unique: ImageWithFile[] = [];
    for (const image of images) {
        if (seen.has(image.image.id)) continue;
        seen.add(image.image.id);
        unique.push(image);
    }
    return unique;
}

function gridLayout(images: ImageWithFile[]) {
    const cols = Math.ceil(Math.sqrt(images.length));
    const colWidths = new Array(cols).fill(0);
    for (let i = 0; i < Math.min(cols, images.length); i++) {
        const aspect = safeAspect(images[i]);
        colWidths[i] = ITEM_HEIGHT * aspect;
    }

    const colX = [0];
    for (let col = 1; col < cols; col++) {
        colX[col] = colX[col - 1] + (colWidths[col - 1] || ITEM_HEIGHT) + ITEM_GAP;
    }

    return images.map((image, index) => {
        const aspect = safeAspect(image);
        const col = index % cols;
        const row = Math.floor(index / cols);
        return {
            x: colX[col],
            y: row * (ITEM_HEIGHT + ITEM_GAP),
            width: ITEM_HEIGHT * aspect,
            height: ITEM_HEIGHT,
        };
    });
}

function compactGridLayout(images: ImageWithFile[], origin: { x: number; y: number }) {
    const cols = Math.max(1, Math.ceil(Math.sqrt(images.length)));
    const colWidths = new Array(cols).fill(ITEM_HEIGHT);
    for (let index = 0; index < images.length; index++) {
        const col = index % cols;
        colWidths[col] = Math.max(colWidths[col], ITEM_HEIGHT * safeAspect(images[index]));
    }

    const colX = [origin.x];
    for (let col = 1; col < cols; col++) {
        colX[col] = colX[col - 1] + colWidths[col - 1] + ITEM_GAP;
    }

    return images.map((image, index) => {
        const col = index % cols;
        const row = Math.floor(index / cols);
        return {
            x: colX[col],
            y: origin.y + row * (ITEM_HEIGHT + ITEM_GAP),
            width: ITEM_HEIGHT * safeAspect(image),
            height: ITEM_HEIGHT,
        };
    });
}

function safeAspect(image: ImageWithFile) {
    if (image.image.height <= 0) return 1;
    return image.image.width / image.image.height;
}

export function normalizeRotation(value: number): number {
    if (!Number.isFinite(value)) return 0;
    return ((Math.round(value / 90) * 90) % 360 + 360) % 360;
}

export function rotateCanvasViewItemClockwise(item: CanvasViewItem): CanvasViewItem {
    return {
        ...item,
        rotationDegrees: normalizeRotation(item.rotationDegrees + 90),
    };
}

const MIN_CROP_SIZE = 0.02;

export function applyCanvasViewItemCrop(item: CanvasViewItem, crop: CanvasCrop | null): CanvasViewItem {
    return {
        ...item,
        crop: normalizeCrop(crop),
    };
}

export function setCanvasViewItemCropFromPoints(
    item: CanvasViewItem,
    anchor: { x: number; y: number },
    current: { x: number; y: number },
): CanvasViewItem {
    const crop = normalizeCrop({
        x: Math.min(anchor.x, current.x),
        y: Math.min(anchor.y, current.y),
        width: Math.abs(current.x - anchor.x),
        height: Math.abs(current.y - anchor.y),
    });

    return applyCanvasViewItemCrop(item, crop);
}

export interface AddCanvasItemAnnotationOptions {
    id?: string;
    x?: number | null;
    y?: number | null;
    createdAt?: string | null;
    author?: string | null;
}

export function addCanvasItemAnnotation(
    document: CanvasDocument,
    itemId: string,
    body: string,
    options: AddCanvasItemAnnotationOptions = {},
): CanvasDocument {
    const trimmed = body.trim();
    if (!trimmed) return document;
    if (!document.items.some(item => item.id === itemId)) {
        throw new Error(`Canvas item '${itemId}' does not exist`);
    }

    const annotation: CanvasAnnotation = {
        id: options.id ?? createAnnotationId(),
        target: { type: 'item', itemId },
        body: trimmed,
        x: options.x ?? null,
        y: options.y ?? null,
        createdAt: options.createdAt ?? new Date().toISOString(),
        author: options.author ?? null,
    };

    return sanitizeCanvasDocumentReferences({
        ...document,
        annotations: [...document.annotations, annotation],
    });
}

export function canvasItemAnnotations(document: CanvasDocument | null, itemId: string): CanvasAnnotation[] {
    if (!document) return [];
    return document.annotations.filter(annotation =>
        annotation.target.type === 'item' && annotation.target.itemId === itemId
    );
}

function createAnnotationId() {
    if (typeof crypto !== 'undefined' && 'randomUUID' in crypto) {
        return `note-${crypto.randomUUID()}`;
    }
    return `note-${Date.now().toString(36)}`;
}

function normalizeCrop(crop: CanvasCrop | null | undefined): CanvasCrop | null {
    if (!crop) return null;
    if (!Number.isFinite(crop.x) || !Number.isFinite(crop.y)
        || !Number.isFinite(crop.width) || !Number.isFinite(crop.height)) {
        return null;
    }

    const x = clamp(crop.x, 0, 1 - MIN_CROP_SIZE);
    const y = clamp(crop.y, 0, 1 - MIN_CROP_SIZE);
    const width = clamp(crop.width, MIN_CROP_SIZE, 1 - x);
    const height = clamp(crop.height, MIN_CROP_SIZE, 1 - y);

    return {
        x: roundCropValue(x),
        y: roundCropValue(y),
        width: roundCropValue(width),
        height: roundCropValue(height),
    };
}

function roundCropValue(value: number) {
    return Math.round(value * 10000) / 10000;
}

function clamp(value: number, min: number, max: number) {
    return Math.max(min, Math.min(max, value));
}

function parseCanvasLayoutOrEmpty(layoutJson: string): CanvasDocument {
    if (layoutJson.trim() === '') {
        return createEmptyCanvasDocument();
    }

    try {
        const value = JSON.parse(layoutJson);
        if (!value || typeof value !== 'object' || Array.isArray(value) || !('version' in value)) {
            return createEmptyCanvasDocument();
        }
    } catch (_) {
        return createEmptyCanvasDocument();
    }

    return parseCanvasDocumentLayout(layoutJson);
}

function sanitizeCanvasDocumentReferences(document: CanvasDocument): CanvasDocument {
    const itemIds = new Set(document.items.map(item => item.id));
    const groups = document.groups
        .map((group): CanvasGroup => ({
            ...group,
            itemIds: group.itemIds.filter(itemId => itemIds.has(itemId)),
        }))
        .filter(group => group.itemIds.length > 0);
    const groupIds = new Set(groups.map(group => group.id));

    return {
        ...document,
        groups,
        connectors: document.connectors.filter(connector =>
            itemIds.has(connector.fromItemId) && itemIds.has(connector.toItemId)
        ),
        annotations: document.annotations.filter(annotation =>
            annotationTargetExists(annotation, itemIds, groupIds)
        ),
    };
}

function annotationTargetExists(annotation: CanvasAnnotation, itemIds: Set<string>, groupIds: Set<string>) {
    if (annotation.target.type === 'canvas') return true;
    if (annotation.target.type === 'item') return itemIds.has(annotation.target.itemId);
    return groupIds.has(annotation.target.groupId);
}
