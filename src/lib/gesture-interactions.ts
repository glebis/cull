export type GestureSurface = 'grid' | 'loupe' | 'compare' | 'canvas';
export type GestureSource = 'trackpad' | 'magic_mouse' | 'wheel' | 'pointer' | 'touch' | 'native_macos';

export type GestureIntent =
    | { type: 'zoom'; factor: number; focalX: number; focalY: number; source: GestureSource }
    | { type: 'pan'; deltaX: number; deltaY: number; source: GestureSource }
    | { type: 'navigate'; direction: 'previous' | 'next'; source: GestureSource }
    | { type: 'smart_zoom'; focalX: number; focalY: number; source: GestureSource }
    | { type: 'crop_adjust'; deltaX: number; deltaY: number; source: GestureSource };

export interface GestureSuppressionState {
    modalOpen?: boolean;
    contextMenuOpen?: boolean;
    commandPaletteOpen?: boolean;
    textEntryOpen?: boolean;
    cropModeActive?: boolean;
}

export interface WheelLikeInput {
    deltaX: number;
    deltaY: number;
    deltaMode: number;
}

export interface SwipeInput {
    deltaX: number;
    deltaY: number;
}

export interface SwipeOptions {
    minDistance?: number;
    dominanceRatio?: number;
}

export interface WheelGestureInput extends WheelLikeInput {
    surface: GestureSurface;
    clientX: number;
    clientY: number;
    ctrlKey?: boolean;
    metaKey?: boolean;
    altKey?: boolean;
    shiftKey?: boolean;
    viewportHeight: number;
    target: EventTarget | null;
    suppression?: GestureSuppressionState;
}

const LINE_DELTA_PX = 16;
const DEFAULT_SWIPE_DISTANCE = 80;
const DEFAULT_SWIPE_DOMINANCE = 1.5;
const WHEEL_ZOOM_BASE = 1.0015;

export function normalizeWheelDelta(input: WheelLikeInput, viewportHeight: number): { deltaX: number; deltaY: number } {
    const multiplier = input.deltaMode === 1
        ? LINE_DELTA_PX
        : input.deltaMode === 2
            ? viewportHeight
            : 1;
    return {
        deltaX: input.deltaX * multiplier,
        deltaY: input.deltaY * multiplier,
    };
}

export function wheelZoomFactor(deltaY: number): number {
    return Math.pow(WHEEL_ZOOM_BASE, -deltaY);
}

export function classifySwipe(input: SwipeInput, options: SwipeOptions = {}): 'previous' | 'next' | null {
    const minDistance = options.minDistance ?? DEFAULT_SWIPE_DISTANCE;
    const dominanceRatio = options.dominanceRatio ?? DEFAULT_SWIPE_DOMINANCE;
    const absX = Math.abs(input.deltaX);
    const absY = Math.abs(input.deltaY);
    if (absX < minDistance) return null;
    if (absX < absY * dominanceRatio) return null;
    return input.deltaX > 0 ? 'previous' : 'next';
}

export function shouldIgnoreGestureTarget(target: EventTarget | null, state: GestureSuppressionState = {}): boolean {
    if (state.modalOpen || state.contextMenuOpen || state.commandPaletteOpen || state.textEntryOpen) {
        return true;
    }
    const element = targetAsElement(target);
    if (!element) return false;
    const tagName = element.tagName?.toUpperCase();
    if (tagName === 'INPUT' || tagName === 'TEXTAREA' || tagName === 'SELECT') return true;
    if (element.isContentEditable) return true;
    return element.closest?.('[role="dialog"], .modal-dialog, .command-palette, .context-menu') !== null;
}

export function wheelGestureIntent(input: WheelGestureInput): GestureIntent | null {
    if (shouldIgnoreGestureTarget(input.target, input.suppression)) return null;
    const delta = normalizeWheelDelta(input, input.viewportHeight);
    if (input.ctrlKey || input.metaKey || input.altKey) {
        return {
            type: 'zoom',
            factor: wheelZoomFactor(delta.deltaY),
            focalX: input.clientX,
            focalY: input.clientY,
            source: 'wheel',
        };
    }
    return {
        type: 'pan',
        deltaX: delta.deltaX,
        deltaY: delta.deltaY,
        source: 'wheel',
    };
}

interface GestureTargetElement {
    tagName?: string;
    isContentEditable?: boolean;
    closest?: (selector: string) => unknown;
}

function targetAsElement(target: EventTarget | null): GestureTargetElement | null {
    if (!target || typeof target !== 'object') return null;
    if (typeof HTMLElement !== 'undefined' && target instanceof HTMLElement) return target;
    if ('tagName' in target || 'isContentEditable' in target || 'closest' in target) {
        return target as GestureTargetElement;
    }
    return null;
}
