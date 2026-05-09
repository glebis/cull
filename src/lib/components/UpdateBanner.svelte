<script lang="ts">
    import { check } from '@tauri-apps/plugin-updater';
    import { relaunch } from '@tauri-apps/plugin-process';
    import { onMount } from 'svelte';

    let updateAvailable = $state(false);
    let version = $state('');
    let installing = $state(false);
    let progress = $state('');
    let dismissed = $state(false);

    onMount(async () => {
        try {
            const update = await check();
            if (update) {
                version = update.version;
                updateAvailable = true;

                (globalThis as any).__pendingUpdate = update;
            }
        } catch {
            // Silently ignore update check failures (offline, no releases yet, etc.)
        }
    });

    async function installUpdate() {
        const update = (globalThis as any).__pendingUpdate;
        if (!update) return;

        installing = true;
        progress = 'Downloading...';

        try {
            await update.downloadAndInstall((event: any) => {
                if (event.event === 'Started' && event.data?.contentLength) {
                    progress = `Downloading (${Math.round(event.data.contentLength / 1024 / 1024)}MB)...`;
                } else if (event.event === 'Finished') {
                    progress = 'Restarting...';
                }
            });
            await relaunch();
        } catch (e) {
            progress = 'Update failed';
            installing = false;
        }
    }

    function dismiss() {
        dismissed = true;
    }
</script>

{#if updateAvailable && !dismissed}
    <div class="update-banner">
        {#if installing}
            <span class="update-text">{progress}</span>
        {:else}
            <span class="update-text">v{version} available</span>
            <button class="update-btn install" onclick={installUpdate}>Install & Restart</button>
            <button class="update-btn dismiss" onclick={dismiss}>Later</button>
        {/if}
    </div>
{/if}

<style>
    .update-banner {
        display: flex;
        align-items: center;
        gap: 8px;
        padding: 6px 12px;
        background: var(--surface);
        border-bottom: 1px solid var(--border);
        font-size: 12px;
    }

    .update-text {
        color: var(--blue);
    }

    .update-btn {
        padding: 2px 8px;
        border: 1px solid var(--border);
        border-radius: var(--radius);
        background: transparent;
        color: var(--text);
        font: inherit;
        font-size: 11px;
        cursor: pointer;
    }

    .update-btn.install {
        background: var(--blue);
        color: var(--bg);
        border-color: var(--blue);
    }

    .update-btn.install:hover {
        opacity: 0.9;
    }

    .update-btn.dismiss:hover {
        border-color: var(--text-secondary);
    }
</style>
