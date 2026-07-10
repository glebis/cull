import { beforeEach, describe, expect, it, vi } from 'vitest';
import { get } from 'svelte/store';
import { readFileSync } from 'node:fs';
import { join } from 'node:path';
import { getCommandPaletteItems } from './command-palette';
import {
    clientToolsEnabled,
    collectMode,
    collectModeTarget,
    collections,
    focusedIndex,
    images,
    selectedIds,
    staticPublishingEnabled,
    voiceDictationEnabled,
} from './stores';

const root = process.cwd();

function readProjectFile(path: string): string {
    return readFileSync(join(root, path), 'utf8');
}

describe('client tools + voice dictation toggle contract', () => {
    beforeEach(() => {
        const store = new Map<string, string>();
        vi.stubGlobal('localStorage', {
            getItem: (key: string) => store.get(key) ?? null,
            setItem: (key: string, value: string) => store.set(key, value),
            removeItem: (key: string) => store.delete(key),
            clear: () => store.clear(),
        });
        images.set([]);
        focusedIndex.set(0);
        selectedIds.set(new Set());
        collections.set([]);
        collectMode.set(false);
        collectModeTarget.set(null);
        staticPublishingEnabled.set(false);
        clientToolsEnabled.set(false);
        voiceDictationEnabled.set(false);
    });

    it('defaults both toggles to off', () => {
        expect(get(clientToolsEnabled)).toBe(false);
        expect(get(voiceDictationEnabled)).toBe(false);
    });

    it('hides the delivery CSV palette command unless Client Tools is enabled', () => {
        expect(getCommandPaletteItems('commands').map(i => i.id))
            .not.toContain('collection.export-delivery-csv');

        clientToolsEnabled.set(true);

        expect(getCommandPaletteItems('commands').map(i => i.id))
            .toContain('collection.export-delivery-csv');
    });

    it('hides the dictation controls in the command bar unless Voice Dictation is enabled', () => {
        const commandBar = readProjectFile('src/lib/components/CommandBar.svelte');
        const guard = commandBar.indexOf('{#if $voiceDictationEnabled}');

        expect(guard).toBeGreaterThan(-1);
        expect(commandBar.indexOf('class="locale-btn"')).toBeGreaterThan(guard);
        expect(commandBar.indexOf('class="mic-btn"')).toBeGreaterThan(guard);
    });

    it('exposes both toggles in Settings, default off, following the module pattern', () => {
        const settings = readProjectFile('src/lib/components/GeneralSettings.svelte');
        const page = readProjectFile('src/routes/+page.svelte');

        expect(settings).toContain("getAppSetting('module_client_tools')");
        expect(settings).toContain("getAppSetting('module_voice_dictation')");
        expect(settings).toContain("toggle('module_client_tools'");
        expect(settings).toContain("toggle('module_voice_dictation'");
        expect(settings).toContain("client === 'true'");
        expect(settings).toContain("voice === 'true'");
        expect(page).toContain("getAppSetting('module_client_tools')");
        expect(page).toContain("getAppSetting('module_voice_dictation')");
    });

    it('keeps client feedback, preview display, and preview web stream entry points ungated', () => {
        const ids = getCommandPaletteItems('commands').map(i => i.id);

        expect(ids).toContain('client.toggle-favorite');
        expect(ids).toContain('client.add-comment');

        const menu = readProjectFile('src/lib/menu.ts');
        expect(menu).toContain('openPreviewDisplay');
        expect(menu).toContain('startPreviewDisplayWebStream');
        expect(menu).not.toContain('clientToolsEnabled');
        expect(menu).not.toContain('voiceDictationEnabled');

        for (const source of ['src/lib/menu.ts', 'src/lib/components/GeneralSettings.svelte', 'src/routes/+page.svelte']) {
            const content = readProjectFile(source);
            expect(content).not.toContain('module_preview_display');
            expect(content).not.toContain('module_client_feedback');
            expect(content).not.toContain('module_preview_web_stream');
        }
    });
});
