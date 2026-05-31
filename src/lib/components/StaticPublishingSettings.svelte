<script lang="ts">
    import { convertFileSrc } from '@tauri-apps/api/core';
    import { openPath, openUrl } from '@tauri-apps/plugin-opener';
    import { getAppSetting, setAppSetting, exportStaticPublishPackage, serveStaticPublishPackage, stopStaticPublishServer } from '$lib/api';
    import type { StaticPublishResult, StaticPublishServerResult } from '$lib/api';
    import { activeCanvas, activeSession, images, selectedIds, showToast } from '$lib/stores';
    import {
        buildStaticPublishRequestFromSavedCanvas,
        buildStaticPublishShareItems,
        countSavedCanvasItems,
        formatStaticPublishLinks,
        parseStaticPublishLinks,
    } from '$lib/static-publishing';
    import type { StaticPublishShareItem } from '$lib/static-publishing';
    import { onMount } from 'svelte';

    type PublishScenario = 'local_preview' | 'client_review' | 'agent_handoff' | 'static_host';

    let loading = $state(true);
    let exporting = $state(false);
    let siteTitle = $state('Current Canvas');
    let siteDescription = $state('');
    let outputDir = $state('');
    let shareUrl = $state('');
    let linksText = $state('');
    let indexable = $state(false);
    let includeThumbnails = $state(true);
    let includeWeb = $state(true);
    let includeFull = $state(false);
    let serverHost = $state('127.0.0.1');
    let serverPort = $state('8000');
    let scenario = $state<PublishScenario>('local_preview');
    let lastResult = $state<StaticPublishResult | null>(null);
    let serverResult = $state<StaticPublishServerResult | null>(null);
    let startingServer = $state(false);
    let stoppingServer = $state(false);
    let parsedLinks = $derived(parseStaticPublishLinks(linksText));
    let publishItems = $derived(lastResult ? buildStaticPublishShareItems(lastResult, serverResult) : []);
    let qrImageSrc = $derived(lastResult ? convertFileSrc(lastResult.qr_svg_path) : '');
    const scenarioLabels: Record<PublishScenario, string> = {
        local_preview: 'Local preview',
        client_review: 'Client review',
        agent_handoff: 'Agent handoff',
        static_host: 'Static host',
    };

    const sourceImages = $derived(
        $selectedIds.size > 0
            ? $images.filter(img => $selectedIds.has(img.image.id))
            : $images
    );
    const savedCanvasItemCount = $derived(countSavedCanvasItems($activeCanvas));
    const exportSourceCount = $derived($activeCanvas ? savedCanvasItemCount : sourceImages.length);
    const exportSourceLabel = $derived($activeCanvas ? $activeCanvas.name : 'Current view');
    const sourceSummary = $derived(`${exportSourceCount} image${exportSourceCount === 1 ? '' : 's'}`);
    const scenarioLabel = $derived(scenarioLabels[scenario]);
    const enabledVariantCount = $derived([includeThumbnails, includeWeb, includeFull].filter(Boolean).length);
    const hasAssetVariant = $derived(enabledVariantCount > 0);
    const variantSummary = $derived(`${enabledVariantCount} asset set${enabledVariantCount === 1 ? '' : 's'}`);
    const canBuild = $derived(exportSourceCount > 0 && hasAssetVariant);
    const buildStatus = $derived(
        exportSourceCount === 0
            ? 'No images available'
            : hasAssetVariant
                ? 'Ready'
                : 'Select an asset set'
    );
    const searchVisibilityLabel = $derived(indexable ? 'Allow indexing' : 'Keep unlisted');

    function settingIsTrue(value: string | null, fallback: boolean): boolean {
        if (value === null) return fallback;
        return value === 'true';
    }

    function normalizeScenario(value: string | null): PublishScenario {
        return value === 'client_review' || value === 'agent_handoff' || value === 'static_host'
            ? value
            : 'local_preview';
    }

    onMount(async () => {
        const [
            savedOutputDir,
            savedThumb,
            savedWeb,
            savedFull,
            savedTitle,
            savedDescription,
            savedLinks,
            savedIndexable,
            savedServerHost,
            savedServerPort,
            savedScenario,
            savedShareUrl,
        ] = await Promise.all([
            getAppSetting('static_publishing_output_dir'),
            getAppSetting('static_publishing_include_thumb'),
            getAppSetting('static_publishing_include_web'),
            getAppSetting('static_publishing_include_full'),
            getAppSetting('static_publishing_site_title'),
            getAppSetting('static_publishing_site_description'),
            getAppSetting('static_publishing_links'),
            getAppSetting('static_publishing_indexable'),
            getAppSetting('static_publishing_server_host'),
            getAppSetting('static_publishing_server_port'),
            getAppSetting('static_publishing_scenario'),
            getAppSetting('static_publishing_share_url'),
        ]);

        outputDir = savedOutputDir ?? '';
        includeThumbnails = settingIsTrue(savedThumb, true);
        includeWeb = settingIsTrue(savedWeb, true);
        includeFull = settingIsTrue(savedFull, false);
        siteTitle = savedTitle ?? ($activeCanvas?.name || ($activeSession?.name ? `${$activeSession.name} Canvas` : 'Current Canvas'));
        siteDescription = savedDescription ?? '';
        linksText = savedLinks ?? '';
        indexable = settingIsTrue(savedIndexable, false);
        serverHost = savedServerHost ?? '127.0.0.1';
        serverPort = savedServerPort ?? '8000';
        scenario = normalizeScenario(savedScenario);
        shareUrl = savedShareUrl ?? '';
        loading = false;
    });

    async function saveSetting(key: string, value: string) {
        await setAppSetting(key, value);
    }

    async function saveBoolean(key: string, value: boolean) {
        await setAppSetting(key, value ? 'true' : 'false');
    }

    async function saveLinks() {
        linksText = formatStaticPublishLinks(parsedLinks);
        await saveSetting('static_publishing_links', linksText);
    }

    async function exportPackage() {
        if (!canBuild) return;
        exporting = true;
        if (serverResult) await stopServer(false);
        lastResult = null;
        try {
            const result = await exportStaticPublishPackage(
                $activeCanvas
                    ? buildStaticPublishRequestFromSavedCanvas({
                        canvas: $activeCanvas,
                        canvasName: siteTitle,
                        outputDir,
                        shareUrl,
                        siteTitle,
                        siteDescription,
                        indexable,
                        links: parsedLinks,
                        includeThumbnails,
                        includeWeb,
                        includeFull,
                    })
                    : {
                        canvas_name: siteTitle.trim() || 'Current Canvas',
                        items: sourceImages.map(img => ({ image_id: img.image.id })),
                        layout_json: JSON.stringify({
                            type: 'current_view_snapshot',
                            image_ids: sourceImages.map(img => img.image.id),
                        }),
                        output_dir: outputDir.trim() || null,
                        share_url: shareUrl.trim() || null,
                        site_title: siteTitle.trim() || null,
                        site_description: siteDescription.trim() || null,
                        indexable,
                        links: parsedLinks,
                        include_thumbnails: includeThumbnails,
                        include_web: includeWeb,
                        include_full: includeFull,
                    }
            );
            lastResult = result;
            serverResult = null;
            showToast('Package built', {
                detail: `${result.image_count} images`,
                type: 'success',
            });
        } catch (e) {
            showToast(`Package build failed: ${e}`, { type: 'error' });
        } finally {
            exporting = false;
        }
    }

    async function copyHandoffPath() {
        if (!lastResult) return;
        await navigator.clipboard.writeText(lastResult.instructions_path);
        showToast('Agent notes path copied', { type: 'success', duration: 2500 });
    }

    async function copyPublishItem(item: StaticPublishShareItem) {
        await navigator.clipboard.writeText(item.value);
        showToast(`${item.label} copied`, { type: 'success', duration: 2500 });
    }

    async function sharePublishItem(item: StaticPublishShareItem) {
        if (navigator.share) {
            try {
                await navigator.share({
                    title: item.label,
                    text: item.value,
                    url: item.kind === 'url' ? item.value : undefined,
                });
                return;
            } catch (e) {
                if (e instanceof DOMException && e.name === 'AbortError') return;
            }
        }
        await copyPublishItem(item);
    }

    async function openPublishItem(item: StaticPublishShareItem) {
        if (!item.openable) return;
        try {
            if (item.kind === 'url') await openUrl(item.value);
            else await openPath(item.value);
        } catch (e) {
            showToast(`Open failed: ${e}`, { type: 'error', duration: 5000 });
        }
    }

    async function startServer() {
        if (!lastResult) return;
        const parsedPort = Number.parseInt(serverPort, 10);
        startingServer = true;
        try {
            const result = await serveStaticPublishPackage(
                lastResult.site_dir,
                serverHost.trim() || '127.0.0.1',
                Number.isFinite(parsedPort) ? parsedPort : 8000,
            );
            serverResult = result;
            showToast('Preview started', {
                detail: result.url,
                type: 'success',
                duration: 5000,
            });
        } catch (e) {
            showToast(`Preview failed: ${e}`, { type: 'error' });
        } finally {
            startingServer = false;
        }
    }

    async function stopServer(showNotification = true) {
        stoppingServer = true;
        try {
            const result = await stopStaticPublishServer();
            serverResult = null;
            if (showNotification) {
                showToast(result.stopped ? 'Preview stopped' : 'Preview already stopped', {
                    detail: result.url ?? undefined,
                    type: 'success',
                    duration: 3500,
                });
            }
        } catch (e) {
            showToast(`Stop preview failed: ${e}`, { type: 'error' });
        } finally {
            stoppingServer = false;
        }
    }

    async function toggleServer() {
        if (serverResult) await stopServer();
        else await startServer();
    }
</script>

{#if loading}
    <div class="loading" role="status">Loading publish settings...</div>
{:else}
    <section class="publish-shell" aria-labelledby="publish-title" aria-busy={exporting}>
        <header class="publish-header">
            <div class="title-block">
                <span class="eyebrow">Publish</span>
                <h1 id="publish-title">Publish site</h1>
                <p>{scenarioLabel} · {sourceSummary} · {variantSummary}</p>
            </div>
            <div class="status-strip" aria-live="polite">
                <span>{buildStatus}</span>
                <span>{exportSourceLabel}</span>
            </div>
        </header>

        <div class="publish-grid">
            <div class="section publish-panel">
                <div class="section-header">
                    <span>Source and package</span>
                    <span class="count">{sourceSummary}</span>
                </div>
                <div class="setting-row stacked">
                    <label for="static-scenario">Workflow</label>
                    <select
                        id="static-scenario"
                        class="wide-input"
                        bind:value={scenario}
                        onchange={() => saveSetting('static_publishing_scenario', scenario)}
                    >
                        <option value="local_preview">Local preview</option>
                        <option value="client_review">Client review</option>
                        <option value="agent_handoff">Agent handoff</option>
                        <option value="static_host">Static host</option>
                    </select>
                </div>
                <div class="setting-row stacked">
                    <label for="static-output-dir">Output folder</label>
                    <input
                        id="static-output-dir"
                        class="wide-input"
                        bind:value={outputDir}
                        placeholder="Default app data folder"
                        autocomplete="off"
                        onblur={() => saveSetting('static_publishing_output_dir', outputDir)}
                    />
                </div>
                <div class="source-box" role="status" aria-live="polite">
                    <span>Source</span>
                    <strong>{exportSourceLabel}</strong>
                    <span>{sourceSummary}</span>
                </div>
                <button class="primary-btn" onclick={exportPackage} disabled={exporting || !canBuild}>
                    {exporting ? 'Building package...' : 'Build package'}
                </button>
            </div>

            <div class="section publish-panel">
                <div class="section-header">
                    <span>Site details</span>
                    <span class="count">{parsedLinks.length} link{parsedLinks.length === 1 ? '' : 's'}</span>
                </div>
                <div class="setting-row stacked">
                    <label for="static-site-title">Site title</label>
                    <input
                        id="static-site-title"
                        class="wide-input"
                        bind:value={siteTitle}
                        autocomplete="off"
                        onblur={() => saveSetting('static_publishing_site_title', siteTitle)}
                    />
                </div>
                <div class="setting-row stacked">
                    <label for="static-site-description">Intro text</label>
                    <textarea
                        id="static-site-description"
                        class="wide-input text-area"
                        rows="3"
                        bind:value={siteDescription}
                        spellcheck="true"
                        onblur={() => saveSetting('static_publishing_site_description', siteDescription)}
                    ></textarea>
                </div>
                <div class="setting-row stacked">
                    <label for="static-share-url">Public/tunnel URL</label>
                    <input
                        id="static-share-url"
                        class="wide-input"
                        type="url"
                        bind:value={shareUrl}
                        placeholder="https://name.ngrok-free.app or https://machine.tailnet.ts.net"
                        autocomplete="off"
                        onblur={() => saveSetting('static_publishing_share_url', shareUrl)}
                    />
                    <span class="count">Use ngrok, Tailscale Funnel, Cloudflare Tunnel, or a static host URL for public handoff.</span>
                </div>
                <div class="setting-row stacked">
                    <label for="static-links">Related links</label>
                    <textarea
                        id="static-links"
                        class="wide-input text-area links-area"
                        rows="4"
                        bind:value={linksText}
                        placeholder="Project brief | https://example.com/brief"
                        aria-describedby="static-links-count"
                        onblur={saveLinks}
                    ></textarea>
                    <span id="static-links-count" class="count">{parsedLinks.length} link{parsedLinks.length === 1 ? '' : 's'} included</span>
                </div>
            </div>

            <div class="section publish-panel delivery-panel">
                <div class="section-header">
                    <span>Delivery</span>
                    <span class="count">{searchVisibilityLabel}</span>
                </div>
                <div class="setting-row">
                    <span>Search visibility</span>
                    <button
                        class="toggle"
                        class:on={indexable}
                        aria-pressed={indexable}
                        onclick={() => { indexable = !indexable; saveBoolean('static_publishing_indexable', indexable); }}
                    >
                        {searchVisibilityLabel}
                    </button>
                </div>
                <fieldset class="asset-fieldset">
                    <legend>Asset files</legend>
                    <label class="check-row">
                        <input type="checkbox" bind:checked={includeThumbnails} onchange={() => saveBoolean('static_publishing_include_thumb', includeThumbnails)} />
                        <span>Thumbnail</span>
                        <span class="count">420 px JPEG</span>
                    </label>
                    <label class="check-row">
                        <input type="checkbox" bind:checked={includeWeb} onchange={() => saveBoolean('static_publishing_include_web', includeWeb)} />
                        <span>Web image</span>
                        <span class="count">1800 px JPEG</span>
                    </label>
                    <label class="check-row">
                        <input type="checkbox" bind:checked={includeFull} onchange={() => saveBoolean('static_publishing_include_full', includeFull)} />
                        <span>Original file</span>
                        <span class="count">source or RAW preview</span>
                    </label>
                </fieldset>
                <div class="preview-group">
                    <div class="section-subheader">Local preview</div>
                    <div class="settings-grid">
                        <div class="setting-row stacked compact">
                            <label for="static-server-host">Host</label>
                            <input
                                id="static-server-host"
                                class="wide-input"
                                bind:value={serverHost}
                                placeholder="127.0.0.1"
                                autocomplete="off"
                                onblur={() => saveSetting('static_publishing_server_host', serverHost)}
                            />
                        </div>
                        <div class="setting-row stacked compact">
                            <label for="static-server-port">Port</label>
                            <input
                                id="static-server-port"
                                class="wide-input"
                                bind:value={serverPort}
                                placeholder="8000"
                                inputmode="numeric"
                                autocomplete="off"
                                onblur={() => saveSetting('static_publishing_server_port', serverPort)}
                            />
                        </div>
                    </div>
                </div>
            </div>
        </div>

        {#if lastResult}
            <div class="section result-section" aria-live="polite">
                <div class="section-header">
                    <span>Latest package</span>
                    <div class="result-actions">
                        <button class="secondary-btn" onclick={toggleServer} disabled={startingServer || stoppingServer}>
                            {startingServer ? 'Starting preview...' : stoppingServer ? 'Stopping preview...' : serverResult ? 'Stop preview' : 'Start preview'}
                        </button>
                        <button class="secondary-btn" onclick={copyHandoffPath}>Copy agent notes</button>
                    </div>
                </div>
                <div class="result-body">
                    <div class="qr-card">
                        <img src={qrImageSrc} alt="QR code for target URL" />
                        <span>QR code</span>
                        <code>{lastResult.qr_target_url}</code>
                    </div>
                    <div class="result-grid">
                        {#each publishItems as item}
                            <div class="path-row">
                                <span>{item.label}</span>
                                {#if item.kind === 'url'}
                                    <a href={item.value} onclick={(event) => { event.preventDefault(); openPublishItem(item); }}>{item.value}</a>
                                {:else if item.openable}
                                    <button class="value-link" onclick={() => openPublishItem(item)}>{item.value}</button>
                                {:else}
                                    <code>{item.value}</code>
                                {/if}
                                <div class="item-actions">
                                    {#if item.openable}
                                        <button class="mini-btn" onclick={() => openPublishItem(item)}>Open</button>
                                    {/if}
                                    <button class="mini-btn" onclick={() => copyPublishItem(item)}>Copy</button>
                                    <button class="mini-btn" onclick={() => sharePublishItem(item)}>Share</button>
                                </div>
                            </div>
                        {/each}
                    </div>
                </div>
                {#if lastResult.warnings.length > 0}
                    <div class="warnings" role="status">
                        {#each lastResult.warnings as warning}
                            <span>{warning}</span>
                        {/each}
                    </div>
                {/if}
            </div>
        {/if}
    </section>
{/if}

<style>
    .loading {
        color: var(--text-secondary);
        padding: 20px;
        text-align: center;
    }
    .publish-shell {
        display: grid;
        gap: 16px;
        padding: 20px;
        min-height: 100%;
        align-content: start;
    }
    .publish-header {
        display: grid;
        grid-template-columns: minmax(0, 1fr) auto;
        gap: 16px;
        align-items: end;
        padding-bottom: 16px;
        border-bottom: 1px solid var(--border);
    }
    .title-block {
        display: grid;
        gap: 4px;
        min-width: 0;
    }
    .eyebrow {
        color: var(--green);
        font-size: 11px;
        font-weight: 700;
        text-transform: uppercase;
    }
    h1 {
        color: var(--text);
        font-size: 22px;
        line-height: 1.2;
        font-weight: 700;
    }
    .title-block p {
        color: var(--text-secondary);
        font-size: 12px;
    }
    .status-strip {
        display: flex;
        flex-wrap: wrap;
        justify-content: flex-end;
        gap: 8px;
        min-width: 0;
    }
    .status-strip span {
        border: 1px solid var(--border);
        border-radius: var(--radius);
        color: var(--text-secondary);
        font-size: 11px;
        padding: 4px 8px;
        max-width: 260px;
        overflow: hidden;
        text-overflow: ellipsis;
        white-space: nowrap;
    }
    .publish-grid {
        display: grid;
        grid-template-columns: repeat(3, minmax(0, 1fr));
        gap: 1px;
        border: 1px solid var(--border);
        background: var(--border);
    }
    .section {
        padding: 16px 20px;
        background: var(--surface);
    }
    .publish-panel {
        display: grid;
        align-content: start;
        gap: 12px;
        min-width: 0;
    }
    .section-header {
        font-size: 11px;
        font-weight: 600;
        text-transform: uppercase;
        letter-spacing: 0;
        color: var(--text-secondary);
        display: flex;
        gap: 10px;
        justify-content: space-between;
        align-items: center;
        min-width: 0;
    }
    .section-header > span:first-child {
        color: var(--text);
    }
    .section-subheader {
        color: var(--text-secondary);
        font-size: 11px;
        font-weight: 600;
        text-transform: uppercase;
    }
    .setting-row {
        display: flex;
        justify-content: space-between;
        align-items: center;
        gap: 10px;
        font-size: 13px;
        color: var(--text);
    }
    .setting-row.stacked {
        align-items: stretch;
        flex-direction: column;
        gap: 6px;
    }
    .setting-row.compact {
        padding: 0;
    }
    label,
    legend {
        color: var(--text);
        font-size: 12px;
    }
    .wide-input {
        width: 100%;
        background: var(--bg);
        border: 1px solid var(--border);
        border-radius: var(--radius);
        padding: 8px 9px;
        color: var(--text);
        font-family: var(--font);
        font-size: 12px;
        min-height: 34px;
    }
    .wide-input:focus-visible,
    .toggle:focus-visible,
    .primary-btn:focus-visible,
    .secondary-btn:focus-visible,
    .mini-btn:focus-visible,
    .value-link:focus-visible,
    .path-row a:focus-visible,
    .check-row input:focus-visible {
        outline: 2px solid var(--blue);
        outline-offset: 2px;
    }
    .wide-input::placeholder {
        color: var(--text-secondary);
        opacity: 0.7;
    }
    .text-area {
        min-height: 72px;
        resize: vertical;
        line-height: 1.4;
    }
    .links-area {
        min-height: 92px;
    }
    .source-box {
        display: grid;
        gap: 4px;
        padding: 10px;
        border: 1px solid var(--border);
        border-radius: var(--radius);
        background: var(--bg);
        color: var(--text-secondary);
        font-size: 11px;
        min-width: 0;
    }
    .source-box strong {
        color: var(--text);
        font-size: 13px;
        overflow: hidden;
        text-overflow: ellipsis;
        white-space: nowrap;
    }
    .asset-fieldset {
        display: grid;
        gap: 4px;
        border: 0;
        min-width: 0;
    }
    .asset-fieldset legend {
        margin-bottom: 2px;
    }
    .check-row {
        display: grid;
        grid-template-columns: auto minmax(0, 1fr) auto;
        gap: 8px;
        align-items: center;
        min-height: 30px;
        color: var(--text);
        font-size: 13px;
    }
    .count {
        color: var(--text-secondary);
        font-size: 11px;
        overflow-wrap: anywhere;
    }
    .settings-grid {
        display: grid;
        grid-template-columns: 1fr 1fr;
        gap: 8px;
    }
    .preview-group {
        display: grid;
        gap: 8px;
        padding-top: 4px;
    }
    .primary-btn,
    .secondary-btn {
        border-radius: var(--radius);
        font-family: var(--font);
        cursor: pointer;
    }
    .primary-btn {
        width: 100%;
        min-height: 36px;
        padding: 9px 12px;
        border: none;
        background: var(--green);
        color: var(--bg);
        font-weight: 700;
        font-size: 12px;
    }
    .primary-btn:disabled {
        opacity: 0.45;
        cursor: not-allowed;
    }
    .secondary-btn {
        background: none;
        border: 1px solid var(--border);
        color: var(--blue);
        min-height: 28px;
        padding: 4px 10px;
        font-size: 11px;
    }
    .secondary-btn:hover {
        border-color: var(--blue);
    }
    .secondary-btn:disabled {
        opacity: 0.45;
        cursor: not-allowed;
    }
    .toggle {
        border: 1px solid var(--border);
        border-radius: var(--radius);
        background: var(--bg);
        color: var(--text-secondary);
        font-family: var(--font);
        font-size: 11px;
        min-height: 28px;
        padding: 4px 8px;
        cursor: pointer;
        white-space: nowrap;
    }
    .toggle.on {
        border-color: var(--green);
        color: var(--green);
    }
    .result-actions {
        display: flex;
        gap: 6px;
        align-items: center;
        flex-wrap: wrap;
        justify-content: flex-end;
    }
    .result-section {
        border: 1px solid var(--border);
    }
    .result-body {
        display: grid;
        grid-template-columns: 190px minmax(0, 1fr);
        gap: 16px;
        margin-top: 12px;
        align-items: start;
    }
    .qr-card {
        display: grid;
        gap: 8px;
        padding: 10px;
        border: 1px solid var(--border);
        border-radius: var(--radius);
        background: var(--bg);
        min-width: 0;
    }
    .qr-card img {
        display: block;
        width: 100%;
        aspect-ratio: 1;
        border-radius: var(--radius);
        background: var(--text);
    }
    .qr-card span {
        color: var(--text);
        font-size: 12px;
        font-weight: 600;
    }
    .qr-card code {
        color: var(--text-secondary);
        font-size: 11px;
        overflow-wrap: anywhere;
    }
    .result-grid {
        display: grid;
        grid-template-columns: repeat(2, minmax(0, 1fr));
        gap: 8px;
    }
    .path-row {
        display: grid;
        grid-template-columns: 86px minmax(0, 1fr) auto;
        gap: 8px;
        align-items: center;
        padding: 8px;
        border: 1px solid var(--border);
        border-radius: var(--radius);
        background: var(--bg);
        color: var(--text-secondary);
        font-size: 11px;
        min-width: 0;
    }
    .path-row > span {
        color: var(--text-secondary);
        font-weight: 600;
    }
    .path-row code,
    .path-row a,
    .value-link {
        color: var(--text);
        overflow-wrap: anywhere;
        min-width: 0;
    }
    .path-row a {
        text-decoration: underline;
        text-underline-offset: 2px;
    }
    .value-link {
        border: 0;
        padding: 0;
        background: none;
        cursor: pointer;
        font-family: var(--font);
        font-size: 11px;
        text-align: left;
    }
    .item-actions {
        display: flex;
        gap: 4px;
        align-items: center;
        flex-wrap: wrap;
        justify-content: flex-end;
    }
    .mini-btn {
        min-height: 24px;
        padding: 3px 6px;
        border: 1px solid var(--border);
        border-radius: var(--radius);
        background: var(--surface);
        color: var(--blue);
        font-family: var(--font);
        font-size: 10px;
        cursor: pointer;
    }
    .mini-btn:hover {
        border-color: var(--blue);
    }
    .warnings {
        display: grid;
        gap: 4px;
        margin-top: 10px;
        color: var(--orange);
        font-size: 11px;
    }
    @media (max-width: 1180px) {
        .publish-grid {
            grid-template-columns: repeat(2, minmax(0, 1fr));
        }
        .delivery-panel {
            grid-column: 1 / -1;
        }
        .delivery-panel {
            grid-template-columns: repeat(2, minmax(0, 1fr));
            align-items: start;
        }
        .delivery-panel .section-header {
            grid-column: 1 / -1;
        }
    }
    @media (max-width: 760px) {
        .publish-shell {
            gap: 12px;
            padding: 8px;
        }
        .publish-header,
        .publish-grid,
        .delivery-panel,
        .result-body,
        .result-grid {
            grid-template-columns: 1fr;
        }
        .publish-header {
            gap: 10px;
            padding-bottom: 12px;
        }
        .section {
            padding: 12px;
        }
        .section-header,
        .setting-row {
            align-items: stretch;
            flex-direction: column;
            gap: 6px;
        }
        .status-strip {
            justify-content: flex-start;
        }
        .status-strip span {
            max-width: 100%;
        }
        .delivery-panel,
        .delivery-panel .section-header {
            grid-column: auto;
        }
        .settings-grid {
            grid-template-columns: 1fr;
        }
        .source-box strong {
            white-space: normal;
        }
        .check-row {
            grid-template-columns: auto minmax(0, 1fr);
            align-items: start;
        }
        .check-row .count {
            grid-column: 2;
        }
        .result-actions {
            justify-content: flex-start;
        }
        .path-row {
            grid-template-columns: 1fr;
            gap: 6px;
        }
        .item-actions {
            justify-content: flex-start;
        }
    }
</style>
