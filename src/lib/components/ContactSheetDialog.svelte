<script lang="ts">
    import { convertFileSrc } from '@tauri-apps/api/core';
    import { save } from '@tauri-apps/plugin-dialog';
    import { contactSheetOpen, images, selectedIds, showToast } from '$lib/stores';
    import { savePngToPath } from '$lib/api';
    import {
        computeContactSheetLayout,
        contactSheetCellLabel,
        DEFAULT_CONTACT_SHEET_CONFIG,
        type ContactSheetConfig,
    } from '$lib/contact-sheet';
    import type { ImageWithFile } from '$lib/api';

    let columns = $state(DEFAULT_CONTACT_SHEET_CONFIG.columns);
    let cellWidth = $state(DEFAULT_CONTACT_SHEET_CONFIG.cellWidth);
    let cellHeight = $state(DEFAULT_CONTACT_SHEET_CONFIG.cellHeight);
    let showFilename = $state(true);
    let showRating = $state(true);
    let showMetadata = $state(false);
    let useSelectionOnly = $state(false);
    let rendering = $state(false);

    let selected = $derived(Array.from($selectedIds));
    let sheetImages = $derived(
        useSelectionOnly && selected.length > 0
            ? $images.filter(i => $selectedIds.has(i.image.id))
            : $images
    );

    function currentConfig(): ContactSheetConfig {
        return {
            ...DEFAULT_CONTACT_SHEET_CONFIG,
            columns,
            cellWidth,
            cellHeight,
            showFilename,
            showRating,
            showMetadata,
        };
    }

    function loadImage(src: string): Promise<HTMLImageElement | null> {
        return new Promise(resolve => {
            const img = new Image();
            img.onload = () => resolve(img);
            img.onerror = () => resolve(null);
            img.src = src;
        });
    }

    function drawCover(
        ctx: CanvasRenderingContext2D,
        img: HTMLImageElement,
        x: number,
        y: number,
        w: number,
        h: number,
    ) {
        const scale = Math.max(w / img.width, h / img.height);
        const dw = img.width * scale;
        const dh = img.height * scale;
        const dx = x + (w - dw) / 2;
        const dy = y + (h - dh) / 2;
        ctx.save();
        ctx.beginPath();
        ctx.rect(x, y, w, h);
        ctx.clip();
        ctx.drawImage(img, dx, dy, dw, dh);
        ctx.restore();
    }

    async function renderToCanvas(items: ImageWithFile[]): Promise<HTMLCanvasElement> {
        const config = currentConfig();
        const layout = computeContactSheetLayout(items.length, config);
        const canvas = document.createElement('canvas');
        canvas.width = layout.width;
        canvas.height = layout.height;
        const ctx = canvas.getContext('2d');
        if (!ctx) throw new Error('Canvas 2D context unavailable');

        // Background.
        ctx.fillStyle = '#08080c';
        ctx.fillRect(0, 0, layout.width, layout.height);

        for (const cell of layout.cells) {
            const item = items[cell.index];
            // Cell backdrop.
            ctx.fillStyle = '#0c0c12';
            ctx.fillRect(cell.x, cell.y, cell.width, cell.height);

            const src = convertFileSrc(item.thumbnail_path ?? item.path);
            const img = await loadImage(src);
            if (img) {
                drawCover(ctx, img, cell.x, cell.y, cell.width, cell.height);
            } else {
                ctx.fillStyle = '#1a1a2e';
                ctx.fillRect(cell.x, cell.y, cell.width, cell.height);
            }

            const label = contactSheetCellLabel(item, config);
            if (label) {
                ctx.fillStyle = '#e0e0e0';
                ctx.font = '13px monospace';
                ctx.textBaseline = 'top';
                ctx.fillText(label, cell.labelX + 2, cell.labelY + 6, cell.width - 4);
            }
        }

        return canvas;
    }

    async function exportSheet() {
        if (rendering) return;
        if (sheetImages.length === 0) {
            showToast('No images to export', { type: 'warning' });
            return;
        }
        const target = await save({
            title: 'Save contact sheet',
            defaultPath: 'contact-sheet.png',
            filters: [{ name: 'PNG', extensions: ['png'] }],
        });
        if (!target) return;

        rendering = true;
        try {
            const canvas = await renderToCanvas(sheetImages);
            const dataUrl = canvas.toDataURL('image/png');
            const base64 = dataUrl.split(',')[1] ?? '';
            const written = await savePngToPath(target, base64);
            showToast(`Contact sheet saved (${sheetImages.length} images)`, {
                detail: written,
                type: 'success',
                duration: 7000,
            });
            contactSheetOpen.set(false);
        } catch (e) {
            showToast('Contact sheet export failed', { detail: String(e), type: 'error', duration: 9000 });
        } finally {
            rendering = false;
        }
    }

    function close() {
        if (!rendering) contactSheetOpen.set(false);
    }

    function onBackdropKeydown(event: KeyboardEvent) {
        if (event.key === 'Escape') close();
    }
</script>

{#if $contactSheetOpen}
    <div
        class="cs-backdrop"
        role="dialog"
        aria-modal="true"
        aria-label="Export contact sheet"
        tabindex="-1"
        onclick={close}
        onkeydown={onBackdropKeydown}
    >
        <!-- svelte-ignore a11y_click_events_have_key_events -->
        <div class="cs-panel" role="presentation" onclick={(e) => e.stopPropagation()}>
            <div class="cs-head">
                <span class="cs-title">Export Contact Sheet</span>
                <button class="cs-close" type="button" onclick={close} aria-label="Close">×</button>
            </div>
            <p class="cs-scope">{sheetImages.length} image{sheetImages.length === 1 ? '' : 's'} in the sheet.</p>

            <div class="cs-grid">
                <label class="cs-field">
                    <span>Columns</span>
                    <input type="number" min="1" max="12" bind:value={columns} />
                </label>
                <label class="cs-field">
                    <span>Cell width</span>
                    <input type="number" min="80" max="800" step="20" bind:value={cellWidth} />
                </label>
                <label class="cs-field">
                    <span>Cell height</span>
                    <input type="number" min="60" max="800" step="20" bind:value={cellHeight} />
                </label>
            </div>

            <div class="cs-checks">
                <label class="cs-check"><input type="checkbox" bind:checked={showFilename} /><span>Filename</span></label>
                <label class="cs-check"><input type="checkbox" bind:checked={showRating} /><span>Rating</span></label>
                <label class="cs-check"><input type="checkbox" bind:checked={showMetadata} /><span>Dimensions</span></label>
                <label class="cs-check"><input type="checkbox" bind:checked={useSelectionOnly} /><span>Selected only</span></label>
            </div>

            <div class="cs-actions">
                <button class="cs-cancel" type="button" onclick={close} disabled={rendering}>Cancel</button>
                <button class="cs-go" type="button" onclick={exportSheet} disabled={rendering}>
                    {rendering ? 'Rendering…' : 'Render & Save PNG'}
                </button>
            </div>
        </div>
    </div>
{/if}

<style>
    .cs-backdrop {
        position: fixed;
        inset: 0;
        background: rgba(0, 0, 0, 0.55);
        display: flex;
        align-items: flex-start;
        justify-content: center;
        padding-top: 12vh;
        z-index: 1210;
    }
    .cs-panel {
        width: min(460px, 92vw);
        background: var(--surface);
        border: 1px solid var(--border);
        border-radius: var(--radius);
        box-shadow: 0 18px 60px rgba(0, 0, 0, 0.5);
        padding: calc(var(--spacing) * 2);
        display: flex;
        flex-direction: column;
        gap: var(--spacing);
    }
    .cs-head {
        display: flex;
        align-items: center;
        justify-content: space-between;
    }
    .cs-title {
        font-weight: 600;
        color: var(--text);
    }
    .cs-close {
        background: transparent;
        border: none;
        color: var(--text-secondary);
        font-size: 18px;
        cursor: pointer;
    }
    .cs-close:hover {
        color: var(--text);
    }
    .cs-scope {
        color: var(--text-secondary);
        margin: 0;
        font-size: 13px;
    }
    .cs-grid {
        display: grid;
        grid-template-columns: repeat(3, 1fr);
        gap: var(--spacing);
    }
    .cs-field {
        display: flex;
        flex-direction: column;
        gap: 4px;
        color: var(--text-secondary);
        font-size: 12px;
    }
    .cs-field input {
        background: var(--bg);
        border: 1px solid var(--border);
        border-radius: var(--radius);
        color: var(--text);
        padding: 6px var(--spacing);
        font-family: var(--font, monospace);
    }
    .cs-checks {
        display: flex;
        flex-wrap: wrap;
        gap: var(--spacing);
    }
    .cs-check {
        display: flex;
        align-items: center;
        gap: 6px;
        color: var(--text-secondary);
        font-size: 12px;
    }
    .cs-actions {
        display: flex;
        justify-content: flex-end;
        gap: var(--spacing);
        margin-top: var(--spacing);
    }
    .cs-cancel,
    .cs-go {
        border: 1px solid var(--border);
        border-radius: var(--radius);
        padding: 6px 12px;
        cursor: pointer;
        font-family: var(--font, monospace);
        font-size: 13px;
    }
    .cs-cancel {
        background: transparent;
        color: var(--text-secondary);
    }
    .cs-go {
        background: var(--blue);
        color: var(--bg);
        border-color: var(--blue);
    }
    .cs-go:disabled,
    .cs-cancel:disabled {
        opacity: 0.5;
        cursor: default;
    }
</style>
