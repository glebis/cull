export const CANVAS_DOCUMENT_VERSION = 1 as const;

export type CanvasItemFit = 'contain' | 'cover' | 'stretch';
export type CanvasConnectorKind = 'lineage' | 'variant' | 'reference' | 'sequence';
export type CanvasExportBackground = 'transparent' | 'canvas' | 'white';
export type CanvasExportBounds = 'content' | 'viewport' | 'selection';

export interface CanvasDocument {
    version: number;
    viewport: CanvasViewport;
    items: CanvasItem[];
    groups: CanvasGroup[];
    connectors: CanvasConnector[];
    annotations: CanvasAnnotation[];
    export: CanvasExportIntent;
}

export interface CanvasViewport {
    panX: number;
    panY: number;
    zoom: number;
}

export interface CanvasItem {
    id: string;
    imageId: string;
    x: number;
    y: number;
    width: number;
    height: number;
    z: number;
    hidden: boolean;
    label?: string | null;
    groupId?: string | null;
    transform: CanvasItemTransform;
    source: CanvasImageReference;
}

export interface CanvasImageReference {
    contentHash?: string | null;
    lastKnownPath?: string | null;
}

export interface CanvasItemTransform {
    crop?: CanvasCrop | null;
    rotationDegrees: number;
    fit: CanvasItemFit;
}

export interface CanvasCrop {
    x: number;
    y: number;
    width: number;
    height: number;
}

export interface CanvasGroup {
    id: string;
    name: string;
    itemIds: string[];
    label?: string | null;
    x: number;
    y: number;
    width: number;
    height: number;
    z: number;
    collapsed: boolean;
}

export interface CanvasConnector {
    id: string;
    fromItemId: string;
    toItemId: string;
    relationship: CanvasConnectorKind;
    label?: string | null;
}

export interface CanvasAnnotation {
    id: string;
    target: CanvasAnnotationTarget;
    body: string;
    x?: number | null;
    y?: number | null;
    createdAt?: string | null;
    author?: string | null;
}

export type CanvasAnnotationTarget =
    | { type: 'canvas' }
    | { type: 'item'; itemId: string }
    | { type: 'group'; groupId: string };

export interface CanvasExportIntent {
    defaultPresetId: string | null;
    background: CanvasExportBackground;
    bounds: CanvasExportBounds;
}

export function createEmptyCanvasDocument(): CanvasDocument {
    return {
        version: CANVAS_DOCUMENT_VERSION,
        viewport: { panX: 0, panY: 0, zoom: 1 },
        items: [],
        groups: [],
        connectors: [],
        annotations: [],
        export: {
            defaultPresetId: null,
            background: 'transparent',
            bounds: 'content',
        },
    };
}

export function parseCanvasDocumentLayout(layoutJson: string): CanvasDocument {
    if (layoutJson.trim() === '') {
        return createEmptyCanvasDocument();
    }

    const parsed = JSON.parse(layoutJson);
    if (isEmptyPlainObject(parsed)) {
        return createEmptyCanvasDocument();
    }

    const document = parsed as CanvasDocument;
    const errors = validateCanvasDocument(document);
    if (errors.length > 0) {
        throw new Error(errors.join('; '));
    }
    return document;
}

export function serializeCanvasDocumentLayout(document: CanvasDocument): string {
    const errors = validateCanvasDocument(document);
    if (errors.length > 0) {
        throw new Error(errors.join('; '));
    }
    return JSON.stringify(document);
}

export function validateCanvasDocument(document: CanvasDocument): string[] {
    const errors: string[] = [];

    if (document.version !== CANVAS_DOCUMENT_VERSION) {
        errors.push(`unsupported canvas document version ${String(document.version)}`);
    }

    const viewport = document.viewport;
    if (!viewport) {
        errors.push('viewport must be present');
    } else {
        validateFinite('viewport panX', viewport.panX, errors);
        validateFinite('viewport panY', viewport.panY, errors);
        if (!Number.isFinite(viewport.zoom) || viewport.zoom <= 0) {
            errors.push('viewport zoom must be greater than 0');
        }
    }

    const items = Array.isArray(document.items) ? document.items : [];
    if (!Array.isArray(document.items)) {
        errors.push('items must be an array');
    }

    const itemIds = new Set<string>();
    for (const item of items) {
        const itemId = typeof item.id === 'string' ? item.id : '';
        if (itemId.trim() === '') {
            errors.push('item id must not be empty');
        } else if (itemIds.has(itemId)) {
            errors.push(`duplicate item id ${itemId}`);
        } else {
            itemIds.add(itemId);
        }

        if (typeof item.imageId !== 'string' || item.imageId.trim() === '') {
            errors.push(`item ${itemId} imageId must not be empty`);
        }
        validateFinite(`item ${itemId} x`, item.x, errors);
        validateFinite(`item ${itemId} y`, item.y, errors);
        validatePositive(`item ${itemId} width`, item.width, errors);
        validatePositive(`item ${itemId} height`, item.height, errors);

        const transform = item.transform ?? defaultCanvasItemTransform();
        validateFinite(`item ${itemId} rotationDegrees`, transform.rotationDegrees, errors);
        if (transform.crop) {
            validateFinite(`item ${itemId} crop x`, transform.crop.x, errors);
            validateFinite(`item ${itemId} crop y`, transform.crop.y, errors);
            validatePositive(`item ${itemId} crop width`, transform.crop.width, errors);
            validatePositive(`item ${itemId} crop height`, transform.crop.height, errors);
        }
    }

    const groups = Array.isArray(document.groups) ? document.groups : [];
    if (!Array.isArray(document.groups)) {
        errors.push('groups must be an array');
    }

    const groupIds = new Set<string>();
    for (const group of groups) {
        const groupId = typeof group.id === 'string' ? group.id : '';
        if (groupId.trim() === '') {
            errors.push('group id must not be empty');
        } else if (groupIds.has(groupId)) {
            errors.push(`duplicate group id ${groupId}`);
        } else {
            groupIds.add(groupId);
        }

        validateFinite(`group ${groupId} x`, group.x, errors);
        validateFinite(`group ${groupId} y`, group.y, errors);
        validatePositive(`group ${groupId} width`, group.width, errors);
        validatePositive(`group ${groupId} height`, group.height, errors);

        const groupItemIds = Array.isArray(group.itemIds) ? group.itemIds : [];
        if (!Array.isArray(group.itemIds)) {
            errors.push(`group ${groupId} itemIds must be an array`);
        }
        for (const itemId of groupItemIds) {
            if (!itemIds.has(itemId)) {
                errors.push(`group ${groupId} references missing item ${itemId}`);
            }
        }
    }

    for (const item of items) {
        if (item.groupId && !groupIds.has(item.groupId)) {
            errors.push(`item ${item.id} references missing group ${item.groupId}`);
        }
    }

    const connectors = Array.isArray(document.connectors) ? document.connectors : [];
    if (!Array.isArray(document.connectors)) {
        errors.push('connectors must be an array');
    }

    const connectorIds = new Set<string>();
    for (const connector of connectors) {
        const connectorId = typeof connector.id === 'string' ? connector.id : '';
        if (connectorId.trim() === '') {
            errors.push('connector id must not be empty');
        } else if (connectorIds.has(connectorId)) {
            errors.push(`duplicate connector id ${connectorId}`);
        } else {
            connectorIds.add(connectorId);
        }

        if (!itemIds.has(connector.fromItemId)) {
            errors.push(`connector ${connectorId} references missing item ${connector.fromItemId}`);
        }
        if (!itemIds.has(connector.toItemId)) {
            errors.push(`connector ${connectorId} references missing item ${connector.toItemId}`);
        }
    }

    const annotations = Array.isArray(document.annotations) ? document.annotations : [];
    if (!Array.isArray(document.annotations)) {
        errors.push('annotations must be an array');
    }

    const annotationIds = new Set<string>();
    for (const annotation of annotations) {
        const annotationId = typeof annotation.id === 'string' ? annotation.id : '';
        if (annotationId.trim() === '') {
            errors.push('annotation id must not be empty');
        } else if (annotationIds.has(annotationId)) {
            errors.push(`duplicate annotation id ${annotationId}`);
        } else {
            annotationIds.add(annotationId);
        }

        if (typeof annotation.body !== 'string' || annotation.body.trim() === '') {
            errors.push(`annotation ${annotationId} body must not be empty`);
        }
        if (annotation.x !== null && annotation.x !== undefined) {
            validateFinite(`annotation ${annotationId} x`, annotation.x, errors);
        }
        if (annotation.y !== null && annotation.y !== undefined) {
            validateFinite(`annotation ${annotationId} y`, annotation.y, errors);
        }
        validateAnnotationTarget(annotation, itemIds, groupIds, errors);
    }

    return errors;
}

function defaultCanvasItemTransform(): CanvasItemTransform {
    return {
        crop: null,
        rotationDegrees: 0,
        fit: 'contain',
    };
}

function validateAnnotationTarget(
    annotation: CanvasAnnotation,
    itemIds: Set<string>,
    groupIds: Set<string>,
    errors: string[],
) {
    if (!annotation.target) {
        errors.push(`annotation ${annotation.id} target must be present`);
        return;
    }

    if (annotation.target.type === 'item' && !itemIds.has(annotation.target.itemId)) {
        errors.push(`annotation ${annotation.id} references missing item ${annotation.target.itemId}`);
    } else if (annotation.target.type === 'group' && !groupIds.has(annotation.target.groupId)) {
        errors.push(`annotation ${annotation.id} references missing group ${annotation.target.groupId}`);
    } else if (!['canvas', 'item', 'group'].includes(annotation.target.type)) {
        errors.push(`annotation ${annotation.id} target type is unsupported`);
    }
}

function validateFinite(label: string, value: number, errors: string[]) {
    if (!Number.isFinite(value)) {
        errors.push(`${label} must be finite`);
    }
}

function validatePositive(label: string, value: number, errors: string[]) {
    if (!Number.isFinite(value) || value <= 0) {
        errors.push(`${label} must be greater than 0`);
    }
}

function isEmptyPlainObject(value: unknown) {
    return Boolean(
        value
        && typeof value === 'object'
        && !Array.isArray(value)
        && Object.keys(value).length === 0,
    );
}
