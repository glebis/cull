import { describe, expect, it } from 'vitest';
import { readFileSync } from 'node:fs';
import { join } from 'node:path';

function source(path: string): string {
    return readFileSync(join(process.cwd(), path), 'utf8');
}

function functionBody(sourceText: string, signature: string): string {
    const start = sourceText.indexOf(signature);
    if (start < 0) throw new Error(`Missing function signature: ${signature}`);
    const next = sourceText.indexOf('\n#[tauri::command]', start + signature.length);
    return next < 0 ? sourceText.slice(start) : sourceText.slice(start, next);
}

const panel = source('src/lib/components/UndoHistoryPanel.svelte');
const selectionCommands = source('src-tauri/src/commands/selection.rs');
const libraryCommands = source('src-tauri/src/commands/library.rs');

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

    it('documents the commands that currently feed action history', () => {
        expect(selectionCommands).toContain('Action::SetRating');
        expect(selectionCommands).toContain('Action::SetDecision');
        expect(libraryCommands).toContain('"trash_image"');

        const permanentDelete = functionBody(libraryCommands, 'pub async fn delete_images_permanently');
        expect(permanentDelete).not.toContain('record_action');
    });
});
