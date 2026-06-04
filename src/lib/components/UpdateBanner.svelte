<script lang="ts">
    import { onMount } from 'svelte';
    import { startAutoUpdates } from '$lib/update-manager';

    let stopAutoUpdates: (() => void) | null = null;
    let autoUpdateLoopGeneration = 0;

    async function restartAutoUpdateLoop(disposed: () => boolean = () => false) {
        const generation = ++autoUpdateLoopGeneration;
        stopAutoUpdates?.();
        stopAutoUpdates = null;
        const stop = await startAutoUpdates();
        if (disposed() || generation !== autoUpdateLoopGeneration) {
            stop();
        } else {
            stopAutoUpdates = stop;
        }
    }

    onMount(() => {
        let disposed = false;
        restartAutoUpdateLoop(() => disposed).catch(e => console.debug('Failed to start auto updates:', e));

        const handleAutoUpdateSettingChange = () => {
            restartAutoUpdateLoop(() => disposed).catch(e => console.debug('Failed to restart auto updates:', e));
        };
        window.addEventListener('auto-update-setting-changed', handleAutoUpdateSettingChange);

        return () => {
            disposed = true;
            autoUpdateLoopGeneration += 1;
            stopAutoUpdates?.();
            stopAutoUpdates = null;
            window.removeEventListener('auto-update-setting-changed', handleAutoUpdateSettingChange);
        };
    });
</script>
