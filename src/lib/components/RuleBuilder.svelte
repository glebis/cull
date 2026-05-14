<script lang="ts">
    import type { FilterNode, FilterGroup, FilterRule } from '$lib/api';

    interface Props {
        filter: FilterNode;
        onchange: (next: FilterNode) => void;
    }
    let { filter, onchange }: Props = $props();

    // ── Field metadata ────────────────────────────────────────────────────────

    type FieldType = 'number' | 'enum' | 'string' | 'boolean' | 'date';

    interface FieldMeta {
        label: string;
        type: FieldType;
        ops: string[];
        enumValues?: string[];
    }

    const FIELDS: Record<string, FieldMeta> = {
        rating:          { label: 'Rating',       type: 'number',  ops: ['eq','gte','lte','gt','lt'] },
        color_label:     { label: 'Color Label',   type: 'enum',    ops: ['eq','neq'],        enumValues: ['red','yellow','green','blue','purple'] },
        decision:        { label: 'Decision',      type: 'enum',    ops: ['eq','neq'],        enumValues: ['accepted','rejected','pending'] },
        format:          { label: 'Format',        type: 'string',  ops: ['eq','neq','contains'] },
        width:           { label: 'Width',         type: 'number',  ops: ['eq','gt','gte','lt','lte'] },
        height:          { label: 'Height',        type: 'number',  ops: ['eq','gt','gte','lt','lte'] },
        orientation:     { label: 'Orientation',   type: 'enum',    ops: ['eq'],              enumValues: ['landscape','portrait','square'] },
        source_label:    { label: 'Source',        type: 'string',  ops: ['eq','neq','contains'] },
        is_ai_generated: { label: 'AI Generated',  type: 'boolean', ops: ['eq'] },
        imported_at:     { label: 'Imported',      type: 'date',    ops: ['last_n_days','this_week','this_month'] },
        ai_prompt:       { label: 'Prompt',        type: 'string',  ops: ['contains','is_empty'] },
        search_text:     { label: 'Search',        type: 'string',  ops: ['contains','neq'] },
        aspect_ratio:    { label: 'Aspect Ratio',  type: 'number',  ops: ['gt','gte','lt','lte','eq'] },
    };

    const OP_LABELS: Record<string, string> = {
        eq: '=', neq: '≠', gt: '>', gte: '≥', lt: '<', lte: '≤',
        contains: 'contains', last_n_days: 'last N days',
        this_week: 'this week', this_month: 'this month', is_empty: 'is empty',
    };

    const TYPE_COLOR: Record<FieldType, string> = {
        number:  'var(--orange)',
        enum:    'var(--purple)',
        string:  'var(--green)',
        boolean: 'var(--teal, #1abc9c)',
        date:    'var(--yellow, #e0af68)',
    };

    // rating gets blue regardless of type
    function chipColor(field: string): string {
        if (field === 'rating') return 'var(--blue)';
        const meta = FIELDS[field];
        return meta ? TYPE_COLOR[meta.type] : 'var(--text-secondary)';
    }

    // ── Normalization ─────────────────────────────────────────────────────────

    interface WorkingRule {
        field: string;
        op: string;
        value: any;
        negated: boolean;
    }

    function normalize(node: FilterNode): FilterGroup {
        if (node.type === 'group') return node;
        if (node.type === 'rule') return { type: 'group', op: 'and', children: [node] };
        // not wrapping a rule → group with one rule (negated flag handled in display)
        if (node.type === 'not' && node.child.type === 'rule') {
            return { type: 'group', op: 'and', children: [node.child] };
        }
        return { type: 'group', op: 'and', children: [] };
    }

    function toWorkingRule(node: FilterNode): WorkingRule {
        if (node.type === 'not' && node.child.type === 'rule') {
            return { field: node.child.field, op: node.child.op, value: node.child.value, negated: true };
        }
        if (node.type === 'rule') {
            return { field: node.field, op: node.op, value: node.value, negated: false };
        }
        return { field: 'rating', op: 'eq', value: 0, negated: false };
    }

    function fromWorkingRule(wr: WorkingRule): FilterRule {
        return { type: 'rule', field: wr.field, op: wr.op, value: wr.value };
    }

    // ── State ─────────────────────────────────────────────────────────────────

    let editingIndex = $state<number | null>(null);
    let draft = $state<WorkingRule>({ field: 'rating', op: 'gte', value: 0, negated: false });

    let group = $derived(normalize(filter));

    // ── Helpers ───────────────────────────────────────────────────────────────

    function formatValue(field: string, op: string, value: any): string {
        if (op === 'this_week') return 'this week';
        if (op === 'this_month') return 'this month';
        if (op === 'is_empty') return '';
        const meta = FIELDS[field];
        if (meta?.type === 'boolean') return value ? 'yes' : 'no';
        if (value === null || value === undefined || value === '') return '';
        return String(value);
    }

    function defaultValue(field: string, op: string): any {
        const meta = FIELDS[field];
        if (!meta) return '';
        if (op === 'this_week' || op === 'this_month' || op === 'is_empty') return null;
        if (meta.type === 'boolean') return true;
        if (meta.type === 'number' || meta.type === 'date') return 0;
        if (meta.type === 'enum' && meta.enumValues?.length) return meta.enumValues[0];
        return '';
    }

    function startEdit(index: number) {
        const rule = group.children[index];
        draft = { ...toWorkingRule(rule) };
        editingIndex = index;
    }

    function confirmEdit() {
        if (editingIndex === null) return;
        const newChildren = group.children.map((child, i) =>
            i === editingIndex ? fromWorkingRule(draft) : child
        );
        editingIndex = null;
        onchange({ ...group, children: newChildren });
    }

    function cancelEdit() {
        editingIndex = null;
    }

    function removeRule(index: number) {
        if (editingIndex === index) editingIndex = null;
        const newChildren = group.children.filter((_, i) => i !== index);
        onchange({ ...group, children: newChildren });
    }

    function addRule() {
        const newRule: FilterRule = { type: 'rule', field: 'rating', op: 'gte', value: 0 };
        const newChildren = [...group.children, newRule];
        const newGroup: FilterGroup = { ...group, children: newChildren };
        onchange(newGroup);
        draft = { field: 'rating', op: 'gte', value: 0, negated: false };
        editingIndex = newChildren.length - 1;
    }

    function toggleGroupOp() {
        onchange({ ...group, op: group.op === 'and' ? 'or' : 'and' });
    }

    function onDraftFieldChange(field: string) {
        const meta = FIELDS[field];
        const op = meta ? meta.ops[0] : 'eq';
        draft = { ...draft, field, op, value: defaultValue(field, op) };
    }

    function onDraftOpChange(op: string) {
        draft = { ...draft, op, value: defaultValue(draft.field, op) };
    }

    function onChipKeydown(e: KeyboardEvent, index: number) {
        if (e.key === 'Enter' || e.key === ' ') { e.preventDefault(); startEdit(index); }
    }

    function onEditKeydown(e: KeyboardEvent) {
        if (e.key === 'Enter') { e.preventDefault(); confirmEdit(); }
        if (e.key === 'Escape') { e.preventDefault(); cancelEdit(); }
    }

    function onEditClickOutside(e: MouseEvent) {
        const target = e.target as HTMLElement;
        if (!target.closest('.chip-edit-container')) confirmEdit();
    }
</script>

<svelte:window onclick={editingIndex !== null ? onEditClickOutside : undefined} />

<div class="rule-builder">
    {#if group.children.length > 1}
        <button
            class="group-op-toggle"
            onclick={toggleGroupOp}
            title="Toggle AND / OR"
        >
            {group.op === 'and' ? 'Match ALL' : 'Match ANY'}
        </button>
    {/if}

    {#each group.children as child, i}
        {@const wr = toWorkingRule(child)}
        {@const color = chipColor(wr.field)}
        {@const meta = FIELDS[wr.field]}

        {#if editingIndex === i}
            <!-- Edit mode: chip expands into inline form -->
            <!-- svelte-ignore a11y_no_noninteractive_tabindex, a11y_no_noninteractive_element_interactions -->
            <div
                class="chip-edit-container"
                style="--chip-color: {color}"
                onkeydown={onEditKeydown}
                role="group"
                aria-label="Edit rule"
                tabindex="0"
            >
                {#if wr.negated}
                    <span class="not-badge">NOT</span>
                {/if}

                <select
                    class="edit-select"
                    value={draft.field}
                    onchange={(e) => onDraftFieldChange((e.target as HTMLSelectElement).value)}
                >
                    {#each Object.entries(FIELDS) as [val, m]}
                        <option value={val}>{m.label}</option>
                    {/each}
                </select>

                <select
                    class="edit-select"
                    value={draft.op}
                    onchange={(e) => onDraftOpChange((e.target as HTMLSelectElement).value)}
                >
                    {#each (FIELDS[draft.field]?.ops ?? []) as op}
                        <option value={op}>{OP_LABELS[op] ?? op}</option>
                    {/each}
                </select>

                {#if draft.op !== 'this_week' && draft.op !== 'this_month' && draft.op !== 'is_empty'}
                    {#if FIELDS[draft.field]?.type === 'boolean'}
                        <select
                            class="edit-select edit-value"
                            value={String(draft.value)}
                            onchange={(e) => { draft = { ...draft, value: (e.target as HTMLSelectElement).value === 'true' }; }}
                        >
                            <option value="true">yes</option>
                            <option value="false">no</option>
                        </select>
                    {:else if FIELDS[draft.field]?.type === 'enum'}
                        <select
                            class="edit-select edit-value"
                            value={draft.value}
                            onchange={(e) => { draft = { ...draft, value: (e.target as HTMLSelectElement).value }; }}
                        >
                            {#each (FIELDS[draft.field]?.enumValues ?? []) as v}
                                <option value={v}>{v}</option>
                            {/each}
                        </select>
                    {:else if FIELDS[draft.field]?.type === 'number' || FIELDS[draft.field]?.type === 'date'}
                        <input
                            class="edit-input edit-value"
                            type="number"
                            min={draft.field === 'rating' ? 0 : undefined}
                            max={draft.field === 'rating' ? 5 : undefined}
                            value={draft.value ?? 0}
                            oninput={(e) => { draft = { ...draft, value: Number((e.target as HTMLInputElement).value) }; }}
                        />
                    {:else}
                        <input
                            class="edit-input edit-value"
                            type="text"
                            value={draft.value ?? ''}
                            oninput={(e) => { draft = { ...draft, value: (e.target as HTMLInputElement).value }; }}
                        />
                    {/if}
                {/if}

                <button class="edit-confirm" onclick={confirmEdit} title="Confirm (Enter)">✓</button>
                <button class="edit-cancel" onclick={cancelEdit} title="Cancel (Esc)">✕</button>
            </div>
        {:else}
            <!-- Display mode: chip -->
            <span
                class="chip"
                role="button"
                tabindex="0"
                style="--chip-color: {color}"
                onclick={() => startEdit(i)}
                onkeydown={(e) => onChipKeydown(e, i)}
                aria-label="Edit rule: {meta?.label ?? wr.field} {OP_LABELS[wr.op] ?? wr.op}"
            >
                {#if wr.negated}
                    <span class="not-badge">NOT</span>
                {/if}
                <span class="chip-field">{meta?.label ?? wr.field}</span>
                <span class="chip-op">{OP_LABELS[wr.op] ?? wr.op}</span>
                {#if formatValue(wr.field, wr.op, wr.value)}
                    <span class="chip-value">{formatValue(wr.field, wr.op, wr.value)}</span>
                {/if}
                <button
                    class="chip-remove"
                    onclick={(e) => { e.stopPropagation(); removeRule(i); }}
                    aria-label="Remove rule"
                >×</button>
            </span>
        {/if}
    {/each}

    <button class="add-rule" onclick={addRule}>+ Add rule</button>
</div>

<style>
    .rule-builder {
        display: flex;
        flex-wrap: wrap;
        gap: 8px;
        align-items: center;
    }

    /* ── Group op toggle ─────────────────────────────────────────────────── */
    .group-op-toggle {
        height: 26px;
        padding: 0 10px;
        border-radius: 999px;
        border: 1px solid var(--border);
        background: var(--surface);
        color: var(--blue);
        font-size: 11px;
        font-weight: 600;
        letter-spacing: 0.04em;
        text-transform: uppercase;
        cursor: pointer;
        transition: border-color 120ms, background 120ms;
    }

    .group-op-toggle:hover {
        border-color: rgba(122, 162, 247, 0.5);
        background: rgba(122, 162, 247, 0.08);
    }

    /* ── Chip (display mode) ─────────────────────────────────────────────── */
    .chip {
        display: inline-flex;
        align-items: center;
        gap: 5px;
        padding: 4px 8px 4px 10px;
        border-radius: 999px;
        font-size: 12px;
        cursor: pointer;
        border: 1px solid color-mix(in srgb, var(--chip-color) 30%, transparent);
        background: color-mix(in srgb, var(--chip-color) 10%, transparent);
        color: var(--text);
        transition: background 120ms, border-color 120ms;
        line-height: 1;
    }

    .chip:hover {
        background: color-mix(in srgb, var(--chip-color) 18%, transparent);
        border-color: color-mix(in srgb, var(--chip-color) 50%, transparent);
    }

    .chip-field {
        font-weight: 600;
        color: var(--chip-color);
    }

    .chip-op {
        color: var(--text-secondary);
        font-size: 11px;
    }

    .chip-value {
        color: var(--text);
    }

    .chip-remove {
        display: inline-flex;
        align-items: center;
        justify-content: center;
        width: 16px;
        height: 16px;
        border-radius: 50%;
        border: none;
        background: transparent;
        color: var(--text-secondary);
        font-size: 13px;
        line-height: 1;
        cursor: pointer;
        padding: 0;
        margin-left: 2px;
        transition: color 120ms, background 120ms;
    }

    .chip-remove:hover {
        color: var(--red);
        background: rgba(247, 118, 142, 0.15);
    }

    /* ── Edit container ──────────────────────────────────────────────────── */
    .chip-edit-container {
        display: inline-flex;
        align-items: center;
        gap: 6px;
        padding: 4px 8px;
        border-radius: 12px;
        border: 1px solid color-mix(in srgb, var(--chip-color) 50%, transparent);
        background: color-mix(in srgb, var(--chip-color) 8%, var(--surface));
        flex-wrap: wrap;
    }

    .edit-select {
        height: 26px;
        padding: 0 6px;
        border-radius: 6px;
        border: 1px solid var(--border);
        background: var(--surface);
        color: var(--text);
        font-size: 12px;
        cursor: pointer;
    }

    .edit-select:focus {
        outline: none;
        border-color: var(--chip-color);
    }

    .edit-input {
        height: 26px;
        padding: 0 8px;
        border-radius: 6px;
        border: 1px solid var(--border);
        background: var(--surface);
        color: var(--text);
        font-size: 12px;
        width: 80px;
    }

    .edit-input:focus {
        outline: none;
        border-color: var(--chip-color);
    }

    .edit-value {
        max-width: 120px;
    }

    .edit-confirm,
    .edit-cancel {
        width: 24px;
        height: 24px;
        border-radius: 50%;
        border: 1px solid var(--border);
        background: var(--surface);
        cursor: pointer;
        font-size: 11px;
        display: inline-flex;
        align-items: center;
        justify-content: center;
        padding: 0;
        transition: color 120ms, border-color 120ms, background 120ms;
        color: var(--text-secondary);
    }

    .edit-confirm:hover {
        color: var(--green);
        border-color: rgba(158, 206, 106, 0.5);
        background: rgba(158, 206, 106, 0.1);
    }

    .edit-cancel:hover {
        color: var(--red);
        border-color: rgba(247, 118, 142, 0.5);
        background: rgba(247, 118, 142, 0.1);
    }

    /* ── NOT badge ───────────────────────────────────────────────────────── */
    .not-badge {
        display: inline-flex;
        align-items: center;
        padding: 1px 6px;
        border-radius: 999px;
        border: 1px solid rgba(224, 175, 104, 0.35);
        background: rgba(224, 175, 104, 0.1);
        color: var(--orange);
        font-size: 10px;
        font-weight: 700;
        letter-spacing: 0.04em;
        text-transform: uppercase;
    }

    /* ── Add rule button ─────────────────────────────────────────────────── */
    .add-rule {
        height: 28px;
        padding: 0 12px;
        border-radius: 999px;
        border: 1px dashed var(--border);
        background: transparent;
        color: var(--text-secondary);
        font-size: 12px;
        cursor: pointer;
        transition: color 120ms, border-color 120ms, background 120ms;
        white-space: nowrap;
    }

    .add-rule:hover {
        color: var(--text);
        border-color: rgba(255, 255, 255, 0.2);
        background: rgba(255, 255, 255, 0.04);
    }
</style>
