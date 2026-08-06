<script lang="ts">
    import ActionMenu, { type ActionMenuItem } from './ActionMenu.svelte';

    let {
        onclose,
        onopen,
        onalpha,
        onbeta,
        ondelete,
    }: {
        onclose: () => void;
        onopen: () => void;
        onalpha: () => void;
        onbeta: () => void;
        ondelete: () => void;
    } = $props();

    const items: ActionMenuItem[] = [
        { id: 'open', label: 'Open', action: () => onopen() },
        {
            id: 'add',
            label: 'Add to Collection',
            children: [
                { id: 'alpha', label: 'Alpha', action: () => onalpha() },
                { id: 'disabled', label: 'Unavailable', disabled: true, action: () => {} },
                { id: 'beta', label: 'Beta', action: () => onbeta() },
            ],
        },
        { id: 'delete', label: 'Delete…', danger: true, separatorBefore: true, action: () => ondelete() },
    ];
</script>

<button type="button">Menu opener</button>
<ActionMenu title="Example" x={40} y={40} {items} {onclose} />
