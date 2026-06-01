<script lang="ts">
    import { getAppSetting, setAppSetting, exportStaticPublishPackage, serveStaticPublishPackage } from '$lib/api';
    import type { StaticPublishResult, StaticPublishServerResult } from '$lib/api';
    import { activeCanvas, activeSession, images, selectedIds, showToast } from '$lib/stores';
    import { buildStaticPublishRequestFromSavedCanvas, countSavedCanvasItems, formatStaticPublishLinks, parseStaticPublishLinks } from '$lib/static-publishing';
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
    let parsedLinks = $derived(parseStaticPublishLinks(linksText));

    const sourceImages = $derived(
        $selectedIds.size > 0
            ? $images.filter(img => $selectedIds.has(img.image.id))
            : $images
    );
    const savedCanvasItemCount = $derived(countSavedCanvasItems($activeCanvas));
    const exportSourceCount = $derived($activeCanvas ? savedCanvasItemCount : sourceImages.length);
    const exportSourceLabel = $derived($activeCanvas ? $activeCanvas.name : 'Current view');

    function settingIsTrue(value: string | null, fallback: boolean): boolean {
        if (value === null) return fallback;
        return value === 'true';
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
        scenario = (savedScenario as PublishScenario | null) ?? 'local_preview';
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
        if (exportSourceCount === 0) return;
        exporting = true;
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
            showToast('Static package exported', {
                detail: `${result.image_count} images`,
                type: 'success',
            });
        } catch (e) {
            showToast(`Static export failed: ${e}`, { type: 'error' });
        } finally {
            exporting = false;
        }
    }

    async function copyHandoffPath() {
        if (!lastResult) return;
        await navigator.clipboard.writeText(lastResult.instructions_path);
        showToast('Claude handoff path copied', { type: 'success', duration: 2500 });
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
            showToast('Static server started', {
                detail: result.url,
                type: 'success',
                duration: 5000,
            });
        } catch (e) {
            showToast(`Static server failed: ${e}`, { type: 'error' });
        } finally {
            startingServer = false;
        }
    }
</script>

{#if loading}
    <p class="loading">Loading...</p>
{:else}
    <div class="section">
        <div class="section-header">Workflow</div>
        <div class="setting-row stacked">
            <label for="static-scenario">Scenario</label>
            <select
                id="static-scenario"
                class="wide-input"
                bind:value={scenario}
                onchange={() => saveSetting('static_publishing_scenario', scenario)}
            >
                <option value="local_preview">Local preview</option>
                <option value="client_review">Client review link</option>
                <option value="agent_handoff">Agent handoff</option>
                <option value="static_host">Static host package</option>
            </select>
        </div>
        <div class="setting-row stacked">
            <label for="static-output-dir">Output folder</label>
            <input
                id="static-output-dir"
                class="wide-input"
                bind:value={outputDir}
                placeholder="Default: app data / static-publishing / canvas"
                onblur={() => saveSetting('static_publishing_output_dir', outputDir)}
            />
        </div>
        <div class="setting-row">
            <span>Source</span>
            <span class="count">{exportSourceLabel} · {exportSourceCount} image{exportSourceCount === 1 ? '' : 's'}</span>
        </div>
        <button class="primary-btn" onclick={exportPackage} disabled={exporting || exportSourceCount === 0}>
            {exporting ? 'Building...' : 'Build Static Site'}
        </button>
    </div>

    <div class="section">
        <div class="section-header">Site</div>
        <div class="setting-row stacked">
            <label for="static-site-title">Title</label>
            <input
                id="static-site-title"
                class="wide-input"
                bind:value={siteTitle}
                onblur={() => saveSetting('static_publishing_site_title', siteTitle)}
            />
        </div>
        <div class="setting-row stacked">
            <label for="static-site-description">Description</label>
            <textarea
                id="static-site-description"
                class="wide-input text-area"
                rows="3"
                bind:value={siteDescription}
                onblur={() => saveSetting('static_publishing_site_description', siteDescription)}
            ></textarea>
        </div>
        <div class="setting-row stacked">
            <label for="static-share-url">Share URL for QR</label>
            <input
                id="static-share-url"
                class="wide-input"
                bind:value={shareUrl}
                placeholder="Cloudflare, Tailscale, or static host URL"
                onblur={() => saveSetting('static_publishing_share_url', shareUrl)}
            />
        </div>
        <div class="setting-row">
            <span>Allow search indexing</span>
            <button
                class="toggle"
                class:on={indexable}
                onclick={() => { indexable = !indexable; saveBoolean('static_publishing_indexable', indexable); }}
                aria-pressed={indexable}
            >
                {indexable ? 'ALLOWED' : 'BLOCKED'}
            </button>
        </div>
        <div class="setting-row stacked">
            <label for="static-links">Links</label>
            <textarea
                id="static-links"
                class="wide-input text-area"
                rows="4"
                bind:value={linksText}
                placeholder="Project brief | https://example.com/brief"
                onblur={saveLinks}
            ></textarea>
            <span class="count">{parsedLinks.length} link{parsedLinks.length === 1 ? '' : 's'} included</span>
        </div>
    </div>

    <div class="section">
        <div class="section-header">Assets</div>
        <label class="check-row">
            <input type="checkbox" bind:checked={includeThumbnails} onchange={() => saveBoolean('static_publishing_include_thumb', includeThumbnails)} />
            <span>Thumb</span>
            <span class="count">420px JPEG</span>
        </label>
        <label class="check-row">
            <input type="checkbox" bind:checked={includeWeb} onchange={() => saveBoolean('static_publishing_include_web', includeWeb)} />
            <span>Web</span>
            <span class="count">1800px JPEG</span>
        </label>
        <label class="check-row">
            <input type="checkbox" bind:checked={includeFull} onchange={() => saveBoolean('static_publishing_include_full', includeFull)} />
            <span>Full</span>
            <span class="count">source or RAW preview</span>
        </label>
    </div>

    <div class="section">
        <div class="section-header">Local Preview</div>
        <div class="settings-grid">
            <div class="setting-row stacked compact">
                <label for="static-server-host">Host</label>
                <input
                    id="static-server-host"
                    class="wide-input"
                    bind:value={serverHost}
                    placeholder="127.0.0.1"
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
                    onblur={() => saveSetting('static_publishing_server_port', serverPort)}
                />
            </div>
        </div>
    </div>

    {#if lastResult}
        <div class="section result-section">
            <div class="section-header">
                Last Package
                <div class="result-actions">
                    <button class="secondary-btn" onclick={startServer} disabled={startingServer}>
                        {startingServer ? 'Starting' : 'Start Local Preview'}
                    </button>
                    <button class="secondary-btn" onclick={copyHandoffPath}>Copy Handoff</button>
                </div>
            </div>
            <div class="path-row"><span>Site</span><code>{lastResult.site_dir}</code></div>
            <div class="path-row"><span>Manifest</span><code>{lastResult.manifest_path}</code></div>
            <div class="path-row"><span>Handoff</span><code>{lastResult.instructions_path}</code></div>
            <div class="path-row"><span>QR</span><code>{lastResult.qr_svg_path}</code></div>
            <div class="path-row"><span>URL</span><code>{lastResult.qr_target_url}</code></div>
            <div class="path-row"><span>Phrase</span><code>{lastResult.access_phrase}</code></div>
            {#if serverResult}
                <div class="path-row"><span>Server</span><code>{serverResult.url}</code></div>
            {/if}
            {#if lastResult.warnings.length > 0}
                <div class="warnings">
                    {#each lastResult.warnings as warning}
                        <span>{warning}</span>
                    {/each}
                </div>
            {/if}
        </div>
    {/if}
{/if}

<style>
    .loading {
        color: var(--text-secondary);
        padding: 20px;
        text-align: center;
    }
    .section {
        padding: 16px 20px;
        border-bottom: 1px solid var(--border);
    }
    .section:last-child {
        border-bottom: none;
    }
    .section-header {
        font-size: 11px;
        font-weight: 600;
        text-transform: uppercase;
        letter-spacing: 0.05em;
        color: var(--text-secondary);
        margin-bottom: 12px;
        display: flex;
        justify-content: space-between;
        align-items: center;
    }
    .setting-row {
        display: flex;
        justify-content: space-between;
        align-items: center;
        padding: 6px 0;
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
    .wide-input {
        width: 100%;
        background: var(--bg);
        border: 1px solid var(--border);
        border-radius: var(--radius);
        padding: 7px 9px;
        color: var(--text);
        font-family: var(--font);
        font-size: 12px;
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
    .check-row {
        display: grid;
        grid-template-columns: auto 1fr auto;
        gap: 8px;
        align-items: center;
        padding: 6px 0;
        color: var(--text);
        font-size: 13px;
    }
    .count {
        color: var(--text-secondary);
        font-size: 11px;
    }
    .settings-grid {
        display: grid;
        grid-template-columns: 1fr 1fr;
        gap: 8px;
    }
    .primary-btn,
    .secondary-btn {
        border-radius: var(--radius);
        font-family: var(--font);
        cursor: pointer;
    }
    .primary-btn {
        width: 100%;
        margin-top: 10px;
        padding: 8px 12px;
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
        padding: 2px 10px;
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
        padding: 4px 8px;
        cursor: pointer;
    }
    .toggle.on {
        border-color: var(--green);
        color: var(--green);
    }
    .result-actions {
        display: flex;
        gap: 6px;
        align-items: center;
    }
    .path-row {
        display: grid;
        grid-template-columns: 70px 1fr;
        gap: 8px;
        align-items: baseline;
        padding: 4px 0;
        color: var(--text-secondary);
        font-size: 11px;
    }
    .path-row code {
        color: var(--text);
        overflow-wrap: anywhere;
    }
    .warnings {
        display: grid;
        gap: 4px;
        margin-top: 10px;
        color: var(--orange);
        font-size: 11px;
    }
    .result-section {
        background: var(--surface);
    }
</style>
