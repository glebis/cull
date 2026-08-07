// @vitest-environment jsdom

import { describe, expect, it } from 'vitest';

type SmokeModule = typeof import('./native-interaction-smoke');

async function loadSmokeModule(): Promise<Partial<SmokeModule>> {
    return import('./native-interaction-smoke').catch(() => ({}));
}

function installInteractiveFixture() {
    document.body.innerHTML = `
        <div class="folder-tree">
            <div class="folder-row"><button class="section-item"><span class="folder-label">Smoke Alpha</span></button></div>
            <div class="folder-row"><button class="section-item"><span class="folder-label">Smoke Beta</span></button></div>
        </div>
        <button class="section-item recent"><span class="item-label">Recent Imports</span></button>
        <div class="grid-container">
            <div class="thumb" aria-selected="false"></div>
            <div class="thumb" aria-selected="false"></div>
        </div>
        <input class="sidebar-filter-input" />
    `;

    const folderButtons = [...document.querySelectorAll<HTMLButtonElement>('.folder-row .section-item')];
    for (const button of folderButtons) {
        button.addEventListener('click', () => button.setAttribute('aria-current', 'true'));
    }

    document.querySelector<HTMLButtonElement>('.recent')!.addEventListener('click', (event) => {
        (event.currentTarget as HTMLButtonElement).setAttribute('aria-current', 'true');
    });

    const input = document.querySelector<HTMLInputElement>('.sidebar-filter-input')!;
    input.addEventListener('input', () => {
        for (const row of document.querySelectorAll<HTMLElement>('.folder-row')) {
            row.hidden = !row.textContent!.toLowerCase().includes(input.value.toLowerCase());
        }
    });

    const thumb = document.querySelector<HTMLElement>('.thumb')!;
    thumb.addEventListener('click', () => thumb.setAttribute('aria-selected', 'true'));
    thumb.addEventListener('contextmenu', (event) => {
        event.preventDefault();
        const menu = document.createElement('div');
        menu.className = 'context-menu';
        menu.setAttribute('role', 'menu');
        document.body.append(menu);
    });
    window.addEventListener('keydown', (event) => {
        if (event.key === 'Escape') document.querySelector('.context-menu')?.remove();
    }, { once: true });
    thumb.addEventListener('dblclick', () => {
        document.querySelector('.grid-container')?.remove();
        const loupe = document.createElement('div');
        loupe.className = 'loupe-container';
        document.body.append(loupe);
    });
}

describe('packaged native interaction smoke', () => {
    it('drives click, Recent Imports, filter, context menu, and double-click through observable UI outcomes', async () => {
        installInteractiveFixture();
        const smokeModule = await loadSmokeModule();
        const finishCodes: number[] = [];
        const screenshots: string[] = [];
        const recordedResults: unknown[] = [];

        expect(smokeModule.runNativeInteractionSmoke).toBeTypeOf('function');
        const result = await smokeModule.runNativeInteractionSmoke!({
            root: document,
            timeoutMs: 500,
            finish: async (code) => { finishCodes.push(code); },
            captureFailure: async (message) => { screenshots.push(message); },
            recordResult: async (result) => { recordedResults.push(result); },
            log: () => {},
            hitTest: target => target,
        });

        expect(result).toEqual({ ok: true, completed: [
            'folder-click',
            'recent-imports-click',
            'sidebar-filter',
            'image-click-selection',
            'image-context-menu',
            'image-double-click-loupe',
        ] });
        expect(finishCodes).toEqual([0]);
        expect(screenshots).toEqual([]);
        expect(recordedResults).toEqual([{ ok: true, completed: [
            'folder-click',
            'recent-imports-click',
            'sidebar-filter',
            'image-click-selection',
            'image-context-menu',
            'image-double-click-loupe',
        ] }]);
    });

    it('captures a failure artifact and exits non-zero when the event root does not react', async () => {
        installInteractiveFixture();
        const smokeModule = await loadSmokeModule();
        const folderButtons = document.querySelectorAll<HTMLButtonElement>('.folder-row .section-item');
        const folderButton = folderButtons[folderButtons.length - 1]!;
        folderButton.replaceWith(folderButton.cloneNode(true));
        const finishCodes: number[] = [];
        const screenshots: string[] = [];
        const recordedResults: unknown[] = [];

        expect(smokeModule.runNativeInteractionSmoke).toBeTypeOf('function');
        const result = await smokeModule.runNativeInteractionSmoke!({
            root: document,
            timeoutMs: 25,
            finish: async (code) => { finishCodes.push(code); },
            captureFailure: async (message) => { screenshots.push(message); },
            recordResult: async (result) => { recordedResults.push(result); },
            log: () => {},
            hitTest: target => target,
        });

        expect(result.ok).toBe(false);
        expect(result.completed).toEqual([]);
        expect(result.error).toContain('folder click did not activate');
        expect(finishCodes).toEqual([1]);
        expect(screenshots).toHaveLength(1);
        expect(recordedResults).toEqual([expect.objectContaining({
            ok: false,
            error: expect.stringContaining('folder click did not activate'),
        })]);
    });

    it('fails before dispatch when an overlay owns the pointer hit target', async () => {
        installInteractiveFixture();
        const smokeModule = await loadSmokeModule();
        const overlay = document.createElement('div');
        overlay.className = 'blocking-overlay';
        document.body.append(overlay);
        const finishCodes: number[] = [];
        const screenshots: string[] = [];
        const recordedResults: unknown[] = [];

        expect(smokeModule.runNativeInteractionSmoke).toBeTypeOf('function');
        const result = await smokeModule.runNativeInteractionSmoke!({
            root: document,
            timeoutMs: 25,
            finish: async (code) => { finishCodes.push(code); },
            captureFailure: async (message) => { screenshots.push(message); },
            recordResult: async (result) => { recordedResults.push(result); },
            log: () => {},
            hitTest: () => overlay,
        });

        expect(result.ok).toBe(false);
        expect(result.error).toContain('blocked by div.blocking-overlay');
        expect(finishCodes).toEqual([1]);
        expect(screenshots).toEqual([expect.stringContaining('blocked by div.blocking-overlay')]);
        expect(recordedResults).toEqual([expect.objectContaining({ ok: false })]);
    });
});
