// cull-publish — Track C3 proof plugin.
//
// The publish settings UI extracted from core StaticPublishingSettings.svelte
// into a standalone ESM bundle. Everything privileged goes through
// host.invoke(tool, args), enforced in Rust by plugin_invoke against the
// grants recorded at install:
//   library:read              -> get_library_stats, list_collections,
//                                list_collection_images
//   export:read               -> export_static_publish_package (capability)
//   module:static-publishing  -> export_static_publish_package (module gate)
//
// Plain DOM + design tokens (CSS variables from app.css); no framework.

const STYLE = `
.cull-publish-root {
    display: grid;
    gap: 16px;
    padding: 20px;
    align-content: start;
    color: var(--text);
    font-family: var(--font);
}
.cull-publish-root .eyebrow {
    color: var(--green);
    font-size: 11px;
    font-weight: 700;
    text-transform: uppercase;
}
.cull-publish-root h1 {
    color: var(--text);
    font-size: 22px;
    font-weight: 700;
    margin: 0;
}
.cull-publish-root .plugin-note,
.cull-publish-root .status-line {
    color: var(--text-secondary);
    font-size: 12px;
}
.cull-publish-root .panel {
    display: grid;
    gap: 12px;
    padding: 16px 20px;
    background: var(--surface);
    border: 1px solid var(--border);
    border-radius: var(--radius);
}
.cull-publish-root .section-header {
    color: var(--text);
    font-size: 11px;
    font-weight: 600;
    text-transform: uppercase;
}
.cull-publish-root label {
    color: var(--text);
    font-size: 12px;
    display: grid;
    gap: 6px;
}
.cull-publish-root .check-row {
    display: flex;
    gap: 8px;
    align-items: center;
    font-size: 13px;
}
.cull-publish-root input[type="text"],
.cull-publish-root select {
    width: 100%;
    background: var(--bg);
    border: 1px solid var(--border);
    border-radius: var(--radius);
    padding: 8px 9px;
    color: var(--text);
    font-family: var(--font);
    font-size: 12px;
    min-height: 34px;
}
.cull-publish-root .primary-btn {
    width: 100%;
    min-height: 36px;
    padding: 9px 12px;
    border: none;
    border-radius: var(--radius);
    background: var(--green);
    color: var(--bg);
    font-family: var(--font);
    font-weight: 700;
    font-size: 12px;
    cursor: pointer;
}
.cull-publish-root .primary-btn:disabled {
    opacity: 0.45;
    cursor: not-allowed;
}
.cull-publish-root .result {
    display: grid;
    gap: 6px;
    font-size: 12px;
}
.cull-publish-root .result code {
    color: var(--text);
    overflow-wrap: anywhere;
}
.cull-publish-root .result .ok { color: var(--green); }
.cull-publish-root .result .warn { color: var(--orange); }
.cull-publish-root .result .error { color: var(--red); }
`;

function el(tag, props = {}, children = []) {
    const node = document.createElement(tag);
    Object.assign(node, props);
    for (const child of children) {
        node.append(child);
    }
    return node;
}

export default {
    async activate(host) {
        const root = el('div', { className: 'cull-publish-root' });
        root.append(el('style', { textContent: STYLE }));

        const header = el('div', {}, [
            el('span', { className: 'eyebrow', textContent: 'Publish' }),
            el('h1', { textContent: 'Publish site' }),
            el('p', {
                className: 'plugin-note',
                textContent: 'Managed by the cull-publish plugin',
            }),
        ]);

        const statusLine = el('p', {
            className: 'status-line',
            textContent: 'Loading library status…',
        });

        const collectionSelect = el('select', { id: 'cull-publish-collection' });
        const titleInput = el('input', {
            type: 'text',
            id: 'cull-publish-title',
            value: 'Published Collection',
            autocomplete: 'off',
        });
        const includeThumb = el('input', { type: 'checkbox', checked: true });
        const includeWeb = el('input', { type: 'checkbox', checked: true });
        const includeFull = el('input', { type: 'checkbox', checked: false });
        const publishButton = el('button', {
            className: 'primary-btn',
            textContent: 'Build package',
            disabled: true,
        });
        const result = el('div', { className: 'result' });

        const panel = el('div', { className: 'panel' }, [
            el('div', { className: 'section-header', textContent: 'Source and package' }),
            el('label', { htmlFor: 'cull-publish-collection', textContent: 'Collection' }, [
                collectionSelect,
            ]),
            el('label', { htmlFor: 'cull-publish-title', textContent: 'Site title' }, [titleInput]),
            el('div', { className: 'check-row' }, [
                el('label', { className: 'check-row' }, [includeThumb, 'Thumbnail']),
                el('label', { className: 'check-row' }, [includeWeb, 'Web image']),
                el('label', { className: 'check-row' }, [includeFull, 'Original file']),
            ]),
            publishButton,
            result,
        ]);

        root.append(header, statusLine, panel);

        function setResult(kind, text) {
            result.replaceChildren(el('span', { className: kind, textContent: text }));
        }

        async function refresh() {
            try {
                const stats = await host.invoke('get_library_stats');
                statusLine.textContent = `${stats.image_count} images · ${stats.collection_count} collections in library`;
                const { collections } = await host.invoke('list_collections');
                collectionSelect.replaceChildren(
                    ...collections.map((collection) =>
                        el('option', {
                            value: collection.id,
                            textContent: `${collection.name} (${collection.image_count})`,
                        })
                    )
                );
                publishButton.disabled = collections.length === 0;
                if (collections.length === 0) {
                    setResult('warn', 'Create a collection first to publish it as a static site.');
                }
            } catch (e) {
                statusLine.textContent = 'Library status unavailable';
                setResult('error', String(e));
            }
        }

        async function publish() {
            const collectionId = collectionSelect.value;
            if (!collectionId) return;
            publishButton.disabled = true;
            publishButton.textContent = 'Building package…';
            try {
                const { images } = await host.invoke('list_collection_images', {
                    collection_id: collectionId,
                });
                if (images.length === 0) {
                    setResult('warn', 'The selected collection has no images.');
                    return;
                }
                const siteTitle = titleInput.value.trim() || 'Published Collection';
                const packageResult = await host.invoke('export_static_publish_package', {
                    canvas_name: siteTitle,
                    items: images.map((image) => ({ image_id: image.id })),
                    layout_json: JSON.stringify({
                        type: 'plugin_collection_publish',
                        image_ids: images.map((image) => image.id),
                    }),
                    site_title: siteTitle,
                    include_thumbnails: includeThumb.checked,
                    include_web: includeWeb.checked,
                    include_full: includeFull.checked,
                    indexable: false,
                    links: [],
                });
                result.replaceChildren(
                    el('span', {
                        className: 'ok',
                        textContent: `Package built · ${packageResult.image_count} images`,
                    }),
                    el('code', { textContent: packageResult.site_dir }),
                    ...(packageResult.warnings ?? []).map((warning) =>
                        el('span', { className: 'warn', textContent: warning })
                    )
                );
            } catch (e) {
                setResult('error', `Package build failed: ${e}`);
            } finally {
                publishButton.disabled = false;
                publishButton.textContent = 'Build package';
            }
        }

        publishButton.addEventListener('click', publish);

        host.registerPaletteCommands([
            {
                id: 'build-package',
                title: 'Publish: Build static site package',
                subtitle: 'cull-publish plugin',
                keywords: ['static', 'site', 'publishing', 'export'],
                run: () => publish(),
            },
        ]);
        host.mountView(root);

        refresh();
    },
};
