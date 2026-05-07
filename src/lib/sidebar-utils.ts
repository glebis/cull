export interface DisplayFolder {
    name: string;
    disambig: string;
    fullPath: string;
    count: number;
}

export function folderName(path: string): string {
    const parts = path.split('/');
    return parts[parts.length - 1] || path;
}

export function buildDisplayFolders(flatFolders: [string, number][]): DisplayFolder[] {
    const result: DisplayFolder[] = flatFolders.map(([fullPath, count]) => {
        const parts = fullPath.split('/').filter(p => p.length > 0);
        const name = parts[parts.length - 1] || fullPath;
        return { name, disambig: '', fullPath, count };
    });

    const byName = new Map<string, DisplayFolder[]>();
    for (const f of result) {
        const group = byName.get(f.name) || [];
        group.push(f);
        byName.set(f.name, group);
    }
    for (const [, group] of byName) {
        if (group.length <= 1) continue;
        for (const f of group) {
            const parts = f.fullPath.split('/').filter(p => p.length > 0);
            const contextParts = parts.slice(Math.max(0, parts.length - 3), parts.length - 1);
            f.disambig = contextParts.join('/');
        }
    }

    result.sort((a, b) => a.name.localeCompare(b.name));
    return result;
}

export function formatImportResult(imported: number, skipped: number, errorCount: number): string {
    let result = `+${imported} imported, ${skipped} skipped`;
    if (errorCount > 0) {
        result += `, ${errorCount} errors`;
    }
    return result;
}
