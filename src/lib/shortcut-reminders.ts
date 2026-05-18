export const SHORTCUT_REMINDER_STORAGE_KEY = 'cull.shortcutReminders.v1';
export const VIEW_CYCLE_SHORTCUT_REMINDER_ID = 'view-cycle';
export const MAX_SHORTCUT_REMINDER_SHOWS = 5;
export const SHORTCUT_USE_SUPPRESSION_COUNT = 2;

export interface ShortcutReminderRecord {
    shown: number;
    used: number;
}

export type ShortcutReminderState = Record<string, ShortcutReminderRecord>;

function storageAvailable(): boolean {
    return typeof localStorage !== 'undefined';
}

function normalizeCount(value: unknown): number {
    if (typeof value !== 'number' || !Number.isFinite(value)) return 0;
    return Math.max(0, Math.trunc(value));
}

export function normalizeShortcutReminderRecord(
    record: Partial<ShortcutReminderRecord> | null | undefined
): ShortcutReminderRecord {
    return {
        shown: normalizeCount(record?.shown),
        used: normalizeCount(record?.used),
    };
}

function readShortcutReminderState(): ShortcutReminderState {
    if (!storageAvailable()) return {};
    try {
        const raw = localStorage.getItem(SHORTCUT_REMINDER_STORAGE_KEY);
        if (!raw) return {};
        const parsed = JSON.parse(raw);
        if (!parsed || typeof parsed !== 'object' || Array.isArray(parsed)) return {};
        return Object.fromEntries(
            Object.entries(parsed).map(([id, record]) => [
                id,
                normalizeShortcutReminderRecord(record as Partial<ShortcutReminderRecord>),
            ])
        );
    } catch {
        return {};
    }
}

function writeShortcutReminderState(state: ShortcutReminderState) {
    if (!storageAvailable()) return;
    localStorage.setItem(SHORTCUT_REMINDER_STORAGE_KEY, JSON.stringify(state));
}

export function canShowShortcutReminder(record: Partial<ShortcutReminderRecord> | null | undefined): boolean {
    const normalized = normalizeShortcutReminderRecord(record);
    return normalized.shown < MAX_SHORTCUT_REMINDER_SHOWS &&
        normalized.used < SHORTCUT_USE_SUPPRESSION_COUNT;
}

export function markShortcutReminderShown(
    record: Partial<ShortcutReminderRecord> | null | undefined
): ShortcutReminderRecord {
    const normalized = normalizeShortcutReminderRecord(record);
    if (!canShowShortcutReminder(normalized)) return normalized;
    return {
        ...normalized,
        shown: normalized.shown + 1,
    };
}

export function markShortcutUsed(
    record: Partial<ShortcutReminderRecord> | null | undefined
): ShortcutReminderRecord {
    const normalized = normalizeShortcutReminderRecord(record);
    return {
        ...normalized,
        used: normalized.used + 1,
    };
}

function updateShortcutReminderRecord(
    id: string,
    update: (record: ShortcutReminderRecord) => ShortcutReminderRecord
): ShortcutReminderRecord {
    const state = readShortcutReminderState();
    const next = update(normalizeShortcutReminderRecord(state[id]));
    state[id] = next;
    writeShortcutReminderState(state);
    return next;
}

export function recordShortcutUse(id: string): ShortcutReminderRecord {
    return updateShortcutReminderRecord(id, markShortcutUsed);
}

export function maybeShowShortcutReminder(id: string, show: () => void): boolean {
    const state = readShortcutReminderState();
    const current = normalizeShortcutReminderRecord(state[id]);
    if (!canShowShortcutReminder(current)) return false;

    state[id] = markShortcutReminderShown(current);
    writeShortcutReminderState(state);
    show();
    return true;
}
