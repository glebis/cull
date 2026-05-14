export type MoveFolderEntry = [string, number];

export function folderDisplayName(path: string): string {
    const trimmed = path.replace(/\/+$/, '');
    if (!trimmed) return path;
    const parts = trimmed.split('/').filter(Boolean);
    return parts[parts.length - 1] ?? path;
}

export function folderParentPath(path: string): string {
    const trimmed = path.replace(/\/+$/, '');
    if (!trimmed) return '';
    const absolute = trimmed.startsWith('/');
    const parts = trimmed.split('/').filter(Boolean);
    if (parts.length <= 1) return absolute ? '/' : '';
    const parent = parts.slice(0, -1).join('/');
    return absolute ? `/${parent}` : parent;
}

export function filterMoveFolders(folders: MoveFolderEntry[], query: string): MoveFolderEntry[] {
    const terms = query.trim().toLowerCase().split(/\s+/).filter(Boolean);
    if (terms.length === 0) return folders;

    return folders.filter(([folder]) => {
        const haystack = `${folderDisplayName(folder)} ${folder}`.toLowerCase();
        return terms.every((term) => haystack.includes(term));
    });
}
