import type { PluginManifest } from '../host';

export const manifest: PluginManifest = {
    id: 'cull-publish',
    name: 'Publish View (Static Site)',
    version: '1.0.0',
    description: 'Build a read-only static site package from a collection.',
    entry: 'bundled',
    permissions: ['library:read', 'export:read', 'module:static-publishing'],
    minAppVersion: '0.2.1',
    checksum: 'bundled',
    repo: 'https://github.com/glebis/cull-plugins/tree/main/cull-publish',
};
