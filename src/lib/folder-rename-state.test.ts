import { describe, expect, it } from 'vitest';
import type { Canvas, Session } from '$lib/api';
import { reconcileRenamedCanvas, reconcileRenamedSession, renamedFolderPath } from './folder-rename-state';

const session: Session = {
    id: 'session', name: 'Session', description: null, folder_path: '/library/old/session',
    settings_json: null, created_at: '2026-08-08', image_count: 1,
};
const canvas: Canvas = {
    id: 'canvas', session_id: 'session', name: 'Canvas', canvas_type: 'manual',
    layout_json: JSON.stringify({ items: [{ source: { lastKnownPath: '/library/old/image.png' } }], label: '/library/old/unchanged' }),
    filter_json: null, grid_config_json: null, sort_order: 0, created_at: '2026-08-08', updated_at: '2026-08-08',
};

describe('folder rename live-state reconciliation', () => {
    it('rewrites only exact and descendant paths', () => {
        expect(renamedFolderPath('/library/old', '/library/old', '/library/new')).toBe('/library/new');
        expect(renamedFolderPath('/library/oldish/a', '/library/old', '/library/new')).toBe('/library/oldish/a');
    });

    it('keeps session and active canvas state aligned with persisted paths', () => {
        expect(reconcileRenamedSession(session, '/library/old', '/library/new').folder_path).toBe('/library/new/session');
        const updated = reconcileRenamedCanvas(canvas, '/library/old', '/library/new');
        expect(JSON.parse(updated.layout_json)).toEqual({
            items: [{ source: { lastKnownPath: '/library/new/image.png' } }],
            label: '/library/old/unchanged',
        });
    });
});
