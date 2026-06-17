export function nextFocusIndexAfterFocusedRemoval(
    removedIndex: number,
    originalLength: number,
): number {
    const remainingLength = Math.max(0, originalLength - 1);
    if (remainingLength === 0) return 0;

    const normalizedIndex = Math.max(0, removedIndex);
    return normalizedIndex < remainingLength ? normalizedIndex : 0;
}

export function clampFocusIndexToList(index: number, length: number): number {
    if (length <= 0) return 0;
    if (index < 0) return 0;
    return index < length ? index : 0;
}
