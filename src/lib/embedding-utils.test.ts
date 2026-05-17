import { describe, it, expect, vi, afterEach } from 'vitest';
import {
    cosineDistance, findNearestNeighbors, computePointOpacity, kMeans,
    formatBytes, formatDownloadRateEta, nameCluster, getClusterPreviewPaths,
    computeUmapNeighborCount, computeClusterCount,
    screenToWorld, worldToScreen,
    computeNeighborLineStyle, computePointRadius,
    computeClusterLabelAlpha, computeTooltipPosition,
    computeZoomTowardCursor, findHoveredPoint,
    isClickWithoutDrag, computeZoomToPointTarget,
    easeOutQuad, computeScatterThumbSize, computeViewportFit,
} from './embedding-utils';

describe('cosineDistance', () => {
    it('returns 0 for identical vectors', () => {
        expect(cosineDistance([1, 2, 3], [1, 2, 3])).toBeCloseTo(0);
    });

    it('returns 1 for orthogonal vectors', () => {
        expect(cosineDistance([1, 0], [0, 1])).toBeCloseTo(1);
    });

    it('returns 2 for opposite vectors', () => {
        expect(cosineDistance([1, 0], [-1, 0])).toBeCloseTo(2);
    });

    it('returns 1 for zero vectors', () => {
        expect(cosineDistance([0, 0], [1, 2])).toBe(1);
        expect(cosineDistance([0, 0], [0, 0])).toBe(1);
    });

    it('is symmetric', () => {
        const a = [1, 3, -5];
        const b = [4, -2, 1];
        expect(cosineDistance(a, b)).toBeCloseTo(cosineDistance(b, a));
    });

    it('handles high-dimensional vectors', () => {
        const a = Array.from({ length: 512 }, (_, i) => Math.sin(i));
        const b = Array.from({ length: 512 }, (_, i) => Math.cos(i));
        const d = cosineDistance(a, b);
        expect(d).toBeGreaterThan(0);
        expect(d).toBeLessThan(2);
    });

    it('similar vectors have small distance', () => {
        const a = [1, 2, 3, 4, 5];
        const b = [1.1, 2.1, 3.1, 4.1, 5.1];
        expect(cosineDistance(a, b)).toBeLessThan(0.01);
    });

    it('returns NaN when b is shorter than a (mismatched lengths)', () => {
        const d = cosineDistance([1, 2, 3], [1, 2]);
        expect(d).toBeNaN();
    });

    it('ignores extra elements when a is shorter than b', () => {
        const d = cosineDistance([1], [1, 2, 3]);
        expect(d).toBeCloseTo(0);
    });

    it('returns NaN when vectors contain NaN', () => {
        expect(cosineDistance([NaN, 1], [1, 1])).toBeNaN();
    });
});

describe('findNearestNeighbors', () => {
    const vectors = new Map<string, number[]>([
        ['a', [1, 0, 0]],
        ['b', [0.9, 0.1, 0]],
        ['c', [0, 1, 0]],
        ['d', [0, 0, 1]],
        ['e', [0.8, 0.2, 0]],
    ]);

    it('returns closest vectors first', () => {
        const { ids, distances } = findNearestNeighbors('a', vectors, 2);
        expect(ids.size).toBe(2);
        expect(ids.has('b')).toBe(true);
        expect(ids.has('e')).toBe(true);
        // b should be closer than e
        expect(distances.get('b')!).toBeLessThan(distances.get('e')!);
    });

    it('respects k limit', () => {
        const { ids } = findNearestNeighbors('a', vectors, 1);
        expect(ids.size).toBe(1);
    });

    it('returns empty for unknown id', () => {
        const { ids, distances } = findNearestNeighbors('unknown', vectors, 3);
        expect(ids.size).toBe(0);
        expect(distances.size).toBe(0);
    });

    it('excludes the target from results', () => {
        const { ids } = findNearestNeighbors('a', vectors, 10);
        expect(ids.has('a')).toBe(false);
    });

    it('handles k larger than available points', () => {
        const { ids } = findNearestNeighbors('a', vectors, 100);
        expect(ids.size).toBe(4); // 5 vectors minus the target
    });

    it('distances are all non-negative', () => {
        const { distances } = findNearestNeighbors('a', vectors, 4);
        for (const d of distances.values()) {
            expect(d).toBeGreaterThanOrEqual(0);
        }
    });
});

describe('computePointOpacity', () => {
    const makePoint = (id: string, cluster: number) => ({ id, cluster });

    it('returns 1 when nothing is focused', () => {
        expect(computePointOpacity(makePoint('p1', 0), null, null, new Set(), false)).toBe(1);
    });

    it('returns 1 for the selected point', () => {
        const selected = makePoint('p1', 0);
        expect(computePointOpacity(selected, selected, null, new Set(), false)).toBe(1);
    });

    it('returns 1 for hovered point even when something else is selected', () => {
        const selected = makePoint('p1', 0);
        const hovered = makePoint('p2', 1);
        expect(computePointOpacity(hovered, selected, null, new Set(), true)).toBe(1);
    });

    it('returns 0.85 for neighbors of selected point', () => {
        const selected = makePoint('p1', 0);
        const neighbor = makePoint('p2', 0);
        const neighborIds = new Set(['p2']);
        expect(computePointOpacity(neighbor, selected, null, neighborIds, false)).toBe(0.85);
    });

    it('returns 0.15 for non-neighbors when a point is selected', () => {
        const selected = makePoint('p1', 0);
        const unrelated = makePoint('p3', 1);
        expect(computePointOpacity(unrelated, selected, null, new Set(['p2']), false)).toBe(0.15);
    });

    it('returns 1 for points in highlighted cluster', () => {
        const point = makePoint('p1', 2);
        expect(computePointOpacity(point, null, 2, new Set(), false)).toBe(1);
    });

    it('returns 0.15 for points outside highlighted cluster', () => {
        const point = makePoint('p1', 0);
        expect(computePointOpacity(point, null, 2, new Set(), false)).toBe(0.15);
    });
});

describe('kMeans', () => {
    afterEach(() => {
        vi.restoreAllMocks();
    });

    it('returns empty array for empty input', () => {
        expect(kMeans([], 3)).toEqual([]);
    });

    it('assigns labels to all points', () => {
        const data = [[0, 0], [1, 0], [10, 10], [11, 10], [20, 20], [21, 20]];
        const labels = kMeans(data, 3);
        expect(labels.length).toBe(6);
    });

    it('labels are valid cluster indices', () => {
        const data = [[0, 0], [1, 1], [5, 5], [6, 6]];
        const labels = kMeans(data, 2);
        for (const l of labels) {
            expect(l).toBeGreaterThanOrEqual(0);
            expect(l).toBeLessThan(2);
        }
    });

    it('nearby points get the same cluster (deterministic)', () => {
        let callCount = 0;
        vi.spyOn(Math, 'random').mockImplementation(() => {
            // Return 0 first (picks index 0), then 0.99 (picks index 5)
            return [0, 0.99][callCount++ % 2];
        });
        const data = [
            [0, 0], [0.1, 0.1], [0.2, 0],
            [100, 100], [100.1, 100.1], [100.2, 100],
        ];
        const labels = kMeans(data, 2);
        expect(labels[0]).toBe(labels[1]);
        expect(labels[1]).toBe(labels[2]);
        expect(labels[3]).toBe(labels[4]);
        expect(labels[4]).toBe(labels[5]);
        expect(labels[0]).not.toBe(labels[3]);
    });

    it('handles k larger than data length', () => {
        const data = [[1, 2], [3, 4]];
        const labels = kMeans(data, 10);
        expect(labels.length).toBe(2);
    });

    it('handles single point', () => {
        const labels = kMeans([[5, 5]], 3);
        expect(labels.length).toBe(1);
        expect(labels[0]).toBe(0);
    });

    it('k=0 returns all-zero labels (no centroids)', () => {
        const labels = kMeans([[1, 2], [3, 4]], 0);
        expect(labels.length).toBe(2);
        expect(labels.every(l => l === 0)).toBe(true);
    });

    it('negative k returns all-zero labels', () => {
        const labels = kMeans([[1, 2], [3, 4]], -1);
        expect(labels.length).toBe(2);
        expect(labels.every(l => l === 0)).toBe(true);
    });
});

describe('formatBytes', () => {
    it('returns 0 B for zero', () => {
        expect(formatBytes(0)).toBe('0 B');
    });

    it('formats kilobytes', () => {
        expect(formatBytes(512 * 1024)).toBe('512 KB');
    });

    it('formats megabytes', () => {
        expect(formatBytes(5 * 1024 * 1024)).toBe('5 MB');
    });

    it('rounds megabytes', () => {
        expect(formatBytes(1.7 * 1024 * 1024)).toBe('2 MB');
    });

    it('small values show as KB', () => {
        expect(formatBytes(1024)).toBe('1 KB');
    });
});

describe('formatDownloadRateEta', () => {
    it('shows transfer rate and estimated time remaining when total is known', () => {
        const startedAt = 1_000;
        const now = 11_000;
        const downloaded = 20 * 1024 * 1024;
        const total = 50 * 1024 * 1024;

        expect(formatDownloadRateEta(downloaded, total, startedAt, now)).toBe('2 MB/s · 15s remaining');
    });

    it('shows only transfer rate until total bytes are known', () => {
        const startedAt = 1_000;
        const now = 6_000;
        const downloaded = 5 * 1024 * 1024;

        expect(formatDownloadRateEta(downloaded, 0, startedAt, now)).toBe('1 MB/s');
    });

    it('returns an empty label before progress starts', () => {
        expect(formatDownloadRateEta(0, 10 * 1024 * 1024, 1_000, 6_000)).toBe('');
    });
});

describe('nameCluster', () => {
    it('returns most common folder', () => {
        const lookup = new Map([['a', '/photos/nature/1.jpg'], ['b', '/photos/nature/2.jpg'], ['c', '/photos/city/3.jpg']]);
        expect(nameCluster(['a', 'b', 'c'], lookup)).toBe('nature');
    });

    it('returns cluster for empty input', () => {
        expect(nameCluster([], new Map())).toBe('cluster');
    });

    it('returns cluster when no paths found', () => {
        expect(nameCluster(['x'], new Map())).toBe('cluster');
    });

    it('handles root-level files', () => {
        const lookup = new Map([['a', 'photo.jpg']]);
        expect(nameCluster(['a'], lookup)).toBe('unknown');
    });
});

describe('getClusterPreviewPaths', () => {
    it('returns up to maxCount thumbnails in order', () => {
        const thumbs = new Map([['a', '/t/a.jpg'], ['b', '/t/b.jpg'], ['c', '/t/c.jpg'], ['d', '/t/d.jpg'], ['e', '/t/e.jpg']]);
        const result = getClusterPreviewPaths(['a', 'b', 'c', 'd', 'e'], thumbs, 3);
        expect(result).toEqual(['/t/a.jpg', '/t/b.jpg', '/t/c.jpg']);
    });

    it('skips ids without thumbnails', () => {
        const thumbs = new Map([['b', '/t/b.jpg']]);
        expect(getClusterPreviewPaths(['a', 'b', 'c'], thumbs)).toEqual(['/t/b.jpg']);
    });

    it('returns empty for no thumbnails', () => {
        expect(getClusterPreviewPaths(['a'], new Map())).toEqual([]);
    });
});

describe('computeUmapNeighborCount', () => {
    it('returns 2 for small datasets', () => {
        expect(computeUmapNeighborCount(5)).toBe(2);
    });

    it('returns 15 for large datasets', () => {
        expect(computeUmapNeighborCount(1000)).toBe(15);
    });

    it('scales with vector count', () => {
        expect(computeUmapNeighborCount(50)).toBe(10);
    });

    it('returns 2 for zero vectors', () => {
        expect(computeUmapNeighborCount(0)).toBe(2);
    });

    it('returns 2 for negative count', () => {
        expect(computeUmapNeighborCount(-10)).toBe(2);
    });

    it('returns NaN for NaN input', () => {
        expect(computeUmapNeighborCount(NaN)).toBeNaN();
    });
});

describe('computeClusterCount', () => {
    it('returns 3 minimum', () => {
        expect(computeClusterCount(4)).toBe(3);
    });

    it('returns 16 maximum', () => {
        expect(computeClusterCount(10000)).toBe(16);
    });

    it('scales with sqrt', () => {
        expect(computeClusterCount(100)).toBe(10);
    });

    it('returns 3 for zero', () => {
        expect(computeClusterCount(0)).toBe(3);
    });

    it('returns NaN for negative input (sqrt of negative)', () => {
        expect(computeClusterCount(-5)).toBeNaN();
    });
});

describe('screenToWorld / worldToScreen', () => {
    it('are inverse operations', () => {
        const [wx, wy] = screenToWorld(150, 200, 50, 30, 2);
        const [sx, sy] = worldToScreen(wx, wy, 50, 30, 2);
        expect(sx).toBeCloseTo(150);
        expect(sy).toBeCloseTo(200);
    });

    it('worldToScreen applies scale and pan', () => {
        const [sx, sy] = worldToScreen(10, 20, 100, 50, 3);
        expect(sx).toBe(10 * 3 + 100);
        expect(sy).toBe(20 * 3 + 50);
    });

    it('screenToWorld reverses transform', () => {
        const [wx, wy] = screenToWorld(130, 110, 100, 50, 3);
        expect(wx).toBeCloseTo(10);
        expect(wy).toBeCloseTo(20);
    });
});

describe('computeNeighborLineStyle', () => {
    it('close distance gives high alpha', () => {
        const style = computeNeighborLineStyle(0);
        expect(style.alpha).toBe(1);
        expect(style.lineWidth).toBe(1.5);
    });

    it('far distance gives minimum alpha', () => {
        const style = computeNeighborLineStyle(1);
        expect(style.alpha).toBeCloseTo(0.15);
        expect(style.lineWidth).toBe(0.5);
    });

    it('mid distance gives proportional values', () => {
        const style = computeNeighborLineStyle(0.2);
        expect(style.alpha).toBeGreaterThan(0.15);
        expect(style.alpha).toBeLessThan(1);
    });
});

describe('computePointRadius', () => {
    it('returns larger radius at low scale', () => {
        expect(computePointRadius(0.5)).toBeGreaterThan(computePointRadius(10));
    });

    it('clamps to minimum of 2', () => {
        expect(computePointRadius(10000)).toBe(2);
    });

    it('clamps to maximum of 5', () => {
        expect(computePointRadius(0.01)).toBe(5);
    });

    it('returns 5 for zero scale (4/sqrt(0) = Infinity, clamped to 5)', () => {
        expect(computePointRadius(0)).toBe(5);
    });

    it('returns NaN for negative scale (sqrt of negative)', () => {
        expect(computePointRadius(-1)).toBeNaN();
    });
});

describe('computeClusterLabelAlpha', () => {
    it('returns 1 when nothing selected', () => {
        expect(computeClusterLabelAlpha(0, null, null)).toBe(1);
    });

    it('returns 0.6 for selected points cluster', () => {
        expect(computeClusterLabelAlpha(2, { cluster: 2 }, null)).toBe(0.6);
    });

    it('returns 0.15 for other clusters when point selected', () => {
        expect(computeClusterLabelAlpha(0, { cluster: 2 }, null)).toBe(0.15);
    });

    it('returns 1 for highlighted cluster', () => {
        expect(computeClusterLabelAlpha(3, null, 3)).toBe(1);
    });

    it('returns 0.25 for non-highlighted clusters', () => {
        expect(computeClusterLabelAlpha(0, null, 3)).toBe(0.25);
    });
});

describe('computeTooltipPosition', () => {
    it('places tooltip to the right of point', () => {
        const pos = computeTooltipPosition(100, 200, 50, 800);
        expect(pos.x).toBe(112);
        expect(pos.y).toBe(192);
    });

    it('clamps to canvas right edge', () => {
        const pos = computeTooltipPosition(780, 200, 50, 800);
        expect(pos.x).toBeLessThanOrEqual(800 - 50 - 16);
    });

    it('clamps to minimum y', () => {
        const pos = computeTooltipPosition(100, 10, 50, 800);
        expect(pos.y).toBe(20);
    });
});

describe('computeZoomTowardCursor', () => {
    it('zoom at center preserves pan', () => {
        const result = computeZoomTowardCursor(400, 300, 0, 0, 1, 2);
        expect(result.scale).toBe(2);
        expect(result.panX).toBeCloseTo(-400);
        expect(result.panY).toBeCloseTo(-300);
    });

    it('zoom at origin scales pan proportionally', () => {
        const result = computeZoomTowardCursor(0, 0, 100, 50, 1, 2);
        expect(result.panX).toBeCloseTo(200);
        expect(result.panY).toBeCloseTo(100);
    });

    it('returns new scale', () => {
        const result = computeZoomTowardCursor(100, 100, 0, 0, 1, 3);
        expect(result.scale).toBe(3);
    });
});

describe('findHoveredPoint', () => {
    const pts = [
        { x: 10, y: 20 },
        { x: 50, y: 60 },
    ];

    it('finds point within hit area', () => {
        const found = findHoveredPoint(pts, 15, 25, 0, 0, 1, 10);
        expect(found).toBe(pts[0]);
    });

    it('returns null when no point hit', () => {
        const found = findHoveredPoint(pts, 200, 200, 0, 0, 1, 5);
        expect(found).toBeNull();
    });

    it('respects scale and pan', () => {
        const found = findHoveredPoint(pts, 25, 45, 5, 5, 2, 5);
        expect(found).toBe(pts[0]);
    });

    it('returns first match', () => {
        const overlapping = [{ x: 10, y: 10 }, { x: 10, y: 10 }];
        const found = findHoveredPoint(overlapping, 10, 10, 0, 0, 1, 5);
        expect(found).toBe(overlapping[0]);
    });

    it('returns null at exact hitHalf boundary (strict <)', () => {
        const pts = [{ x: 10, y: 20 }];
        const found = findHoveredPoint(pts, 15, 20, 0, 0, 1, 5);
        expect(found).toBeNull();
    });

    it('finds point just inside hitHalf boundary', () => {
        const pts = [{ x: 10, y: 20 }];
        const found = findHoveredPoint(pts, 14.99, 20, 0, 0, 1, 5);
        expect(found).toBe(pts[0]);
    });
});

describe('isClickWithoutDrag', () => {
    it('returns true for no movement', () => {
        expect(isClickWithoutDrag(100, 200, 100, 200)).toBe(true);
    });

    it('returns true for small movement', () => {
        expect(isClickWithoutDrag(100, 200, 101, 201)).toBe(true);
    });

    it('returns false for large movement', () => {
        expect(isClickWithoutDrag(100, 200, 110, 210)).toBe(false);
    });

    it('respects custom threshold', () => {
        expect(isClickWithoutDrag(0, 0, 5, 5, 20)).toBe(true);
        expect(isClickWithoutDrag(0, 0, 15, 15, 20)).toBe(false);
    });

    it('returns false at exact threshold boundary (strict <)', () => {
        expect(isClickWithoutDrag(0, 0, 2, 2, 4)).toBe(false);
    });

    it('returns true just below threshold', () => {
        expect(isClickWithoutDrag(0, 0, 1.99, 1.99, 4)).toBe(true);
    });
});

describe('computeZoomToPointTarget', () => {
    it('centers on the point', () => {
        const target = computeZoomToPointTarget(10, 20, 1, 800, 600);
        expect(target.panX).toBe(800 / 2 - 10 * target.scale);
        expect(target.panY).toBe(600 / 2 - 20 * target.scale);
    });

    it('scale is at least 800', () => {
        const target = computeZoomToPointTarget(0, 0, 1, 800, 600);
        expect(target.scale).toBe(800);
    });

    it('scale is 2.5x current when already high', () => {
        const target = computeZoomToPointTarget(0, 0, 500, 800, 600);
        expect(target.scale).toBe(1250);
    });
});

describe('easeOutQuad', () => {
    it('returns 0 at start', () => {
        expect(easeOutQuad(0)).toBe(0);
    });

    it('returns 1 at end', () => {
        expect(easeOutQuad(1)).toBe(1);
    });

    it('is monotonically increasing', () => {
        const values = [0, 0.25, 0.5, 0.75, 1].map(easeOutQuad);
        for (let i = 1; i < values.length; i++) {
            expect(values[i]).toBeGreaterThan(values[i - 1]);
        }
    });

    it('is faster at the start (ease out)', () => {
        expect(easeOutQuad(0.5)).toBeGreaterThan(0.5);
    });
});

describe('computeScatterThumbSize', () => {
    it('uses thumbnails when size >= 8', () => {
        const result = computeScatterThumbSize(10, 10);
        expect(result.useThumb).toBe(true);
        expect(result.size).toBeGreaterThanOrEqual(8);
    });

    it('disables thumbnails for very small sizes', () => {
        const result = computeScatterThumbSize(0.01, 1000);
        expect(result.useThumb).toBe(false);
    });

    it('smaller with more points at same scale', () => {
        const few = computeScatterThumbSize(5, 10);
        const many = computeScatterThumbSize(5, 1000);
        expect(many.size).toBeLessThan(few.size);
    });

    it('density penalty fades at high zoom', () => {
        const fewHigh = computeScatterThumbSize(500, 10);
        const manyHigh = computeScatterThumbSize(500, 1000);
        const fewLow = computeScatterThumbSize(5, 10);
        const manyLow = computeScatterThumbSize(5, 1000);
        const ratioHigh = manyHigh.size / fewHigh.size;
        const ratioLow = manyLow.size / fewLow.size;
        expect(ratioHigh).toBeGreaterThan(ratioLow);
    });

    it('grows large when zoomed in (up to 192)', () => {
        const result = computeScatterThumbSize(800, 310);
        expect(result.size).toBeGreaterThan(150);
        expect(result.size).toBeLessThanOrEqual(192);
    });

    it('clamps to 4 minimum', () => {
        const result = computeScatterThumbSize(0.001, 10000);
        expect(result.size).toBe(4);
    });

    it('handles zero scale', () => {
        const result = computeScatterThumbSize(0, 10);
        expect(result.size).toBe(4);
        expect(result.useThumb).toBe(false);
    });

    it('handles zero points (no density penalty)', () => {
        const result = computeScatterThumbSize(10, 0);
        expect(result.size).toBeGreaterThan(0);
        const withPoints = computeScatterThumbSize(10, 310);
        expect(result.size).toBeGreaterThanOrEqual(withPoints.size);
    });
});

describe('computeViewportFit', () => {
    it('returns default for empty points', () => {
        expect(computeViewportFit([], 800, 600)).toEqual({ scale: 1, panX: 0, panY: 0 });
    });

    it('centers a single point', () => {
        const fit = computeViewportFit([{ x: 10, y: 20 }], 800, 600);
        const [sx, sy] = worldToScreen(10, 20, fit.panX, fit.panY, fit.scale);
        expect(sx).toBeCloseTo(400);
        expect(sy).toBeCloseTo(300);
    });

    it('fits all points within canvas', () => {
        const pts = [{ x: 0, y: 0 }, { x: 100, y: 100 }];
        const fit = computeViewportFit(pts, 800, 600, 60);
        const [sx0, sy0] = worldToScreen(0, 0, fit.panX, fit.panY, fit.scale);
        const [sx1, sy1] = worldToScreen(100, 100, fit.panX, fit.panY, fit.scale);
        expect(sx0).toBeGreaterThanOrEqual(0);
        expect(sy0).toBeGreaterThanOrEqual(0);
        expect(sx1).toBeLessThanOrEqual(800);
        expect(sy1).toBeLessThanOrEqual(600);
    });

    it('respects padding parameter', () => {
        const pts = [{ x: 0, y: 0 }, { x: 10, y: 10 }];
        const tight = computeViewportFit(pts, 800, 600, 10);
        const loose = computeViewportFit(pts, 800, 600, 200);
        expect(tight.scale).toBeGreaterThan(loose.scale);
    });
});
