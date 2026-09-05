export interface NativeInteractionSmokeResult {
    ok: boolean;
    completed: string[];
    error?: string;
}

export interface NativeInteractionSmokeOptions {
    root?: Document;
    timeoutMs?: number;
    finish?: (code: number) => Promise<void>;
    captureFailure?: (message: string) => Promise<void>;
    recordResult?: (result: NativeInteractionSmokeResult) => Promise<void>;
    log?: (message: string) => void;
    hitTest?: (target: Element, x: number, y: number) => Element | null;
    selectionPersistence?: () => Promise<{ resumed: boolean; completed: string[] }>;
}

const DEFAULT_TIMEOUT_MS = 15_000;

function visibleFolderRows(root: Document): HTMLElement[] {
    return [...root.querySelectorAll<HTMLElement>('.folder-tree .folder-row')]
        .filter(row => !row.hidden && row.getAttribute('aria-hidden') !== 'true');
}

function requireElement<T extends Element>(root: ParentNode, selector: string, description: string): T {
    const element = root.querySelector<T>(selector);
    if (!element) throw new Error(`${description} is missing`);
    return element;
}

function recentImportsButton(root: Document): HTMLButtonElement {
    const button = [...root.querySelectorAll<HTMLButtonElement>('button.section-item')]
        .find(candidate => candidate.querySelector('.item-label')?.textContent?.trim() === 'Recent Imports');
    if (!button) throw new Error('Recent Imports button is missing');
    return button;
}

function describeElement(element: Element | null): string {
    if (!element) return '<none>';
    const id = element.id ? `#${element.id}` : '';
    const classes = [...element.classList].map(value => `.${value}`).join('');
    return `${element.tagName.toLowerCase()}${id}${classes}`;
}

function dispatchMouse(root: Document, target: Element, type: string, init: MouseEventInit = {}) {
    const windowRef = root.defaultView;
    const EventCtor = type.startsWith('pointer') && windowRef?.PointerEvent
        ? windowRef.PointerEvent
        : (windowRef?.MouseEvent ?? MouseEvent);
    target.dispatchEvent(new EventCtor(type, { bubbles: true, cancelable: true, ...init }));
}

function dispatchPointerAction(
    root: Document,
    target: Element,
    action: 'click' | 'dblclick' | 'contextmenu',
    hitTest: NativeInteractionSmokeOptions['hitTest'],
    init: MouseEventInit = {},
) {
    const rect = target.getBoundingClientRect();
    const x = rect.left + rect.width / 2;
    const y = rect.top + rect.height / 2;
    const hit = hitTest
        ? hitTest(target, x, y)
        : root.elementFromPoint(x, y);
    if (!hit || (hit !== target && !target.contains(hit))) {
        throw new Error(
            `${action} hit-test for ${describeElement(target)} was blocked by ${describeElement(hit)}`,
        );
    }

    const button = action === 'contextmenu' ? 2 : 0;
    const buttons = action === 'contextmenu' ? 2 : 1;
    const base = { clientX: x, clientY: y, button, buttons, ...init };
    dispatchMouse(root, hit, 'pointerdown', base);
    dispatchMouse(root, hit, 'pointerup', { ...base, buttons: 0 });
    if (action === 'click') {
        dispatchMouse(root, hit, 'click', { ...base, buttons: 0 });
    } else if (action === 'dblclick') {
        dispatchMouse(root, hit, 'click', { ...base, buttons: 0, detail: 1 });
        dispatchMouse(root, hit, 'click', { ...base, buttons: 0, detail: 2 });
        dispatchMouse(root, hit, 'dblclick', { ...base, buttons: 0, detail: 2 });
    } else {
        dispatchMouse(root, hit, 'contextmenu', base);
    }
}

function setBoundInput(root: Document, input: HTMLInputElement, value: string) {
    const InputCtor = root.defaultView?.HTMLInputElement ?? HTMLInputElement;
    const setter = Object.getOwnPropertyDescriptor(InputCtor.prototype, 'value')?.set;
    if (!setter) throw new Error('native input value setter is unavailable');
    setter.call(input, value);
    const EventCtor = root.defaultView?.Event ?? Event;
    input.dispatchEvent(new EventCtor('input', { bubbles: true, cancelable: true }));
}

function dispatchEscape(root: Document) {
    const KeyboardEventCtor = root.defaultView?.KeyboardEvent ?? KeyboardEvent;
    (root.defaultView ?? window).dispatchEvent(new KeyboardEventCtor('keydown', {
        key: 'Escape',
        code: 'Escape',
        bubbles: true,
        cancelable: true,
    }));
}

async function waitFor(
    predicate: () => boolean,
    failure: string,
    timeoutMs: number,
): Promise<void> {
    const deadline = Date.now() + timeoutMs;
    while (Date.now() <= deadline) {
        if (predicate()) return;
        await new Promise(resolve => setTimeout(resolve, 20));
    }
    throw new Error(failure);
}

async function defaultCaptureFailure(message: string): Promise<void> {
    const [{ captureAgentWindowSnapshot, completeAgentViewSnapshot }, { toPng }] = await Promise.all([
        import('$lib/api'),
        import('html-to-image'),
    ]);
    const snapshotId = `native_interaction_smoke_failure_${Date.now()}`;
    let rawPng: string;
    try {
        rawPng = await captureAgentWindowSnapshot();
    } catch (nativeCaptureError) {
        console.error('[native-interaction-smoke] native capture failed; using DOM capture', nativeCaptureError);
        rawPng = await toPng(document.documentElement, {
            cacheBust: true,
            pixelRatio: 1,
            skipFonts: true,
        });
    }

    await completeAgentViewSnapshot({
        snapshot_id: snapshotId,
        manifest: {
            schema_version: 1,
            snapshot_id: snapshotId,
            created_at: new Date().toISOString(),
            capture_reason: 'native-interaction-smoke-failure',
            error: message,
        },
        raw_png_base64: rawPng,
        annotated_png_base64: rawPng,
        clipboard: false,
    });
}

async function defaultFinish(code: number): Promise<void> {
    const { exit } = await import('@tauri-apps/plugin-process');
    await exit(code);
}

const ONE_PIXEL_PNG_BASE64 = 'iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAQAAAC1HAwCAAAAC0lEQVR42mNk+A8AAQUBAScY42YAAAAASUVORK5CYII=';

async function defaultRecordResult(result: NativeInteractionSmokeResult): Promise<void> {
    const { completeAgentViewSnapshot } = await import('$lib/api');
    await completeAgentViewSnapshot({
        snapshot_id: 'native_interaction_smoke_result',
        manifest: {
            schema_version: 1,
            capture_reason: 'native-interaction-smoke-result',
            smoke_result: result,
        },
        raw_png_base64: ONE_PIXEL_PNG_BASE64,
        annotated_png_base64: ONE_PIXEL_PNG_BASE64,
        clipboard: false,
    });
}

export async function runNativeInteractionSmoke(
    options: NativeInteractionSmokeOptions = {},
): Promise<NativeInteractionSmokeResult> {
    const root = options.root ?? document;
    const timeoutMs = options.timeoutMs ?? DEFAULT_TIMEOUT_MS;
    const finish = options.finish ?? defaultFinish;
    const captureFailure = options.captureFailure ?? defaultCaptureFailure;
    const recordResult = options.recordResult ?? defaultRecordResult;
    const log = options.log ?? console.log;
    const hitTest = options.hitTest;
    const completed: string[] = [];

    try {
        {
            const checkSelection = options.selectionPersistence ?? (async () => {
                const { runNativeSelectionPersistenceSmoke } = await import('./native-selection-smoke');
                return runNativeSelectionPersistenceSmoke();
            });
            const selection = await checkSelection();
            completed.push(...selection.completed);
            if (selection.resumed) {
                const result = { ok: true, completed } satisfies NativeInteractionSmokeResult;
                log(`[native-interaction-smoke] PASS ${completed.join(', ')}`);
                await recordResult(result);
                await finish(0);
                return result;
            }
        }
        await waitFor(
            () => visibleFolderRows(root).length >= 1 && root.querySelectorAll('.thumb').length >= 2,
            'seeded folder and images did not render',
            timeoutMs,
        );
        if (visibleFolderRows(root).length < 2) {
            const expandButton = requireElement<HTMLButtonElement>(
                visibleFolderRows(root)[0],
                '.twisty',
                'seeded folder expand button',
            );
            dispatchPointerAction(root, expandButton, 'click', hitTest);
            await waitFor(
                () => visibleFolderRows(root).length >= 2,
                'seeded folder tree did not expand',
                timeoutMs,
            );
        }

        const folderButton = requireElement<HTMLButtonElement>(
            visibleFolderRows(root).at(-1)!,
            '.section-item',
            'folder button',
        );
        dispatchPointerAction(root, folderButton, 'click', hitTest);
        await waitFor(
            () => folderButton.getAttribute('aria-current') === 'true' && root.querySelectorAll('.thumb').length >= 1,
            'folder click did not activate the folder',
            timeoutMs,
        );
        completed.push('folder-click');

        const recentButton = recentImportsButton(root);
        dispatchPointerAction(root, recentButton, 'click', hitTest);
        await waitFor(
            () => recentButton.getAttribute('aria-current') === 'true' && root.querySelectorAll('.thumb').length >= 2,
            'Recent Imports click did not activate the smart collection',
            timeoutMs,
        );
        completed.push('recent-imports-click');

        const filterInput = requireElement<HTMLInputElement>(
            root,
            '.sidebar-filter-input',
            'sidebar filter',
        );
        const foldersBeforeFilter = visibleFolderRows(root);
        const filterValue = foldersBeforeFilter.at(-1)?.querySelector('.folder-label')?.textContent?.trim();
        if (!filterValue) throw new Error('seeded folder label is empty');
        setBoundInput(root, filterInput, filterValue);
        await waitFor(
            () => {
                const rows = visibleFolderRows(root);
                return rows.length > 0 && rows.length < foldersBeforeFilter.length;
            },
            'sidebar filter input did not change visible folder results',
            timeoutMs,
        );
        setBoundInput(root, filterInput, '');
        await waitFor(
            () => visibleFolderRows(root).length === foldersBeforeFilter.length,
            'clearing sidebar filter did not restore folder results',
            timeoutMs,
        );
        completed.push('sidebar-filter');

        const thumb = requireElement<HTMLElement>(root, '.thumb', 'image thumbnail');
        const selectionBefore = thumb.getAttribute('aria-selected');
        dispatchPointerAction(root, thumb, 'click', hitTest, { metaKey: true });
        await waitFor(
            () => thumb.getAttribute('aria-selected') !== selectionBefore,
            'image click did not change selection',
            timeoutMs,
        );
        completed.push('image-click-selection');

        dispatchPointerAction(root, thumb, 'contextmenu', hitTest);
        await waitFor(
            () => root.querySelector('.context-menu, [role="menu"]') !== null,
            'image context menu did not appear',
            timeoutMs,
        );
        completed.push('image-context-menu');
        dispatchEscape(root);
        await waitFor(
            () => root.querySelector('.context-menu, [role="menu"]') === null,
            'image context menu did not close with Escape',
            timeoutMs,
        );

        dispatchPointerAction(root, thumb, 'dblclick', hitTest);
        await waitFor(
            () => root.querySelector('.loupe-container') !== null,
            'image double-click did not open Loupe',
            timeoutMs,
        );
        completed.push('image-double-click-loupe');

        log(`[native-interaction-smoke] PASS ${completed.join(', ')}`);
        const result = { ok: true, completed } satisfies NativeInteractionSmokeResult;
        await recordResult(result);
        await finish(0);
        return result;
    } catch (error) {
        const message = error instanceof Error ? error.message : String(error);
        log(`[native-interaction-smoke] FAIL ${message}`);
        try {
            await captureFailure(message);
        } catch (captureError) {
            log(`[native-interaction-smoke] failure capture also failed: ${String(captureError)}`);
        }
        const result = { ok: false, completed, error: message } satisfies NativeInteractionSmokeResult;
        try {
            await recordResult(result);
        } catch (recordError) {
            log(`[native-interaction-smoke] result recording also failed: ${String(recordError)}`);
        }
        await finish(1);
        return result;
    }
}
