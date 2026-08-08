// @vitest-environment jsdom
import { beforeEach, describe, expect, it } from 'vitest';
import { get } from 'svelte/store';
import { restoreAppStateBeforeImages, saveAppState } from './persistence';
import { showRejected } from './stores';
describe('rejected visibility persistence', () => {
    beforeEach(() => { localStorage.clear(); showRejected.set(false); });
    it('defaults old state to hidden and restores an explicit Show Rejected choice', () => { saveAppState(); showRejected.set(true); restoreAppStateBeforeImages(); expect(get(showRejected)).toBe(false); showRejected.set(true); saveAppState(); showRejected.set(false); restoreAppStateBeforeImages(); expect(get(showRejected)).toBe(true); });
});
