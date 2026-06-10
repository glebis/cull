<script lang="ts">
    import { exportImageOnly, images, selectedIds, showToast, zenMode } from '$lib/stores';
    import { createExportManifest, getExportAsset, listExportPresets } from '$lib/export-api';
    import type { ExportManifest, ExportTarget, PresetInfo } from '$lib/export-types';
    import {
        EXPORT_INTENTS,
        EXPORT_LAYOUT_DENSITIES,
        EXPORT_TARGET_OPTIONS,
        PDF_TEMPLATE_OPTIONS,
        buildExportMasterSummary,
        describePdfTextAmount,
        getExportIntent,
        getExportLayoutDensity,
        getExportTargetOption,
        getPdfTemplateOption,
        hasPdfOutput,
        type ExportIntentId,
        type ExportLayoutDensityId,
        type ExportOutputKey,
        type ExportSectionId,
        type ExportTargetId,
        type PdfLayout,
        type PdfTemplateId,
        type PdfTextAmount,
        type SlideTemplate,
    } from '$lib/export-master';
    import { convertFileSrc, invoke } from '@tauri-apps/api/core';
    import { toPng } from 'html-to-image';
    import { buildHtmlToImageOptions, formatExportError } from '$lib/export-renderer';
    import ExportSlideBleed from './ExportSlideBleed.svelte';
    import ExportSlideEditorial from './ExportSlideEditorial.svelte';
    import ExportSlideTerminal from './ExportSlideTerminal.svelte';

    type TextField = 'headline' | 'body' | 'caption';
    type PerImageText = Record<string, { headline: string; body: string; caption: string }>;

    const OUTPUT_OPTIONS: { id: ExportOutputKey; label: string; note: string }[] = [
        { id: 'originals', label: 'Originals', note: 'Keep untouched source files in the set.' },
        { id: 'web', label: 'Web', note: 'Website-ready files with predictable dimensions.' },
        { id: 'social', label: 'Social', note: 'Frames for Instagram, Facebook, LinkedIn, and similar channels.' },
        { id: 'pdf', label: 'PDF', note: 'Portfolio, lookbook, review, or contact-sheet document.' },
        { id: 'contact', label: 'Contact sheet', note: 'Dense review pages with identifiers and ratings.' },
        { id: 'csv', label: 'CSV', note: 'Metadata, filenames, ratings, captions, and decisions.' },
    ];

    let manifest: ExportManifest | null = $state(null);
    let presets: PresetInfo[] = $state([]);
    let imageSrcs: Record<string, string> = $state({});
    let template: SlideTemplate = $state('editorial');
    let densityId: ExportLayoutDensityId = $state('basic');
    let intentId: ExportIntentId = $state('portfolio-pdf');
    let selectedOutputs: ExportOutputKey[] = $state(['pdf', 'web', 'csv']);
    let targetId: ExportTargetId = $state('portfolio-pdf');
    let pdfTemplateId: PdfTemplateId = $state('artistic');
    let pdfTextAmount: PdfTextAmount = $state('extended');
    let pdfLayout: PdfLayout = $state('sequence spreads');
    let pdfHtmlTemplate = $state(getPdfTemplateOption('artistic').htmlScaffold);
    let openSections: ExportSectionId[] = $state([]);
    let perImageText: PerImageText = $state({});
    let exporting = $state(false);
    let exportProgress = $state({ current: 0, total: 0, label: '' });
    let renderRefs: Record<string, HTMLDivElement> = $state({});
    let imageOnly = $derived($zenMode && $exportImageOnly);

    let selectedImages = $derived(
        $selectedIds.size > 0
            ? $images.filter(img => $selectedIds.has(img.image.id))
            : $images.slice(0, 10)
    );
    let activeDensity = $derived(getExportLayoutDensity(densityId));
    let activeIntent = $derived(getExportIntent(intentId));
    let activeTargetOption = $derived(getExportTargetOption(targetId));
    let activePdfTemplate = $derived(getPdfTemplateOption(pdfTemplateId));
    let selectedPreset = $derived(activeTargetOption.presetId);
    let showPdfTemplate = $derived(hasPdfOutput(selectedOutputs));
    let masterSummary = $derived(buildExportMasterSummary({
        intentId,
        targetId,
        outputs: selectedOutputs,
        pdfTemplateId,
        pdfTextAmount,
        pdfLayout,
    }));

    function getActiveTarget(): ExportTarget | undefined {
        return manifest !== null ? manifest.targets[0] : undefined;
    }
    let activeTarget = $derived(getActiveTarget());

    let imageOnlySrcByImageId = $derived.by(() => {
        const srcs: Record<string, string> = {};
        if (!manifest) return srcs;

        manifest.source.image_ids.forEach((imageId, index) => {
            const slide = manifest?.slides[index];
            if (!slide) return;
            srcs[imageId] = imageSrcs[slide.image.asset_id] ?? '';
        });
        return srcs;
    });

    let previewScale = $derived(
        activeTarget ? Math.min(260 / activeTarget.width, 340 / activeTarget.height, 1) : 0.25
    );

    function sectionHidden(section: ExportSectionId) {
        if (section === 'pdf-template' && showPdfTemplate) return false;
        return activeDensity.hiddenSections.includes(section);
    }

    function sectionOpen(section: ExportSectionId) {
        return openSections.includes(section);
    }

    function toggleSection(section: ExportSectionId) {
        if (openSections.includes(section)) {
            openSections = openSections.filter(item => item !== section);
        } else {
            openSections = [...openSections, section];
        }
    }

    function ensureSectionOpen(section: ExportSectionId) {
        if (!openSections.includes(section)) {
            openSections = [...openSections, section];
        }
    }

    function applyDensity(nextId: ExportLayoutDensityId) {
        const next = getExportLayoutDensity(nextId);
        densityId = next.id;
        openSections = [...next.openSections];
    }

    function applyIntent(nextId: ExportIntentId) {
        const next = getExportIntent(nextId);
        intentId = next.id;
        selectedOutputs = [...next.outputs];
        targetId = next.targetId;
        template = next.slideTemplate;

        if (next.pdfTemplateId) {
            applyPdfTemplate(next.pdfTemplateId);
        }

        if (hasPdfOutput(next.outputs) && densityId !== 'basic') ensureSectionOpen('pdf-template');
    }

    function toggleOutput(output: ExportOutputKey) {
        const next = selectedOutputs.includes(output)
            ? selectedOutputs.filter(item => item !== output)
            : [...selectedOutputs, output];
        selectedOutputs = next.length > 0 ? next : ['web'];

        if (output === 'pdf' && selectedOutputs.includes('pdf')) {
            targetId = 'portfolio-pdf';
        } else if (output === 'pdf' && targetId === 'portfolio-pdf') {
            targetId = 'instagram-feed';
        }
    }

    function applyTarget(nextId: ExportTargetId) {
        targetId = nextId;
        if (nextId === 'portfolio-pdf' && !selectedOutputs.includes('pdf')) {
            selectedOutputs = [...selectedOutputs, 'pdf'];
        }
        if (nextId === 'portfolio-pdf' && densityId !== 'basic') ensureSectionOpen('pdf-template');
    }

    function applyPdfTemplate(nextId: PdfTemplateId) {
        const next = getPdfTemplateOption(nextId);
        pdfTemplateId = next.id;
        pdfTextAmount = next.defaultTextAmount;
        pdfLayout = next.defaultLayout;
        pdfHtmlTemplate = next.htmlScaffold;
        template = next.slideTemplate;
    }

    function imageLabel(path: string) {
        return path.split('/').pop() ?? 'image';
    }

    function getSlideText(imageId: string, index: number) {
        return perImageText[imageId] ?? manifest?.slides[index]?.text ?? { headline: '', body: '', caption: '' };
    }

    function applyPerImageText(nextManifest: ExportManifest): ExportManifest {
        const slides = nextManifest.slides.map((slide, index) => {
            const imageId = nextManifest.source.image_ids[index];
            const override = perImageText[imageId];
            if (!override) return slide;

            return {
                ...slide,
                text: {
                    ...slide.text,
                    headline: override.headline || slide.text.headline,
                    body: override.body || slide.text.body,
                    caption: override.caption || slide.text.caption,
                },
            };
        });

        return { ...nextManifest, slides };
    }

    function updateImageText(imageId: string, field: TextField, value: string) {
        const current = perImageText[imageId] ?? { headline: '', body: '', caption: '' };
        perImageText = {
            ...perImageText,
            [imageId]: { ...current, [field]: value },
        };

        if (manifest) {
            manifest = applyPerImageText(manifest);
        }
    }

    async function loadPresets() {
        presets = await listExportPresets();
    }

    async function buildManifest() {
        if (selectedImages.length === 0) return;
        const imageIds = selectedImages.map(img => img.image.id);
        const nextManifest = await createExportManifest(imageIds, [selectedPreset], undefined, template);
        const nextSrcs: Record<string, string> = {};

        for (const asset of nextManifest.assets) {
            if (asset.kind === 'source') {
                const resp = await getExportAsset(asset.uri, 'preview');
                nextSrcs[asset.id] = convertFileSrc(resp.path);
            }
        }

        imageSrcs = nextSrcs;
        manifest = applyPerImageText(nextManifest);
    }

    async function exportSlides() {
        if (exporting || !manifest || !activeTarget) return;
        exporting = true;
        exportProgress = { current: 0, total: manifest.slides.length + (activeTarget.mime === 'application/pdf' ? 1 : 0), label: 'Preparing export' };

        try {
            const pngPaths: string[] = [];
            const pdfSlideIds: string[] = [];

            for (const [index, slide] of manifest.slides.entries()) {
                const el = renderRefs[slide.id];
                if (!el) continue;
                exportProgress = { ...exportProgress, current: index, label: `Rendering slide ${index + 1}` };

                const dataUrl = await toPng(
                    el,
                    buildHtmlToImageOptions(activeTarget.width, activeTarget.height)
                );

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
            showToast(`Export failed: ${formatExportError(e, exportProgress.label)}`, { type: 'error' });
        } finally {
            exporting = false;
            exportProgress = { current: 0, total: 0, label: '' };
        }
    }

    function handleExportLaunch() {
        void exportSlides();
    }

    $effect(() => {
        loadPresets();
    });

    $effect(() => {
        if (selectedImages.length > 0) {
            void buildManifest();
        }
    });

    $effect(() => {
        window.addEventListener('cull-export-launch', handleExportLaunch);
        return () => window.removeEventListener('cull-export-launch', handleExportLaunch);
    });
</script>

<div class="export-view" class:images-only={imageOnly}>
    {#if !imageOnly}
        <div class="export-master">
            <aside class="master-panel" aria-label="Export master">
                <div class="master-header">
                    <div>
                        <span class="eyebrow">Export Master</span>
                        <h2>What are you making?</h2>
                    </div>
                    <button class="export-btn" onclick={exportSlides} disabled={exporting || !manifest}>
                        {exporting ? 'Exporting...' : activeTarget?.mime === 'application/pdf' ? 'Export PDF' : 'Export set'}
                    </button>
                </div>

                <div class="density-row" aria-label="Layout density">
                    <span>Layout density</span>
                    <div class="density-tabs">
                        {#each EXPORT_LAYOUT_DENSITIES as density}
                            <button
                                class:active={densityId === density.id}
                                title={density.description}
                                onclick={() => applyDensity(density.id)}
                            >
                                {density.label}
                            </button>
                        {/each}
                    </div>
                </div>

                <div class="answer-grid">
                    {#each EXPORT_INTENTS as intent}
                        <button
                            class="answer-card"
                            class:active={intentId === intent.id}
                            onclick={() => applyIntent(intent.id)}
                        >
                            <span>{intent.label}</span>
                            <small>{intent.prompt}</small>
                        </button>
                    {/each}
                </div>

                <div class="block-stack">
                    {#if !sectionHidden('outputs')}
                        <section class="master-block" data-section="outputs">
                            <button class="block-header" onclick={() => toggleSection('outputs')} aria-expanded={sectionOpen('outputs')}>
                                <span>Outputs</span>
                                <strong>{selectedOutputs.join(' + ')}</strong>
                            </button>
                            {#if sectionOpen('outputs')}
                                <div class="block-body option-grid">
                                    {#each OUTPUT_OPTIONS as output}
                                        <label class="check-option">
                                            <input
                                                type="checkbox"
                                                checked={selectedOutputs.includes(output.id)}
                                                onchange={() => toggleOutput(output.id)}
                                            />
                                            <span>
                                                <strong>{output.label}</strong>
                                                <small>{output.note}</small>
                                            </span>
                                        </label>
                                    {/each}
                                </div>
                            {/if}
                        </section>
                    {/if}

                    {#if !sectionHidden('targets')}
                        <section class="master-block" data-section="targets">
                            <button class="block-header" onclick={() => toggleSection('targets')} aria-expanded={sectionOpen('targets')}>
                                <span>Target formats</span>
                                <strong>{activeTargetOption.label}</strong>
                            </button>
                            {#if sectionOpen('targets')}
                                <div class="block-body target-grid">
                                    {#each EXPORT_TARGET_OPTIONS as option}
                                        <button
                                            class="target-option"
                                            class:active={targetId === option.id}
                                            onclick={() => applyTarget(option.id)}
                                        >
                                            <span>{option.label}</span>
                                            <strong>{option.width}×{option.height}</strong>
                                            <small>{option.note}</small>
                                        </button>
                                    {/each}
                                </div>
                            {/if}
                        </section>
                    {/if}

                    {#if !sectionHidden('pdf-template')}
                        <section class="master-block" data-section="pdf-template">
                            <button class="block-header" onclick={() => toggleSection('pdf-template')} aria-expanded={sectionOpen('pdf-template')}>
                                <span>PDF HTML template</span>
                                <strong>{activePdfTemplate.label}</strong>
                            </button>
                            {#if sectionOpen('pdf-template')}
                                <div class="block-body pdf-template-grid">
                                    <div class="template-options">
                                        {#each PDF_TEMPLATE_OPTIONS as pdfOption}
                                            <button
                                                class="template-option"
                                                class:active={pdfTemplateId === pdfOption.id}
                                                onclick={() => applyPdfTemplate(pdfOption.id)}
                                            >
                                                <span>{pdfOption.label}</span>
                                                <small>{pdfOption.description}</small>
                                            </button>
                                        {/each}
                                    </div>
                                    <div class="template-editor">
                                        <div class="compact-fields">
                                            <label>
                                                Text amount
                                                <select
                                                    value={pdfTextAmount}
                                                    onchange={(e) => pdfTextAmount = e.currentTarget.value as PdfTextAmount}
                                                >
                                                    <option value="minimal">Minimal captions</option>
                                                    <option value="standard">Standard notes</option>
                                                    <option value="extended">Extended statements</option>
                                                </select>
                                            </label>
                                            <label>
                                                Page rhythm
                                                <select
                                                    value={pdfLayout}
                                                    onchange={(e) => pdfLayout = e.currentTarget.value as PdfLayout}
                                                >
                                                    <option value="one image per page">One image per page</option>
                                                    <option value="gallery grid">Gallery grid</option>
                                                    <option value="sequence spreads">Sequence spreads</option>
                                                    <option value="cover + chapters">Cover + chapters</option>
                                                </select>
                                            </label>
                                        </div>
                                        <label>
                                            Editable HTML scaffold
                                            <textarea bind:value={pdfHtmlTemplate} spellcheck="false"></textarea>
                                        </label>
                                    </div>
                                </div>
                            {/if}
                        </section>
                    {/if}

                    {#if !sectionHidden('text')}
                        <section class="master-block" data-section="text">
                            <button class="block-header" onclick={() => toggleSection('text')} aria-expanded={sectionOpen('text')}>
                                <span>Per-image text</span>
                                <strong>{selectedImages.length} images</strong>
                            </button>
                            {#if sectionOpen('text')}
                                <div class="block-body image-text-list">
                                    {#each selectedImages as selectedImage, i}
                                        {@const imageText = getSlideText(selectedImage.image.id, i)}
                                        <div class="image-text-row">
                                            <span class="image-index">{i + 1}</span>
                                            <div class="image-text-fields">
                                                <label>
                                                    <span>{imageLabel(selectedImage.path)}</span>
                                                    <input
                                                        value={imageText.headline}
                                                        placeholder="Headline"
                                                        oninput={(e) => updateImageText(selectedImage.image.id, 'headline', e.currentTarget.value)}
                                                    />
                                                </label>
                                                <input
                                                    value={imageText.caption}
                                                    placeholder="Caption"
                                                    oninput={(e) => updateImageText(selectedImage.image.id, 'caption', e.currentTarget.value)}
                                                />
                                                <textarea
                                                    value={imageText.body}
                                                    placeholder="Notes or statement"
                                                    oninput={(e) => updateImageText(selectedImage.image.id, 'body', e.currentTarget.value)}
                                                ></textarea>
                                            </div>
                                        </div>
                                    {/each}
                                </div>
                            {/if}
                        </section>
                    {/if}

                    {#if !sectionHidden('metadata')}
                        <section class="master-block" data-section="metadata">
                            <button class="block-header" onclick={() => toggleSection('metadata')} aria-expanded={sectionOpen('metadata')}>
                                <span>Metadata</span>
                                <strong>CSV + captions</strong>
                            </button>
                            {#if sectionOpen('metadata')}
                                <div class="block-body metadata-grid">
                                    <label class="check-option">
                                        <input type="checkbox" checked />
                                        <span>
                                            <strong>Include filenames and ratings</strong>
                                            <small>Use Cull metadata in the manifest and contact sheets.</small>
                                        </span>
                                    </label>
                                    <label class="check-option">
                                        <input type="checkbox" checked />
                                        <span>
                                            <strong>Include alt text field</strong>
                                            <small>Carry generated or edited alt text into the export manifest.</small>
                                        </span>
                                    </label>
                                </div>
                            {/if}
                        </section>
                    {/if}

                    {#if !sectionHidden('advanced')}
                        <section class="master-block" data-section="advanced">
                            <button class="block-header" onclick={() => toggleSection('advanced')} aria-expanded={sectionOpen('advanced')}>
                                <span>Advanced</span>
                                <strong>{selectedPreset}</strong>
                            </button>
                            {#if sectionOpen('advanced')}
                                <div class="block-body advanced-grid">
                                    <label>
                                        Slide renderer
                                        <select value={template} onchange={(e) => template = e.currentTarget.value as SlideTemplate}>
                                            <option value="bleed">Bleed</option>
                                            <option value="editorial">Editorial</option>
                                            <option value="terminal">Terminal</option>
                                        </select>
                                    </label>
                                    <label>
                                        Backend preset
                                        <select value={targetId} onchange={(e) => applyTarget(e.currentTarget.value as ExportTargetId)}>
                                            {#each EXPORT_TARGET_OPTIONS as option}
                                                <option value={option.id}>{option.presetId}</option>
                                            {/each}
                                        </select>
                                    </label>
                                    <div class="preset-count">{presets.length} backend presets available</div>
                                    {#if activeDensity.showPromptDrawer}
                                        <textarea class="prompt-drawer" readonly value={pdfHtmlTemplate}></textarea>
                                    {/if}
                                </div>
                            {/if}
                        </section>
                    {/if}
                </div>
            </aside>

            <main class="export-queue" aria-label="Export preview queue">
                <div class="queue-header">
                    <div>
                        <span class="eyebrow">Current preset</span>
                        <h3>{activeIntent.label}</h3>
                    </div>
                    <div class="queue-summary">{masterSummary}</div>
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
                                <div class="preview-label">Slide {i + 1} · {activeTarget.width}×{activeTarget.height} · {describePdfTextAmount(pdfTextAmount)}</div>
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
            </main>
        </div>
    {:else if selectedImages.length === 0}
        <div class="empty-state">
            <span class="empty-title">No images selected</span>
            <span class="empty-hint">Select images in the grid view, then switch to export</span>
        </div>
    {:else if manifest}
        <div class="image-only-grid">
            {#each selectedImages as selectedImage}
                {@const src = imageOnlySrcByImageId[selectedImage.image.id] ?? ''}
                {#if src}
                    <img src={src} alt="" draggable="false" />
                {/if}
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
        min-width: 0;
        min-height: 0;
        overflow: hidden;
        background: var(--bg);
        color: var(--text);
    }

    .export-master {
        flex: 1;
        min-height: 0;
        display: grid;
        grid-template-columns: minmax(380px, 460px) minmax(0, 1fr);
        border-top: 1px solid var(--border);
    }

    /* Image-only contract: .export-view.images-only .export-toolbar and .export-view.images-only .preview-label stay hidden. */
    .export-view.images-only .master-panel,
    .export-view.images-only .preview-label {
        display: none;
    }

    .master-panel {
        min-height: 0;
        overflow-y: auto;
        border-right: 1px solid var(--border);
        background: var(--surface);
    }

    .master-header,
    .queue-header {
        display: flex;
        align-items: flex-start;
        justify-content: space-between;
        gap: 16px;
        padding: 16px;
        border-bottom: 1px solid var(--border);
    }

    .master-header h2,
    .queue-header h3 {
        margin: 4px 0 0;
        color: var(--text);
        font-size: 16px;
        line-height: 1.25;
        letter-spacing: 0;
    }

    .eyebrow {
        color: var(--text-secondary);
        font-size: 10px;
        font-weight: 700;
        text-transform: uppercase;
    }

    .export-btn {
        flex: 0 0 auto;
        background: var(--green);
        color: var(--bg);
        border: none;
        font-family: var(--font);
        font-size: 12px;
        font-weight: 700;
        padding: 8px 14px;
        border-radius: var(--radius);
        cursor: pointer;
    }

    .export-btn:disabled {
        opacity: 0.45;
        cursor: not-allowed;
    }

    .density-row {
        display: grid;
        grid-template-columns: auto 1fr;
        align-items: center;
        gap: 12px;
        padding: 12px 16px;
        border-bottom: 1px solid var(--border);
        color: var(--text-secondary);
        font-size: 11px;
    }

    .density-tabs {
        display: grid;
        grid-template-columns: repeat(4, minmax(0, 1fr));
        gap: 2px;
        padding: 2px;
        border: 1px solid var(--border);
        border-radius: var(--radius);
        background: var(--bg);
    }

    .density-tabs button,
    .answer-card,
    .target-option,
    .template-option,
    .block-header {
        font-family: var(--font);
        cursor: pointer;
    }

    .density-tabs button {
        min-width: 0;
        border: none;
        border-radius: 3px;
        background: transparent;
        color: var(--text-secondary);
        padding: 6px 4px;
        font-size: 11px;
    }

    .density-tabs button.active {
        background: var(--border);
        color: var(--text);
    }

    .answer-grid {
        display: grid;
        grid-template-columns: repeat(2, minmax(0, 1fr));
        gap: 8px;
        padding: 16px;
        border-bottom: 1px solid var(--border);
    }

    .answer-card {
        min-height: 96px;
        display: flex;
        flex-direction: column;
        gap: 8px;
        text-align: left;
        padding: 12px;
        border: 1px solid var(--border);
        border-radius: var(--radius);
        background: var(--bg);
        color: var(--text);
    }

    .answer-card.active,
    .target-option.active,
    .template-option.active {
        border-color: var(--blue);
        box-shadow: inset 0 0 0 1px var(--blue);
    }

    .answer-card span,
    .target-option span,
    .template-option span {
        font-size: 12px;
        font-weight: 700;
        line-height: 1.3;
    }

    small {
        color: var(--text-secondary);
        font-size: 10px;
        line-height: 1.45;
    }

    .block-stack {
        display: flex;
        flex-direction: column;
    }

    .master-block {
        border-bottom: 1px solid var(--border);
    }

    .block-header {
        width: 100%;
        display: grid;
        grid-template-columns: auto minmax(0, 1fr);
        gap: 12px;
        align-items: center;
        text-align: left;
        border: none;
        background: transparent;
        color: var(--text);
        padding: 12px 16px;
    }

    .block-header span {
        font-size: 12px;
        font-weight: 700;
    }

    .block-header strong {
        min-width: 0;
        overflow: hidden;
        text-overflow: ellipsis;
        white-space: nowrap;
        color: var(--text-secondary);
        font-size: 10px;
        font-weight: 400;
        text-align: right;
    }

    .block-body {
        padding: 0 16px 16px;
    }

    .option-grid,
    .metadata-grid {
        display: grid;
        gap: 8px;
    }

    .check-option {
        display: grid;
        grid-template-columns: 18px minmax(0, 1fr);
        gap: 10px;
        padding: 10px;
        border: 1px solid var(--border);
        border-radius: var(--radius);
        background: var(--bg);
        color: var(--text);
        font-size: 12px;
    }

    .check-option span {
        display: grid;
        gap: 3px;
    }

    input,
    select,
    textarea {
        width: 100%;
        min-width: 0;
        box-sizing: border-box;
        border: 1px solid var(--border);
        border-radius: var(--radius);
        background: var(--bg);
        color: var(--text);
        font-family: var(--font);
        font-size: 11px;
    }

    input,
    select {
        min-height: 30px;
        padding: 6px 8px;
    }

    input[type='checkbox'] {
        width: 14px;
        min-height: 14px;
        margin-top: 2px;
        accent-color: var(--blue);
    }

    textarea {
        min-height: 84px;
        resize: vertical;
        padding: 8px;
        line-height: 1.45;
    }

    .target-grid {
        display: grid;
        grid-template-columns: repeat(2, minmax(0, 1fr));
        gap: 8px;
    }

    .target-option,
    .template-option {
        display: grid;
        gap: 6px;
        text-align: left;
        padding: 10px;
        border: 1px solid var(--border);
        border-radius: var(--radius);
        background: var(--bg);
        color: var(--text);
    }

    .target-option strong {
        color: var(--orange);
        font-size: 10px;
    }

    .pdf-template-grid {
        display: grid;
        grid-template-columns: minmax(0, 0.9fr) minmax(0, 1.1fr);
        gap: 10px;
    }

    .template-options,
    .template-editor,
    .image-text-fields,
    .advanced-grid {
        display: grid;
        gap: 8px;
    }

    .compact-fields {
        display: grid;
        grid-template-columns: repeat(2, minmax(0, 1fr));
        gap: 8px;
    }

    label {
        display: grid;
        gap: 5px;
        color: var(--text-secondary);
        font-size: 10px;
    }

    .image-text-list {
        display: grid;
        gap: 10px;
        max-height: 360px;
        overflow-y: auto;
        padding-right: 4px;
    }

    .image-text-row {
        display: grid;
        grid-template-columns: 24px minmax(0, 1fr);
        gap: 10px;
    }

    .image-index {
        display: grid;
        place-items: center;
        width: 24px;
        height: 24px;
        border: 1px solid var(--border);
        border-radius: var(--radius);
        color: var(--purple);
        font-size: 10px;
        font-weight: 700;
    }

    .preset-count {
        color: var(--text-secondary);
        font-size: 10px;
    }

    .prompt-drawer {
        min-height: 140px;
        color: var(--text-secondary);
    }

    .export-queue {
        min-width: 0;
        min-height: 0;
        display: flex;
        flex-direction: column;
        overflow: hidden;
        background: var(--bg);
    }

    .queue-summary {
        max-width: 560px;
        color: var(--text-secondary);
        font-size: 11px;
        line-height: 1.5;
        text-align: right;
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
        overflow: hidden;
        border-radius: var(--radius);
        background: var(--border);
    }

    .export-progress-fill {
        height: 100%;
        background: var(--green);
        transition: width 0.2s ease;
    }

    .preview-grid {
        flex: 1;
        min-height: 0;
        overflow-y: auto;
        padding: 18px;
        display: grid;
        grid-template-columns: repeat(auto-fill, minmax(220px, 1fr));
        gap: 18px;
        align-content: flex-start;
    }

    .preview-card {
        min-width: 0;
        display: flex;
        flex-direction: column;
        gap: 6px;
    }

    .preview-label {
        overflow: hidden;
        text-overflow: ellipsis;
        white-space: nowrap;
        color: var(--text-secondary);
        font-size: 10px;
        text-transform: uppercase;
    }

    .preview-container {
        overflow: hidden;
        border: 1px solid var(--border);
        border-radius: var(--radius);
        background: var(--surface);
    }

    .render-target {
        position: relative;
    }

    .image-only-grid {
        flex: 1;
        min-height: 0;
        display: grid;
        grid-template-columns: repeat(auto-fit, minmax(min(100%, 360px), 1fr));
        grid-auto-rows: minmax(0, 1fr);
        gap: 2px;
        padding: 0;
        overflow: hidden;
        background: var(--bg);
    }

    .image-only-grid img {
        width: 100%;
        height: 100%;
        min-width: 0;
        min-height: 0;
        object-fit: contain;
        display: block;
        background: var(--bg);
    }

    .empty-state {
        grid-area: main;
        display: flex;
        flex: 1;
        flex-direction: column;
        align-items: center;
        justify-content: center;
        gap: 8px;
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
        opacity: 0.55;
    }

    @media (max-width: 1100px) {
        .export-master {
            grid-template-columns: 1fr;
            grid-template-rows: minmax(420px, 52vh) minmax(0, 1fr);
        }

        .master-panel {
            border-right: none;
            border-bottom: 1px solid var(--border);
        }

        .answer-grid {
            grid-template-columns: repeat(3, minmax(0, 1fr));
        }

        .answer-card {
            min-height: 88px;
        }
    }
</style>
