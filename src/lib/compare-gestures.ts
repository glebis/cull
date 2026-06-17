export function nextCompareFocusedIndex(current: number, total: number, direction: 'previous' | 'next'): number {
    const delta = direction === 'next' ? 2 : -2;
    return Math.max(0, Math.min(current + delta, Math.max(0, total - 2)));
}
