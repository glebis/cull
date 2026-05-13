<script lang="ts">
    import { getAppSetting, setAppSetting, exportStaticPublishPackage, serveStaticPublishPackage } from '$lib/api';
    import type { StaticPublishResult, StaticPublishServerResult } from '$lib/api';
    import { activeSession, images, selectedIds, showToast } from '$lib/stores';
    import { onMount } from 'svelte';

    type PublishPolicy = 'manual_review' | 'auto_checks' | 'auto_agent' | 'full_auto';
    type Schedule = 'manual' | 'daily' | 'weekly' | 'monthly' | 'on_canvas_change';
    type Destination = 'local' | 'vercel_handoff' | 's3';

    let loading = $state(true);
    let exporting = $state(false);
    let canvasName = $state('Current Canvas');
    let outputDir = $state('');
    let includeThumbnails = $state(true);
    let includeWeb = $state(true);
    let includeFull = $state(false);
    let serverHost = $state('127.0.0.1');
    let serverPort = $state('8000');
    let publishPolicy = $state<PublishPolicy>('manual_review');
    let schedule = $state<Schedule>('manual');
    let destination = $state<Destination>('local');
    let provider = $state('cloudflare_r2');
    let endpoint = $state('');
    let region = $state('');
    let bucket = $state('');
    let prefix = $state('canvas');
    let shareUrl = $state('');
    let lastResult = $state<StaticPublishResult | null>(null);
    let serverResult = $state<StaticPublishServerResult | null>(null);
    let startingServer = $state(false);

    const sourceImages = $derived(
        $selectedIds.size > 0
            ? $images.filter(img => $selectedIds.has(img.image.id))
            : $images
    );

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
            savedServerHost,
            savedServerPort,
            savedPolicy,
            savedSchedule,
            savedDestination,
            savedProvider,
            savedEndpoint,
            savedRegion,
            savedBucket,
            savedPrefix,
            savedShareUrl,
        ] = await Promise.all([
            getAppSetting('static_publishing_output_dir'),
            getAppSetting('static_publishing_include_thumb'),
            getAppSetting('static_publishing_include_web'),
            getAppSetting('static_publishing_include_full'),
            getAppSetting('static_publishing_server_host'),
            getAppSetting('static_publishing_server_port'),
            getAppSetting('static_publishing_publish_policy'),
            getAppSetting('static_publishing_schedule'),
            getAppSetting('static_publishing_destination'),
            getAppSetting('static_publishing_s3_provider'),
            getAppSetting('static_publishing_s3_endpoint'),
            getAppSetting('static_publishing_s3_region'),
            getAppSetting('static_publishing_s3_bucket'),
            getAppSetting('static_publishing_s3_prefix'),
            getAppSetting('static_publishing_share_url'),
        ]);

        outputDir = savedOutputDir ?? '';
        includeThumbnails = settingIsTrue(savedThumb, true);
        includeWeb = settingIsTrue(savedWeb, true);
        includeFull = settingIsTrue(savedFull, false);
        serverHost = savedServerHost ?? '127.0.0.1';
        serverPort = savedServerPort ?? '8000';
        publishPolicy = (savedPolicy as PublishPolicy | null) ?? 'manual_review';
        schedule = (savedSchedule as Schedule | null) ?? 'manual';
        destination = (savedDestination as Destination | null) ?? 'local';
        provider = savedProvider ?? 'cloudflare_r2';
        endpoint = savedEndpoint ?? '';
        region = savedRegion ?? '';
        bucket = savedBucket ?? '';
        prefix = savedPrefix ?? 'canvas';
        shareUrl = savedShareUrl ?? '';
        canvasName = $activeSession?.name ? `${$activeSession.name} Canvas` : 'Current Canvas';
        loading = false;
    });

    async function saveSetting(key: string, value: string) {
        await setAppSetting(key, value);
    }

    async function saveBoolean(key: string, value: boolean) {
        await setAppSetting(key, value ? 'true' : 'false');
    }

    async function exportPackage() {
        if (sourceImages.length === 0) return;
        exporting = true;
        lastResult = null;
        try {
            const result = await exportStaticPublishPackage({
                canvas_name: canvasName.trim() || 'Current Canvas',
                items: sourceImages.map(img => ({ image_id: img.image.id })),
                layout_json: JSON.stringify({
                    type: 'current_view_snapshot',
                    image_ids: sourceImages.map(img => img.image.id),
                }),
                output_dir: outputDir.trim() || null,
                share_url: shareUrl.trim() || null,
                include_thumbnails: includeThumbnails,
                include_web: includeWeb,
                include_full: includeFull,
            });
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
        <div class="section-header">Canvas Package</div>
        <div class="setting-row stacked">
            <label for="static-canvas-name">Canvas name</label>
            <input id="static-canvas-name" class="wide-input" bind:value={canvasName} />
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
            <span>Canvas source</span>
            <span class="count">{sourceImages.length} image{sourceImages.length === 1 ? '' : 's'}</span>
        </div>
        <button class="primary-btn" onclick={exportPackage} disabled={exporting || sourceImages.length === 0}>
            {exporting ? 'Exporting...' : 'Export Static Site Package'}
        </button>
    </div>

    <div class="section">
        <div class="section-header">Image Variants</div>
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
        <div class="section-header">Local Server</div>
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

    <div class="section">
        <div class="section-header">Automation</div>
        <div class="setting-row stacked">
            <label for="static-policy">Publish policy</label>
            <select
                id="static-policy"
                class="wide-input"
                bind:value={publishPolicy}
                onchange={() => saveSetting('static_publishing_publish_policy', publishPolicy)}
            >
                <option value="manual_review">Manual review</option>
                <option value="auto_checks">Auto-publish if checks pass</option>
                <option value="auto_agent">Auto-publish with agent edits</option>
                <option value="full_auto">Fully automatic</option>
            </select>
        </div>
        <div class="setting-row stacked">
            <label for="static-schedule">Schedule</label>
            <select
                id="static-schedule"
                class="wide-input"
                bind:value={schedule}
                onchange={() => saveSetting('static_publishing_schedule', schedule)}
            >
                <option value="manual">Manual</option>
                <option value="daily">Daily</option>
                <option value="weekly">Weekly</option>
                <option value="monthly">Monthly</option>
                <option value="on_canvas_change">On canvas change</option>
            </select>
        </div>
    </div>

    <div class="section">
        <div class="section-header">Destination</div>
        <div class="setting-row stacked">
            <label for="static-destination">Target</label>
            <select
                id="static-destination"
                class="wide-input"
                bind:value={destination}
                onchange={() => saveSetting('static_publishing_destination', destination)}
            >
                <option value="local">Local package</option>
                <option value="vercel_handoff">Vercel handoff</option>
                <option value="s3">S3-compatible bucket</option>
            </select>
        </div>
        {#if destination === 's3'}
            <div class="setting-row stacked">
                <label for="static-provider">Provider profile</label>
                <select
                    id="static-provider"
                    class="wide-input"
                    bind:value={provider}
                    onchange={() => saveSetting('static_publishing_s3_provider', provider)}
                >
                    <option value="cloudflare_r2">Cloudflare R2</option>
                    <option value="aws_s3">AWS S3</option>
                    <option value="scaleway">Scaleway</option>
                    <option value="ovh">OVHcloud</option>
                    <option value="hetzner">Hetzner</option>
                    <option value="exoscale">Exoscale</option>
                    <option value="ionos">IONOS</option>
                    <option value="custom">Custom S3</option>
                </select>
            </div>
            <div class="settings-grid">
                <input class="wide-input" bind:value={endpoint} placeholder="Endpoint" onblur={() => saveSetting('static_publishing_s3_endpoint', endpoint)} />
                <input class="wide-input" bind:value={region} placeholder="Region" onblur={() => saveSetting('static_publishing_s3_region', region)} />
                <input class="wide-input" bind:value={bucket} placeholder="Bucket" onblur={() => saveSetting('static_publishing_s3_bucket', bucket)} />
                <input class="wide-input" bind:value={prefix} placeholder="Path prefix" onblur={() => saveSetting('static_publishing_s3_prefix', prefix)} />
            </div>
        {/if}
    </div>

    {#if lastResult}
        <div class="section result-section">
            <div class="section-header">
                Last Export
                <div class="result-actions">
                    <button class="secondary-btn" onclick={startServer} disabled={startingServer}>
                        {startingServer ? 'Starting' : 'Start Server'}
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
