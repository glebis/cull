// @vitest-environment jsdom
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import '@testing-library/jest-dom/vitest';
import { cleanup, render, screen, waitFor } from '@testing-library/svelte';
import userEvent from '@testing-library/user-event';
import { get } from 'svelte/store';

const requestMocks = vi.hoisted(() => ({ requestConfirm: vi.fn() }));
const apiMocks = vi.hoisted(() => ({
    listSessions: vi.fn(),
    listCollections: vi.fn(),
    createSession: vi.fn(),
    listCanvases: vi.fn(),
    validateSessionFolder: vi.fn(),
    deleteSession: vi.fn(),
    convertSessionToCollection: vi.fn(),
}));

vi.mock('$lib/stores', async (importOriginal) => ({
    ...await importOriginal<typeof import('$lib/stores')>(),
    requestConfirm: requestMocks.requestConfirm,
}));
vi.mock('$lib/api', () => apiMocks);
vi.mock('@tauri-apps/plugin-opener', () => ({ revealItemInDir: vi.fn() }));

import SessionSwitcher from './SessionSwitcher.svelte';
import { activeCanvas, activeSession, collections, sessionCanvases, sessions, toasts } from '$lib/stores';

const session = {
    id: 'session-1', name: 'Review', description: null, folder_path: '/mock/review',
    settings_json: null, created_at: '2026-01-01', image_count: 2,
};
const canvas = {
    id: 'canvas-1', session_id: session.id, name: 'Selects', canvas_type: 'manual' as const,
    layout_json: '{}', filter_json: null, grid_config_json: null, sort_order: 0,
    created_at: '2026-01-01', updated_at: '2026-01-01',
};

afterEach(() => cleanup());
beforeEach(() => {
    vi.clearAllMocks();
    sessions.set([]);
    collections.set([]);
    activeSession.set(session);
    activeCanvas.set(canvas);
    sessionCanvases.set([canvas]);
    toasts.set([]);
    requestMocks.requestConfirm.mockResolvedValue(true);
    apiMocks.listSessions.mockResolvedValue([session]);
    apiMocks.listCollections.mockResolvedValue([]);
    apiMocks.listCanvases.mockResolvedValue([canvas]);
    apiMocks.validateSessionFolder.mockResolvedValue(true);
});

async function openSessionMenu(user: ReturnType<typeof userEvent.setup>) {
    await user.click(screen.getByRole('button', { name: /Review/ }));
    await user.click(await screen.findByRole('button', { name: 'Session actions: Review' }));
}

describe('SessionSwitcher context action behavior', () => {
    it('confirms safe session deletion and reconciles the active stores', async () => {
        const user = userEvent.setup();
        apiMocks.listSessions
            .mockResolvedValueOnce([session])
            .mockResolvedValue([]);
        render(SessionSwitcher);
        await openSessionMenu(user);
        await user.click(await screen.findByRole('menuitem', { name: 'Delete Session…' }));

        await waitFor(() => expect(apiMocks.deleteSession).toHaveBeenCalledWith('session-1', false));
        expect(requestMocks.requestConfirm).toHaveBeenCalledWith(expect.objectContaining({
            title: 'Delete Session',
            danger: true,
        }));
        expect(get(activeSession)).toBeNull();
        expect(get(activeCanvas)).toBeNull();
        expect(get(sessionCanvases)).toEqual([]);
        await waitFor(() => expect(get(sessions)).toEqual([]));

        cleanup();
        render(SessionSwitcher);
        await waitFor(() => expect(apiMocks.listSessions).toHaveBeenCalledTimes(3));
        await user.click(screen.getByRole('button', { name: /All Images/ }));
        expect(screen.queryByRole('button', { name: /Review/ })).not.toBeInTheDocument();
    });

    it('converts a session, refreshes collections, and remains converted after remount', async () => {
        const user = userEvent.setup();
        const converted = ['collection-1', 'Review', 2] as const;
        apiMocks.listSessions
            .mockResolvedValueOnce([session])
            .mockResolvedValue([]);
        apiMocks.listCollections.mockResolvedValue([converted]);
        apiMocks.convertSessionToCollection.mockResolvedValue(undefined);
        render(SessionSwitcher);
        await openSessionMenu(user);
        await user.click(await screen.findByRole('menuitem', { name: 'Convert to Collection…' }));

        await waitFor(() => expect(apiMocks.convertSessionToCollection).toHaveBeenCalledWith('session-1'));
        expect(get(activeSession)).toBeNull();
        expect(get(activeCanvas)).toBeNull();
        expect(get(sessionCanvases)).toEqual([]);
        await waitFor(() => expect(get(collections)).toEqual([converted]));

        cleanup();
        render(SessionSwitcher);
        await waitFor(() => expect(apiMocks.listSessions).toHaveBeenCalledTimes(3));
        await user.click(screen.getByRole('button', { name: /All Images/ }));
        expect(screen.queryByRole('button', { name: /Review/ })).not.toBeInTheDocument();
    });

    it('keeps session state intact and reports a failed conversion', async () => {
        const user = userEvent.setup();
        apiMocks.convertSessionToCollection.mockRejectedValueOnce(new Error('database busy'));
        render(SessionSwitcher);
        await openSessionMenu(user);
        await user.click(await screen.findByRole('menuitem', { name: 'Convert to Collection…' }));

        await waitFor(() => expect(apiMocks.convertSessionToCollection).toHaveBeenCalledWith('session-1'));
        expect(get(activeSession)).toEqual(session);
        expect(get(activeCanvas)).toEqual(canvas);
        expect(get(sessionCanvases)).toEqual([canvas]);
        expect(get(toasts).at(-1)).toMatchObject({
            message: 'Failed to convert session to collection',
            type: 'error',
        });
    });
});
