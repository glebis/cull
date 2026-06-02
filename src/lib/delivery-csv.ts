// Builds a delivery list CSV combining curator selections with client feedback.
// Curator data (rating, decision) and client data (favorite, comment) are kept
// in distinct columns so the two perspectives stay legible side by side.

export interface DeliveryRow {
    filename: string;
    path: string;
    rating: number; // curator star rating, 0-5
    decision: string; // curator decision
    clientFavorite: boolean;
    clientComment: string;
}

export const DELIVERY_CSV_HEADERS = [
    'filename',
    'path',
    'curator_rating',
    'curator_decision',
    'client_favorite',
    'client_comment',
] as const;

// Quote a field per RFC 4180 when it contains a comma, quote, or newline.
function csvField(value: string): string {
    if (/[",\n\r]/.test(value)) {
        return `"${value.replace(/"/g, '""')}"`;
    }
    return value;
}

export function buildDeliveryCsv(rows: DeliveryRow[]): string {
    const lines = [DELIVERY_CSV_HEADERS.join(',')];
    for (const row of rows) {
        lines.push([
            csvField(row.filename),
            csvField(row.path),
            String(row.rating),
            csvField(row.decision),
            row.clientFavorite ? 'yes' : 'no',
            csvField(row.clientComment),
        ].join(','));
    }
    // Trailing newline so the file is POSIX-friendly.
    return lines.join('\n') + '\n';
}
