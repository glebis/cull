<script lang="ts">
    import ModalDialog from './ModalDialog.svelte';

    let {
        onclose,
        visible = true,
    }: {
        onclose: () => void;
        visible?: boolean;
    } = $props();

    function initializeShell(node: HTMLElement & { inert: boolean }) {
        node.inert = false;
    }
</script>

<div class="app-shell" aria-hidden="false" use:initializeShell>
    <button type="button">Background action</button>
</div>

{#if visible}
    <ModalDialog
        titleId="test-modal-title"
        descriptionId="test-modal-description"
        {onclose}
    >
        <h2 id="test-modal-title">Accessible test dialog</h2>
        <p id="test-modal-description">Modal behavior under test</p>
        <button type="button" data-modal-initial-focus>First action</button>
        <button type="button">Last action</button>
    </ModalDialog>
{/if}
