<script lang="ts">
    import { open } from '@tauri-apps/plugin-dialog';
    import {
        exportFolderOpen,
        activeCollection,
        activeFolder,
        selectedIds,
        collections,
        showToast,
    } from '$lib/stores';
    import { exportImagesToFolder, listImageIds } from '$lib/api';
    import { buildExportParams, describeExportScope, type ExportScope } from '$lib/export-helpers';

    let format = $state('original');
    let naming = $state('{name}');
    let flatten = $state(true);
    let exporting = $state(false);

    const FORMATS = ['original', 'png', 'jpg', 'webp'];

    let selected = $derived(Array.from($selectedIds));
    let scopeInfo = $derived(
        describeExportScope({
            activeCollection: $activeCollection,
            activeFolder: $activeFolder,
            selectedIds: selected,
            allImageIds: [],
        })
    );
    let scopeLabel = $derived(scopeLabelFor(scopeInfo.kind));

    function scopeLabelFor(kind: string): string {
        switch (kind) {
            case 'selection':
                return `${selected.length} selected image${selected.length === 1 ? '' : 's'}`;
            case 'collection': {
                const name = $collections.find(c => c[0] === $activeCollection)?.[1] ?? 'collection';
                return `collection “${name}”`;
            }
            case 'folder':
                return `folder “${($activeFolder ?? '').split('/').filter(Boolean).pop() ?? $activeFolder}”`;
            default:
                return 'the entire library';
        }
    }

    async function runExport() {
        if (exporting) return;
        const outputDir = await open({ directory: true, multiple: false, title: 'Choose export destination' });
        if (!outputDir || typeof outputDir !== 'string') return;

        exporting = true;
        try {
            const scope: ExportScope = {
                activeCollection: $activeCollection,
                activeFolder: $activeFolder,
                selectedIds: selected,
                allImageIds: scopeInfo.kind === 'all' ? await listImageIds() : [],
            };
            const params = buildExportParams(scope, { outputDir, format, naming, flatten });
            const result = await exportImagesToFolder(params);
            const detail = result.skipped > 0 ? `${result.skipped} skipped` : undefined;
            showToast(`Exported ${result.exported} image${result.exported === 1 ? '' : 's'}`, {
                detail,
                type: result.errors.length > 0 ? 'warning' : 'success',
                duration: 7000,
            });
            exportFolderOpen.set(false);
        } catch (e) {
            showToast('Export failed', { detail: String(e), type: 'error', duration: 9000 });
        } finally {
            exporting = false;
        }
    }

    function close() {
        if (!exporting) exportFolderOpen.set(false);
    }

    function onBackdropKeydown(event: KeyboardEvent) {
        if (event.key === 'Escape') close();
    }
</script>

{#if $exportFolderOpen}
    <div
        class="export-backdrop"
        role="dialog"
        aria-modal="true"
        aria-label="Export images to folder"
        tabindex="-1"
        onclick={close}
        onkeydown={onBackdropKeydown}
    >
        <!-- svelte-ignore a11y_click_events_have_key_events -->
        <div class="export-panel" role="presentation" onclick={(e) => e.stopPropagation()}>
            <div class="export-head">
                <span class="export-title">Export to Folder</span>
                <button class="export-close" type="button" onclick={close} aria-label="Close">×</button>
            </div>
            <p class="export-scope">Exporting <strong>{scopeLabel}</strong>.</p>

            <label class="export-field">
                <span>Format</span>
                <select bind:value={format}>
                    {#each FORMATS as f}
                        <option value={f}>{f === 'original' ? 'Original (no conversion)' : f.toUpperCase()}</option>
                    {/each}
                </select>
            </label>

            <label class="export-field">
                <span>Filename template</span>
                <input type="text" bind:value={naming} placeholder="{'{name}'}" />
            </label>
            <p class="export-hint">
                Tokens: <code>{'{name}'}</code> <code>{'{id}'}</code> <code>{'{index}'}</code> <code>{'{index1}'}</code>.
                Use a preset keyword (<code>original</code>, <code>id</code>, <code>index</code>) for the built-in behaviors.
            </p>

            {#if scopeInfo.kind === 'folder'}
                <label class="export-check">
                    <input type="checkbox" bind:checked={flatten} />
                    <span>Flatten subfolders into one directory</span>
                </label>
            {/if}

            <div class="export-actions">
                <button class="export-cancel" type="button" onclick={close} disabled={exporting}>Cancel</button>
                <button class="export-go" type="button" onclick={runExport} disabled={exporting}>
                    {exporting ? 'Exporting…' : 'Choose Folder & Export'}
                </button>
            </div>
        </div>
    </div>
{/if}

<style>
    .export-backdrop {
        position: fixed;
        inset: 0;
        background: rgba(0, 0, 0, 0.55);
        display: flex;
        align-items: flex-start;
        justify-content: center;
        padding-top: 12vh;
        z-index: 1210;
    }
    .export-panel {
        width: min(440px, 92vw);
        background: var(--surface);
        border: 1px solid var(--border);
        border-radius: var(--radius);
        box-shadow: 0 18px 60px rgba(0, 0, 0, 0.5);
        padding: calc(var(--spacing) * 2);
        display: flex;
        flex-direction: column;
        gap: var(--spacing);
    }
    .export-head {
        display: flex;
        align-items: center;
        justify-content: space-between;
    }
    .export-title {
        font-weight: 600;
        color: var(--text);
    }
    .export-close {
        background: transparent;
        border: none;
        color: var(--text-secondary);
        font-size: 18px;
        cursor: pointer;
    }
    .export-close:hover {
        color: var(--text);
    }
    .export-scope {
        color: var(--text-secondary);
        margin: 0;
        font-size: 13px;
    }
    .export-field {
        display: flex;
        flex-direction: column;
        gap: 4px;
        color: var(--text-secondary);
        font-size: 12px;
    }
    .export-field select,
    .export-field input {
        background: var(--bg);
        border: 1px solid var(--border);
        border-radius: var(--radius);
        color: var(--text);
        padding: 6px var(--spacing);
        font-family: var(--font, monospace);
    }
    .export-hint {
        color: var(--text-secondary);
        font-size: 11px;
        margin: 0;
        line-height: 1.5;
    }
    .export-hint code {
        background: var(--bg);
        border: 1px solid var(--border);
        border-radius: 3px;
        padding: 0 4px;
        color: var(--blue);
    }
    .export-check {
        display: flex;
        align-items: center;
        gap: 6px;
        color: var(--text-secondary);
        font-size: 12px;
    }
    .export-actions {
        display: flex;
        justify-content: flex-end;
        gap: var(--spacing);
        margin-top: var(--spacing);
    }
    .export-cancel,
    .export-go {
        border: 1px solid var(--border);
        border-radius: var(--radius);
        padding: 6px 12px;
        cursor: pointer;
        font-family: var(--font, monospace);
        font-size: 13px;
    }
    .export-cancel {
        background: transparent;
        color: var(--text-secondary);
    }
    .export-go {
        background: var(--blue);
        color: var(--bg);
        border-color: var(--blue);
    }
    .export-go:disabled,
    .export-cancel:disabled {
        opacity: 0.5;
        cursor: default;
    }
</style>
