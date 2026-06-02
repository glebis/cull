export const CLIPBOARD_PASTE_DATE_FORMAT_SETTING = 'clipboard_paste_date_format';
export const DEFAULT_CLIPBOARD_PASTE_DATE_FORMAT = '%Y-%m-%d';

export function parentFolderForPath(path: string): string | null {
    const trimmed = path.trim().replace(/\/+$/, '');
    const idx = trimmed.lastIndexOf('/');
    if (idx <= 0) return idx === 0 ? '/' : null;
    return trimmed.slice(0, idx);
}

export function pasteDestinationForContext(
    activeFolder: string | null,
    focusedImagePath: string | null,
): string | null {
    if (activeFolder?.trim()) return activeFolder;
    if (!focusedImagePath?.trim()) return null;
    return parentFolderForPath(focusedImagePath);
}
