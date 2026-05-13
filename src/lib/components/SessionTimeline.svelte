<script lang="ts">
    import { listSessionEvents, type SessionEvent } from '$lib/api';
    import { onMount } from 'svelte';

    let { sessionId } = $props<{ sessionId: string }>();
    let events = $state<SessionEvent[]>([]);
    let loading = $state(false);
    let loadedSessionId = $state<string | null>(null);

    async function loadEvents() {
        if (!sessionId || loading) return;
        loading = true;
        try {
            events = await listSessionEvents(sessionId, 8);
            loadedSessionId = sessionId;
        } catch (e) {
            console.error('Failed to load session timeline:', e);
        } finally {
            loading = false;
        }
    }

    function payload(event: SessionEvent): Record<string, unknown> {
        try {
            return JSON.parse(event.payload_json || '{}');
        } catch {
            return {};
        }
    }

    function eventLabel(event: SessionEvent): string {
        const data = payload(event);
        switch (event.event_type) {
            case 'session_created':
                return 'Session created';
            case 'canvas_created':
                return `Canvas: ${String(data.name ?? 'New Canvas')}`;
            case 'canvas_layout_updated':
                return 'Canvas layout saved';
            case 'import_completed':
                return `Import: ${Number(data.imported ?? 0)} added`;
            case 'rating_set':
                return `Rated ${String(data.rating ?? '')}`;
            case 'decision_set':
                return `Decision: ${String(data.decision ?? '')}`;
            default:
                return event.event_type.replaceAll('_', ' ');
        }
    }

    function eventMeta(event: SessionEvent): string {
        const date = new Date(event.created_at);
        const when = Number.isNaN(date.getTime()) ? event.created_at : date.toLocaleString();
        return `${event.actor_type} / ${when}`;
    }

    onMount(() => {
        loadEvents();
        const handler = () => loadEvents();
        window.addEventListener('session-events-refresh', handler);
        return () => window.removeEventListener('session-events-refresh', handler);
    });

    $effect(() => {
        if (sessionId && sessionId !== loadedSessionId) {
            loadEvents();
        }
    });
</script>

<div class="section timeline-section">
    <div class="section-header">
        <span>TIMELINE</span>
        <button class="section-action" onclick={loadEvents} disabled={loading} title="Refresh timeline">r</button>
    </div>

    {#if events.length === 0}
        <div class="section-empty">{loading ? 'Loading...' : 'No activity yet'}</div>
    {:else}
        {#each events as event}
            <div class="timeline-item">
                <span class="timeline-dot"></span>
                <span class="timeline-body">
                    <span class="timeline-label">{eventLabel(event)}</span>
                    <span class="timeline-meta">{eventMeta(event)}</span>
                </span>
            </div>
        {/each}
    {/if}
</div>

<style>
    .timeline-section {
        border-top: 1px solid var(--border);
    }

    .section {
        padding: var(--spacing);
    }

    .section-header {
        font-size: 10px;
        font-weight: 700;
        color: var(--text-secondary);
        text-transform: uppercase;
        margin-bottom: 8px;
        display: flex;
        justify-content: space-between;
        align-items: center;
    }

    .section-empty {
        color: var(--text-secondary);
        font-size: 10px;
        padding: 4px 0;
    }

    .section-action {
        background: none;
        border: none;
        color: var(--text-secondary);
        cursor: pointer;
        font: inherit;
        padding: 0 2px;
    }

    .section-action:hover {
        color: var(--blue);
    }

    .section-action:disabled {
        cursor: default;
        opacity: 0.5;
    }

    .timeline-item {
        display: grid;
        grid-template-columns: 8px 1fr;
        gap: 8px;
        padding: 5px 0;
        min-width: 0;
    }

    .timeline-dot {
        width: 5px;
        height: 5px;
        margin-top: 5px;
        border-radius: 50%;
        background: var(--blue);
    }

    .timeline-body {
        display: flex;
        flex-direction: column;
        gap: 2px;
        min-width: 0;
    }

    .timeline-label {
        color: var(--text);
        font-size: 11px;
        overflow: hidden;
        text-overflow: ellipsis;
        white-space: nowrap;
    }

    .timeline-meta {
        color: var(--text-secondary);
        font-size: 9px;
        overflow: hidden;
        text-overflow: ellipsis;
        white-space: nowrap;
    }
</style>
