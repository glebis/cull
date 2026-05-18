import { describe, expect, it } from 'vitest';
import {
    canShowShortcutReminder,
    markShortcutReminderShown,
    markShortcutUsed,
    MAX_SHORTCUT_REMINDER_SHOWS,
    SHORTCUT_USE_SUPPRESSION_COUNT,
    type ShortcutReminderRecord,
} from './shortcut-reminders';

describe('shortcut reminder helpers', () => {
    it('allows a reminder to be shown at most five times', () => {
        let record: ShortcutReminderRecord | undefined = undefined;

        for (let i = 0; i < MAX_SHORTCUT_REMINDER_SHOWS; i++) {
            expect(canShowShortcutReminder(record)).toBe(true);
            record = markShortcutReminderShown(record);
        }

        expect(record?.shown).toBe(MAX_SHORTCUT_REMINDER_SHOWS);
        expect(canShowShortcutReminder(record)).toBe(false);
    });

    it('suppresses reminders once the shortcut has been used twice', () => {
        let record: ShortcutReminderRecord | undefined = undefined;

        for (let i = 0; i < SHORTCUT_USE_SUPPRESSION_COUNT; i++) {
            expect(canShowShortcutReminder(record)).toBe(true);
            record = markShortcutUsed(record);
        }

        expect(record?.used).toBe(SHORTCUT_USE_SUPPRESSION_COUNT);
        expect(canShowShortcutReminder(record)).toBe(false);
    });

    it('keeps existing reminder count when recording shortcut use', () => {
        const shown = markShortcutReminderShown(undefined);
        const used = markShortcutUsed(shown);

        expect(used).toEqual({ shown: 1, used: 1 });
    });
});
