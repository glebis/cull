import { describe, expect, it } from 'vitest';
import { existsSync, readFileSync, readdirSync, statSync } from 'node:fs';
import { join } from 'node:path';

const root = process.cwd();

function walk(dir: string): string[] {
    if (!existsSync(dir)) return [];
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

function invokeHandlerSource(): string {
    const source = readFileSync(join(root, 'src-tauri/src/lib.rs'), 'utf8');
    const startToken = '.invoke_handler(tauri::generate_handler![';
    const start = source.indexOf(startToken);
    if (start === -1) throw new Error('Could not find Tauri invoke handler');
    const end = source.indexOf('])', start);
    if (end === -1) throw new Error('Could not find Tauri invoke handler end');
    return source.slice(start, end);
}

function registeredCommandNames(): string[] {
    const source = invokeHandlerSource();
    const names = new Set<string>();
    const commandRe = /(?:commands::[a-zA-Z0-9_]+|menu)::([a-zA-Z0-9_]+)/g;

    for (const match of source.matchAll(commandRe)) {
        names.add(match[1]);
    }

    return [...names].sort();
}

function appPermissionCommands(): Map<string, string[]> {
    const permissions = new Map<string, string[]>();
    const files = walk(join(root, 'src-tauri/permissions')).filter((file) => file.endsWith('.toml'));

    for (const file of files) {
        const source = readFileSync(file, 'utf8');
        const blocks = source
            .split(/\n(?=\[\[permission\]\])/)
            .filter((block) => block.includes('[[permission]]'));

        for (const block of blocks) {
            const id = block.match(/identifier\s*=\s*"([^"]+)"/)?.[1];
            const allowBody = block.match(/commands\.allow\s*=\s*\[([\s\S]*?)\]/)?.[1];
            if (!id || !allowBody) continue;
            const commands = [...allowBody.matchAll(/"([a-zA-Z0-9_]+)"/g)].map((match) => match[1]).sort();
            permissions.set(id, commands);
        }
    }

    return permissions;
}

type Capability = {
    identifier: string;
    windows?: string[];
    permissions: Array<string | { identifier: string }>;
};

function capabilityFiles(): Capability[] {
    const files = walk(join(root, 'src-tauri/capabilities')).filter((file) => file.endsWith('.json'));
    return files.flatMap((file) => {
        const parsed = JSON.parse(readFileSync(file, 'utf8'));
        if (Array.isArray(parsed)) return parsed;
        if (Array.isArray(parsed.capabilities)) return parsed.capabilities;
        return [parsed];
    });
}

function activeAppPermissionIds(): string[] {
    const ids = new Set<string>();

    for (const capability of capabilityFiles()) {
        for (const permission of capability.permissions ?? []) {
            const id = typeof permission === 'string' ? permission : permission.identifier;
            if (!id.includes(':')) ids.add(id);
        }
    }

    return [...ids].sort();
}

function activeAppCommandPermissions(): Map<string, string[]> {
    const definitions = appPermissionCommands();
    const activeIds = activeAppPermissionIds();
    const commands = new Map<string, string[]>();

    for (const id of activeIds) {
        for (const command of definitions.get(id) ?? []) {
            const ids = commands.get(command) ?? [];
            ids.push(id);
            commands.set(command, ids);
        }
    }

    return commands;
}

describe('Tauri command contract', () => {
    it('registers every frontend invoke command', () => {
        const registered = new Set(registeredCommandNames());
        const missing = frontendInvokeNames().filter((name) => !registered.has(name));

        expect(missing).toEqual([]);
    });

    it('grants every registered app command through an active capability permission', () => {
        const registered = registeredCommandNames();
        const activePermissions = new Set(activeAppPermissionIds());
        const definedPermissions = appPermissionCommands();
        const commandPermissions = activeAppCommandPermissions();

        const unknownPermissions = [...activePermissions].filter((id) => !definedPermissions.has(id));
        const missing = registered.filter((command) => !commandPermissions.has(command));
        const extra = [...commandPermissions.keys()].filter((command) => !registered.includes(command));
        const duplicates = [...commandPermissions.entries()]
            .filter(([, ids]) => ids.length > 1)
            .map(([command, ids]) => `${command}: ${ids.join(', ')}`);

        expect(unknownPermissions).toEqual([]);
        expect(missing).toEqual([]);
        expect(extra).toEqual([]);
        expect(duplicates).toEqual([]);
    });

    it('splits high-risk commands into dedicated app capability groups', () => {
        const commandPermissions = activeAppCommandPermissions();

        expect(commandPermissions.get('delete_images_permanently')).toEqual(['app-file-access']);
        expect(commandPermissions.get('set_api_key')).toEqual(['app-ai-processing']);
        expect(commandPermissions.get('assemble_export_pdf')).toEqual(['app-export-publishing']);
        expect(commandPermissions.get('create_mcp_token')).toEqual(['app-admin']);
        expect(commandPermissions.get('start_dictation')).toEqual(['app-ui']);
        expect(commandPermissions.get('set_rating')).toEqual(['app-curation']);
        expect(commandPermissions.get('get_clipboard_monitor_status')).toEqual(['app-read']);
        expect(commandPermissions.get('start_clipboard_monitor')).toEqual(['app-file-access']);
        expect(commandPermissions.get('stop_clipboard_monitor')).toEqual(['app-file-access']);
        expect(commandPermissions.get('set_clipboard_monitor_capture_dir')).toEqual(['app-file-access']);
        expect(commandPermissions.get('move_clipboard_capture_folder')).toEqual(['app-file-access']);
        expect(commandPermissions.get('publish_clipboard_collection')).toEqual(['app-export-publishing']);
    });

    it('targets app command capabilities only at first-party app windows', () => {
        const appCapabilities = capabilityFiles().filter((capability) => capability.identifier.startsWith('app-'));

        expect(appCapabilities.map((capability) => capability.identifier).sort()).toEqual([
            'app-admin',
            'app-ai-processing',
            'app-curation',
            'app-export-publishing',
            'app-file-access',
            'app-read',
            'app-ui',
        ]);

        for (const capability of appCapabilities) {
            expect(capability.windows).toEqual(['main', 'window-*']);
        }
    });

    it('allows the native Tauri header drag command used by data-tauri-drag-region', () => {
        const defaultCapability = capabilityFiles().find((capability) => capability.identifier === 'default');
        const permissions = defaultCapability?.permissions.map((permission) =>
            typeof permission === 'string' ? permission : permission.identifier
        );

        expect(permissions).toContain('core:window:allow-start-dragging');
    });
});
