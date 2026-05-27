<script lang="ts">
    import { images, selectedIds, showToast } from '$lib/stores';
    import { createExportManifest, listExportPresets, getExportAsset } from '$lib/export-api';
    import type { ExportManifest, PresetInfo, ExportTarget } from '$lib/export-types';
    import { convertFileSrc } from '@tauri-apps/api/core';
    import { invoke } from '@tauri-apps/api/core';
    import { toPng } from 'html-to-image';
    import ExportSlideBleed from './ExportSlideBleed.svelte';
    import ExportSlideEditorial from './ExportSlideEditorial.svelte';
    import ExportSlideTerminal from './ExportSlideTerminal.svelte';

    let manifest: ExportManifest | null = $state(null);
    let presets: PresetInfo[] = $state([]);
    let imageSrcs: Record<string, string> = $state({});
    let template: 'terminal' | 'editorial' | 'bleed' = $state('bleed');
    let selectedPreset: string = $state('ig_carousel');
    let exporting = $state(false);
    let exportProgress = $state({ current: 0, total: 0, label: '' });
    let renderRefs: Record<string, HTMLDivElement> = $state({});

    let selectedImages = $derived(
        $selectedIds.size > 0
            ? $images.filter(img => $selectedIds.has(img.image.id))
            : $images.slice(0, 10)
    );

    function getActiveTarget(): ExportTarget | undefined {
        return manifest !== null ? manifest.targets[0] : undefined;
    }
    let activeTarget = $derived(getActiveTarget());

    let previewScale = $derived(
        activeTarget ? Math.min(300 / activeTarget.width, 400 / activeTarget.height, 1) : 0.25
    );

    async function loadPresets() {
        presets = await listExportPresets();
    }

    async function buildManifest() {
        if (selectedImages.length === 0) return;
        const imageIds = selectedImages.map(img => img.image.id);
        manifest = await createExportManifest(imageIds, [selectedPreset], undefined, template);

        for (const asset of manifest.assets) {
            if (asset.kind === 'source') {
                const resp = await getExportAsset(asset.uri, 'original');
                imageSrcs[asset.id] = convertFileSrc(resp.path);
            }
        }
    }

    async function handleTemplateChange(tmpl: 'terminal' | 'editorial' | 'bleed') {
        template = tmpl;
        await buildManifest();
    }

    async function handlePresetChange(presetId: string) {
        selectedPreset = presetId;
        await buildManifest();
    }

    async function exportSlides() {
        if (!manifest || !activeTarget) return;
        exporting = true;
        exportProgress = { current: 0, total: manifest.slides.length + (activeTarget.mime === 'application/pdf' ? 1 : 0), label: 'Preparing export' };

        try {
            const pngPaths: string[] = [];
            const pdfSlideIds: string[] = [];

            for (const [index, slide] of manifest.slides.entries()) {
                const el = renderRefs[slide.id];
                if (!el) continue;
                exportProgress = { ...exportProgress, current: index, label: `Rendering slide ${index + 1}` };

                const dataUrl = await toPng(el, {
                    width: activeTarget.width,
                    height: activeTarget.height,
                    pixelRatio: 1,
                    cacheBust: true,
                });

                const base64 = dataUrl.split(',')[1];
                const path = await invoke<string>('save_export_image', {
                    base64Data: base64,
                    slideId: slide.id,
                    targetId: activeTarget.id,
                    manifestId: manifest.id,
                });
                pngPaths.push(path);
                pdfSlideIds.push(slide.id);
                exportProgress = { ...exportProgress, current: index + 1, label: `Saved slide ${index + 1}` };
            }

            showToast(`Exported ${pngPaths.length} slides`, { type: 'success' });

            if (activeTarget.mime === 'application/pdf' && pngPaths.length > 0) {
                exportProgress = { ...exportProgress, current: manifest.slides.length, label: 'Assembling PDF' };
                const pdfPath = await invoke<string>('assemble_export_pdf', {
                    slideIds: pdfSlideIds,
                    widthPx: activeTarget.width,
                    heightPx: activeTarget.height,
                    manifestId: manifest.id,
                    targetId: activeTarget.id,
                });
                exportProgress = { ...exportProgress, current: exportProgress.total, label: 'PDF saved' };
                showToast(`PDF saved: ${pdfPath.split('/').pop()}`, { type: 'success' });
            }
        } catch (e) {
            showToast(`Export failed: ${e}`, { type: 'error' });
        } finally {
            exporting = false;
            exportProgress = { current: 0, total: 0, label: '' };
        }
    }

    $effect(() => {
        loadPresets();
    });

    $effect(() => {
        if (selectedImages.length > 0) {
            buildManifest();
        }
    });
</script>

<div class="export-view">
    <div class="export-toolbar">
        <div class="template-picker">
            <button class:active={template === 'bleed'} onclick={() => handleTemplateChange('bleed')}>Bleed</button>
            <button class:active={template === 'editorial'} onclick={() => handleTemplateChange('editorial')}>Editorial</button>
            <button class:active={template === 'terminal'} onclick={() => handleTemplateChange('terminal')}>Terminal</button>
        </div>

        <div class="preset-picker">
            <select onchange={(e) => handlePresetChange(e.currentTarget.value)} value={selectedPreset}>
                {#each presets as preset}
                    <option value={preset.id}>{preset.platform} — {preset.format} ({preset.width}×{preset.height})</option>
                {/each}
            </select>
        </div>

        <button class="export-btn" onclick={exportSlides} disabled={exporting || !manifest}>
            {exporting ? 'Exporting...' : activeTarget?.mime === 'application/pdf' ? 'Export PDF' : 'Export PNGs'}
        </button>
    </div>

    {#if exporting && exportProgress.total > 0}
        <div class="export-progress" role="status" aria-live="polite">
            <span>{exportProgress.label}</span>
            <span>{exportProgress.current}/{exportProgress.total}</span>
            <div class="export-progress-track" role="progressbar" aria-valuemin="0" aria-valuemax={exportProgress.total} aria-valuenow={exportProgress.current}>
                <div class="export-progress-fill" style="width: {(exportProgress.current / exportProgress.total) * 100}%"></div>
            </div>
        </div>
    {/if}

    {#if selectedImages.length === 0}
        <div class="empty-state">
            <span class="empty-title">No images selected</span>
            <span class="empty-hint">Select images in the grid view, then switch to export</span>
        </div>
    {:else if manifest && activeTarget}
        <div class="preview-grid">
            {#each manifest.slides as slide, i}
                <div class="preview-card">
                    <div class="preview-label">Slide {i + 1}</div>
                    <div
                        class="preview-container"
                        style:width="{activeTarget.width * previewScale}px"
                        style:height="{activeTarget.height * previewScale}px"
                    >
                        <div
                            class="render-target"
                            style:width="{activeTarget.width}px"
                            style:height="{activeTarget.height}px"
                            style:transform="scale({previewScale})"
                            style:transform-origin="top left"
                            bind:this={renderRefs[slide.id]}
                        >
                            {#if template === 'bleed'}
                                <ExportSlideBleed {slide} defaults={manifest.defaults} target={activeTarget} imageSrc={imageSrcs[slide.image.asset_id] ?? ''} />
                            {:else if template === 'editorial'}
                                <ExportSlideEditorial {slide} defaults={manifest.defaults} target={activeTarget} imageSrc={imageSrcs[slide.image.asset_id] ?? ''} />
                            {:else}
                                <ExportSlideTerminal {slide} defaults={manifest.defaults} target={activeTarget} imageSrc={imageSrcs[slide.image.asset_id] ?? ''} />
                            {/if}
                        </div>
                    </div>
                </div>
            {/each}
        </div>
    {:else}
        <div class="empty-state">
            <span class="empty-title">Loading...</span>
        </div>
    {/if}
</div>

<style>
    .export-view {
        grid-area: main;
        display: flex;
        flex-direction: column;
        overflow: hidden;
        background: var(--bg);
    }

    .export-toolbar {
        display: flex;
        align-items: center;
        gap: 12px;
        padding: 12px 16px;
        border-bottom: 1px solid var(--border);
        flex-shrink: 0;
    }

    .template-picker {
        display: flex;
        gap: 2px;
        background: var(--surface);
        border-radius: var(--radius);
        padding: 2px;
    }

    .template-picker button {
        background: none;
        border: none;
        color: var(--text-secondary);
        font-family: var(--font);
        font-size: 12px;
        padding: 6px 12px;
        border-radius: 3px;
        cursor: pointer;
    }

    .template-picker button.active {
        background: var(--border);
        color: var(--text);
    }

    .preset-picker select {
        background: var(--surface);
        border: 1px solid var(--border);
        color: var(--text);
        font-family: var(--font);
        font-size: 12px;
        padding: 6px 8px;
        border-radius: var(--radius);
    }

    .export-btn {
        margin-left: auto;
        background: var(--green);
        color: #08080c;
        border: none;
        font-family: var(--font);
        font-size: 12px;
        font-weight: 700;
        padding: 8px 20px;
        border-radius: var(--radius);
        cursor: pointer;
    }

    .export-btn:disabled {
        opacity: 0.5;
        cursor: not-allowed;
    }

    .export-progress {
        display: grid;
        grid-template-columns: 1fr auto;
        gap: 6px 12px;
        padding: 8px 16px;
        border-bottom: 1px solid var(--border);
        color: var(--text-secondary);
        font-size: 11px;
    }

    .export-progress-track {
        grid-column: 1 / -1;
        height: 4px;
        background: var(--border);
        border-radius: var(--radius);
        overflow: hidden;
    }

    .export-progress-fill {
        height: 100%;
        background: var(--green);
        transition: width 0.2s ease;
    }

    .preview-grid {
        flex: 1;
        overflow-y: auto;
        padding: 20px;
        display: flex;
        flex-wrap: wrap;
        gap: 20px;
        align-content: flex-start;
    }

    .preview-card {
        display: flex;
        flex-direction: column;
        gap: 6px;
    }

    .preview-label {
        font-size: 11px;
        color: var(--text-secondary);
        text-transform: uppercase;
        letter-spacing: 0.05em;
    }

    .preview-container {
        overflow: hidden;
        border-radius: var(--radius);
        border: 1px solid var(--border);
        background: var(--surface);
    }

    .render-target {
        position: relative;
    }

    .empty-state {
        grid-area: main;
        display: flex;
        flex-direction: column;
        align-items: center;
        justify-content: center;
        gap: 8px;
        flex: 1;
    }

    .empty-title {
        color: var(--text-secondary);
        font-size: 14px;
        font-weight: 700;
        text-transform: uppercase;
    }

    .empty-hint {
        color: var(--text-secondary);
        font-size: 12px;
        opacity: 0.5;
    }
</style>
