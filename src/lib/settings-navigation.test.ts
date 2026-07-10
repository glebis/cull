import { get } from 'svelte/store';
import { beforeEach, describe, expect, it } from 'vitest';
import { settingsOpen } from './stores';
import { openSettings, settingsTab } from './settings-navigation';

describe('settings navigation', () => {
    beforeEach(() => {
        settingsOpen.set(false);
        settingsTab.set('plugins');
    });

    it('opens Settings at an explicit tab', () => {
        openSettings('ai');
        expect(get(settingsOpen)).toBe(true);
        expect(get(settingsTab)).toBe('ai');
    });

    it('defaults generic opens to General', () => {
        openSettings();
        expect(get(settingsOpen)).toBe(true);
        expect(get(settingsTab)).toBe('general');
    });
});
