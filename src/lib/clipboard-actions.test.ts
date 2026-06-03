import { describe, expect, it } from 'vitest';
import {
    CLIPBOARD_PASTE_DATE_FORMAT_SETTING,
    DEFAULT_CLIPBOARD_PASTE_DATE_FORMAT,
    pasteDestinationForContext,
} from './clipboard-actions';

describe('clipboard image actions', () => {
    it('uses the active folder as the paste destination', () => {
        expect(pasteDestinationForContext('/Library/Active', '/Library/Other/image.png')).toBe('/Library/Active');
    });

    it('falls back to the focused image parent folder', () => {
        expect(pasteDestinationForContext(null, '/Library/Other/image.png')).toBe('/Library/Other');
    });

    it('returns null when no folder context is available', () => {
        expect(pasteDestinationForContext(null, null)).toBeNull();
    });

    it('defines the persisted date format setting key and default', () => {
        expect(CLIPBOARD_PASTE_DATE_FORMAT_SETTING).toBe('clipboard_paste_date_format');
        expect(DEFAULT_CLIPBOARD_PASTE_DATE_FORMAT).toBe('%Y-%m-%d');
    });
});
