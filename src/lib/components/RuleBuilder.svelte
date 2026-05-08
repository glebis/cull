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
    .rule-builder { display: flex; flex-direction: column; gap: 6px; }
    .match-toggle { display: flex; align-items: center; gap: 6px; margin-bottom: 4px; }
    .match-label { font-size: 13px; color: var(--muted, #757575); }
    .match-select { padding: 2px 8px; border-radius: 4px; border: 1px solid var(--accent, #4a9eed); font-size: 13px; color: var(--accent, #4a9eed); }
    .rule-row { display: flex; align-items: center; gap: 6px; }
    .field-select { padding: 4px 8px; border-radius: 6px; background: #d0bfff; border: 1px solid #8b5cf6; font-size: 13px; }
    .op-select { padding: 4px 8px; border-radius: 6px; background: #fff; border: 1px solid #ccc; font-size: 13px; }
    .value-input { padding: 4px 8px; border-radius: 6px; background: #fff; border: 1px solid #ccc; font-size: 13px; width: 120px; }
    .not-badge { padding: 2px 6px; border-radius: 4px; background: #ffc9c9; border: 1px solid #ef4444; font-size: 11px; font-weight: bold; color: #dc2626; }
    .remove-rule { background: none; border: none; color: #999; cursor: pointer; font-size: 16px; }
    .add-rule { background: none; border: 1px dashed var(--accent, #4a9eed); border-radius: 6px; color: var(--accent, #4a9eed); padding: 4px 12px; cursor: pointer; font-size: 13px; width: fit-content; }
</style>
