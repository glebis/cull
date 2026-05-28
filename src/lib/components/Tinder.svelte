<script lang="ts">
    import { convertFileSrc } from '@tauri-apps/api/core';
    import { listen } from '@tauri-apps/api/event';
    import { images, focusedIndex, statusHint, navigateTo, activeSession } from '$lib/stores';
    import { setDecision, isRawFormat } from '$lib/api';
    import { onMount, onDestroy } from 'svelte';
    import type { ImageWithFile } from '$lib/api';
    import { parseVoiceCommand, filterByDecision } from '$lib/tinder-utils';
    import { invalidateImageCache } from '$lib/image-loading';
    import { withDecision, type ImageDecision } from '$lib/selection-updates';

    let pairIndex = $state(0);
    let swipeDirection = $state<'left' | 'right' | 'skip' | null>(null);
    let animating = $state(false);
    let done = $state(false);
    let stats = $state({ accepted: 0, rejected: 0, skipped: 0 });
    let history = $state<Array<{ index: number; leftId: string; rightId: string; choice: string }>>([]);

    let pair = $derived.by(() => {
        const imgs = $images;
        const base = pairIndex * 2;
        const left = imgs[base] ?? null;
        const right = imgs[base + 1] ?? null;
        return [left, right] as const;
    });

    let totalPairs = $derived(Math.ceil($images.length / 2));
    let leftImage = $derived(pair[0]);
    let rightImage = $derived(pair[1]);
    let leftSrc = $derived(leftImage ? (isRawFormat(leftImage.image.format) ? convertFileSrc(leftImage.thumbnail_path ?? leftImage.path) : convertFileSrc(leftImage.path)) : '');
    let rightSrc = $derived(rightImage ? (isRawFormat(rightImage.image.format) ? convertFileSrc(rightImage.thumbnail_path ?? rightImage.path) : convertFileSrc(rightImage.path)) : '');

    $effect(() => {
        if (done) {
            statusHint.set(`Done! ${stats.accepted} accepted, ${stats.rejected} rejected, ${stats.skipped} skipped`);
        } else {
            const current = pairIndex + 1;
            statusHint.set(`Tinder ${current}/${totalPairs} — ← accept left, → accept right, ↓ skip, Z undo`);
        }
        return () => statusHint.set(null);
    });

    function updateImageDecision(imageId: string, decision: ImageDecision) {
        images.update(all => all.map(img => img.image.id === imageId ? withDecision(img, decision) : img));
    }

    function choose(side: 'left' | 'right' | 'skip') {
        if (animating || done || !leftImage || !rightImage) return;
        swipeDirection = side;
        animating = true;

        const leftId = leftImage.image.id;
        const rightId = rightImage.image.id;
        const sessionId = $activeSession?.id ?? null;

        if (side === 'left') {
            setDecision(leftId, 'accept', sessionId).catch(console.error);
            setDecision(rightId, 'reject', sessionId).catch(console.error);
            updateImageDecision(leftId, 'accept');
            updateImageDecision(rightId, 'reject');
            stats.accepted++;
            stats.rejected++;
        } else if (side === 'right') {
            setDecision(rightId, 'accept', sessionId).catch(console.error);
            setDecision(leftId, 'reject', sessionId).catch(console.error);
            updateImageDecision(rightId, 'accept');
            updateImageDecision(leftId, 'reject');
            stats.accepted++;
            stats.rejected++;
        } else {
            stats.skipped += 2;
        }
        if (side !== 'skip') invalidateImageCache();

        history.push({ index: pairIndex, leftId, rightId, choice: side });

        setTimeout(() => {
            swipeDirection = null;
            animating = false;
            if (pairIndex < totalPairs - 1) {
                pairIndex++;
                focusedIndex.set(pairIndex * 2);
            } else {
                done = true;
            }
        }, 350);
    }

    function viewAccepted() {
        const ids = filterByDecision(history, 'accepted');
        const filtered = $images.filter(img => ids.has(img.image.id));
        if (filtered.length > 0) {
            $images = filtered;
            focusedIndex.set(0);
            navigateTo('grid');
        }
    }

    function viewRejected() {
        const ids = filterByDecision(history, 'rejected');
        const filtered = $images.filter(img => ids.has(img.image.id));
        if (filtered.length > 0) {
            $images = filtered;
            focusedIndex.set(0);
            navigateTo('grid');
        }
    }

    function startOver() {
        pairIndex = 0;
        done = false;
        stats = { accepted: 0, rejected: 0, skipped: 0 };
        history = [];
        focusedIndex.set(0);
    }

    function goToGrid() {
        focusedIndex.set(0);
        navigateTo('grid');
    }

    function undo() {
        const last = history.pop();
        if (!last) return;
        pairIndex = last.index;
        focusedIndex.set(pairIndex * 2);
        const sessionId = $activeSession?.id ?? null;
        setDecision(last.leftId, 'undecided', sessionId).catch(console.error);
        setDecision(last.rightId, 'undecided', sessionId).catch(console.error);
        updateImageDecision(last.leftId, 'undecided');
        updateImageDecision(last.rightId, 'undecided');
        invalidateImageCache();
    }

    let unlisteners: Array<() => void> = [];

    onMount(async () => {
        pairIndex = Math.floor($focusedIndex / 2);

        const unChoose = await listen<string>('tinder-choose', () => {});
        // Use window events instead (dispatched from keys.ts)
        unChoose();

        const handleChoose = (e: Event) => {
            const detail = (e as CustomEvent).detail;
            choose(detail);
        };
        const handleUndo = () => undo();

        window.addEventListener('tinder-choose', handleChoose);
        window.addEventListener('tinder-undo', handleUndo);
        unlisteners.push(
            () => window.removeEventListener('tinder-choose', handleChoose),
            () => window.removeEventListener('tinder-undo', handleUndo),
        );

        // Listen for dictation results for voice commands
        const unDictation = await listen<{ text: string; is_final: boolean }>('dictation-result', (event) => {
            if (!event.payload.is_final) return;
            const cmd = parseVoiceCommand(event.payload.text);
            if (cmd === 'undo') undo();
            else if (cmd) choose(cmd);
        });
        unlisteners.push(unDictation);
    });

    onDestroy(() => {
        unlisteners.forEach(fn => fn());
    });

    let touchStartX = 0;
    function handleTouchStart(e: TouchEvent) {
        touchStartX = e.touches[0].clientX;
    }
    function handleTouchEnd(e: TouchEvent) {
        const dx = e.changedTouches[0].clientX - touchStartX;
        if (Math.abs(dx) > 80) {
            choose(dx < 0 ? 'left' : 'right');
        }
    }
</script>

<div
    class="tinder-container"
    ontouchstart={handleTouchStart}
    ontouchend={handleTouchEnd}
    role="application"
    aria-label="Tinder comparison mode"
>
    {#if done}
        <div class="done-screen">
            <div class="done-icon">&#10003;</div>
            <h2 class="done-title">All pairs reviewed</h2>
            <div class="done-stats">
                <div class="stat">
                    <span class="stat-value accepted">{stats.accepted}</span>
                    <span class="stat-label">accepted</span>
                </div>
                <div class="stat">
                    <span class="stat-value rejected">{stats.rejected}</span>
                    <span class="stat-label">rejected</span>
                </div>
                <div class="stat">
                    <span class="stat-value skipped">{stats.skipped}</span>
                    <span class="stat-label">skipped</span>
                </div>
            </div>
            <div class="done-actions">
                <button class="action-btn primary" onclick={viewAccepted}>
                    View accepted
                </button>
                <button class="action-btn" onclick={viewRejected}>
                    View rejected
                </button>
                <button class="action-btn" onclick={goToGrid}>
                    Back to grid
                </button>
                <button class="action-btn subtle" onclick={startOver}>
                    Start over
                </button>
            </div>
        </div>
    {:else}
        <div class="tinder-pair" class:swipe-left={swipeDirection === 'left'} class:swipe-right={swipeDirection === 'right'} class:swipe-skip={swipeDirection === 'skip'}>
            <div class="tinder-card left" class:winner={swipeDirection === 'left'} class:loser={swipeDirection === 'right'}>
                {#if leftImage}
                    <img src={leftSrc} alt={leftImage.path.split('/').pop()} draggable="false" />
                    <div class="card-label">{leftImage.path.split('/').pop()}</div>
                    <button class="choose-btn" onclick={() => choose('left')} aria-label="Choose left">
                        <span class="choose-icon">&#10003;</span>
                    </button>
                {/if}
            </div>

            <div class="tinder-vs">VS</div>

            <div class="tinder-card right" class:winner={swipeDirection === 'right'} class:loser={swipeDirection === 'left'}>
                {#if rightImage}
                    <img src={rightSrc} alt={rightImage.path.split('/').pop()} draggable="false" />
                    <div class="card-label">{rightImage.path.split('/').pop()}</div>
                    <button class="choose-btn" onclick={() => choose('right')} aria-label="Choose right">
                        <span class="choose-icon">&#10003;</span>
                    </button>
                {/if}
            </div>
        </div>

        <div class="tinder-controls">
            <button class="ctrl-btn undo" onclick={undo} disabled={history.length === 0}>
                Z Undo
            </button>
            <button class="ctrl-btn skip" onclick={() => choose('skip')}>
                ↓ Skip
            </button>
            <div class="progress">
                {pairIndex + 1} / {totalPairs}
            </div>
        </div>
    {/if}
</div>

<style>
    .tinder-container {
        display: flex;
        flex-direction: column;
        align-items: center;
        justify-content: center;
        height: 100%;
        width: 100%;
        padding: 24px;
        gap: 24px;
        user-select: none;
    }

    .tinder-pair {
        display: flex;
        align-items: center;
        gap: 24px;
        flex: 1;
        min-height: 0;
        width: 100%;
        max-width: 1200px;
        transition: opacity 200ms ease, transform 300ms ease;
    }

    .tinder-pair.swipe-left {
        transform: translateX(-40px);
        opacity: 0.7;
    }

    .tinder-pair.swipe-right {
        transform: translateX(40px);
        opacity: 0.7;
    }

    .tinder-pair.swipe-skip {
        transform: translateY(30px);
        opacity: 0.5;
    }

    .tinder-card {
        flex: 1;
        display: flex;
        flex-direction: column;
        align-items: center;
        justify-content: center;
        position: relative;
        height: 100%;
        min-width: 0;
        border-radius: 12px;
        border: 2px solid var(--border);
        overflow: hidden;
        background: var(--surface);
        transition: border-color 200ms ease, transform 200ms ease, box-shadow 200ms ease;
    }

    .tinder-card:hover {
        border-color: var(--blue);
        cursor: pointer;
    }

    .tinder-card.winner {
        border-color: var(--green);
        box-shadow: 0 0 0 3px rgba(158, 206, 106, 0.3);
        transform: scale(1.02);
    }

    .tinder-card.loser {
        border-color: var(--red);
        opacity: 0.5;
        transform: scale(0.97);
    }

    .tinder-card img {
        max-width: 100%;
        max-height: calc(100% - 48px);
        object-fit: contain;
    }

    .card-label {
        position: absolute;
        bottom: 0;
        left: 0;
        right: 0;
        padding: 8px 12px;
        background: rgba(0, 0, 0, 0.7);
        color: var(--text-secondary);
        font-size: 11px;
        text-align: center;
        overflow: hidden;
        text-overflow: ellipsis;
        white-space: nowrap;
    }

    .choose-btn {
        position: absolute;
        bottom: 48px;
        left: 50%;
        transform: translateX(-50%);
        width: 48px;
        height: 48px;
        border-radius: 50%;
        border: 2px solid var(--green);
        background: rgba(158, 206, 106, 0.15);
        color: var(--green);
        font-size: 20px;
        cursor: pointer;
        display: flex;
        align-items: center;
        justify-content: center;
        opacity: 0;
        transition: opacity 150ms ease, transform 100ms ease;
    }

    .tinder-card:hover .choose-btn {
        opacity: 1;
    }

    .choose-btn:hover {
        background: rgba(158, 206, 106, 0.3);
        transform: translateX(-50%) scale(1.1);
    }

    .tinder-vs {
        font-size: 18px;
        font-weight: 700;
        color: var(--text-secondary);
        opacity: 0.4;
        flex-shrink: 0;
    }

    .tinder-controls {
        display: flex;
        align-items: center;
        gap: 16px;
    }

    .ctrl-btn {
        height: 36px;
        padding: 0 16px;
        border-radius: 8px;
        border: 1px solid var(--border);
        background: var(--surface);
        color: var(--text-secondary);
        font-size: 13px;
        cursor: pointer;
        transition: border-color 120ms, color 120ms, background 120ms;
    }

    .ctrl-btn:hover:not(:disabled) {
        border-color: var(--blue);
        color: var(--text);
    }

    .ctrl-btn:disabled {
        opacity: 0.3;
        cursor: default;
    }

    .progress {
        font-size: 12px;
        color: var(--text-secondary);
        opacity: 0.6;
    }

    .done-screen {
        display: flex;
        flex-direction: column;
        align-items: center;
        justify-content: center;
        gap: 24px;
        animation: done-in 400ms ease-out;
    }

    @keyframes done-in {
        from { opacity: 0; transform: scale(0.95); }
        to { opacity: 1; transform: scale(1); }
    }

    .done-icon {
        width: 64px;
        height: 64px;
        border-radius: 50%;
        border: 3px solid var(--green);
        display: flex;
        align-items: center;
        justify-content: center;
        font-size: 28px;
        color: var(--green);
        background: rgba(158, 206, 106, 0.1);
    }

    .done-title {
        font-size: 20px;
        font-weight: 600;
        color: var(--text);
        margin: 0;
    }

    .done-stats {
        display: flex;
        gap: 32px;
    }

    .stat {
        display: flex;
        flex-direction: column;
        align-items: center;
        gap: 4px;
    }

    .stat-value {
        font-size: 28px;
        font-weight: 700;
        font-family: var(--font);
    }

    .stat-value.accepted { color: var(--green); }
    .stat-value.rejected { color: var(--red); }
    .stat-value.skipped { color: var(--text-secondary); }

    .stat-label {
        font-size: 11px;
        color: var(--text-secondary);
        text-transform: uppercase;
        letter-spacing: 0.06em;
    }

    .done-actions {
        display: flex;
        gap: 12px;
        margin-top: 8px;
    }

    .action-btn {
        height: 40px;
        padding: 0 20px;
        border-radius: 8px;
        border: 1px solid var(--border);
        background: var(--surface);
        color: var(--text);
        font-size: 13px;
        font-weight: 500;
        cursor: pointer;
        transition: border-color 120ms, background 120ms, color 120ms, transform 80ms;
    }

    .action-btn:hover {
        border-color: var(--blue);
        background: rgba(122, 162, 247, 0.08);
    }

    .action-btn:active {
        transform: translateY(1px);
    }

    .action-btn.primary {
        border-color: var(--green);
        background: rgba(158, 206, 106, 0.15);
        color: var(--green);
    }

    .action-btn.primary:hover {
        background: rgba(158, 206, 106, 0.25);
    }

    .action-btn.subtle {
        color: var(--text-secondary);
        border-color: transparent;
    }

    .action-btn.subtle:hover {
        border-color: var(--border);
    }
</style>
