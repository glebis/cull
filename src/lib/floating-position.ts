export interface FloatingPoint {
    x: number;
    y: number;
}

export interface FloatingSize {
    width: number;
    height: number;
}

export interface ViewportSize {
    width: number;
    height: number;
}

export interface FloatingRect extends FloatingPoint, FloatingSize {}

export interface SubmenuPlacement {
    left: number;
    top: number;
    maxHeight: number;
}

function clampAxis(anchor: number, size: number, viewportSize: number, margin: number) {
    const max = viewportSize - size - margin;
    if (max < margin) return margin;
    return Math.min(Math.max(anchor, margin), max);
}

export function clampFloatingPosition(
    anchor: FloatingPoint,
    size: FloatingSize,
    viewport: ViewportSize,
    margin = 8,
): FloatingPoint {
    return {
        x: clampAxis(anchor.x, size.width, viewport.width, margin),
        y: clampAxis(anchor.y, size.height, viewport.height, margin),
    };
}

export function placeAdjacentSubmenu(
    parent: FloatingRect,
    submenu: FloatingSize,
    viewport: ViewportSize,
    preferredMaxHeight: number,
    margin = 8,
): SubmenuPlacement {
    const maxHeight = Math.max(160, Math.min(preferredMaxHeight, viewport.height - margin * 2));
    const height = Math.min(submenu.height, maxHeight);
    const minTop = margin - parent.y;
    const maxTop = viewport.height - margin - parent.y - height;

    let top = -4;
    if (parent.y + top + height > viewport.height - margin) {
        top = parent.height - height + 4;
    }
    top = maxTop < minTop ? minTop : Math.min(Math.max(top, minTop), maxTop);

    const right = parent.width - 1;
    const left = -submenu.width + 1;
    const fitsRight = parent.x + parent.width + submenu.width <= viewport.width - margin;
    const fitsLeft = parent.x - submenu.width >= margin;
    let horizontal = right;

    if (!fitsRight && fitsLeft) {
        horizontal = left;
    } else if (!fitsRight) {
        const rightSpace = viewport.width - margin - (parent.x + parent.width);
        const leftSpace = parent.x - margin;
        horizontal = leftSpace > rightSpace
            ? margin - parent.x
            : viewport.width - margin - parent.x - submenu.width;
    }

    return {
        left: Math.round(horizontal),
        top: Math.round(top),
        maxHeight: Math.round(maxHeight),
    };
}
