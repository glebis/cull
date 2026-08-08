import { beforeEach, describe, expect, it } from 'vitest';
import {
    getCommandPaletteItems,
    isCommandPaletteItemVisible,
    scoreCommandPaletteItem,
    type CommandPaletteItem,
} from './command-palette';
import {
    previewDisplayAlwaysOnTop,
    previewDisplayBlanked,
    previewDisplayFrozen,
    previewDisplayLayout,
    previewDisplayMode,
    previewDisplayOverlay,
    previewDisplayWebStreamStatus,
} from './preview-display-store';
import { DEFAULT_PREVIEW_OVERLAY } from './preview-display';

const IDLE_STREAM = {
    active: false,
    url: null,
    host: null,
    bound_host: null,
    port: null,
    remote_access: false,
};

const LIVE_STREAM = {
    active: true,
    url: 'http://127.0.0.1:8899/',
    host: '127.0.0.1',
    bound_host: '127.0.0.1',
    port: 8899,
    remote_access: false,
};

/** Preview commands as Cmd+P would present them: command-only, `when` applied. */
function previewCommands(): CommandPaletteItem[] {
    return getCommandPaletteItems('commands')
        .filter(item => item.category === 'Preview')
        .filter(isCommandPaletteItemVisible);
}

function byId(id: string): CommandPaletteItem | undefined {
    return previewCommands().find(item => item.id === id);
}

describe('preview display command palette entries', () => {
    beforeEach(() => {
        previewDisplayFrozen.set(false);
        previewDisplayBlanked.set(false);
        previewDisplayAlwaysOnTop.set(false);
        previewDisplayMode.set('image_only');
        previewDisplayLayout.set('single');
        previewDisplayOverlay.set(DEFAULT_PREVIEW_OVERLAY);
        previewDisplayWebStreamStatus.set(IDLE_STREAM);
    });

    it('exposes every preview command in the Cmd+P command-only palette', () => {
        const ids = previewCommands().map(item => item.id);
        expect(ids).toEqual(expect.arrayContaining([
            'preview.open',
            'preview.move-monitor',
            'preview.fullscreen',
            'preview.toggle-always-on-top',
            'preview.toggle-freeze',
            'preview.toggle-blank',
            'preview.preset-image_only',
            'preview.preset-client_review',
            'preview.preset-metadata_review',
            'preview.layout-single',
            'preview.layout-compare',
            'preview.layout-grid',
            'preview.field-filename',
            'preview.field-rating',
            'preview.field-decision',
            'preview.field-dimensions',
            'preview.field-format',
            'preview.copy-to-clipboard',
            'preview.export-png',
            'preview.start-web-stream',
            'preview.start-lan-web-stream',
        ]));
    });

    it('registers them as commands, not destinations, and adds no shortcuts', () => {
        for (const item of previewCommands()) {
            expect(item.kind).toBe('command');
            expect(item.defaultShortcut).toBeUndefined();
        }
    });

    it('flips the freeze toggle title and subtitle with store state', () => {
        expect(byId('preview.toggle-freeze')?.title).toBe('Freeze Preview Display');
        previewDisplayFrozen.set(true);
        expect(byId('preview.toggle-freeze')?.title).toBe('Unfreeze Preview Display');
        expect(byId('preview.toggle-freeze')?.subtitle).toContain('Resume');
    });

    it('flips the blank toggle with store state', () => {
        expect(byId('preview.toggle-blank')?.title).toBe('Blank Preview Display');
        previewDisplayBlanked.set(true);
        expect(byId('preview.toggle-blank')?.title).toBe('Unblank Preview Display');
    });

    it('flips the always-on-top toggle with store state', () => {
        expect(byId('preview.toggle-always-on-top')?.title).toContain('Always on Top');
        previewDisplayAlwaysOnTop.set(true);
        expect(byId('preview.toggle-always-on-top')?.title).toContain('Normal Stacking');
    });

    it('flips overlay field toggles with the overlay config', () => {
        previewDisplayOverlay.set({ ...DEFAULT_PREVIEW_OVERLAY, showRating: false });
        expect(byId('preview.field-rating')?.title).toBe('Preview Overlay: Show Rating');
        previewDisplayOverlay.set({ ...DEFAULT_PREVIEW_OVERLAY, showRating: true });
        expect(byId('preview.field-rating')?.title).toBe('Preview Overlay: Hide Rating');
    });

    it('marks the active preset and layout as current', () => {
        previewDisplayMode.set('client_review');
        previewDisplayLayout.set('grid');
        expect(byId('preview.preset-client_review')?.subtitle).toContain('(current)');
        expect(byId('preview.preset-image_only')?.subtitle).not.toContain('(current)');
        expect(byId('preview.layout-grid')?.subtitle).toContain('(current)');
        expect(byId('preview.layout-single')?.subtitle).not.toContain('(current)');
    });

    it('hides copy-url and stop while no web stream is running', () => {
        expect(byId('preview.copy-web-stream-url')).toBeUndefined();
        expect(byId('preview.stop-web-stream')).toBeUndefined();
        expect(byId('preview.start-web-stream')).toBeDefined();
        expect(byId('preview.start-lan-web-stream')).toBeDefined();
    });

    it('swaps start for copy/stop once a web stream is live', () => {
        previewDisplayWebStreamStatus.set(LIVE_STREAM);
        expect(byId('preview.start-web-stream')).toBeUndefined();
        expect(byId('preview.start-lan-web-stream')).toBeUndefined();
        expect(byId('preview.copy-web-stream-url')?.subtitle).toBe(LIVE_STREAM.url);
        expect(byId('preview.stop-web-stream')).toBeDefined();
    });

    it.each(['second screen', 'projector', 'beamer', 'external display'])(
        'finds preview commands when searching %s',
        (query) => {
            const matches = previewCommands().filter(item => scoreCommandPaletteItem(query, item) > 0);
            expect(matches.length).toBeGreaterThan(0);
        },
    );
});
