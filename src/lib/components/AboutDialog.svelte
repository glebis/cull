<script lang="ts">
    import { tick } from 'svelte';
    import { openUrl } from '@tauri-apps/plugin-opener';
    import packageJson from '../../../package.json';

    let { onclose }: { onclose: () => void } = $props();

    let closeButton: HTMLButtonElement | undefined = $state();

    const credits = [
        {
            label: 'Gleb Kalinin',
            role: 'architecture, product direction, design',
            href: 'https://github.com/glebis',
        },
        {
            label: 'Claude by Anthropic',
            role: 'implementation assistance',
            href: 'https://www.anthropic.com/claude-code',
        },
        {
            label: 'Tauri',
            role: 'desktop application framework',
            href: 'https://tauri.app/',
        },
        {
            label: 'SvelteKit',
            role: 'application UI framework',
            href: 'https://svelte.dev/docs/kit',
        },
        {
            label: 'Rust',
            role: 'native backend',
            href: 'https://www.rust-lang.org/',
        },
        {
            label: 'SQLite',
            role: 'local library database',
            href: 'https://www.sqlite.org/',
        },
        {
            label: 'ONNX Runtime',
            role: 'local model execution',
            href: 'https://onnxruntime.ai/',
        },
        {
            label: 'OpenAI CLIP',
            role: 'semantic image embeddings',
            href: 'https://github.com/openai/CLIP',
        },
    ];

    tick().then(() => closeButton?.focus());

    function handleKeydown(e: KeyboardEvent) {
        if (e.key === 'Escape') {
            e.preventDefault();
            onclose();
        }
    }

    async function openExternal(href: string) {
        try {
            await openUrl(href);
        } catch {
            window.open(href, '_blank', 'noopener,noreferrer');
        }
    }

    function handleCreditClick(e: MouseEvent, href: string) {
        e.preventDefault();
        void openExternal(href);
    }
</script>

<svelte:window onkeydown={handleKeydown} />

<!-- svelte-ignore a11y_no_static_element_interactions, a11y_click_events_have_key_events -->
<div class="about-overlay" onclick={onclose}>
    <!-- svelte-ignore a11y_no_static_element_interactions -->
    <div
        class="about-dialog"
        role="dialog"
        aria-modal="true"
        aria-labelledby="about-title"
        tabindex="-1"
        onclick={(e: MouseEvent) => e.stopPropagation()}
    >
        <header class="about-header">
            <img class="app-icon" src="/icon-variants/cull-primary.png" alt="" aria-hidden="true" />
            <div class="title-group">
                <p class="eyebrow">Local-first image review</p>
                <h2 id="about-title">Cull</h2>
                <p class="version">Version {packageJson.version}</p>
            </div>
            <button
                bind:this={closeButton}
                class="close-btn"
                type="button"
                aria-label="Close"
                onclick={onclose}
            >X</button>
        </header>

        <div class="about-body">
            <p class="summary">
                A desktop image viewer for AI-generated art workflows, built around fast curation,
                local data, generation metadata, and agent-accessible library tools.
            </p>

            <div class="meta-grid" aria-label="Application details">
                <div>
                    <span class="meta-label">License</span>
                    <span class="meta-value">Apache-2.0</span>
                </div>
                <div>
                    <span class="meta-label">Repository</span>
                    <a
                        href="https://github.com/glebis/cull"
                        onclick={(e) => handleCreditClick(e, 'https://github.com/glebis/cull')}
                    >github.com/glebis/cull</a>
                </div>
                <div>
                    <span class="meta-label">Copyright</span>
                    <span class="meta-value">(c) 2026-present Gleb Kalinin</span>
                </div>
            </div>

            <section class="credits-section" aria-labelledby="credits-title">
                <h3 id="credits-title">Credits</h3>
                <div class="credits-list">
                    {#each credits as credit}
                        <a
                            class="credit-row"
                            href={credit.href}
                            rel="noreferrer"
                            onclick={(e) => handleCreditClick(e, credit.href)}
                        >
                            <span class="credit-label">{credit.label}</span>
                            <span class="credit-role">{credit.role}</span>
                        </a>
                    {/each}
                </div>
            </section>
        </div>
    </div>
</div>

<style>
    .about-overlay {
        position: fixed;
        inset: 0;
        z-index: 12000;
        display: flex;
        align-items: center;
        justify-content: center;
        padding: calc(var(--spacing) * 2);
        background: color-mix(in srgb, var(--bg) 76%, transparent);
    }

    .about-dialog {
        width: min(620px, 100%);
        max-height: calc(100vh - 32px);
        overflow: hidden;
        background: var(--surface);
        border: 1px solid var(--border);
        border-radius: calc(var(--radius) * 2);
        box-shadow: 0 24px 80px color-mix(in srgb, var(--bg) 86%, transparent);
    }

    .about-header {
        display: grid;
        grid-template-columns: 84px minmax(0, 1fr) 32px;
        gap: calc(var(--spacing) * 2);
        align-items: center;
        padding: calc(var(--spacing) * 3);
        border-bottom: 1px solid var(--border);
    }

    .app-icon {
        width: 84px;
        height: 84px;
        border-radius: calc(var(--radius) * 2);
        border: 1px solid var(--border-subtle);
        background: var(--bg);
        box-shadow: 0 12px 36px color-mix(in srgb, var(--bg) 70%, transparent);
    }

    .title-group {
        min-width: 0;
    }

    .eyebrow {
        margin: 0 0 2px;
        color: var(--blue);
        font-size: 10px;
        font-weight: 700;
        letter-spacing: 0;
        text-transform: uppercase;
    }

    h2,
    h3,
    p {
        margin: 0;
    }

    h2 {
        color: var(--text);
        font-size: 26px;
        line-height: 1.1;
        letter-spacing: 0;
    }

    .version {
        margin-top: 6px;
        color: var(--text-secondary);
        font-size: 12px;
    }

    .close-btn {
        width: 32px;
        height: 32px;
        border: 1px solid var(--border);
        border-radius: var(--radius);
        background: var(--bg);
        color: var(--text-secondary);
        cursor: pointer;
        font-family: var(--font);
        font-size: 12px;
        font-weight: 700;
    }

    .close-btn:hover,
    .close-btn:focus-visible {
        border-color: var(--blue);
        color: var(--text);
    }

    .about-body {
        display: flex;
        max-height: calc(100vh - 168px);
        flex-direction: column;
        gap: calc(var(--spacing) * 2);
        overflow: auto;
        padding: calc(var(--spacing) * 3);
    }

    .summary {
        color: var(--text);
        font-size: 13px;
        line-height: 1.6;
        overflow-wrap: anywhere;
    }

    .meta-grid {
        display: grid;
        grid-template-columns: minmax(0, 0.7fr) minmax(0, 1.6fr) minmax(0, 1fr);
        gap: var(--spacing);
    }

    .meta-grid > div {
        min-width: 0;
        padding: calc(var(--spacing) * 1.5);
        border: 1px solid var(--border-subtle);
        border-radius: var(--radius);
        background: var(--bg);
    }

    .meta-label,
    .meta-value,
    .meta-grid a,
    .credit-label,
    .credit-role {
        display: block;
        min-width: 0;
        overflow-wrap: anywhere;
    }

    .meta-label {
        margin-bottom: 4px;
        color: var(--text-secondary);
        font-size: 10px;
        font-weight: 700;
        letter-spacing: 0;
        text-transform: uppercase;
    }

    .meta-value,
    .meta-grid a {
        color: var(--text);
        font-size: 11px;
        line-height: 1.35;
    }

    .meta-grid a {
        color: var(--green);
        text-decoration: none;
    }

    .meta-grid a:hover,
    .credit-row:hover .credit-label {
        color: var(--blue);
        text-decoration: underline;
    }

    .credits-section {
        display: flex;
        flex-direction: column;
        gap: var(--spacing);
    }

    h3 {
        color: var(--orange);
        font-size: 11px;
        font-weight: 700;
        letter-spacing: 0;
        text-transform: uppercase;
    }

    .credits-list {
        display: grid;
        grid-template-columns: repeat(2, minmax(0, 1fr));
        gap: var(--spacing);
    }

    .credit-row {
        display: flex;
        min-width: 0;
        flex-direction: column;
        gap: 2px;
        padding: calc(var(--spacing) * 1.5);
        border: 1px solid var(--border-subtle);
        border-radius: var(--radius);
        background: var(--bg);
        color: var(--text);
        text-decoration: none;
    }

    .credit-row:focus-visible {
        border-color: var(--blue);
        outline: none;
    }

    .credit-label {
        color: var(--text);
        font-size: 12px;
        font-weight: 700;
    }

    .credit-role {
        color: var(--text-secondary);
        font-size: 11px;
        line-height: 1.35;
    }

    @media (max-width: 620px) {
        .about-header {
            grid-template-columns: 64px minmax(0, 1fr) 32px;
            gap: var(--spacing);
            padding: calc(var(--spacing) * 2);
        }

        .app-icon {
            width: 64px;
            height: 64px;
        }

        .about-body {
            padding: calc(var(--spacing) * 2);
        }

        .meta-grid,
        .credits-list {
            grid-template-columns: 1fr;
        }

        h2 {
            font-size: 22px;
        }
    }
</style>
