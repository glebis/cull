import { describe, expect, it } from 'vitest';
import { buildDeliveryCsv, DELIVERY_CSV_HEADERS, type DeliveryRow } from './delivery-csv';

function row(partial: Partial<DeliveryRow>): DeliveryRow {
    return {
        filename: 'a.jpg',
        path: '/photos/a.jpg',
        rating: 0,
        decision: 'undecided',
        clientFavorite: false,
        clientComment: '',
        ...partial,
    };
}

describe('delivery CSV', () => {
    it('emits a header row followed by one line per image', () => {
        const csv = buildDeliveryCsv([row({ filename: 'sunset.jpg', rating: 5 })]);
        const lines = csv.trimEnd().split('\n');
        expect(lines[0]).toBe(DELIVERY_CSV_HEADERS.join(','));
        expect(lines).toHaveLength(2);
        expect(lines[1]).toContain('sunset.jpg');
        expect(lines[1]).toContain('5');
    });

    it('keeps curator and client columns distinct', () => {
        const csv = buildDeliveryCsv([
            row({ rating: 4, decision: 'accept', clientFavorite: true, clientComment: 'love it' }),
        ]);
        const line = csv.trimEnd().split('\n')[1];
        expect(line).toBe('a.jpg,/photos/a.jpg,4,accept,yes,love it');
    });

    it('escapes commas, quotes, and newlines in comments', () => {
        const csv = buildDeliveryCsv([
            row({ clientComment: 'great, but "dark"\nfix shadows' }),
        ]);
        expect(csv).toContain('"great, but ""dark""\nfix shadows"');
    });

    it('renders an empty set as just the header', () => {
        const csv = buildDeliveryCsv([]);
        expect(csv).toBe(DELIVERY_CSV_HEADERS.join(',') + '\n');
    });
});
