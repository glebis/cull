export interface DisplayFolder {
    name: string;
    disambig: string;
    fullPath: string;
    count: number;
    depth: number;
    hasChildren: boolean;
}

export function folderName(path: string): string {
    const parts = path.split('/');
    return parts[parts.length - 1] || path;
}

interface TrieNode {
    segment: string;
    fullPath: string;
    count: number;
    children: Map<string, TrieNode>;
}

function findCommonPrefixLength(paths: string[][]): number {
    if (paths.length === 0) return 0;
    if (paths.length === 1) return Math.max(0, paths[0].length - 1);
    let prefix = 0;
    for (let i = 0; i < Math.min(...paths.map(p => p.length)); i++) {
        const seg = paths[0][i];
        if (paths.some(p => p[i] !== seg)) break;
        prefix = i + 1;
    }
    return prefix;
}

export function buildDisplayFolders(flatFolders: [string, number][]): DisplayFolder[] {
    if (flatFolders.length === 0) return [];

    const allSegments = flatFolders.map(([p]) => p.split('/').filter(s => s.length > 0));
    const commonPrefix = findCommonPrefixLength(allSegments);

    const root: TrieNode = { segment: '', fullPath: '', count: 0, children: new Map() };

    for (const [fullPath, count] of flatFolders) {
        const segments = fullPath.split('/').filter(s => s.length > 0).slice(commonPrefix);
        let node = root;
        for (let i = 0; i < segments.length; i++) {
            const seg = segments[i];
            if (!node.children.has(seg)) {
                node.children.set(seg, {
                    segment: seg,
                    fullPath: '',
                    count: 0,
                    children: new Map(),
                });
            }
            node = node.children.get(seg)!;
        }
        node.fullPath = fullPath;
        node.count = count;
    }

    const result: DisplayFolder[] = [];

    function flatten(node: TrieNode, depth: number) {
        const children = [...node.children.values()].sort((a, b) =>
            a.segment.localeCompare(b.segment)
        );

        for (const child of children) {
            let current = child;
            let displayName = current.segment;
            while (current.children.size === 1 && current.count === 0) {
                const only = [...current.children.values()][0];
                displayName += '/' + only.segment;
                current = only;
            }

            result.push({
                name: displayName,
                disambig: '',
                fullPath: current.fullPath,
                count: current.count,
                depth,
                hasChildren: current.children.size > 0,
            });

            if (current.children.size > 0) {
                flatten(current, depth + 1);
            }
        }
    }

    flatten(root, 0);
    return result;
}

export function formatImportResult(imported: number, skipped: number, errorCount: number): string {
    let result = `+${imported} imported, ${skipped} skipped`;
    if (errorCount > 0) {
        result += `, ${errorCount} errors`;
    }
    return result;
}

export function formatSidebarCount(count: number | null | undefined): string {
    return String(count ?? 0);
}

export type CollectionRow = [string, string, number];

export function buildPinnedCollectionRows(
    collections: CollectionRow[],
    pinnedIds: string[]
): CollectionRow[] {
    if (collections.length === 0 || pinnedIds.length === 0) return collections;

    const byId = new Map(collections.map(row => [row[0], row]));
    const pinned = pinnedIds
        .map(id => byId.get(id))
        .filter((row): row is CollectionRow => row !== undefined);
    const pinnedSet = new Set(pinned.map(([id]) => id));
    const unpinned = collections.filter(([id]) => !pinnedSet.has(id));

    return [...pinned, ...unpinned];
}
