<script lang="ts">
    import { onMount } from 'svelte';
    import { convertFileSrc } from '@tauri-apps/api/core';
    import { redo, undoMany, listUndoHistory, getActivityContext, type SessionEvent, type UndoHistoryEntry } from '$lib/api';
    import { showToast, undoHistoryOpen } from '$lib/stores';

    let loading = $state(false);
    let working = $state(false);
    let error = $state<string | null>(null);
    let undoHistory = $state<UndoHistoryEntry[]>([]);
    let activityEvents = $state<SessionEvent[]>([]);
    let selectedCount = $state(0);

    const undoBackedEventTypes = new Set(['rating_set', 'decision_set', 'image_moved_to_trash']);
    const actionLabel = (actionType: string) => {
        if (actionType === 'set_rating') return 'Set rating';
        if (actionType === 'set_decision') return 'Set decision';
        if (actionType === 'trash_image') return 'Move to Trash';
        return actionType.replace(/_/g, ' ');
    };
    const formatTime = (ts: string) => {
        const d = new Date(ts);
        return Number.isNaN(d.getTime()) ? 'Unknown time' : d.toLocaleString('en-GB', { day: '2-digit', month: 'short', year: 'numeric', hour: '2-digit', minute: '2-digit', hour12: false });
    };
    const parsePayload = (raw: string): Record<string, unknown> => { try { return JSON.parse(raw); } catch { return {}; } };
    const eventTypeLabel = (type: string) => ({ image_deleted_permanently: 'Permanent delete', collection_created: 'Create collection', collection_items_added: 'Add to collection', collection_items_removed: 'Remove from collection', collection_deleted: 'Delete collection', import_completed: 'Import completed', folder_removed_from_library: 'Remove folder' }[type] ?? type.replace(/_/g, ' '));
    const eventLabel = (event: SessionEvent) => {
        const p = parsePayload(event.payload_json);
        const label = p.filename ?? p.name ?? p.new_name ?? p.path ?? p.source;
        return typeof label === 'string' && label.trim() ? label : eventTypeLabel(event.event_type);
    };
    const eventCount = (event: SessionEvent) => {
        const p = parsePayload(event.payload_json);
        const count = p.image_count ?? p.imported ?? p.approved_image_count;
        return typeof count === 'number' ? `${count} image${count === 1 ? '' : 's'}` : (event.subject_type ?? 'event');
    };
    const selectableCount = () => undoHistory.findIndex(entry => !entry.can_undo) < 0 ? undoHistory.length : undoHistory.findIndex(entry => !entry.can_undo);

    function selectThrough(index: number) {
        if (!undoHistory[index]?.can_undo) return;
        selectedCount = selectedCount === index + 1 ? 0 : index + 1;
    }
    function closeHistory() { undoHistoryOpen.set(false); }
    async function loadHistory() {
        loading = true; error = null;
        try {
            const [rows, activity] = await Promise.all([listUndoHistory(40), getActivityContext(null, 40)]);
            undoHistory = rows;
            selectedCount = Math.min(selectedCount, selectableCount());
            activityEvents = activity.recent_events.filter(event => !undoBackedEventTypes.has(event.event_type));
        } catch (e) { error = String(e); } finally { loading = false; }
    }
    async function performUndo() {
        const count = selectedCount || 1;
        if (working || !undoHistory[0]?.can_undo) return;
        working = true;
        try {
            const result = await undoMany(count);
            if (result.completed.length) showToast(`Undone ${result.completed.length} action${result.completed.length === 1 ? '' : 's'}`, { type: 'info', duration: 3500 });
            if (result.failure) showToast('Some actions could not be undone', { detail: result.failure, type: 'error', duration: 6000 });
            selectedCount = 0; await loadHistory(); window.dispatchEvent(new CustomEvent('reload-images'));
        } catch (e) { showToast('Undo failed', { detail: String(e), type: 'error', duration: 6000 }); } finally { working = false; }
    }
    async function performRedo() {
        if (working) return; working = true;
        try { const label = await redo(); if (label) { showToast(`Redone: ${label}`, { type: 'info' }); await loadHistory(); window.dispatchEvent(new CustomEvent('reload-images')); } }
        catch (e) { showToast('Redo failed', { detail: String(e), type: 'error' }); } finally { working = false; }
    }
    $effect(() => { if ($undoHistoryOpen) loadHistory(); });
    onMount(() => { const onReload = () => { if ($undoHistoryOpen) loadHistory(); }; window.addEventListener('reload-images', onReload); window.addEventListener('session-events-refresh', onReload); return () => { window.removeEventListener('reload-images', onReload); window.removeEventListener('session-events-refresh', onReload); }; });
</script>

{#if $undoHistoryOpen}
<div class="history-backdrop" role="dialog" aria-modal="true" aria-label="Action history" tabindex="-1" onclick={closeHistory} onkeydown={(e) => e.key === 'Escape' && closeHistory()}>
<section class="history-panel" role="presentation" onclick={(e) => e.stopPropagation()}>
    <header class="history-head"><div><h2>Action History</h2><p>Choose one or more recent actions to undo</p></div><div class="head-actions"><button onclick={loadHistory} disabled={loading}>Refresh</button><button onclick={closeHistory} aria-label="Close history">×</button></div></header>
    <div class="history-toolbar"><button class="undo" onclick={performUndo} disabled={working || !undoHistory[0]?.can_undo}>{selectedCount ? `Undo ${selectedCount} actions` : 'Undo latest'}</button><button class="redo" onclick={performRedo} disabled={working}>Redo</button><span>{selectedCount ? `${selectedCount} newest actions selected` : 'Select a row to undo through it'}</span></div>
    {#if loading}<p class="history-state">Loading history…</p>
    {:else if error}<p class="history-state error">Failed to load history: {error}</p>
    {:else if undoHistory.length === 0 && activityEvents.length === 0}<div class="history-empty" role="status"><svg class="history-empty-image" viewBox="0 0 160 112" role="img" aria-label="Empty action timeline"><rect x="18" y="18" width="124" height="76" rx="4"/><path d="M46 36h68M46 56h48M46 76h58"/></svg><h3>No undoable actions yet</h3><p>Recorded actions will appear here.</p></div>
    {:else}<div class="history-list" role="list">
        {#if undoHistory.length}<h3 class="history-section-title">Undoable actions</h3>{#each undoHistory as entry, index (entry.record.id)}
        <button class:selected={index < selectedCount} class="history-item" class:disabled={!entry.can_undo} onclick={() => selectThrough(index)} disabled={!entry.can_undo} aria-pressed={index < selectedCount}>
            <span class="selection-mark">{index < selectedCount ? '✓' : index + 1}</span>
            <span class="preview">{#if entry.previews[0]?.thumbnail_path}<img src={convertFileSrc(entry.previews[0].thumbnail_path)} alt=""/>{:else}<span>IMG</span>{/if}{#if entry.affected_count > 1}<b>+{entry.affected_count - 1}</b>{/if}</span>
            <span class="history-summary"><strong>{entry.action_title || actionLabel(entry.record.action_type)}</strong><span class:unavailable={entry.target.unavailable}>{entry.target.display_name}</span>{#if entry.target.context}<small>{entry.target.context}</small>{/if}</span>
            <span class="change">{entry.change_summary ?? entry.record.label}</span>
            <span class="history-count">{entry.affected_count} image{entry.affected_count === 1 ? '' : 's'}</span>
            <time class="history-time" datetime={entry.record.created_at}>{formatTime(entry.record.created_at)}</time>
        </button>{/each}{/if}
        {#if activityEvents.length}<h3 class="history-section-title">Critical activity</h3>{#each activityEvents as event (event.id)}<article class="history-item activity" role="listitem"><span></span><span class="preview"><span>ACT</span></span><span class="history-summary"><strong>{eventTypeLabel(event.event_type)}</strong><span>{eventLabel(event)}</span></span><span class="change">Recorded activity</span><span class="history-count">{eventCount(event)}</span><time class="history-time">{formatTime(event.created_at)}</time></article>{/each}{/if}
    </div>{/if}
</section></div>{/if}

<style>
/* History rows must retain their readable height: flex: 0 0 auto; */
.history-backdrop{position:fixed;inset:0;background:rgba(0,0,0,.58);display:flex;align-items:flex-start;justify-content:center;padding:8vh 16px 24px;z-index:var(--z-modal);color:var(--text)}
.history-panel{width:min(1120px,100%);max-height:84vh;background:var(--surface);border:1px solid var(--border);border-radius:var(--radius);padding:14px;display:flex;flex-direction:column;gap:12px;box-shadow:0 20px 60px rgba(0,0,0,.55);overflow:hidden}
.history-head{display:flex;justify-content:space-between;align-items:flex-start;gap:12px}.history-head h2{margin:0;font-size:16px}.history-head p{margin:3px 0 0;color:var(--text-secondary);font-size:11px}.head-actions,.history-toolbar{display:flex;align-items:center;gap:8px}button{font:inherit}.head-actions button,.history-toolbar button{height:28px;padding:0 10px;color:var(--text);background:var(--surface);border:1px solid var(--border);border-radius:var(--radius);cursor:pointer}.head-actions button:hover,.history-toolbar button:hover{border-color:var(--blue)}.history-toolbar .undo{color:var(--green)}.history-toolbar .redo{color:var(--blue)}.history-toolbar span{color:var(--text-secondary);font-size:10px}.history-toolbar button:disabled{opacity:.45;cursor:not-allowed}
.history-state{margin:0;color:var(--text-secondary)}.error{color:var(--red)}.history-empty{min-height:260px;display:grid;place-items:center;text-align:center;border:1px solid var(--border);background:var(--bg)}.history-empty-image{width:160px;fill:none;stroke:var(--text-secondary);stroke-width:2}.history-empty h3,.history-empty p{margin:0}.history-list{overflow:auto;display:flex;flex-direction:column;gap:7px;min-height:0}.history-section-title{flex:0 0 auto;margin:6px 0 1px;color:var(--text-secondary);font-size:10px;text-transform:uppercase;letter-spacing:.5px}
.history-item{flex:0 0 auto;width:100%;display:grid;grid-template-columns:24px 58px minmax(180px,1.4fr) minmax(160px,1fr) 80px 145px;gap:12px;align-items:start;min-height:76px;padding:10px;text-align:left;color:var(--text);background:var(--bg);border:1px solid var(--border);border-radius:var(--radius);box-sizing:border-box;transition:background .12s,border-color .12s,transform .12s;cursor:pointer}.history-item:hover{background:color-mix(in srgb,var(--surface) 72%,var(--blue) 28%);border-color:var(--blue);transform:translateY(-1px)}.history-item:focus-visible{outline:2px solid var(--blue);outline-offset:1px}.history-item.selected{background:color-mix(in srgb,var(--surface) 78%,var(--green) 22%);border-color:var(--green)}.history-item.disabled{cursor:default;opacity:.62}.history-item.activity{cursor:default}.history-item.activity:hover{transform:none;border-color:var(--border)}
.selection-mark{display:grid;place-items:center;width:20px;height:20px;border:1px solid var(--border);border-radius:50%;color:var(--text-secondary);font-size:10px}.selected .selection-mark{color:var(--bg);background:var(--green);border-color:var(--green)}.preview{position:relative;width:56px;height:56px;display:grid;place-items:center;background:var(--surface);border:1px solid var(--border);border-radius:var(--radius);overflow:hidden;color:var(--text-secondary);font-size:9px}.preview img{width:100%;height:100%;object-fit:cover}.preview b{position:absolute;right:2px;bottom:2px;padding:1px 3px;background:var(--bg);color:var(--text);font-size:9px}.history-summary{display:flex;flex-direction:column;align-items:flex-start;gap:4px;min-width:0}.history-summary strong{color:var(--blue);font-size:10px;text-transform:uppercase}.history-summary span{max-width:100%;overflow-wrap:anywhere}.history-summary small,.change,.history-count,.history-time{color:var(--text-secondary);font-size:10px;line-height:1.45}.unavailable{color:var(--orange)!important}.change{color:var(--text)}
@media(max-width:850px){.history-backdrop{padding:0}.history-panel{max-height:100vh;border-radius:0}.history-item{grid-template-columns:22px 50px minmax(140px,1fr) 120px}.history-count,.history-time{display:none}.preview{width:48px;height:48px}}
</style>
