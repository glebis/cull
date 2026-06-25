<script lang="ts">
    import { tick } from 'svelte';
    import { openUrl } from '@tauri-apps/plugin-opener';
    import packageJson from '../../../package.json';

    let { onclose }: { onclose: () => void } = $props();

    let closeButton: HTMLButtonElement | undefined = $state();

    type Link = {
        label: string;
        href: string;
    };

    type ProvenanceRow = {
        label: string;
        value: string;
        note: string;
        links: Link[];
    };

    type LicensePacket = {
        name: string;
        role: string;
        sourceHref: string;
        licenseLinks: Link[];
    };

    const provenanceRows: ProvenanceRow[] = [
        {
            label: 'Author',
            value: 'Gleb Kalinin',
            note: 'Architecture, product direction, visual system, release decisions.',
            links: [
                { label: 'GitHub', href: 'https://github.com/glebis' },
                { label: 'Authorship', href: 'https://github.com/glebis/cull/blob/main/AUTHORSHIP.md' },
            ],
        },
        {
            label: 'AI assistance',
            value: 'Claude by Anthropic',
            note: 'Implementation assistance, reviewed and integrated under human direction.',
            links: [
                { label: 'Disclosure', href: 'https://github.com/glebis/cull/blob/main/NOTICE' },
                { label: 'Terms', href: 'https://www.anthropic.com/legal/commercial-terms' },
            ],
        },
    ];

    const licensePackets: LicensePacket[] = [
        {
            name: 'Cull source',
            role: 'application code',
            sourceHref: 'https://github.com/glebis/cull',
            licenseLinks: [{ label: 'Apache-2.0', href: 'https://github.com/glebis/cull/blob/main/LICENSE' }],
        },
        {
            name: 'Tauri 2',
            role: 'desktop shell',
            sourceHref: 'https://github.com/tauri-apps/tauri',
            licenseLinks: [
                { label: 'Apache-2.0', href: 'https://github.com/tauri-apps/tauri/blob/dev/LICENSE_APACHE-2.0' },
                { label: 'MIT', href: 'https://github.com/tauri-apps/tauri/blob/dev/LICENSE_MIT' },
            ],
        },
        {
            name: 'SvelteKit',
            role: 'application UI',
            sourceHref: 'https://github.com/sveltejs/kit',
            licenseLinks: [{ label: 'MIT', href: 'https://github.com/sveltejs/kit/blob/main/LICENSE' }],
        },
        {
            name: 'Svelte',
            role: 'component runtime',
            sourceHref: 'https://github.com/sveltejs/svelte',
            licenseLinks: [{ label: 'MIT', href: 'https://github.com/sveltejs/svelte/blob/main/LICENSE.md' }],
        },
        {
            name: 'rusqlite',
            role: 'SQLite access',
            sourceHref: 'https://github.com/rusqlite/rusqlite',
            licenseLinks: [{ label: 'MIT', href: 'https://github.com/rusqlite/rusqlite/blob/master/LICENSE' }],
        },
        {
            name: 'SQLite',
            role: 'bundled database engine',
            sourceHref: 'https://www.sqlite.org/',
            licenseLinks: [{ label: 'public domain', href: 'https://www.sqlite.org/copyright.html' }],
        },
        {
            name: 'ort',
            role: 'local model execution',
            sourceHref: 'https://github.com/pykeio/ort',
            licenseLinks: [
                { label: 'MIT', href: 'https://github.com/pykeio/ort/blob/main/LICENSE-MIT' },
                { label: 'Apache-2.0', href: 'https://github.com/pykeio/ort/blob/main/LICENSE-APACHE' },
            ],
        },
        {
            name: 'ONNX Runtime',
            role: 'inference engine binaries',
            sourceHref: 'https://github.com/microsoft/onnxruntime',
            licenseLinks: [{ label: 'MIT', href: 'https://github.com/microsoft/onnxruntime/blob/main/LICENSE' }],
        },
        {
            name: 'CLIP ViT-B/32 ONNX',
            role: 'local image embeddings',
            sourceHref: 'https://huggingface.co/Qdrant/clip-ViT-B-32-vision',
            licenseLinks: [{ label: 'MIT', href: 'https://github.com/openai/CLIP/blob/main/LICENSE' }],
        },
        {
            name: 'DINOv2 ViT-S/14 ONNX',
            role: 'local image embeddings',
            sourceHref: 'https://huggingface.co/sefaburak/dinov2-small-onnx',
            licenseLinks: [{ label: 'Apache-2.0', href: 'https://github.com/facebookresearch/dinov2/blob/main/LICENSE' }],
        },
        {
            name: 'Claude Agent SDK',
            role: 'agent runtime integration',
            sourceHref: 'https://github.com/anthropics/claude-agent-sdk-typescript',
            licenseLinks: [
                { label: 'terms', href: 'https://code.claude.com/docs/en/legal-and-compliance' },
            ],
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

    function handleLinkClick(e: MouseEvent, href: string) {
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
            <img class="app-icon" src="/icon-variants/cull-dark.png" alt="" aria-hidden="true" />
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
                    <a
                        href="https://github.com/glebis/cull/blob/main/LICENSE"
                        onclick={(e) => handleLinkClick(e, 'https://github.com/glebis/cull/blob/main/LICENSE')}
                    >Apache-2.0</a>
                </div>
                <div>
                    <span class="meta-label">Repository</span>
                    <a
                        href="https://github.com/glebis/cull"
                        onclick={(e) => handleLinkClick(e, 'https://github.com/glebis/cull')}
                    >github.com/glebis/cull</a>
                </div>
                <div>
                    <span class="meta-label">Copyright</span>
                    <span class="meta-value">(c) 2026-present Gleb Kalinin</span>
                </div>
            </div>

            <section class="provenance-section" aria-labelledby="provenance-title">
                <h3 id="provenance-title">Authorship</h3>
                <div class="provenance-list">
                    {#each provenanceRows as row}
                        <article class="provenance-row">
                            <div class="provenance-copy">
                                <span class="row-label">{row.label}</span>
                                <span class="row-value">{row.value}</span>
                                <span class="row-note">{row.note}</span>
                            </div>
                            <div class="row-links" aria-label={`${row.label} links`}>
                                {#each row.links as link}
                                    <a
                                        href={link.href}
                                        rel="noreferrer"
                                        onclick={(e) => handleLinkClick(e, link.href)}
                                    >{link.label}</a>
                                {/each}
                            </div>
                        </article>
                    {/each}
                </div>
            </section>

            <section class="license-section" aria-labelledby="licenses-title">
                <div class="section-heading">
                    <h3 id="licenses-title">Core packages and models</h3>
                    <a
                        href="https://github.com/glebis/cull/blob/main/docs/OPEN_SOURCE_AUDIT.md"
                        rel="noreferrer"
                        onclick={(e) => handleLinkClick(e, 'https://github.com/glebis/cull/blob/main/docs/OPEN_SOURCE_AUDIT.md')}
                    >audit</a>
                </div>
                <div class="license-list">
                    {#each licensePackets as packet}
                        <article class="license-row">
                            <div class="packet-copy">
                                <span class="packet-name">{packet.name}</span>
                                <span class="packet-role">{packet.role}</span>
                            </div>
                            <div class="packet-links">
                                <a
                                    href={packet.sourceHref}
                                    rel="noreferrer"
                                    onclick={(e) => handleLinkClick(e, packet.sourceHref)}
                                >source</a>
                                {#each packet.licenseLinks as link}
                                    <a
                                        href={link.href}
                                        rel="noreferrer"
                                        onclick={(e) => handleLinkClick(e, link.href)}
                                    >{link.label}</a>
                                {/each}
                            </div>
                        </article>
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
    .row-label,
    .row-value,
    .row-note,
    .packet-name,
    .packet-role {
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
    .section-heading a:hover,
    .row-links a:hover,
    .packet-links a:hover {
        color: var(--blue);
        text-decoration: underline;
    }

    .provenance-section,
    .license-section {
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

    .section-heading {
        display: flex;
        min-width: 0;
        align-items: baseline;
        justify-content: space-between;
        gap: var(--spacing);
    }

    .section-heading a {
        color: var(--green);
        font-size: 11px;
        text-decoration: none;
    }

    .provenance-list,
    .license-list {
        display: grid;
        gap: var(--spacing);
    }

    .provenance-row,
    .license-row {
        display: grid;
        min-width: 0;
        gap: var(--spacing);
        padding: calc(var(--spacing) * 1.5);
        border: 1px solid var(--border-subtle);
        border-radius: var(--radius);
        background: var(--bg);
        color: var(--text);
        text-decoration: none;
    }

    .provenance-row {
        grid-template-columns: minmax(0, 1fr) auto;
        align-items: start;
    }

    .license-row {
        grid-template-columns: minmax(0, 1fr) minmax(168px, auto);
        align-items: center;
    }

    .provenance-copy,
    .packet-copy {
        min-width: 0;
    }

    .row-label {
        margin-bottom: 2px;
        color: var(--text-secondary);
        font-size: 10px;
        font-weight: 700;
        letter-spacing: 0;
        text-transform: uppercase;
    }

    .row-value,
    .packet-name {
        color: var(--text);
        font-size: 12px;
        font-weight: 700;
    }

    .row-note,
    .packet-role {
        color: var(--text-secondary);
        font-size: 11px;
        line-height: 1.35;
    }

    .row-note {
        margin-top: 3px;
    }

    .row-links,
    .packet-links {
        display: flex;
        flex-wrap: wrap;
        justify-content: flex-end;
        gap: 6px;
        min-width: 0;
    }

    .row-links a,
    .packet-links a {
        min-height: 24px;
        padding: 3px 6px;
        border: 1px solid var(--border-subtle);
        border-radius: var(--radius);
        color: var(--green);
        font-size: 10px;
        line-height: 16px;
        text-decoration: none;
    }

    .row-links a:focus-visible,
    .packet-links a:focus-visible {
        border-color: var(--blue);
        outline: none;
    }

    .section-heading a:focus-visible,
    .meta-grid a:focus-visible {
        outline: 1px solid var(--blue);
        outline-offset: 2px;
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
        .provenance-row,
        .license-row {
            grid-template-columns: 1fr;
        }

        .row-links,
        .packet-links {
            justify-content: flex-start;
        }

        h2 {
            font-size: 22px;
        }
    }
</style>
