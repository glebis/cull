import { describe, it, expect, beforeEach, vi } from 'vitest';
import { applySuggestedHotkey } from './loader';

describe('applySuggestedHotkey', () => {
    // The repo runs vitest in the node environment (no jsdom); stub a minimal
    // localStorage so setCommandHotkey has a backing store, matching the
    // existing command-palette.test.ts pattern.
    beforeEach(() => {
        const store = new Map<string, string>();
        vi.stubGlobal('localStorage', {
            getItem: (k: string) => store.get(k) ?? null,
            setItem: (k: string, v: string) => { store.set(k, v); },
            removeItem: (k: string) => { store.delete(k); },
            clear: () => { store.clear(); },
        });
        localStorage.clear();
    });

    it('binds the suggested hotkey when it is free', () => {
        const set = applySuggestedHotkey('cull-publish.publish', 'Cmd+9', { 'Cmd+9': undefined });
        expect(set).toBe(true);
    });

    it('does not bind when the hotkey collides with a built-in', () => {
        const set = applySuggestedHotkey('cull-publish.publish', 'Cmd+1', { 'Cmd+1': 'Grid view' });
        expect(set).toBe(false);
    });

    it('does not bind when the hotkey is already user-assigned to another command', () => {
        // simulate an existing user hotkey
        const set = applySuggestedHotkey('cull-publish.publish', 'Cmd+9', { 'Cmd+9': 'Some command' });
        expect(set).toBe(false);
    });
});
