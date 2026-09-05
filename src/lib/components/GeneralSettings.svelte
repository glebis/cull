<script lang="ts">
    import { onMount } from 'svelte';
    import { backfillRawPreviews, cliToolStatus, getAppSetting, installCliTool, setAppSetting, uninstallCliTool } from '$lib/api';
    import type { CliToolStatus } from '$lib/api';
    import { CLIPBOARD_PASTE_DATE_FORMAT_SETTING, DEFAULT_CLIPBOARD_PASTE_DATE_FORMAT } from '$lib/clipboard-actions';
    import { clientToolsEnabled, navigateTo, showHiddenFiles as showHiddenFilesStore, showToast, staticPublishingEnabled, viewMode, voiceDictationEnabled } from '$lib/stores';

    let closeToTray = $state(true);
    let confirmTrash = $state(true);
    let autoUpdate = $state(true);
    let autoPurge = $state(false);
    let showHiddenFiles = $state(false);
    let pasteDateFormat = $state(DEFAULT_CLIPBOARD_PASTE_DATE_FORMAT);
    let moduleRaw = $state(true);
    let moduleStaticPublishing = $state(false);
    let moduleClientTools = $state(false);
    let moduleVoiceDictation = $state(false);
    let cliTool = $state<CliToolStatus | null>(null);
    let cliToolBusy = $state(false);
    // Availability of the status check itself: the tool being absent is a normal
    // state, but a failed check must never read as OFF.
    let cliStatusState = $state<'loading' | 'ready' | 'error'>('loading');
    let cliPendingAction = $state<'install' | 'remove' | 'repair' | null>(null);
    let cliActionError = $state<'install' | 'remove' | 'repair' | null>(null);

    const CLI_RETRY_LABELS = { install: 'Install', remove: 'Remove', repair: 'Repair' } as const;
    type CliAction = keyof typeof CLI_RETRY_LABELS;

    onMount(() => {
        // Independent loads: an unrelated settings failure must not leave the
        // command line tool row stuck on “Checking…” forever.
        void refreshCliTool();
        void loadGeneralSettings();
    });

    async function loadGeneralSettings() {
        try {
            const [tray, trash, update, purge, hidden, date, raw, publishing, client, voice] = await Promise.all([
                getAppSetting('close_to_tray'), getAppSetting('skip_trash_confirm'), getAppSetting('auto_update_enabled'),
                getAppSetting('auto_purge_missing'), getAppSetting('show_hidden_files'), getAppSetting(CLIPBOARD_PASTE_DATE_FORMAT_SETTING),
                getAppSetting('module_raw'), getAppSetting('module_static_publishing'),
                getAppSetting('module_client_tools'), getAppSetting('module_voice_dictation'),
            ]);
            closeToTray = tray !== 'false';
            confirmTrash = trash !== 'true';
            autoUpdate = update !== 'false';
            autoPurge = purge === 'true';
            showHiddenFiles = hidden === 'true';
            pasteDateFormat = date || pasteDateFormat;
            moduleRaw = raw !== 'false';
            moduleStaticPublishing = publishing === 'true';
            moduleClientTools = client === 'true';
            moduleVoiceDictation = voice === 'true';
            staticPublishingEnabled.set(moduleStaticPublishing);
            clientToolsEnabled.set(moduleClientTools);
            voiceDictationEnabled.set(moduleVoiceDictation);
            showHiddenFilesStore.set(showHiddenFiles);
        } catch (e) {
            console.error('Failed to load general settings:', e);
        }
    }

    async function toggle(key: string, value: boolean) { await setAppSetting(key, value ? 'true' : 'false'); }
    async function changeRaw() {
        await toggle('module_raw', moduleRaw);
        if (moduleRaw) showToast('RAW support enabled.', { type: 'success', duration: 10000, actions: [{ label: 'Rescan library', onclick: () => backfillRawPreviews() }] });
    }
    async function changePublishing() {
        await toggle('module_static_publishing', moduleStaticPublishing);
        staticPublishingEnabled.set(moduleStaticPublishing);
        if (!moduleStaticPublishing && $viewMode === 'publish') navigateTo('export');
    }
    async function changeClientTools() { await toggle('module_client_tools', moduleClientTools); clientToolsEnabled.set(moduleClientTools); }
    async function changeVoice() { await toggle('module_voice_dictation', moduleVoiceDictation); voiceDictationEnabled.set(moduleVoiceDictation); }
    async function changeHiddenFiles() {
        showHiddenFiles = !showHiddenFiles;
        showHiddenFilesStore.set(showHiddenFiles);
        await setAppSetting('show_hidden_files', showHiddenFiles ? 'true' : 'false');
        window.dispatchEvent(new CustomEvent('cull-hidden-files-changed'));
    }
    async function refreshCliTool() {
        cliStatusState = 'loading';
        try {
            cliTool = await cliToolStatus();
            cliStatusState = 'ready';
        } catch (e) {
            console.error('Failed to read command line tool status:', e);
            cliTool = null;
            cliStatusState = 'error';
        }
    }
    function currentCliAction(): CliAction {
        if (cliTool?.installed === true && cliTool?.stale !== true) return 'remove';
        if (cliTool?.stale) return 'repair';
        return 'install';
    }
    function onCliToolClick() {
        if (cliToolBusy || cliStatusState === 'loading') return;
        if (cliStatusState === 'error') {
            void refreshCliTool();
            return;
        }
        void changeCliTool(currentCliAction());
    }
    async function changeCliTool(action: CliAction) {
        if (cliToolBusy) return;
        cliToolBusy = true;
        cliPendingAction = action;
        cliActionError = null;
        try {
            cliTool = action === 'remove' ? await uninstallCliTool() : await installCliTool();
            if (action === 'remove') {
                showToast('Command line tool removed', { type: 'success', duration: 2500 });
            } else if (cliTool?.path_hint) {
                showToast(`Installed to ${cliTool.link_path}`, {
                    type: 'success',
                    duration: 10000,
                    detail: `That directory is not on your PATH yet. Add this to your shell profile:\n${cliTool.path_hint}`,
                });
            } else {
                showToast('Command line tool installed — run `cull --help`', {
                    type: 'success',
                    duration: 4000,
                });
            }
        } catch (e) {
            console.error(`Failed to ${action} command line tool:`, e);
            cliActionError = action;
            showToast(action === 'remove' ? 'Could not remove command line tool' : 'Could not install command line tool', {
                detail: String(e),
                type: 'error',
                duration: 6000,
            });
        } finally {
            cliToolBusy = false;
            cliPendingAction = null;
        }
    }
    async function saveDateFormat() {
        pasteDateFormat = pasteDateFormat.trim() || DEFAULT_CLIPBOARD_PASTE_DATE_FORMAT;
        await setAppSetting(CLIPBOARD_PASTE_DATE_FORMAT_SETTING, pasteDateFormat);
    }

    let cliToolLabel = $derived.by(() => {
        if (cliPendingAction === 'install') return 'INSTALLING…';
        if (cliPendingAction === 'remove') return 'REMOVING…';
        if (cliPendingAction === 'repair') return 'REPAIRING…';
        if (cliStatusState === 'loading') return 'CHECKING…';
        if (cliStatusState === 'error') return 'RETRY';
        if (cliTool?.stale) return 'REPAIR';
        if (cliTool?.installed) return 'REMOVE';
        return 'INSTALL';
    });
    let cliToolAriaLabel = $derived.by(() => {
        if (cliPendingAction === 'install') return 'Installing command line tool';
        if (cliPendingAction === 'remove') return 'Removing command line tool';
        if (cliPendingAction === 'repair') return 'Repairing command line tool';
        if (cliStatusState === 'loading') return 'Checking command line tool status';
        if (cliStatusState === 'error') return 'Retry command line tool status check';
        if (cliTool?.stale) return 'Repair command line tool';
        if (cliTool?.installed) return 'Remove command line tool';
        return 'Install command line tool';
    });
</script>

<section class="settings-section">
    <h3>General</h3>
    <div class="setting-row"><span>Close to tray</span><button class:on={closeToTray} aria-pressed={closeToTray} onclick={() => { closeToTray = !closeToTray; toggle('close_to_tray', closeToTray); }}>{closeToTray ? 'ON' : 'OFF'}</button></div>
    <div class="setting-row"><span>Confirm before Trash</span><button class:on={confirmTrash} aria-pressed={confirmTrash} onclick={() => { confirmTrash = !confirmTrash; setAppSetting('skip_trash_confirm', confirmTrash ? 'false' : 'true'); }}>{confirmTrash ? 'ON' : 'OFF'}</button></div>
    <div class="setting-row"><span>Auto update</span><button class:on={autoUpdate} aria-pressed={autoUpdate} onclick={() => { autoUpdate = !autoUpdate; toggle('auto_update_enabled', autoUpdate); window.dispatchEvent(new CustomEvent('auto-update-setting-changed')); }}>{autoUpdate ? 'ON' : 'OFF'}</button></div>
    <div class="setting-row"><span>Auto-purge missing files</span><button class:on={autoPurge} aria-pressed={autoPurge} onclick={() => { autoPurge = !autoPurge; toggle('auto_purge_missing', autoPurge); }}>{autoPurge ? 'ON' : 'OFF'}</button></div>
    <div class="setting-row"><span>Show hidden files</span><button aria-label="Show hidden files" class:on={showHiddenFiles} aria-pressed={showHiddenFiles} onclick={changeHiddenFiles}>{showHiddenFiles ? 'ON' : 'OFF'}</button></div>
        <div class="setting-row">
            <span>Command line tool</span>
            <button
                class:on={cliTool?.installed && !cliTool?.stale}
                aria-label={cliToolAriaLabel}
                aria-busy={cliToolBusy || cliStatusState === 'loading'}
                disabled={cliToolBusy || cliStatusState === 'loading'}
                onclick={onCliToolClick}
            >{cliToolLabel}</button>
        </div>
        {#if cliStatusState === 'loading'}
            <p class="note" role="status">Checking the command line tool…</p>
        {:else if cliStatusState === 'error'}
            <p class="note" role="alert">Cull could not check the command line tool just now. Press Retry to check again.</p>
        {:else if cliTool?.stale}
            <p class="note"><code>{cliTool.link_path}</code> points at a different copy of Cull. Repair it to use this one.</p>
        {:else if cliTool?.installed}
            <p class="note"><code>cull</code> is available at <code>{cliTool.link_path}</code>.</p>
        {:else if cliTool?.path_hint}
            <p class="note">Installs to <code>{cliTool.candidate_dir}</code>, which is not on your PATH. You will need to add <code>{cliTool.path_hint}</code> to your shell profile.</p>
        {:else if cliTool}
            <p class="note">Links <code>cull</code> into <code>{cliTool.candidate_dir}</code> so the CLI and MCP examples in the docs work as written.</p>
        {/if}
        {#if cliActionError}
            <p class="note" role="alert">Cull could not {cliActionError} the command line tool just now. Press {CLI_RETRY_LABELS[cliActionError]} to try again.</p>
        {/if}
    <label class="setting-row"><span>Paste filename date</span><input bind:value={pasteDateFormat} onblur={saveDateFormat} /></label>
    <p class="note">Used when the destination folder has no numeric filename sequence.</p>
</section>

<section class="settings-section">
    <h3>Modules</h3>
    <label class="module"><input type="checkbox" bind:checked={moduleRaw} onchange={changeRaw} /><span><strong>RAW File Support</strong><small>Import and preview RAW camera files.</small></span></label>
    <label class="module"><input type="checkbox" bind:checked={moduleStaticPublishing} onchange={changePublishing} /><span><strong>Static Publishing</strong><small>Canvas packages, gallery assets, handoffs, and scheduled publishing.</small></span></label>
    <label class="module"><input type="checkbox" bind:checked={moduleClientTools} onchange={changeClientTools} /><span><strong>Client Tools</strong><small>Client delivery list export in the command palette.</small></span></label>
    <label class="module"><input type="checkbox" bind:checked={moduleVoiceDictation} onchange={changeVoice} /><span><strong>Voice Dictation</strong><small>Microphone dictation controls in the search bar.</small></span></label>
</section>

<style>
    .settings-section { padding: 16px 20px; border-bottom: 1px solid var(--border); }
    h3 { margin: 0 0 12px; color: var(--text-secondary); font-size: 11px; letter-spacing: .08em; text-transform: uppercase; }
    .setting-row { min-height: 36px; display: flex; align-items: center; justify-content: space-between; gap: 16px; color: var(--text); font-size: 12px; }
    button { min-width: 42px; padding: 5px 8px; background: var(--bg); border: 1px solid var(--border); border-radius: var(--radius); color: var(--text-secondary); font: 10px var(--font); cursor: pointer; }
    button.on { color: var(--green); border-color: var(--green); }
    .setting-row input { min-width: 190px; box-sizing: border-box; background: var(--bg); color: var(--text); border: 1px solid var(--border); border-radius: var(--radius); padding: 6px 8px; font: 11px var(--font); }
    .module { display: flex; gap: 10px; margin: 12px 0; color: var(--text); cursor: pointer; }
    .module span { display: grid; gap: 3px; }
    strong { font-size: 12px; font-weight: 500; }
    small, .note { color: var(--text-secondary); font-size: 10px; line-height: 1.5; }
    .note { margin: 4px 0 0; }
</style>
