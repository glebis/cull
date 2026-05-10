export interface TinderStats {
    accepted: number;
    rejected: number;
    skipped: number;
}

export function computeTinderStats(
    history: Array<{ choice: string }>
): TinderStats {
    const stats: TinderStats = { accepted: 0, rejected: 0, skipped: 0 };
    for (const entry of history) {
        if (entry.choice === 'left' || entry.choice === 'right') {
            stats.accepted++;
            stats.rejected++;
        } else if (entry.choice === 'skip') {
            stats.skipped += 2;
        }
    }
    return stats;
}

export function totalPairs(imageCount: number): number {
    return Math.ceil(imageCount / 2);
}

export function pairBaseIndex(pairIndex: number): number {
    return pairIndex * 2;
}

export function isDone(pairIndex: number, imageCount: number): boolean {
    return pairIndex >= totalPairs(imageCount) - 1;
}

export function parseVoiceCommand(text: string): 'left' | 'right' | 'skip' | 'undo' | null {
    const t = text.toLowerCase().trim();
    if (t.includes('left') || t.includes('лев')) return 'left';
    if (t.includes('right') || t.includes('прав')) return 'right';
    if (t.includes('skip') || t.includes('пропуст')) return 'skip';
    if (t.includes('undo') || t.includes('отмен')) return 'undo';
    return null;
}

export function filterByDecision(
    history: Array<{ leftId: string; rightId: string; choice: string }>,
    decision: 'accepted' | 'rejected'
): Set<string> {
    const ids = new Set<string>();
    for (const h of history) {
        if (decision === 'accepted') {
            if (h.choice === 'left') ids.add(h.leftId);
            if (h.choice === 'right') ids.add(h.rightId);
        } else {
            if (h.choice === 'left') ids.add(h.rightId);
            if (h.choice === 'right') ids.add(h.leftId);
        }
    }
    return ids;
}
