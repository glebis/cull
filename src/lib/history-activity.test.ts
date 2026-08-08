import { describe, expect, it } from 'vitest';
import type { SessionEvent, UndoRecord } from './api';
import { filterUndoBackedActivity } from './history-activity';

type ActivityFixture = Pick<SessionEvent, 'id' | 'event_type' | 'subject_id'>;
type UndoFixture = Pick<UndoRecord, 'action_type' | 'affected_image_ids'>;

function activity(id: string, eventType: string, subjectId: string | null): ActivityFixture {
    return { id, event_type: eventType, subject_id: subjectId };
}

function undo(actionType: string, affectedImageIds: string | null): UndoFixture {
    return { action_type: actionType, affected_image_ids: affectedImageIds };
}

describe('filterUndoBackedActivity', () => {
    it('keeps undo-backed activity when no loaded undo record represents it', () => {
        const events = [activity('event-1', 'rating_set', 'image-1')];

        expect(filterUndoBackedActivity(events, [])).toEqual(events);
    });

    it('suppresses only as many matching events as loaded undo records', () => {
        const events = [
            activity('event-new', 'rating_set', 'image-1'),
            activity('event-old', 'rating_set', 'image-1'),
        ];

        expect(filterUndoBackedActivity(events, [undo('set_rating', 'image-1')])).toEqual([
            events[1],
        ]);
    });

    it('matches each image in a batch trash undo record', () => {
        const events = [
            activity('event-1', 'image_moved_to_trash', 'image-1'),
            activity('event-2', 'image_moved_to_trash', 'image-2'),
            activity('event-3', 'image_moved_to_trash', 'image-3'),
        ];

        expect(filterUndoBackedActivity(events, [undo('trash_image', 'image-1, image-2')])).toEqual([
            events[2],
        ]);
    });

    it('preserves unrelated event types and events without a subject', () => {
        const events = [
            activity('event-1', 'collection_deleted', 'collection-1'),
            activity('event-2', 'decision_set', null),
        ];

        expect(filterUndoBackedActivity(events, [undo('set_decision', 'image-1')])).toEqual(events);
    });
});
