#!/usr/bin/env node
// Prepare a plugin fixture from plugins/<id>/entry.mjs:
//   1. write the source SHA-256 into plugins/<id>/manifest.json
//      (the installer writes these bytes to the manifest entry path)
//   2. regenerate tests/fixtures/plugin-registry/registry.json with a
//      repo-relative file:// download URL, so the end-to-end Rust proof test
//      (and manual testing via the plugin_registry_url setting) installs the
//      exact bytes the checksum describes.
//
// Usage: node scripts/build-plugin.mjs [pluginId]   (default: cull-publish)

import { createHash } from 'node:crypto';
import { mkdirSync, readFileSync, writeFileSync } from 'node:fs';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';

const repoRoot = join(dirname(fileURLToPath(import.meta.url)), '..');
const pluginId = process.argv[2] ?? 'cull-publish';
const pluginDir = join(repoRoot, 'plugins', pluginId);

// The plugin source is a single dependency-free ESM file.
const source = readFileSync(join(pluginDir, 'entry.mjs'));

// 1. Checksum into the manifest.
const checksum = `sha256:${createHash('sha256').update(source).digest('hex')}`;
const manifestPath = join(pluginDir, 'manifest.json');
const manifest = JSON.parse(readFileSync(manifestPath, 'utf8'));
manifest.checksum = checksum;
writeFileSync(manifestPath, `${JSON.stringify(manifest, null, 2)}\n`);

// 2. Registry fixture (schema cull.plugins.registry.v1). The download URL is
// repo-relative file://, resolved against the repo root by consumers of the
// fixture. No `updated` field: the fixture must be deterministic.
const registry = {
    schema: 'cull.plugins.registry.v1',
    plugins: [
        {
            id: manifest.id,
            name: manifest.name,
            version: manifest.version,
            description: manifest.description,
            permissions: manifest.permissions,
            minAppVersion: manifest.minAppVersion,
            checksum: manifest.checksum,
            repo: manifest.repo,
            download: `file://plugins/${pluginId}/entry.mjs`,
        },
    ],
};
const fixtureDir = join(repoRoot, 'tests', 'fixtures', 'plugin-registry');
mkdirSync(fixtureDir, { recursive: true });
const registryPath = join(fixtureDir, 'registry.json');
writeFileSync(registryPath, `${JSON.stringify(registry, null, 2)}\n`);

console.log(`built ${pluginId}`);
console.log(`  source:   plugins/${pluginId}/entry.mjs (${source.length} bytes)`);
console.log(`  checksum: ${checksum}`);
console.log('  registry: tests/fixtures/plugin-registry/registry.json');
