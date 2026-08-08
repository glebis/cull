import type { Canvas, Session } from '$lib/api';

export function renamedFolderPath(path: string, oldPath: string, newPath: string): string {
    if (path === oldPath) return newPath;
    return path.startsWith(`${oldPath}/`) ? `${newPath}${path.slice(oldPath.length)}` : path;
}

export function reconcileRenamedSession(session: Session, oldPath: string, newPath: string): Session {
    return { ...session, folder_path: renamedFolderPath(session.folder_path, oldPath, newPath) };
}

export function reconcileRenamedCanvas(canvas: Canvas, oldPath: string, newPath: string): Canvas {
    try {
        const layout = JSON.parse(canvas.layout_json) as unknown;
        const rewrite = (value: unknown): void => {
            if (Array.isArray(value)) {
                value.forEach(rewrite);
            } else if (value && typeof value === 'object') {
                for (const [key, child] of Object.entries(value)) {
                    if (key === 'lastKnownPath' && typeof child === 'string') {
                        (value as Record<string, unknown>)[key] = renamedFolderPath(child, oldPath, newPath);
                    } else {
                        rewrite(child);
                    }
                }
            }
        };
        rewrite(layout);
        return { ...canvas, layout_json: JSON.stringify(layout) };
    } catch {
        return canvas;
    }
}
