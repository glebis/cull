<script lang="ts">
    interface Props {
        position: number;
        size: number;
        minSize: number;
        maxSize: number;
        groupLabel: string;
        sliderLabel: string;
        outLabel: string;
        inLabel: string;
        onposition: (position: number) => void;
        onstep: (direction: -1 | 1) => void;
    }

    let {
        position,
        size,
        minSize,
        maxSize,
        groupLabel,
        sliderLabel,
        outLabel,
        inLabel,
        onposition,
        onstep,
    }: Props = $props();
</script>

<div class="slider-group" role="group" aria-label={groupLabel}>
    <button class="slider-icon" type="button" aria-label={outLabel} title="Zoom out" disabled={size <= minSize} onclick={() => onstep(-1)}>▪▪</button>
    <div class="slider-track">
        <input
            type="range"
            min="0"
            max="100"
            step="1"
            value={position}
            oninput={(event) => onposition(Number(event.currentTarget.value))}
            aria-label={sliderLabel}
            aria-valuetext={`${size} pixel previews`}
        />
    </div>
    <button class="slider-icon" type="button" aria-label={inLabel} title="Zoom in" disabled={size >= maxSize} onclick={() => onstep(1)}>▪</button>
</div>

<style>
    .slider-group { display: flex; align-items: center; gap: 6px; }
    .slider-icon {
        display: grid;
        place-items: center;
        min-width: 18px;
        height: 22px;
        padding: 0 3px;
        border: 0;
        border-radius: var(--radius);
        background: transparent;
        color: var(--text-secondary);
        font-family: var(--font);
        font-size: 8px;
        opacity: 0.5;
        letter-spacing: 1px;
        cursor: pointer;
        transition: color 120ms, opacity 120ms, background 120ms, transform 80ms;
    }
    .slider-icon:hover { color: var(--text); background: color-mix(in srgb, var(--blue) 10%, transparent); opacity: 1; }
    .slider-icon:active { transform: scale(0.88); }
    .slider-icon:disabled { opacity: 0.18; cursor: default; }
    .slider-icon:disabled:hover { color: var(--text-secondary); background: transparent; }
    .slider-icon:focus-visible { outline: 1px solid var(--blue); outline-offset: 1px; }
    .slider-track { width: 80px; display: flex; align-items: center; }
    input[type="range"] {
        -webkit-appearance: none;
        appearance: none;
        width: 100%;
        height: 2px;
        background: var(--border);
        border-radius: 1px;
        outline: none;
        cursor: pointer;
    }
    input[type="range"]::-webkit-slider-thumb {
        -webkit-appearance: none;
        appearance: none;
        width: 16px;
        height: 16px;
        border-radius: 50%;
        background: var(--blue);
        cursor: pointer;
    }
    input[type="range"]::-webkit-slider-thumb:hover { background: var(--green); }
    @media (prefers-reduced-motion: reduce) { .slider-icon { transition: none; } }
</style>
