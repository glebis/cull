<script lang="ts">
    import { onMount } from 'svelte';
    import { backfillRawPreviews, getAppSetting, setAppSetting } from '$lib/api';
    import { CLIPBOARD_PASTE_DATE_FORMAT_SETTING, DEFAULT_CLIPBOARD_PASTE_DATE_FORMAT } from '$lib/clipboard-actions';
    import { clientToolsEnabled, navigateTo, showToast, staticPublishingEnabled, viewMode, voiceDictationEnabled } from '$lib/stores';

    let closeToTray = $state(true);
    let confirmTrash = $state(true);
    let autoUpdate = $state(true);
    let autoPurge = $state(false);
    let pasteDateFormat = $state(DEFAULT_CLIPBOARD_PASTE_DATE_FORMAT);
    let moduleRaw = $state(true);
    let moduleStaticPublishing = $state(false);
    let moduleClientTools = $state(false);
    let moduleVoiceDictation = $state(false);

    onMount(async () => {
        const [tray, trash, update, purge, date, raw, publishing, client, voice] = await Promise.all([
            getAppSetting('close_to_tray'), getAppSetting('skip_trash_confirm'), getAppSetting('auto_update_enabled'),
            getAppSetting('auto_purge_missing'), getAppSetting(CLIPBOARD_PASTE_DATE_FORMAT_SETTING),
            getAppSetting('module_raw'), getAppSetting('module_static_publishing'),
            getAppSetting('module_client_tools'), getAppSetting('module_voice_dictation'),
        ]);
        closeToTray = tray !== 'false';
        confirmTrash = trash !== 'true';
        autoUpdate = update !== 'false';
        autoPurge = purge === 'true';
        pasteDateFormat = date || pasteDateFormat;
        moduleRaw = raw !== 'false';
        moduleStaticPublishing = publishing === 'true';
        moduleClientTools = client === 'true';
        moduleVoiceDictation = voice === 'true';
        staticPublishingEnabled.set(moduleStaticPublishing);
        clientToolsEnabled.set(moduleClientTools);
        voiceDictationEnabled.set(moduleVoiceDictation);
    });

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
    async function saveDateFormat() {
        pasteDateFormat = pasteDateFormat.trim() || DEFAULT_CLIPBOARD_PASTE_DATE_FORMAT;
        await setAppSetting(CLIPBOARD_PASTE_DATE_FORMAT_SETTING, pasteDateFormat);
    }
</script>

<section class="settings-section">
    <h3>General</h3>
    <div class="setting-row"><span>Close to tray</span><button class:on={closeToTray} aria-pressed={closeToTray} onclick={() => { closeToTray = !closeToTray; toggle('close_to_tray', closeToTray); }}>{closeToTray ? 'ON' : 'OFF'}</button></div>
    <div class="setting-row"><span>Confirm before Trash</span><button class:on={confirmTrash} aria-pressed={confirmTrash} onclick={() => { confirmTrash = !confirmTrash; setAppSetting('skip_trash_confirm', confirmTrash ? 'false' : 'true'); }}>{confirmTrash ? 'ON' : 'OFF'}</button></div>
    <div class="setting-row"><span>Auto update</span><button class:on={autoUpdate} aria-pressed={autoUpdate} onclick={() => { autoUpdate = !autoUpdate; toggle('auto_update_enabled', autoUpdate); window.dispatchEvent(new CustomEvent('auto-update-setting-changed')); }}>{autoUpdate ? 'ON' : 'OFF'}</button></div>
    <div class="setting-row"><span>Auto-purge missing files</span><button class:on={autoPurge} aria-pressed={autoPurge} onclick={() => { autoPurge = !autoPurge; toggle('auto_purge_missing', autoPurge); }}>{autoPurge ? 'ON' : 'OFF'}</button></div>
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
