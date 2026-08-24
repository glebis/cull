export interface DisplayFolder {
    /** Display label. May be a compressed chain ("a/b/c") when intermediate
     *  directories hold no images of their own. */
    name: string;
    disambig: string;
    /** Real absolute path. Reconstructed for group nodes the backend never
     *  reported, so every row is navigable. */
    fullPath: string;
    /** Images whose immediate parent is this directory. */
    count: number;
    /** count plus every descendant — what opening the row actually loads,
     *  because list_images_by_folder matches on the `path/%` prefix. */
    subtreeCount: number;
    depth: number;
    hasChildren: boolean;
    /** True when the directory holds no images of its own and exists in the
     *  tree only to carry descendants. */
    isGroup: boolean;
}

/**
 * The count shown on a folder row. Opening a folder is recursive while the
 * backend's per-folder count is not, so a single number is always a lie for
 * one of the two. Show both when they differ: "4 (27)" reads as "4 here, 27
 * including subfolders".
 */
export function formatFolderCount(direct: number, subtree: number): string {
    // imageview-1i2k.3: zero counts are noise, not information. An empty
    // subtree renders nothing; a folder whose images all live deeper shows
    // the subtree number alone instead of "0 (27)".
    if (subtree === 0) return '';
    return direct === subtree || direct === 0 ? String(subtree) : `${direct} (${subtree})`;
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
    // Never strip a path down to nothing: a folder that is itself the common
    // prefix (e.g. /lib alongside /lib/sub) would land on the sentinel root,
    // which flatten() never emits, silently dropping its images from the tree.
    const shortestPath = Math.min(...allSegments.map(s => s.length));
    const commonPrefix = Math.min(findCommonPrefixLength(allSegments), Math.max(0, shortestPath - 1));

    const root: TrieNode = { segment: '', fullPath: '', count: 0, children: new Map() };

    for (const [fullPath, count] of flatFolders) {
        const allSegments = fullPath.split('/').filter(s => s.length > 0);
        const segments = allSegments.slice(commonPrefix);
        let node = root;
        for (let i = 0; i < segments.length; i++) {
            const seg = segments[i];
            if (!node.children.has(seg)) {
                node.children.set(seg, {
                    segment: seg,
                    // Intermediate directories are never reported by the
                    // backend (they hold no images of their own), so rebuild
                    // their absolute path from the segments we walked through.
                    // Without this they carry '' and cannot be navigated to.
                    fullPath: '/' + allSegments.slice(0, commonPrefix + i + 1).join('/'),
                    count: 0,
                    children: new Map(),
                });
            }
            node = node.children.get(seg)!;
        }
        node.fullPath = fullPath;
        node.count = count;
    }

    // Post-order subtree sums, computed once so each row can show both numbers.
    const subtreeCounts = new Map<TrieNode, number>();
    function sumSubtree(node: TrieNode): number {
        let total = node.count;
        for (const child of node.children.values()) {
            total += sumSubtree(child);
        }
        subtreeCounts.set(node, total);
        return total;
    }
    sumSubtree(root);

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
                subtreeCount: subtreeCounts.get(current) ?? current.count,
                depth,
                hasChildren: current.children.size > 0,
                isGroup: current.count === 0,
            });

            if (current.children.size > 0) {
                flatten(current, depth + 1);
            }
        }
    }

    // A path with no segments at all (the filesystem root, "/") never enters the
    // walk, so its images would vanish from the sidebar entirely. list_folders()
    // does return ("/", n) for files stored at the root, so emit it explicitly.
    if (root.count > 0) {
        result.unshift({
            name: '/',
            disambig: '',
            fullPath: '/',
            count: root.count,
            subtreeCount: subtreeCounts.get(root) ?? root.count,
            depth: 0,
            hasChildren: false,
            isGroup: false,
        });
    }

    flatten(root, 0);
    return result;
}

/**
 * The ordered list of rows the tree should actually render, after applying
 * per-node expansion and the sidebar filter. This is the single source the
 * render loop and the keyboard handler both read, so arrow-key movement can
 * never land on a row that is not on screen.
 *
 * Filter semantics: a row is kept when its own label matches. Ancestors are
 * kept as context (otherwise a match renders at an orphaned indent) and so are
 * descendants. Descendants have to be included: a matched *group* row shows a
 * subtree count but has no images of its own, so excluding its children would
 * advertise "8 images" while offering no way to see them — and the chevron
 * cannot rescue it, because filtering ignores the expansion state. While a
 * filter is active that state is ignored, since a hit hidden inside a collapsed
 * branch would make the filter look broken.
 */
export function visibleFolderRows(
    folders: DisplayFolder[],
    expanded: Set<string>,
    filterQuery = ''
): DisplayFolder[] {
    const query = filterQuery.trim().toLowerCase();

    if (query) {
        const keep = new Array<boolean>(folders.length).fill(false);
        // Forward pre-order walk. `openAncestors` holds the indices of the rows
        // currently on the path to the root, so a match can mark its context
        // without parent pointers.
        const openAncestors: number[] = [];
        for (let i = 0; i < folders.length; i++) {
            const row = folders[i];
            while (openAncestors.length && folders[openAncestors[openAncestors.length - 1]].depth >= row.depth) {
                openAncestors.pop();
            }
            if (row.name.toLowerCase().includes(query)) {
                keep[i] = true;
                for (const a of openAncestors) keep[a] = true;
                // Descendants are a contiguous strictly-deeper run in pre-order.
                for (let k = i + 1; k < folders.length && folders[k].depth > row.depth; k++) {
                    keep[k] = true;
                }
            }
            openAncestors.push(i);
        }
        return folders.filter((_, i) => keep[i]);
    }

    const result: DisplayFolder[] = [];
    // Depth of the shallowest collapsed ancestor; rows deeper than it are hidden
    // until we return to that depth or above.
    let hiddenBelowDepth: number | null = null;
    for (const row of folders) {
        if (hiddenBelowDepth !== null && row.depth > hiddenBelowDepth) continue;
        hiddenBelowDepth = null;
        result.push(row);
        if (row.hasChildren && !expanded.has(row.fullPath)) {
            hiddenBelowDepth = row.depth;
        }
    }
    return result;
}

export function formatImportResult(imported: number, skipped: number, errorCount: number): string {
    let result = `+${imported} imported, ${skipped} skipped`;
    if (errorCount > 0) {
        result += `, ${errorCount} errors`;
    }
    return result;
}

/** imageview-1i2k.3: a zero count on every row is noise ("too many 0
 *  values"). Rows render the string as-is, so empty means no badge at all. */
export function formatSidebarCount(count: number | null | undefined): string {
    return count ? String(count) : '';
}

export type CollectionRow = [string, string, number];

/**
 * Drop pinned ids whose collection no longer exists. Pins are restored from
 * localStorage before the collection list arrives, so a collection deleted in
 * another window (or in a previous session) would otherwise linger as a pin
 * that can never be seen or cleared.
 */
/** Case-insensitive substring match used by the sidebar filter. An empty or
 *  whitespace-only query matches everything, so clearing the box restores the
 *  full list rather than emptying it. */
export function matchesSidebarFilter(name: string, query: string): boolean {
    const q = query.trim().toLowerCase();
    return q === '' || name.toLowerCase().includes(q);
}

export function prunePinnedIds(
    pinnedIds: string[],
    collections: CollectionRow[]
): string[] {
    const live = new Set(collections.map(([id]) => id));
    return pinnedIds.filter(id => live.has(id));
}

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

/* ------------------------------------------------------------------ */
/* Recent scopes — what the user was looking at most recently, so      */
/* "where did that just-imported folder go?" has a persistent answer.  */
/*                                                                     */
/* A scope is the full identity of a sidebar row: kind matters because */
/* a folder path and a collection id could look the same as strings.   */
/* `fresh` marks "imported just now, not visited yet" — the highlight  */
/* clears on visit, not on a timer, so the marker can't expire while   */
/* the user is heads-down in another window (the failure mode of the   */
/* 8-second toast this rail replaces).                                 */
/* ------------------------------------------------------------------ */

export interface RecentScope {
    kind: 'folder' | 'collection' | 'smart';
    /** Real folder path, collection id, or smart-collection id. */
    id: string;
    /** Display name snapshot; re-resolved against live lists at render. */
    name: string;
    /** Unix ms of last visit or import. */
    ts: number;
    /** Just-imported, not yet visited. */
    fresh?: boolean;
}

export const RECENT_SCOPES_CAP = 8;

export function recordRecentScope(
    list: RecentScope[],
    scopeEntry: RecentScope,
    cap = RECENT_SCOPES_CAP
): RecentScope[] {
    const rest = list.filter(s => !(s.kind === scopeEntry.kind && s.id === scopeEntry.id));
    return [{ ...scopeEntry, fresh: scopeEntry.fresh ?? false }, ...rest].slice(0, cap);
}

export function markRecentScopeVisited(
    list: RecentScope[],
    kind: RecentScope['kind'],
    id: string
): RecentScope[] {
    if (!list.some(s => s.kind === kind && s.id === id)) return list;
    return list.map(s => (s.kind === kind && s.id === id ? { ...s, fresh: false } : s));
}

/** Recents die quietly with their target — a rail row that opens
 *  nothing is worse than no row. Same rule as prunePinnedIds. */
export function pruneRecentScopes(
    list: RecentScope[],
    liveFolderPaths: Set<string>,
    liveCollectionIds: Set<string>,
    liveSmartIds: Set<string>
): RecentScope[] {
    return list.filter(s => {
        if (s.kind === 'folder') return liveFolderPaths.has(s.id);
        if (s.kind === 'collection') return liveCollectionIds.has(s.id);
        return liveSmartIds.has(s.id);
    });
}

/** fullPaths of every display row that must be expanded for `targetPath`
 *  to be visible. Walks the pre-order rows tracking the depth chain, the
 *  same technique as visibleFolderRows, so compression chains ("a/b/c")
 *  resolve to the rows that actually exist. */
export function ancestorFolderPaths(rows: DisplayFolder[], targetPath: string): string[] {
    const chain: DisplayFolder[] = [];
    for (const row of rows) {
        while (chain.length && chain[chain.length - 1].depth >= row.depth) {
            chain.pop();
        }
        if (row.fullPath === targetPath) {
            return chain.filter(r => r.hasChildren).map(r => r.fullPath);
        }
        chain.push(row);
    }
    return [];
}

/** Right-edge tag for a recents row: words, not another glyph dialect. */
export function kindLabel(kind: RecentScope['kind']): string {
    switch (kind) {
        case 'folder': return 'Folder';
        case 'collection': return 'Collection';
        case 'smart': return 'Smart';
    }
}
