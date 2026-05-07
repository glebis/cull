export function cosineDistance(a: number[], b: number[]): number {
    let dot = 0, normA = 0, normB = 0;
    for (let i = 0; i < a.length; i++) {
        dot += a[i] * b[i];
        normA += a[i] * a[i];
        normB += b[i] * b[i];
    }
    const denom = Math.sqrt(normA) * Math.sqrt(normB);
    return denom === 0 ? 1 : 1 - dot / denom;
}

export function findNearestNeighbors(
    targetId: string,
    vectors: Map<string, number[]>,
    k: number
): { ids: Set<string>; distances: Map<string, number> } {
    const vec = vectors.get(targetId);
    if (!vec) return { ids: new Set(), distances: new Map() };

    const ranked: [string, number][] = [];
    for (const [id, otherVec] of vectors) {
        if (id === targetId) continue;
        ranked.push([id, cosineDistance(vec, otherVec)]);
    }
    ranked.sort((a, b) => a[1] - b[1]);
    const top = ranked.slice(0, k);
    return {
        ids: new Set(top.map(([id]) => id)),
        distances: new Map(top),
    };
}

export function kMeans(data: number[][], k: number): number[] {
    if (data.length === 0) return [];
    if (k <= 0) return new Array(data.length).fill(0);

    const indices = new Set<number>();
    while (indices.size < k && indices.size < data.length) {
        indices.add(Math.floor(Math.random() * data.length));
    }
    let centroids = Array.from(indices).map(i => [...data[i]]);
    let labels = new Array(data.length).fill(0);

    for (let iter = 0; iter < 20; iter++) {
        for (let i = 0; i < data.length; i++) {
            let minDist = Infinity;
            for (let c = 0; c < centroids.length; c++) {
                const dx = data[i][0] - centroids[c][0];
                const dy = data[i][1] - centroids[c][1];
                const dist = dx * dx + dy * dy;
                if (dist < minDist) {
                    minDist = dist;
                    labels[i] = c;
                }
            }
        }

        const sums = centroids.map(() => [0, 0]);
        const counts = new Array(centroids.length).fill(0);
        for (let i = 0; i < data.length; i++) {
            sums[labels[i]][0] += data[i][0];
            sums[labels[i]][1] += data[i][1];
            counts[labels[i]]++;
        }
        for (let c = 0; c < centroids.length; c++) {
            if (counts[c] > 0) {
                centroids[c] = [sums[c][0] / counts[c], sums[c][1] / counts[c]];
            }
        }
    }

    return labels;
}

export function formatBytes(bytes: number): string {
    if (bytes === 0) return '0 B';
    const mb = bytes / (1024 * 1024);
    if (mb >= 1) return `${mb.toFixed(0)} MB`;
    const kb = bytes / 1024;
    return `${kb.toFixed(0)} KB`;
}

export function nameCluster(
    clusterPointIds: string[],
    pathLookup: Map<string, string>
): string {
    const folderCounts: Map<string, number> = new Map();
    for (const id of clusterPointIds) {
        const path = pathLookup.get(id);
        if (!path) continue;
        const parts = path.split('/');
        const folder = parts.length >= 2 ? parts[parts.length - 2] : 'unknown';
        folderCounts.set(folder, (folderCounts.get(folder) || 0) + 1);
    }
    let best = 'cluster';
    let bestCount = 0;
    for (const [name, count] of folderCounts) {
        if (count > bestCount) { best = name; bestCount = count; }
    }
    return best;
}

export function getClusterPreviewPaths(
    clusterPointIds: string[],
    thumbnailLookup: Map<string, string>,
    maxCount: number = 4
): string[] {
    const paths: string[] = [];
    for (const id of clusterPointIds) {
        if (paths.length >= maxCount) break;
        const thumb = thumbnailLookup.get(id);
        if (thumb) paths.push(thumb);
    }
    return paths;
}

export function computeUmapNeighborCount(vectorCount: number): number {
    return Math.min(15, Math.max(2, Math.floor(vectorCount / 5)));
}

export function computeClusterCount(projectionLength: number): number {
    return Math.min(16, Math.max(3, Math.floor(Math.sqrt(projectionLength))));
}

export function screenToWorld(sx: number, sy: number, panX: number, panY: number, scale: number): [number, number] {
    return [(sx - panX) / scale, (sy - panY) / scale];
}

export function worldToScreen(wx: number, wy: number, panX: number, panY: number, scale: number): [number, number] {
    return [wx * scale + panX, wy * scale + panY];
}

export function computeNeighborLineStyle(distance: number): { alpha: number; lineWidth: number } {
    return {
        alpha: Math.max(0.15, 1 - distance * 3),
        lineWidth: Math.max(0.5, 1.5 * (1 - distance)),
    };
}

export function computePointRadius(scale: number): number {
    return Math.max(2, Math.min(5, 4 / Math.sqrt(scale)));
}

export function computeClusterLabelAlpha(
    clusterId: number,
    selectedPoint: { cluster: number } | null,
    highlightedCluster: number | null
): number {
    if (selectedPoint) {
        return selectedPoint.cluster === clusterId ? 0.6 : 0.15;
    }
    if (highlightedCluster !== null) {
        return highlightedCluster === clusterId ? 1 : 0.25;
    }
    return 1;
}

export function computeTooltipPosition(
    sx: number,
    sy: number,
    textWidth: number,
    canvasWidth: number
): { x: number; y: number } {
    return {
        x: Math.min(sx + 12, canvasWidth - textWidth - 16),
        y: Math.max(sy - 8, 20),
    };
}

export function computeZoomTowardCursor(
    mx: number,
    my: number,
    panX: number,
    panY: number,
    oldScale: number,
    newScale: number
): { panX: number; panY: number; scale: number } {
    return {
        panX: mx - (mx - panX) * (newScale / oldScale),
        panY: my - (my - panY) * (newScale / oldScale),
        scale: newScale,
    };
}

export function findHoveredPoint<T extends { x: number; y: number }>(
    points: T[],
    mx: number,
    my: number,
    panX: number,
    panY: number,
    scale: number,
    hitHalf: number
): T | null {
    for (const p of points) {
        const sx = p.x * scale + panX;
        const sy = p.y * scale + panY;
        if (Math.abs(mx - sx) < hitHalf && Math.abs(my - sy) < hitHalf) {
            return p;
        }
    }
    return null;
}

export function isClickWithoutDrag(startX: number, startY: number, endX: number, endY: number, threshold: number = 4): boolean {
    return Math.abs(endX - startX) + Math.abs(endY - startY) < threshold;
}

export function computeZoomToPointTarget(
    pointX: number,
    pointY: number,
    currentScale: number,
    canvasWidth: number,
    canvasHeight: number
): { scale: number; panX: number; panY: number } {
    const targetScale = Math.max(currentScale * 2.5, 800);
    return {
        scale: targetScale,
        panX: canvasWidth / 2 - pointX * targetScale,
        panY: canvasHeight / 2 - pointY * targetScale,
    };
}

export function easeOutQuad(t: number): number {
    return 1 - (1 - t) * (1 - t);
}

export function computeScatterThumbSize(scale: number, pointCount: number): { size: number; useThumb: boolean } {
    const densityFactor = Math.max(1, Math.sqrt(pointCount / 10));
    const size = Math.max(4, Math.min(48, (8 * Math.sqrt(scale)) / densityFactor));
    return { size, useThumb: size >= 8 };
}

export function computeViewportFit(
    points: { x: number; y: number }[],
    canvasWidth: number,
    canvasHeight: number,
    padding: number = 60
): { scale: number; panX: number; panY: number } {
    if (points.length === 0) return { scale: 1, panX: 0, panY: 0 };
    const xs = points.map(p => p.x);
    const ys = points.map(p => p.y);
    const minX = Math.min(...xs);
    const maxX = Math.max(...xs);
    const minY = Math.min(...ys);
    const maxY = Math.max(...ys);
    const rangeX = maxX - minX || 1;
    const rangeY = maxY - minY || 1;
    const scaleX = (canvasWidth - padding * 2) / rangeX;
    const scaleY = (canvasHeight - padding * 2) / rangeY;
    const scale = Math.min(scaleX, scaleY);
    return {
        scale,
        panX: canvasWidth / 2 - ((minX + maxX) / 2) * scale,
        panY: canvasHeight / 2 - ((minY + maxY) / 2) * scale,
    };
}

export function computePointOpacity(
    point: { id: string; cluster: number },
    selectedPoint: { id: string; cluster: number } | null,
    highlightedCluster: number | null,
    neighborIds: Set<string>,
    isHovered: boolean
): number {
    const hasFocus = selectedPoint !== null || highlightedCluster !== null;
    const isSelected = selectedPoint?.id === point.id;

    if (!hasFocus || isSelected || isHovered) return 1;

    if (selectedPoint) {
        return neighborIds.has(point.id) ? 0.85 : 0.15;
    }
    if (highlightedCluster !== null) {
        return point.cluster === highlightedCluster ? 1 : 0.15;
    }
    return 1;
}
