// src/lib/plugins/cull-publish/index.ts
import { mount } from 'svelte';
import type { BundledPlugin } from '../loader';
import type { PluginHost } from '../host';
import { manifest } from './manifest';
import PublishView from './PublishView.svelte';

const cullPublish: BundledPlugin = {
    manifest,
    activate(host: PluginHost) {
        host.registerTab({
            id: 'publish',
            label: 'Publish View',
            subtitle: 'Build a static site package',
            mountView: (el: HTMLElement) => { mount(PublishView, { target: el, props: { host } }); },
        });
    },
};

export default cullPublish;
