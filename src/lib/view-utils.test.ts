import { describe, it, expect } from 'vitest';
import {
    getFilename,
    getThumbnailBorderClass,
    buildRangeSelectionIds,
    buildThumbnailAriaLabel,
    computeAnchoredGridScrollTop,
    computeGridClickSelection,
    computeGridLayout,
    computeVisibleItems,
    formatLoupeInfo,
    computeWheelZoom,
    computePanDrag,
    clientToImagePoint,
    cropRectFromImagePoints,
    cropSelectionPercentFromImagePoints,
    moveCropRect,
    resizeCropRectFromHandle,
    chooseLoupeImagePath,
    isAssetProtocolSafePath,
    safeAssetPreviewPath,
    pickThumbnailVariant,
    THUMBNAIL_SIZES,
    computeScrollDirection,
    computeOverscan,
    computePrefetchIndices,
} from './view-utils';

describe('getFilename', () => {
    it('extracts filename from absolute path', () => {
        expect(getFilename('/Users/test/Photos/IMG_001.jpg')).toBe('IMG_001.jpg');
    });

    it('extracts filename from relative path', () => {
        expect(getFilename('photos/vacation/sunset.png')).toBe('sunset.png');
    });

    it('returns the string itself when no slashes', () => {
        expect(getFilename('image.tiff')).toBe('image.tiff');
    });

    it('returns default fallback for empty string', () => {
        expect(getFilename('')).toBe('image');
    });

    it('returns custom fallback for trailing slash', () => {
        expect(getFilename('/some/dir/', 'unknown')).toBe('unknown');
    });
});

describe('buildThumbnailAriaLabel', () => {
    it('includes all announced fields', () => {
        expect(buildThumbnailAriaLabel({
            filename: 'image-1.png',
            rating: 4,
            decision: 'accept',
            sourceTag: 'MJ',
            selected: true,
            missing: false,
        })).toBe('image-1.png, rating 4, decision accept, source MJ, selected, present');
    });

    it('includes default states when fields are absent or falsey', () => {
        expect(buildThumbnailAriaLabel({
            filename: '',
            rating: 0,
            decision: '',
            sourceTag: null,
            selected: false,
            missing: true,
        })).toBe('image, rating 0, decision undecided, source unknown, not selected, missing');
    });
});

describe('getThumbnailBorderClass', () => {
    it('returns focused when focused', () => {
        expect(getThumbnailBorderClass(true, false)).toBe('focused');
    });

    it('returns selected when selected but not focused', () => {
        expect(getThumbnailBorderClass(false, true)).toBe('selected');
    });

    it('returns focused when both focused and selected (focus wins)', () => {
        expect(getThumbnailBorderClass(true, true)).toBe('focused');
    });

    it('returns empty string when neither', () => {
        expect(getThumbnailBorderClass(false, false)).toBe('');
    });
});

describe('buildRangeSelectionIds', () => {
    const items = ['a', 'b', 'c', 'd', 'e'].map(id => ({ id }));

    it('selects an inclusive forward range', () => {
        expect(buildRangeSelectionIds(items, 1, 3, item => item.id)).toEqual(new Set(['b', 'c', 'd']));
    });

    it('selects an inclusive reverse range', () => {
        expect(buildRangeSelectionIds(items, 4, 2, item => item.id)).toEqual(new Set(['c', 'd', 'e']));
    });

    it('clamps out-of-bounds endpoints', () => {
        expect(buildRangeSelectionIds(items, -10, 99, item => item.id)).toEqual(new Set(['a', 'b', 'c', 'd', 'e']));
    });

    it('returns an empty selection when there are no items', () => {
        expect(buildRangeSelectionIds([], 0, 2, item => item)).toEqual(new Set());
    });
});

describe('computeGridClickSelection', () => {
    const items = ['a', 'b', 'c', 'd', 'e', 'f'].map(id => ({ id }));

    it('plain click only moves the range anchor and leaves selection intact', () => {
        const result = computeGridClickSelection({
            items,
            selectedIds: new Set(['b']),
            focusedIndex: 1,
            anchorIndex: null,
            targetIndex: 4,
            shiftKey: false,
            toggleKey: false,
            getId: item => item.id,
        });

        expect(result.selectedIds).toBeNull();
        expect(result.anchorIndex).toBe(4);
    });

    it('shift click adds the anchor range without dropping existing selection', () => {
        const result = computeGridClickSelection({
            items,
            selectedIds: new Set(['a']),
            focusedIndex: 1,
            anchorIndex: null,
            targetIndex: 4,
            shiftKey: true,
            toggleKey: false,
            getId: item => item.id,
        });

        expect(result.selectedIds).toEqual(new Set(['a', 'b', 'c', 'd', 'e']));
        expect(result.anchorIndex).toBe(1);
    });

    it('shift click keeps using the existing anchor instead of the current focus', () => {
        const result = computeGridClickSelection({
            items,
            selectedIds: new Set(['f']),
            focusedIndex: 4,
            anchorIndex: 1,
            targetIndex: 3,
            shiftKey: true,
            toggleKey: false,
            getId: item => item.id,
        });

        expect(result.selectedIds).toEqual(new Set(['f', 'b', 'c', 'd']));
        expect(result.anchorIndex).toBe(1);
    });

    it('option click toggles the clicked image without affecting the rest of the selection', () => {
        const result = computeGridClickSelection({
            items,
            selectedIds: new Set(['b', 'd']),
            focusedIndex: 1,
            anchorIndex: 1,
            targetIndex: 3,
            shiftKey: false,
            toggleKey: true,
            getId: item => item.id,
        });

        expect(result.selectedIds).toEqual(new Set(['b']));
        expect(result.anchorIndex).toBe(3);
    });

    it('shift option click toggles the anchor range', () => {
        const result = computeGridClickSelection({
            items,
            selectedIds: new Set(['a', 'b', 'e']),
            focusedIndex: 0,
            anchorIndex: 1,
            targetIndex: 3,
            shiftKey: true,
            toggleKey: true,
            getId: item => item.id,
        });

        expect(result.selectedIds).toEqual(new Set(['a', 'e', 'c', 'd']));
        expect(result.anchorIndex).toBe(1);
    });
});

describe('chooseLoupeImagePath', () => {
    const item = {
        path: '/Users/test/Pictures/full.png',
        thumbnail_path: '/Users/test/Library/Application Support/com.glebkalinin.cull/thumbnails/img-1.jpg',
    };

    it('uses the original path for regular Loupe images', () => {
        expect(chooseLoupeImagePath(item, false, false)).toBe(item.path);
    });

    it('falls back to the thumbnail after a source load failure', () => {
        expect(chooseLoupeImagePath(item, false, true)).toBe(item.thumbnail_path);
    });

    it('uses the thumbnail for RAW images', () => {
        expect(chooseLoupeImagePath(item, true, false)).toBe(item.thumbnail_path);
    });

    it('does not fall back to an imported original after source load failure', () => {
        expect(chooseLoupeImagePath({
            path: '/Users/test/Pictures/imported/full.png',
            thumbnail_path: null,
        }, false, true)).toBeNull();
    });

    it('allows imported originals when no thumbnail exists for Loupe full-quality loading', () => {
        const original = '/Users/test/Pictures/imported/full.png';
        expect(chooseLoupeImagePath({
            path: original,
            thumbnail_path: null,
        }, false, false)).toBe(original);
    });

    it('allows app-owned generated images when no thumbnail exists', () => {
        const generated = '/Users/test/Library/Application Support/com.glebkalinin.cull/generated/img.png';
        expect(chooseLoupeImagePath({
            path: generated,
            thumbnail_path: null,
        }, false, false)).toBe(generated);
    });
});

describe('safeAssetPreviewPath', () => {
    it('prefers app-owned thumbnails over imported originals', () => {
        const thumbnail = '/Users/test/Library/Application Support/com.glebkalinin.cull/thumbnails/img-1.jpg';
        expect(safeAssetPreviewPath({
            path: '/Users/test/Pictures/imported/full.png',
            thumbnail_path: thumbnail,
        })).toBe(thumbnail);
    });

    it('does not return imported originals without a safe preview', () => {
        expect(safeAssetPreviewPath({
            path: '/Users/test/Pictures/imported/full.png',
            thumbnail_path: null,
        })).toBeNull();
    });

    it('allows static app-owned generated paths', () => {
        expect(isAssetProtocolSafePath('/Users/test/Library/Application Support/com.glebkalinin.cull/generated/img.png')).toBe(true);
        // Developer-personal output dirs are no longer in the shipped scope.
        expect(isAssetProtocolSafePath('/Users/test/.codex/generated_images/img.png')).toBe(false);
    });

    it('rejects user-selected clipboard capture directories', () => {
        expect(isAssetProtocolSafePath('/Users/test/Desktop/Cull Clipboard/capture.png')).toBe(false);
    });
});

describe('computeGridLayout', () => {
    it('computes columns from container width', () => {
        // floor((800+4)/(160+4)) = floor(804/164) = 4
        const layout = computeGridLayout(800, 160, 4, 100);
        expect(layout.cols).toBe(4);
        expect(layout.cellSize).toBe(164);
        expect(layout.rows).toBe(25);
        expect(layout.totalHeight).toBe(4100);
    });

    it('ensures at least 1 column', () => {
        const layout = computeGridLayout(50, 160, 4, 10);
        expect(layout.cols).toBe(1);
        expect(layout.rows).toBe(10);
    });

    it('computes correct row count with partial last row', () => {
        // floor((400+10)/(100+10)) = floor(410/110) = 3 cols, ceil(10/3) = 4 rows
        const layout = computeGridLayout(400, 100, 10, 10);
        expect(layout.cols).toBe(3);
        expect(layout.cellSize).toBe(110);
        expect(layout.rows).toBe(4);
        expect(layout.totalHeight).toBe(440);
    });

    it('computes cellSize as thumbSize + gap', () => {
        const layout = computeGridLayout(800, 120, 8, 50);
        expect(layout.cellSize).toBe(128);
    });

    it('computes totalHeight with hardcoded expectation', () => {
        // floor((800+4)/(160+4)) = 4 cols, ceil(20/4) = 5 rows, 5*164 = 820
        const layout = computeGridLayout(800, 160, 4, 20);
        expect(layout.cols).toBe(4);
        expect(layout.rows).toBe(5);
        expect(layout.totalHeight).toBe(820);
    });

    it('handles zero items', () => {
        const layout = computeGridLayout(800, 160, 4, 0);
        expect(layout.rows).toBe(0);
        expect(layout.totalHeight).toBe(0);
    });

    it('handles single item', () => {
        const layout = computeGridLayout(800, 160, 4, 1);
        expect(layout.rows).toBe(1);
        expect(layout.cols).toBe(4);
        expect(layout.totalHeight).toBe(164);
    });

    it('handles thumbSize=0', () => {
        // floor((800+4)/(0+4)) = 201 cols
        const layout = computeGridLayout(800, 0, 4, 10);
        expect(layout.cols).toBe(201);
        expect(layout.cellSize).toBe(4);
    });

    it('handles negative gap', () => {
        // floor((800+(-5))/(100+(-5))) = floor(795/95) = 8
        const layout = computeGridLayout(800, 100, -5, 20);
        expect(layout.cols).toBe(8);
        expect(layout.cellSize).toBe(95);
    });
});

describe('computeAnchoredGridScrollTop', () => {
    it('keeps the same anchor image at the same viewport position when zooming in changes columns', () => {
        const scrollTop = computeAnchoredGridScrollTop({
            oldScrollTop: 164,
            viewportWidth: 800,
            viewportHeight: 600,
            anchorX: 400,
            anchorY: 300,
            oldCols: 4,
            oldCellSize: 164,
            newCols: 2,
            newCellSize: 288,
            totalItems: 100,
        });

        // Old anchor: row 2, col 2 => item 10. New layout puts item 10 in row 5,
        // preserving the anchor's fractional offset inside that cell.
        expect(scrollTop).toBeCloseTo(1378.8293, 4);
    });

    it('keeps the same anchor image in view when zooming out adds columns', () => {
        const scrollTop = computeAnchoredGridScrollTop({
            oldScrollTop: 1378.8293,
            viewportWidth: 800,
            viewportHeight: 600,
            anchorX: 400,
            anchorY: 300,
            oldCols: 2,
            oldCellSize: 288,
            newCols: 4,
            newCellSize: 164,
            totalItems: 100,
        });

        expect(scrollTop).toBeCloseTo(164, 0);
    });

    it('keeps the same anchor image when the viewport changes column count without zooming', () => {
        const scrollTop = computeAnchoredGridScrollTop({
            oldScrollTop: 820,
            viewportWidth: 800,
            viewportHeight: 600,
            anchorX: 400,
            anchorY: 300,
            oldCols: 4,
            oldCellSize: 164,
            newCols: 5,
            newCellSize: 164,
            totalItems: 100,
        });

        expect(scrollTop).toBeCloseTo(656, 0);
    });

    it('clamps anchored scroll to the new layout bounds', () => {
        const scrollTop = computeAnchoredGridScrollTop({
            oldScrollTop: 4000,
            viewportWidth: 800,
            viewportHeight: 600,
            anchorX: 400,
            anchorY: 300,
            oldCols: 4,
            oldCellSize: 164,
            newCols: 8,
            newCellSize: 82,
            totalItems: 40,
        });

        expect(scrollTop).toBe(0);
    });
});

describe('computeVisibleItems', () => {
    it('returns items visible at scroll position 0', () => {
        const items = computeVisibleItems(0, 600, 4, 164, 100);
        expect(items.length).toBeGreaterThan(0);
        expect(items[0].index).toBe(0);
    });

    it('positions items on correct grid coordinates', () => {
        const items = computeVisibleItems(0, 600, 3, 100, 9);
        const first = items[0];
        expect(first.x).toBe(0);
        expect(first.y).toBe(0);
        const second = items[1];
        expect(second.x).toBe(100);
        expect(second.y).toBe(0);
        const fourthItem = items[3];
        expect(fourthItem.x).toBe(0);
        expect(fourthItem.y).toBe(100);
    });

    it('does not exceed total item count', () => {
        const items = computeVisibleItems(0, 10000, 4, 164, 5);
        expect(items.length).toBe(5);
    });

    it('skips items above the scroll position', () => {
        // scrollTop=500, cellSize=100 → firstVisibleRow = floor(500/100) = 5
        // cols=4, so first index = 5*4 = 20
        const items = computeVisibleItems(500, 300, 4, 100, 100);
        expect(items[0].index).toBe(20);
        expect(items[0].x).toBe(0);
        expect(items[0].y).toBe(500);
    });

    it('returns empty for zero items', () => {
        const items = computeVisibleItems(0, 600, 4, 164, 0);
        expect(items).toEqual([]);
    });

    it('includes buffer rows (2 extra)', () => {
        // containerHeight=100, cellSize=100 → ceil(100/100)+2 = 3 visible rows
        // cols=1, totalItems=10 → 3 items
        const items = computeVisibleItems(0, 100, 1, 100, 10);
        expect(items.length).toBe(3);
        expect(items[0].index).toBe(0);
        expect(items[1].index).toBe(1);
        expect(items[2].index).toBe(2);
    });

    it('supports explicit overscan before and after the visible rows', () => {
        const items = computeVisibleItems(500, 100, 1, 100, 20, {
            overscanRowsBefore: 2,
            overscanRowsAfter: 3,
        });

        expect(items.map(item => item.index)).toEqual([3, 4, 5, 6, 7, 8]);
    });

    it('handles negative scrollTop', () => {
        const items = computeVisibleItems(-100, 300, 4, 100, 20);
        expect(items[0].index).toBe(0);
        expect(items.every(i => i.index >= 0)).toBe(true);
    });

    it('handles containerHeight=0', () => {
        // ceil(0/100)+2 = 2 visible rows → up to 2*4=8 items
        const items = computeVisibleItems(0, 0, 4, 100, 20);
        expect(items.length).toBe(8);
    });

    it('returns empty for invalid layout inputs', () => {
        expect(computeVisibleItems(0, 600, 0, 100, 20)).toEqual([]);
        expect(computeVisibleItems(0, 600, 4, 0, 20)).toEqual([]);
        expect(computeVisibleItems(0, 600, 4, 100, -1)).toEqual([]);
    });
});

describe('grid rendering performance budget', () => {
    const totalItems = 100_000;
    const containerWidth = 3840;
    const containerHeight = 2160;
    const thumbSize = 120;
    const gap = 4;

    it('keeps the render plan bounded to visible thumbnails plus overscan', () => {
        const layout = computeGridLayout(containerWidth, thumbSize, gap, totalItems);
        const items = computeVisibleItems(520_000, containerHeight, layout.cols, layout.cellSize, totalItems);
        const maxVisibleItems = layout.cols * (Math.ceil(containerHeight / layout.cellSize) + 2);

        expect(items.length).toBeLessThanOrEqual(maxVisibleItems);
        expect(items.length).toBeLessThan(700);
    });

    it('computes a large-library render window within the CPU budget', () => {
        const layout = computeGridLayout(containerWidth, thumbSize, gap, totalItems);
        const iterations = 1_000;
        let checksum = 0;

        for (let i = 0; i < 50; i++) {
            checksum += computeVisibleItems(i * 997, containerHeight, layout.cols, layout.cellSize, totalItems).length;
        }

        const started = globalThis.performance.now();
        for (let i = 0; i < iterations; i++) {
            const items = computeVisibleItems(i * 997, containerHeight, layout.cols, layout.cellSize, totalItems);
            checksum += items.length + (items[0]?.index ?? 0);
        }
        const elapsedMs = globalThis.performance.now() - started;

        expect(checksum).toBeGreaterThan(0);
        expect(elapsedMs).toBeLessThan(250);
    });

    // PERF-07 (bd imageview-dkz.31): thumbnail load-path p95 < 200ms at 10k images.
    //
    // Measures the real frontend thumbnail-load pipeline exactly as Grid.svelte +
    // Thumbnail.svelte execute it per scroll event: scroll-direction hysteresis →
    // asymmetric overscan → visible-window computation → per-tile path resolution
    // (base + dpr-sized variant via safeAssetPreviewPath/pickThumbnailVariant) →
    // a11y label construction → prefetch-index warming path resolution. The webview's
    // asset fetch + JPEG decode of the pre-generated thumbnail files is outside what
    // a vitest harness can observe and is NOT included; this measures (and guards)
    // the entire synchronous JS cost on the load path. Numbers are printed so the
    // release-audit report can record p50/p95 against the 200ms threshold.
    it('keeps the 10k-library per-thumbnail load-path p95 under the 200ms budget (PERF-07)', () => {
        const totalImages = 10_000;
        const appDir = '/Users/cull/Library/Application Support/com.glebkalinin.cull';
        const sources = ['midjourney', 'stable_diffusion', 'gpt_image_2', null, 'comfyui'];
        const decisions = ['keep', 'reject', 'undecided'];

        // Synthesize 10k ImageWithFile-shaped records matching what the grid binds to.
        const items = Array.from({ length: totalImages }, (_, i) => ({
            image: { id: `img-${i.toString(16).padStart(8, '0')}` },
            path: `/Users/cull/Pictures/ai/batch-${i % 37}/generation_${i}_${(i * 2654435761) % 4096}.png`,
            thumbnail_path: `${appDir}/thumbnails/${((i * 2654435761) >>> 0).toString(16)}_${i}.jpg`,
            selection: i % 3 === 0
                ? { star_rating: i % 6, decision: decisions[i % decisions.length] }
                : null,
            source_label: sources[i % sources.length],
            missing_at: i % 1009 === 0 ? '2026-06-01T00:00:00Z' : null,
        }));

        // Realistic browse setup: 1600x900 viewport, default 160px thumbs, retina.
        const browseWidth = 1600;
        const browseHeight = 900;
        const browseThumb = 160;
        const browseGap = 4;
        const dpr = 2;
        const layout = computeGridLayout(browseWidth, browseThumb, browseGap, totalImages);
        const preloadRows = Math.max(2, Math.ceil(200 / Math.max(layout.cols, 1)));

        const perItemMs: number[] = [];
        const perBatchMs: number[] = [];
        const seen = new Set<number>();
        let direction: ReturnType<typeof computeScrollDirection> = 'none';
        let prevScrollTop = 0;
        let checksum = 0;

        // Page through the entire 10k library viewport-by-viewport (full-library browse).
        for (let scrollTop = 0; scrollTop <= layout.totalHeight; scrollTop += browseHeight) {
            const started = globalThis.performance.now();

            // Grid.svelte onScroll pipeline.
            direction = computeScrollDirection(prevScrollTop, scrollTop, direction);
            prevScrollTop = scrollTop;
            const overscan = computeOverscan(direction, preloadRows);
            const visible = computeVisibleItems(
                scrollTop,
                browseHeight,
                layout.cols,
                layout.cellSize,
                totalImages,
                { overscanRowsBefore: overscan.before, overscanRowsAfter: overscan.after }
            );

            // Thumbnail.svelte per-tile work for every mounted tile.
            for (const { index } of visible) {
                const item = items[index];
                seen.add(index);
                const basePath = safeAssetPreviewPath(item);
                const variantPath = safeAssetPreviewPath(item, { displayPx: browseThumb, dpr });
                const label = buildThumbnailAriaLabel({
                    filename: item.path.split('/').pop() ?? 'image',
                    rating: item.selection?.star_rating ?? 0,
                    decision: item.selection?.decision ?? 'undecided',
                    sourceTag: item.source_label,
                    selected: index % 11 === 0,
                    missing: !!item.missing_at,
                });
                checksum += (basePath?.length ?? 0) + (variantPath?.length ?? 0) + label.length;
            }

            // Grid.svelte warmPrefetch: decode-warm the next screen's paths.
            const prefetchIndices = computePrefetchIndices(
                scrollTop,
                browseHeight,
                layout.cols,
                layout.cellSize,
                totalImages,
                direction,
                Math.max(2, preloadRows)
            );
            for (const index of prefetchIndices) {
                const warmPath = safeAssetPreviewPath(items[index], { displayPx: browseThumb, dpr });
                checksum += warmPath?.length ?? 0;
            }

            const elapsed = globalThis.performance.now() - started;
            perBatchMs.push(elapsed);
            if (visible.length > 0) perItemMs.push(elapsed / visible.length);
        }

        const percentile = (samples: number[], p: number): number => {
            const sorted = [...samples].sort((a, b) => a - b);
            const rank = Math.min(sorted.length - 1, Math.ceil((p / 100) * sorted.length) - 1);
            return sorted[Math.max(0, rank)];
        };

        const itemP50 = percentile(perItemMs, 50);
        const itemP95 = percentile(perItemMs, 95);
        const batchP50 = percentile(perBatchMs, 50);
        const batchP95 = percentile(perBatchMs, 95);

        // Recorded in docs/release-audit-2026-06-09/report.md §0 (PERF-07 gate).
        console.info(
            `[PERF-07] 10k thumbnail load-path: per-item p50=${itemP50.toFixed(4)}ms ` +
            `p95=${itemP95.toFixed(4)}ms; per-scroll-batch p50=${batchP50.toFixed(3)}ms ` +
            `p95=${batchP95.toFixed(3)}ms (${perBatchMs.length} batches, ${seen.size} items rendered)`
        );

        expect(checksum).toBeGreaterThan(0);
        // The full-library browse must actually visit every one of the 10k records.
        expect(seen.size).toBe(totalImages);
        // PERF-07 threshold: thumbnail load p95 < 200ms. Guard it per thumbnail AND
        // per whole scroll batch (the stricter, user-perceived unit).
        expect(itemP95).toBeLessThan(200);
        expect(batchP95).toBeLessThan(200);
    });
});

describe('formatLoupeInfo', () => {
    it('formats basic info string', () => {
        expect(formatLoupeInfo('IMG_001.jpg', 6000, 4000, 'JPEG')).toBe(
            'IMG_001.jpg | 6000x4000 | JPEG'
        );
    });

    it('handles empty filename', () => {
        expect(formatLoupeInfo('', 100, 200, 'PNG')).toBe(' | 100x200 | PNG');
    });

    it('handles zero dimensions', () => {
        expect(formatLoupeInfo('test.raw', 0, 0, 'RAW')).toBe('test.raw | 0x0 | RAW');
    });

    it('handles empty format', () => {
        expect(formatLoupeInfo('photo.arw', 3000, 2000, '')).toBe('photo.arw | 3000x2000 | ');
    });
});

describe('computeWheelZoom', () => {
    it('zooms in on negative deltaY', () => {
        const result = computeWheelZoom(1, -100);
        expect(result).toBeGreaterThan(1);
    });

    it('zooms out on positive deltaY', () => {
        const result = computeWheelZoom(1, 100);
        expect(result).toBeLessThan(1);
    });

    it('clamps to minimum', () => {
        // 0.11 * (1/1.15) ≈ 0.0957, clamped to 0.1
        const result = computeWheelZoom(0.11, 100);
        expect(result).toBe(0.1);
    });

    it('clamps to maximum', () => {
        // 19.5 * 1.15 = 22.425, clamped to 20
        const result = computeWheelZoom(19.5, -100);
        expect(result).toBe(20);
    });

    it('uses custom min/max and clamps correctly', () => {
        // 0.5 * (1/1.15) ≈ 0.4348, clamped to 0.5
        const result = computeWheelZoom(0.5, 100, 0.5, 10);
        expect(result).toBe(0.5);
    });

    it('returns currentScale when deltaY is 0', () => {
        // deltaY=0 is not < 0, so factor = 1/1.15 ≈ 0.8696
        // 1 * 0.8696 ≈ 0.8696
        const result = computeWheelZoom(1, 0);
        expect(result).toBeCloseTo(1 / 1.15, 5);
    });

    it('is roughly reversible (zoom in then out)', () => {
        const zoomed = computeWheelZoom(1, -100);
        const back = computeWheelZoom(zoomed, 100);
        expect(back).toBeCloseTo(1, 5);
    });
});

describe('computePanDrag', () => {
    it('returns correct offset for rightward drag', () => {
        const result = computePanDrag(
            { x: 0, y: 0 },
            { x: 100, y: 100 },
            { x: 150, y: 100 }
        );
        expect(result).toEqual({ x: 50, y: 0 });
    });

    it('returns correct offset for diagonal drag', () => {
        const result = computePanDrag(
            { x: 10, y: 20 },
            { x: 100, y: 200 },
            { x: 130, y: 250 }
        );
        expect(result).toEqual({ x: 40, y: 70 });
    });

    it('handles zero movement', () => {
        const result = computePanDrag(
            { x: 50, y: 50 },
            { x: 100, y: 100 },
            { x: 100, y: 100 }
        );
        expect(result).toEqual({ x: 50, y: 50 });
    });

    it('handles negative drag direction', () => {
        const result = computePanDrag(
            { x: 100, y: 100 },
            { x: 200, y: 200 },
            { x: 150, y: 150 }
        );
        expect(result).toEqual({ x: 50, y: 50 });
    });
});

describe('loupe crop coordinates', () => {
    it('maps client coordinates through the displayed image bounds', () => {
        const point = clientToImagePoint(
            250,
            175,
            { left: 100, top: 50, width: 300, height: 250 },
            1200,
            1000
        );

        expect(point).toEqual({ x: 600, y: 500 });
    });

    it('clamps crop pointer positions to the image', () => {
        const point = clientToImagePoint(
            500,
            0,
            { left: 100, top: 50, width: 300, height: 250 },
            1200,
            1000
        );

        expect(point).toEqual({ x: 1200, y: 0 });
    });

    it('builds rounded image-pixel crop rectangles from either drag direction', () => {
        const rect = cropRectFromImagePoints(
            { x: 900.4, y: 700.6 },
            { x: 100.2, y: 50.1 },
            1200,
            1000
        );

        expect(rect).toEqual({ x: 100, y: 50, width: 800, height: 651 });
    });

    it('keeps crop selection geometry in image percentages', () => {
        const rect = cropSelectionPercentFromImagePoints(
            { x: 100, y: 50 },
            { x: 900, y: 550 },
            1000,
            1000
        );

        expect(rect).toEqual({ left: 10, top: 5, width: 80, height: 50 });
    });

    it('returns null for invalid image dimensions', () => {
        expect(clientToImagePoint(0, 0, { left: 0, top: 0, width: 0, height: 10 }, 100, 100)).toBeNull();
        expect(cropSelectionPercentFromImagePoints({ x: 0, y: 0 }, { x: 1, y: 1 }, 0, 100)).toBeNull();
    });

    it('moves crop rectangles while keeping them inside the image', () => {
        expect(moveCropRect({ x: 100, y: 100, width: 300, height: 200 }, 50, -25, 1000, 800)).toEqual({
            x: 150,
            y: 75,
            width: 300,
            height: 200,
        });
        expect(moveCropRect({ x: 800, y: 650, width: 300, height: 200 }, 100, 100, 1000, 800)).toEqual({
            x: 700,
            y: 600,
            width: 300,
            height: 200,
        });
    });

    it('resizes crop rectangles from handles with minimum dimensions', () => {
        const rect = { x: 100, y: 100, width: 300, height: 200 };

        expect(resizeCropRectFromHandle(rect, 'nw', { x: 50, y: 75 }, 1000, 800, 10)).toEqual({
            x: 50,
            y: 75,
            width: 350,
            height: 225,
        });
        expect(resizeCropRectFromHandle(rect, 'se', { x: 105, y: 105 }, 1000, 800, 10)).toEqual({
            x: 100,
            y: 100,
            width: 10,
            height: 10,
        });
    });
});

describe('pickThumbnailVariant', () => {
    const base = '/Users/u/Library/Application Support/com.glebkalinin.cull/thumbnails/abc123.jpg';

    it('exposes the four generated sizes mirroring the Rust array', () => {
        expect(THUMBNAIL_SIZES).toEqual([64, 128, 256, 800]);
    });

    it('picks the smallest variant for a 64px tile at dpr 1', () => {
        expect(pickThumbnailVariant(base, 64, { dpr: 1 })).toBe(
            '/Users/u/Library/Application Support/com.glebkalinin.cull/thumbnails/abc123_64.jpg'
        );
    });

    it('accounts for device pixel ratio (64px @ dpr 2 -> 128)', () => {
        expect(pickThumbnailVariant(base, 64, { dpr: 2 })).toBe(
            '/Users/u/Library/Application Support/com.glebkalinin.cull/thumbnails/abc123_128.jpg'
        );
    });

    it('returns the base path (800px) when target exceeds 256 (160px @ dpr 2 = 320)', () => {
        expect(pickThumbnailVariant(base, 160, { dpr: 2 })).toBe(base);
    });

    it('picks 256 for a 256px tile at dpr 1', () => {
        expect(pickThumbnailVariant(base, 256, { dpr: 1 })).toBe(
            '/Users/u/Library/Application Support/com.glebkalinin.cull/thumbnails/abc123_256.jpg'
        );
    });

    it('clamps dpr by maxDprMultiplier (default 2)', () => {
        // 160px @ dpr 3 would be 480, but clamped to 2 -> 320 -> base
        expect(pickThumbnailVariant(base, 160, { dpr: 3 })).toBe(base);
    });

    it('returns the base when the target exceeds all sizes', () => {
        expect(pickThumbnailVariant(base, 2000, { dpr: 2 })).toBe(base);
    });

    it('uses the smallest size for non-positive display sizes', () => {
        expect(pickThumbnailVariant(base, 0, { dpr: 1 })).toBe(
            '/Users/u/Library/Application Support/com.glebkalinin.cull/thumbnails/abc123_64.jpg'
        );
    });

    it('leaves a path without a .jpg filename untouched', () => {
        const weird = '/some/dir/no-extension';
        expect(pickThumbnailVariant(weird, 64, { dpr: 1 })).toBe(weird);
    });

    it('preserves the extension casing for .JPG', () => {
        const jpgUpper =
            '/Users/u/Library/Application Support/com.glebkalinin.cull/thumbnails/abc123.JPG';
        expect(pickThumbnailVariant(jpgUpper, 64, { dpr: 1 })).toBe(
            '/Users/u/Library/Application Support/com.glebkalinin.cull/thumbnails/abc123_64.JPG'
        );
    });
});

describe('safeAssetPreviewPath with display size', () => {
    const thumb = '/Users/u/Library/Application Support/com.glebkalinin.cull/thumbnails/abc123.jpg';

    it('is back-compatible without options (returns base thumbnail)', () => {
        expect(safeAssetPreviewPath({ path: '/x/orig.png', thumbnail_path: thumb })).toBe(thumb);
    });

    it('selects a variant when displayPx is provided', () => {
        expect(
            safeAssetPreviewPath({ path: '/x/orig.png', thumbnail_path: thumb }, { displayPx: 64, dpr: 1 })
        ).toBe('/Users/u/Library/Application Support/com.glebkalinin.cull/thumbnails/abc123_64.jpg');
    });

    it('never rewrites the original image path (no variants exist for originals)', () => {
        const safeOriginal =
            '/Users/u/Library/Application Support/com.glebkalinin.cull/generated/orig.jpg';
        expect(
            safeAssetPreviewPath({ path: safeOriginal, thumbnail_path: null }, { displayPx: 64, dpr: 1 })
        ).toBe(safeOriginal);
    });

    it('returns null for unsafe paths', () => {
        expect(
            safeAssetPreviewPath({ path: '/etc/passwd', thumbnail_path: null }, { displayPx: 64 })
        ).toBeNull();
    });
});

describe('computeScrollDirection', () => {
    it('reports down when scrolling forward beyond the threshold', () => {
        expect(computeScrollDirection(0, 100, 'none')).toBe('down');
    });

    it('reports up when scrolling backward beyond the threshold', () => {
        expect(computeScrollDirection(200, 100, 'down')).toBe('up');
    });

    it('keeps the previous direction for sub-threshold jitter (hysteresis)', () => {
        expect(computeScrollDirection(100, 102, 'down', 4)).toBe('down');
        expect(computeScrollDirection(100, 98, 'up', 4)).toBe('up');
    });

    it('defaults to none when there is no prior direction and no movement', () => {
        expect(computeScrollDirection(100, 100, 'none')).toBe('none');
    });
});

describe('computeOverscan', () => {
    it('biases overscan ahead when scrolling down', () => {
        expect(computeOverscan('down', 3)).toEqual({ before: 1, after: 6 });
    });

    it('biases overscan behind (above) when scrolling up', () => {
        expect(computeOverscan('up', 3)).toEqual({ before: 6, after: 1 });
    });

    it('uses a balanced window when direction is none', () => {
        expect(computeOverscan('none', 3)).toEqual({ before: 2, after: 3 });
    });

    it('treats baseRows as at least 1', () => {
        expect(computeOverscan('down', 0)).toEqual({ before: 1, after: 2 });
    });
});

describe('computePrefetchIndices', () => {
    // 4 cols, cellSize 100, container 300 tall (3 visible rows), 100 items (25 rows)
    const args = { cols: 4, cellSize: 100, containerHeight: 300, total: 100, prefetchRows: 2 };

    it('warms the rows just below the window when scrolling down', () => {
        // scrollTop 0 -> visible rows 0..2 -> warm rows 3..4 -> indices 12..19
        const idx = computePrefetchIndices(0, args.containerHeight, args.cols, args.cellSize, args.total, 'down', args.prefetchRows);
        expect(idx).toEqual([12, 13, 14, 15, 16, 17, 18, 19]);
    });

    it('warms the rows just above the window when scrolling up', () => {
        // scrollTop 1000 -> firstVisibleRow 10 -> warm rows 8..9 -> indices 32..39
        const idx = computePrefetchIndices(1000, args.containerHeight, args.cols, args.cellSize, args.total, 'up', args.prefetchRows);
        expect(idx).toEqual([32, 33, 34, 35, 36, 37, 38, 39]);
    });

    it('clamps at the end of the list', () => {
        // near the bottom there is nothing more to warm downward
        const idx = computePrefetchIndices(2200, args.containerHeight, args.cols, args.cellSize, args.total, 'down', args.prefetchRows);
        expect(idx).toEqual([]);
    });

    it('clamps at the top of the list when scrolling up', () => {
        const idx = computePrefetchIndices(0, args.containerHeight, args.cols, args.cellSize, args.total, 'up', args.prefetchRows);
        expect(idx).toEqual([]);
    });

    it('returns nothing for a degenerate layout', () => {
        expect(computePrefetchIndices(0, 300, 0, 100, 100, 'down', 2)).toEqual([]);
        expect(computePrefetchIndices(0, 300, 4, 0, 100, 'down', 2)).toEqual([]);
    });
});
