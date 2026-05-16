import { describe, expect, it } from 'vitest';
import { readFileSync, readdirSync, statSync } from 'node:fs';
import { join } from 'node:path';

const root = process.cwd();

function walk(dir: string): string[] {
    return readdirSync(dir).flatMap((entry) => {
        const path = join(dir, entry);
        if (entry === 'node_modules' || entry === 'target' || entry === '.svelte-kit') return [];
        if (statSync(path).isDirectory()) return walk(path);
        return path;
    });
}

function frontendInvokeNames(): string[] {
    const files = walk(join(root, 'src')).filter((file) => {
        if (!/\.(ts|svelte)$/.test(file)) return false;
        if (/\.test\.ts$/.test(file)) return false;
        if (file.endsWith('tauri-mock.ts')) return false;
        return true;
    });

    const names = new Set<string>();
    const invokeRe = /invoke(?:<[^>]+>)?\(\s*['"]([a-zA-Z0-9_]+)['"]/g;

    for (const file of files) {
        const source = readFileSync(file, 'utf8');
        for (const match of source.matchAll(invokeRe)) {
            names.add(match[1]);
        }
    }

    return [...names].sort();
}

function registeredCommandNames(): string[] {
    const source = readFileSync(join(root, 'src-tauri/src/lib.rs'), 'utf8');
    const names = new Set<string>();
    const commandRe = /commands::[a-zA-Z0-9_]+::([a-zA-Z0-9_]+)/g;

    for (const match of source.matchAll(commandRe)) {
        names.add(match[1]);
    }

    return [...names].sort();
}

describe('Tauri command contract', () => {
    it('registers every frontend invoke command', () => {
        const registered = new Set(registeredCommandNames());
        const missing = frontendInvokeNames().filter((name) => !registered.has(name));

        expect(missing).toEqual([]);
    });
});
