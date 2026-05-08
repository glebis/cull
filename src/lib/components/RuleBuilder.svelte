<script lang="ts">
    import type { FilterNode } from '$lib/api';

    interface Props { filter: FilterNode; }
    let { filter }: Props = $props();

    const FIELD_OPTIONS: [string, string][] = [
        ['rating', 'Rating'], ['color_label', 'Color Label'], ['decision', 'Decision'],
        ['format', 'Format'], ['width', 'Width'], ['height', 'Height'],
        ['orientation', 'Orientation'], ['source_label', 'Source'],
        ['is_ai_generated', 'AI Generated'], ['imported_at', 'Imported'],
        ['ai_prompt', 'Prompt'], ['aspect_ratio', 'Aspect Ratio'],
    ];

    const OP_OPTIONS: [string, string][] = [
        ['eq', 'is'], ['neq', 'is not'], ['gt', '>'], ['gte', '>='],
        ['lt', '<'], ['lte', '<='], ['contains', 'contains'],
        ['last_n_days', 'in last N days'], ['this_week', 'this week'],
        ['this_month', 'this month'], ['is_empty', 'is empty'],
    ];

    function getRules(node: FilterNode): FilterNode[] {
        if (node.type === 'group') return node.children;
        if (node.type === 'rule') return [node];
        return [];
    }

    function getGroupOp(node: FilterNode): string {
        return node.type === 'group' ? node.op : 'and';
    }

    function formatValue(value: any): string {
        if (typeof value === 'boolean') return value ? 'Yes' : 'No';
        if (typeof value === 'number') return String(value);
        if (typeof value === 'string') return value;
        if (Array.isArray(value)) return value.join(', ');
        return JSON.stringify(value);
    }

    function getRuleData(node: FilterNode): { field: string; op: string; value: any; negated: boolean } {
        if (node.type === 'not' && node.child.type === 'rule') {
            return { field: node.child.field, op: node.child.op, value: node.child.value, negated: true };
        }
        if (node.type === 'rule') {
            return { field: node.field, op: node.op, value: node.value, negated: false };
        }
        return { field: '', op: '', value: '', negated: false };
    }
</script>

<div class="rule-builder">
    <div class="match-toggle">
        <span class="match-label">Match</span>
        <select class="match-select" value={getGroupOp(filter)}>
            <option value="and">All (AND)</option>
            <option value="or">Any (OR)</option>
        </select>
    </div>

    {#each getRules(filter) as rule}
        {@const data = getRuleData(rule)}
        <div class="rule-row">
            {#if data.negated}
                <span class="not-badge">NOT</span>
            {/if}
            <select class="field-select" value={data.field}>
                {#each FIELD_OPTIONS as [val, label]}
                    <option value={val}>{label}</option>
                {/each}
            </select>
            <select class="op-select" value={data.op}>
                {#each OP_OPTIONS as [val, label]}
                    <option value={val}>{label}</option>
                {/each}
            </select>
            <input class="value-input" type="text" value={formatValue(data.value)} />
            <button class="remove-rule" onclick={() => {}}>&times;</button>
        </div>
    {/each}
    <button class="add-rule" onclick={() => {}}>+ Add rule</button>
</div>

<style>
    .rule-builder {
        display: flex;
        flex-direction: column;
        gap: 10px;
    }

    .match-toggle {
        display: inline-flex;
        align-items: center;
        gap: 8px;
        margin-bottom: 4px;
    }

    .match-label {
        font-size: 12px;
        font-weight: 500;
        color: var(--text-secondary);
        text-transform: uppercase;
        letter-spacing: 0.04em;
    }

    .match-select {
        appearance: none;
        height: 30px;
        padding: 0 28px 0 10px;
        border-radius: 6px;
        border: 1px solid var(--border);
        background: linear-gradient(180deg, rgba(255,255,255,0.04), rgba(255,255,255,0.015)), var(--surface);
        background-image: linear-gradient(45deg, transparent 50%, var(--text-secondary) 50%),
            linear-gradient(135deg, var(--text-secondary) 50%, transparent 50%),
            linear-gradient(180deg, rgba(255,255,255,0.04), rgba(255,255,255,0.015));
        background-position: calc(100% - 12px) 13px, calc(100% - 7px) 13px, 0 0;
        background-size: 5px 5px, 5px 5px, 100% 100%;
        background-repeat: no-repeat;
        background-color: var(--surface);
        color: var(--blue);
        font-size: 13px;
        font-weight: 500;
        cursor: pointer;
        transition: border-color 120ms, background-color 120ms;
        box-shadow: inset 0 1px 0 rgba(255,255,255,0.04);
    }

    .match-select:hover {
        border-color: rgba(122, 162, 247, 0.4);
    }

    .match-select:focus {
        outline: none;
        border-color: var(--blue);
        box-shadow: 0 0 0 1px var(--blue), 0 0 0 4px rgba(122, 162, 247, 0.12);
    }

    .rule-row {
        display: flex;
        align-items: center;
        gap: 8px;
    }

    .field-select,
    .op-select {
        appearance: none;
        height: 36px;
        padding: 0 28px 0 12px;
        border: 1px solid var(--border);
        border-radius: 6px;
        background: linear-gradient(180deg, rgba(255,255,255,0.04), rgba(255,255,255,0.015)), var(--surface);
        background-image: linear-gradient(45deg, transparent 50%, var(--text-secondary) 50%),
            linear-gradient(135deg, var(--text-secondary) 50%, transparent 50%),
            linear-gradient(180deg, rgba(255,255,255,0.04), rgba(255,255,255,0.015));
        background-position: calc(100% - 12px) 15px, calc(100% - 7px) 15px, 0 0;
        background-size: 5px 5px, 5px 5px, 100% 100%;
        background-repeat: no-repeat;
        background-color: var(--surface);
        color: var(--text);
        font-size: 13px;
        font-weight: 500;
        cursor: pointer;
        box-shadow: inset 0 1px 0 rgba(255,255,255,0.04);
        transition: border-color 120ms, background-color 120ms;
    }

    .field-select {
        color: var(--purple);
        border-color: rgba(187, 154, 247, 0.3);
    }

    .field-select:hover {
        border-color: rgba(187, 154, 247, 0.5);
        background-color: rgba(187, 154, 247, 0.06);
    }

    .op-select:hover {
        border-color: var(--border);
        background-color: rgba(255, 255, 255, 0.03);
    }

    .field-select:focus,
    .op-select:focus {
        outline: none;
        border-color: var(--blue);
        box-shadow: 0 0 0 1px var(--blue), 0 0 0 4px rgba(122, 162, 247, 0.12);
    }

    .value-input {
        height: 36px;
        padding: 0 12px;
        border: 1px solid var(--border);
        border-radius: 6px;
        background: linear-gradient(180deg, rgba(255,255,255,0.04), rgba(255,255,255,0.015)), var(--surface);
        color: var(--text);
        font-size: 13px;
        font-weight: 500;
        width: 140px;
        box-shadow: inset 0 1px 0 rgba(255,255,255,0.04);
        transition: border-color 120ms;
    }

    .value-input::placeholder {
        color: var(--text-secondary);
        opacity: 0.5;
    }

    .value-input:hover {
        border-color: rgba(255, 255, 255, 0.12);
    }

    .value-input:focus {
        outline: none;
        border-color: var(--blue);
        box-shadow: 0 0 0 1px var(--blue), 0 0 0 4px rgba(122, 162, 247, 0.12);
    }

    .not-badge {
        height: 26px;
        display: inline-flex;
        align-items: center;
        justify-content: center;
        padding: 0 8px;
        border-radius: 999px;
        border: 1px solid rgba(224, 175, 104, 0.35);
        background: rgba(224, 175, 104, 0.1);
        color: var(--orange);
        font-size: 11px;
        font-weight: 600;
        letter-spacing: 0.02em;
        text-transform: uppercase;
        font-family: var(--font);
    }

    .remove-rule {
        width: 36px;
        height: 36px;
        display: inline-flex;
        align-items: center;
        justify-content: center;
        background: linear-gradient(180deg, rgba(255,255,255,0.04), rgba(255,255,255,0.015)), var(--surface);
        border: 1px solid var(--border);
        border-radius: 6px;
        color: var(--text-secondary);
        cursor: pointer;
        font-size: 14px;
        transition: color 120ms, border-color 120ms, background 120ms, transform 80ms;
    }

    .remove-rule:hover {
        color: var(--red);
        border-color: rgba(247, 118, 142, 0.4);
        background: rgba(247, 118, 142, 0.08);
    }

    .remove-rule:active {
        transform: translateY(1px);
    }

    .add-rule {
        height: 36px;
        display: inline-flex;
        align-items: center;
        gap: 6px;
        padding: 0 16px;
        border: 1px solid var(--border);
        border-radius: 6px;
        background: linear-gradient(180deg, rgba(255,255,255,0.04), rgba(255,255,255,0.015)), var(--surface);
        color: var(--text);
        font-size: 13px;
        font-weight: 500;
        cursor: pointer;
        width: fit-content;
        box-shadow: inset 0 1px 0 rgba(255,255,255,0.04);
        transition: border-color 120ms, background-color 120ms, transform 80ms;
    }

    .add-rule:hover {
        border-color: rgba(255, 255, 255, 0.12);
        background-color: rgba(255, 255, 255, 0.03);
    }

    .add-rule:active {
        transform: translateY(1px);
    }
</style>
