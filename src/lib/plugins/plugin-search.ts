export interface SearchablePlugin {
    id: string;
    name: string;
    description?: string;
    permissions?: string[];
}

/** Case-insensitive match over name, description, and permissions. Empty
 * query returns the list unchanged. Pure — safe to call on every render. */
export function filterPlugins<T extends SearchablePlugin>(items: T[], query: string): T[] {
    const q = query.trim().toLowerCase();
    if (!q) return items;
    return items.filter(p =>
        p.name.toLowerCase().includes(q) ||
        (p.description ?? '').toLowerCase().includes(q) ||
        (p.permissions ?? []).some(perm => perm.toLowerCase().includes(q)));
}
