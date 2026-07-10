import { beforeEach, describe, expect, it, vi } from 'vitest';
import { invoke } from '@tauri-apps/api/core';
import { getUndoStatus, listUndoHistory, redo, undo, type UndoRecord, type UndoStatus } from './api';

vi.mock('@tauri-apps/api/core', () => ({
    invoke: vi.fn(),
}));

const invokeMock = vi.mocked(invoke);

describe('undo API wrappers', () => {
    beforeEach(() => {
        invokeMock.mockReset();
    });

    it('loads typed undo status through the registered Tauri command', async () => {
        const status: UndoStatus = {
            can_undo: true,
            can_redo: false,
            undo_label: 'Set rating to 5',
            redo_label: null,
            stack_depth: 3,
        };
        invokeMock.mockResolvedValueOnce(status);

        await expect(getUndoStatus()).resolves.toBe(status);

        expect(invokeMock).toHaveBeenCalledWith('get_undo_status');
    });

    it('loads undo history with an explicit nullable limit', async () => {
        const history: UndoRecord[] = [
            {
                seq: 7,
                id: 'undo-7',
                action_type: 'set_decision',
                label: 'Set decision to accepted',
                before_json: '{"decision":"undecided"}',
                after_json: '{"decision":"accepted"}',
                affected_image_ids: 'img-1',
                group_id: null,
                has_file_backup: false,
                created_at: '2026-06-22T20:30:00Z',
            },
        ];
        invokeMock.mockResolvedValueOnce(history);

        await expect(listUndoHistory(12)).resolves.toBe(history);

        expect(invokeMock).toHaveBeenCalledWith('list_undo_history', { limit: 12 });
    });

    it('normalizes omitted undo history limit to null', async () => {
        invokeMock.mockResolvedValueOnce([]);

        await expect(listUndoHistory()).resolves.toEqual([]);

        expect(invokeMock).toHaveBeenCalledWith('list_undo_history', { limit: null });
    });

    it('keeps undo and redo return values typed as nullable action labels', async () => {
        invokeMock.mockResolvedValueOnce('Set rating to 4').mockResolvedValueOnce(null);

        await expect(undo()).resolves.toBe('Set rating to 4');
        await expect(redo()).resolves.toBeNull();

        expect(invokeMock).toHaveBeenNthCalledWith(1, 'undo');
        expect(invokeMock).toHaveBeenNthCalledWith(2, 'redo');
    });
});
