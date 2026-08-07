import type { SessionEvent, UndoRecord } from './api';

type ActivityIdentity = Pick<SessionEvent, 'event_type' | 'subject_id'>;
type UndoIdentity = Pick<UndoRecord, 'action_type' | 'affected_image_ids'>;

const undoActionByEventType: Readonly<Record<string, string>> = {
    rating_set: 'set_rating',
    decision_set: 'set_decision',
    image_moved_to_trash: 'trash_image',
};

function identityKey(actionType: string, subjectId: string): string {
    return `${actionType}\u0000${subjectId}`;
}

export function filterUndoBackedActivity<T extends ActivityIdentity>(
    events: readonly T[],
    undoRecords: readonly UndoIdentity[],
): T[] {
    const availableMatches = new Map<string, number>();

    for (const record of undoRecords) {
        for (const subjectId of record.affected_image_ids?.split(',') ?? []) {
            const normalizedId = subjectId.trim();
            if (!normalizedId) continue;
            const key = identityKey(record.action_type, normalizedId);
            availableMatches.set(key, (availableMatches.get(key) ?? 0) + 1);
        }
    }

    return events.filter(event => {
        const undoAction = undoActionByEventType[event.event_type];
        if (!undoAction || !event.subject_id) return true;

        const key = identityKey(undoAction, event.subject_id);
        const remaining = availableMatches.get(key) ?? 0;
        if (remaining === 0) return true;

        availableMatches.set(key, remaining - 1);
        return false;
    });
}
