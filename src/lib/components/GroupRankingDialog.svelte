<script lang="ts">
    import { untrack } from 'svelte';
    import { convertFileSrc } from '@tauri-apps/api/core';
    import { groupRankingOpen, showToast } from '$lib/stores';
    import {
        listSimilarityGroups,
        listSimilarityGroupImages,
        getImageQuality,
        getAppSetting,
        setAppSetting,
        type SimilarityGroupSummary,
        type ImageWithFile,
    } from '$lib/api';
    import { rankGroupMembers, type GroupMemberInput, type RankedGroup } from '$lib/group-ranking';

    let groups = $state<SimilarityGroupSummary[]>([]);
    let activeGroupId = $state<string | null>(null);
    let groupImages = $state<ImageWithFile[]>([]);
    let ranked = $state<RankedGroup | null>(null);
    let loading = $state(false);

    function overrideKey(groupId: string): string {
        return `group_winner:${groupId}`;
    }

    // Depend only on $groupRankingOpen; untrack loadGroups so its state reads
    // don't make this effect re-run and freeze the dialog's reactivity.
    $effect(() => {
        if ($groupRankingOpen) {
            untrack(() => loadGroups());
        }
    });

    async function loadGroups() {
        loading = true;
        try {
            groups = await listSimilarityGroups(100, 0);
            if (groups.length > 0) {
                await selectGroup(groups[0].id);
            } else {
                activeGroupId = null;
                ranked = null;
            }
        } catch (e) {
            showToast('Failed to load similarity groups', { detail: String(e), type: 'error' });
        } finally {
            loading = false;
        }
    }

    async function selectGroup(groupId: string) {
        activeGroupId = groupId;
        loading = true;
        try {
            groupImages = await listSimilarityGroupImages(groupId);
            const members: GroupMemberInput[] = [];
            for (let i = 0; i < groupImages.length; i += 1) {
                const img = groupImages[i];
                let quality = null;
                try {
                    const q = await getImageQuality(img.image.id);
                    quality = q ? { focus_score: q.focus_score, blur_score: q.blur_score, exposure_score: q.exposure_score } : null;
                } catch (_) {
                    quality = null;
                }
                members.push({
                    imageId: img.image.id,
                    starRating: img.selection?.star_rating ?? 0,
                    decision: (img.selection?.decision as GroupMemberInput['decision']) ?? null,
                    similarityRank: i,
                    quality,
                });
            }
            const override = await getAppSetting(overrideKey(groupId));
            ranked = rankGroupMembers(members, undefined, override || null);
        } catch (e) {
            showToast('Failed to rank group', { detail: String(e), type: 'error' });
        } finally {
            loading = false;
        }
    }

    function imageFor(id: string): ImageWithFile | undefined {
        return groupImages.find(i => i.image.id === id);
    }

    function thumbFor(id: string): string {
        const img = imageFor(id);
        if (!img) return '';
        return convertFileSrc(img.thumbnail_path ?? img.path);
    }

    function nameFor(id: string): string {
        const img = imageFor(id);
        return img ? (img.path.split('/').filter(Boolean).pop() ?? id) : id;
    }

    async function setWinner(imageId: string) {
        if (!activeGroupId) return;
        await setAppSetting(overrideKey(activeGroupId), imageId);
        await selectGroup(activeGroupId);
        showToast('Winner override saved', { type: 'success', duration: 2500 });
    }

    async function clearOverride() {
        if (!activeGroupId) return;
        await setAppSetting(overrideKey(activeGroupId), '');
        await selectGroup(activeGroupId);
    }

    function pct(value: number): string {
        return `${Math.round(value * 100)}%`;
    }

    function close() {
        groupRankingOpen.set(false);
    }

    function onBackdropKeydown(event: KeyboardEvent) {
        if (event.key === 'Escape') close();
    }
</script>

{#if $groupRankingOpen}
    <div
        class="gr-backdrop"
        role="dialog"
        aria-modal="true"
        aria-label="Best of group ranking"
        tabindex="-1"
        onclick={close}
        onkeydown={onBackdropKeydown}
    >
        <!-- svelte-ignore a11y_click_events_have_key_events -->
        <div class="gr-panel" role="presentation" onclick={(e) => e.stopPropagation()}>
            <div class="gr-head">
                <span class="gr-title">Best of Group</span>
                <button class="gr-close" type="button" onclick={close} aria-label="Close">×</button>
            </div>

            {#if groups.length === 0}
                <p class="gr-empty">No similarity groups yet. Generate similarity groups from the Embeddings view first.</p>
            {:else}
                <label class="gr-field">
                    <span>Group</span>
                    <select value={activeGroupId} onchange={(e) => selectGroup((e.target as HTMLSelectElement).value)}>
                        {#each groups as g}
                            <option value={g.id}>{g.image_count} images · {g.method} · {g.model_name}</option>
                        {/each}
                    </select>
                </label>

                {#if ranked}
                    {#if ranked.overriddenWinnerId}
                        <p class="gr-note">
                            Winner overridden to <strong>{nameFor(ranked.overriddenWinnerId)}</strong>.
                            <button class="gr-link" type="button" onclick={clearOverride}>Clear override</button>
                        </p>
                    {/if}
                    <div class="gr-list">
                        {#each ranked.members as m (m.imageId)}
                            <div class="gr-row" class:winner={m.imageId === ranked.effectiveWinnerId}>
                                <img class="gr-thumb" src={thumbFor(m.imageId)} alt={nameFor(m.imageId)} />
                                <div class="gr-info">
                                    <div class="gr-name">
                                        {nameFor(m.imageId)}
                                        {#if m.isOverriddenWinner}<span class="gr-badge override">override</span>
                                        {:else if m.isSuggestedWinner}<span class="gr-badge">suggested</span>{/if}
                                    </div>
                                    <div class="gr-components">
                                        <span title="Rating">R {pct(m.components.rating)}</span>
                                        <span title="Decision">D {pct(m.components.decision)}</span>
                                        <span title="Quality">Q {pct(m.components.quality)}</span>
                                        <span title="Representativeness">Rep {pct(m.components.representativeness)}</span>
                                        <span class="gr-total">Σ {pct(m.score)}</span>
                                    </div>
                                </div>
                                {#if m.imageId !== ranked.effectiveWinnerId}
                                    <button class="gr-set" type="button" onclick={() => setWinner(m.imageId)}>Set winner</button>
                                {/if}
                            </div>
                        {/each}
                    </div>
                    <p class="gr-hint">Ranking is advisory — nothing is deleted or selected automatically.</p>
                {:else if loading}
                    <p class="gr-empty">Ranking…</p>
                {/if}
            {/if}
        </div>
    </div>
{/if}

<style>
    .gr-backdrop {
        position: fixed;
        inset: 0;
        background: rgba(0, 0, 0, 0.55);
        display: flex;
        align-items: flex-start;
        justify-content: center;
        padding-top: 8vh;
        z-index: 1210;
    }
    .gr-panel {
        width: min(560px, 94vw);
        max-height: 82vh;
        overflow-y: auto;
        background: var(--surface);
        border: 1px solid var(--border);
        border-radius: var(--radius);
        box-shadow: 0 18px 60px rgba(0, 0, 0, 0.5);
        padding: calc(var(--spacing) * 2);
        display: flex;
        flex-direction: column;
        gap: var(--spacing);
    }
    .gr-head {
        display: flex;
        align-items: center;
        justify-content: space-between;
    }
    .gr-title {
        font-weight: 600;
        color: var(--text);
    }
    .gr-close {
        background: transparent;
        border: none;
        color: var(--text-secondary);
        font-size: 18px;
        cursor: pointer;
    }
    .gr-empty,
    .gr-note,
    .gr-hint {
        color: var(--text-secondary);
        font-size: 12px;
        margin: 0;
    }
    .gr-field {
        display: flex;
        flex-direction: column;
        gap: 4px;
        color: var(--text-secondary);
        font-size: 12px;
    }
    .gr-field select {
        background: var(--bg);
        border: 1px solid var(--border);
        border-radius: var(--radius);
        color: var(--text);
        padding: 6px var(--spacing);
        font-family: var(--font, monospace);
    }
    .gr-link {
        background: none;
        border: none;
        color: var(--blue);
        cursor: pointer;
        font-size: 12px;
        padding: 0;
    }
    .gr-list {
        display: flex;
        flex-direction: column;
        gap: 6px;
    }
    .gr-row {
        display: flex;
        align-items: center;
        gap: var(--spacing);
        padding: 6px;
        border: 1px solid var(--border);
        border-radius: var(--radius);
    }
    .gr-row.winner {
        border-color: var(--green);
        background: rgba(158, 206, 106, 0.08);
    }
    .gr-thumb {
        width: 56px;
        height: 56px;
        object-fit: cover;
        border-radius: var(--radius);
        background: var(--bg);
        flex-shrink: 0;
    }
    .gr-info {
        flex: 1;
        min-width: 0;
    }
    .gr-name {
        color: var(--text);
        font-size: 13px;
        white-space: nowrap;
        overflow: hidden;
        text-overflow: ellipsis;
    }
    .gr-badge {
        margin-left: 6px;
        font-size: 10px;
        color: var(--green);
        border: 1px solid var(--green);
        border-radius: 3px;
        padding: 0 4px;
    }
    .gr-badge.override {
        color: var(--orange);
        border-color: var(--orange);
    }
    .gr-components {
        display: flex;
        gap: 8px;
        flex-wrap: wrap;
        color: var(--text-secondary);
        font-size: 11px;
        margin-top: 2px;
    }
    .gr-total {
        color: var(--blue);
    }
    .gr-set {
        background: transparent;
        border: 1px solid var(--border);
        color: var(--text-secondary);
        border-radius: var(--radius);
        padding: 4px 8px;
        cursor: pointer;
        font-size: 12px;
        flex-shrink: 0;
    }
    .gr-set:hover {
        color: var(--text);
        border-color: var(--blue);
    }
</style>
