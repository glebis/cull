import { describe, expect, it } from 'vitest';
import { readFileSync } from 'node:fs';
import { join } from 'node:path';

function source(path: string): string {
    return readFileSync(join(process.cwd(), path), 'utf8');
}

function functionBody(sourceText: string, signature: string): string {
    const start = sourceText.indexOf(signature);
    if (start < 0) throw new Error(`Missing function signature: ${signature}`);
    const searchFrom = start + signature.length;
    const next = [
        sourceText.indexOf('\n#[tauri::command]', searchFrom),
        sourceText.indexOf('\nexport async function ', searchFrom),
    ].filter(index => index >= 0).sort((a, b) => a - b)[0] ?? -1;
    return next < 0 ? sourceText.slice(start) : sourceText.slice(start, next);
}

const panel = source('src/lib/components/UndoHistoryPanel.svelte');
const selectionCommands = source('src-tauri/src/commands/selection.rs');
const libraryCommands = source('src-tauri/src/commands/library.rs');
const collectionCommands = source('src-tauri/src/commands/collections.rs');
const smartCollectionCommands = source('src-tauri/src/commands/smart_collections.rs');
const fileCommands = source('src-tauri/src/commands/files.rs');
const transformCommands = source('src-tauri/src/commands/transform.rs');
const api = source('src/lib/api.ts');

describe('undo history panel contract', () => {
    it('renders an illustrated empty state instead of a blank list', () => {
        expect(panel).toContain('class="history-empty"');
        expect(panel).toContain('class="history-empty-image"');
        expect(panel).toContain('role="img"');
        expect(panel).toContain('No undoable actions yet');
        expect(panel).not.toContain('<p class="history-state">No action history yet.</p>');
    });

    it('names every currently recorded undo history action type', () => {
        expect(panel).toContain("if (actionType === 'set_rating') return 'Set rating';");
        expect(panel).toContain("if (actionType === 'set_decision') return 'Set decision';");
        expect(panel).toContain("if (actionType === 'trash_image') return 'Move to Trash';");
    });

    it('loads undo records and critical activity as separate history sections', () => {
        expect(panel).toContain('listUndoHistory(40)');
        expect(panel).toContain('getActivityContext(null, 40)');
        expect(panel).toContain('Undoable actions');
        expect(panel).toContain('Critical activity');
        expect(panel).toContain('image_deleted_permanently');
        expect(panel).toContain("window.addEventListener('session-events-refresh', onReload);");
    });

    it('renders readable history rows without expandable diagnostic output', () => {
        expect(panel).toContain('class="history-summary"');
        expect(panel).toContain('class="history-time"');
        expect(panel).toContain('flex: 0 0 auto;');
        expect(panel).not.toContain('class="history-row"');
        expect(panel).not.toContain('class="history-details"');
        expect(panel).not.toContain('<pre>{formatJson(');
        expect(panel).not.toContain('Affected image IDs');
        expect(panel).not.toContain('Event ID');
    });

    it('does not repeat undo-backed activity in the secondary activity section', () => {
        expect(panel).toContain('const undoBackedEventTypes = new Set([');
        expect(panel).toContain("'rating_set'");
        expect(panel).toContain("'decision_set'");
        expect(panel).toContain('activityEvents = activity.recent_events.filter');
    });

    it('documents the commands that currently feed action history', () => {
        expect(selectionCommands).toContain('Action::SetRating');
        expect(selectionCommands).toContain('Action::SetDecision');
        expect(libraryCommands).toContain('"trash_image"');

        const permanentDelete = functionBody(libraryCommands, 'pub async fn delete_images_permanently');
        expect(permanentDelete).not.toContain('record_action');
    });

    it('records critical library mutations as activity events', () => {
        for (const [sourceText, eventType] of [
            [selectionCommands, 'rating_set'],
            [selectionCommands, 'decision_set'],
            [selectionCommands, 'client_feedback_set'],
            [collectionCommands, 'collection_items_removed'],
            [smartCollectionCommands, 'smart_collection_created'],
            [smartCollectionCommands, 'smart_collection_updated'],
            [smartCollectionCommands, 'smart_collection_deleted'],
            [libraryCommands, 'folder_removed_from_library'],
            [libraryCommands, 'image_moved_to_trash'],
            [libraryCommands, 'image_deleted_permanently'],
            [fileCommands, 'clipboard_image_pasted'],
            [fileCommands, 'image_moved'],
            [fileCommands, 'image_renamed'],
            [fileCommands, 'folder_created'],
            [transformCommands, 'image_cropped'],
            [transformCommands, 'image_rotated'],
        ] as const) {
            expect(sourceText, eventType).toContain(eventType);
        }
    });

    it('refreshes activity after frontend critical mutation wrappers complete', () => {
        for (const fnName of [
            'deleteFolder',
            'createCollection',
            'addToCollection',
            'removeFromCollection',
            'deleteCollectionApi',
            'setClientFeedback',
            'createSmartCollection',
            'deleteSmartCollectionApi',
            'updateSmartCollectionApi',
            'trashImages',
            'trashImagesDetailed',
            'deleteImagesPermanently',
            'cropImage',
            'rotateImage',
            'pasteImageFromClipboard',
            'moveImage',
            'renameImage',
            'createSubfolder',
        ]) {
            const body = functionBody(api, `export async function ${fnName}`);
            expect(body, fnName).toContain('emitSessionEventsRefresh();');
        }
    });
});
