let activeClose: (() => void) | null = null;

/** Ensures only one contextual menu owns focus anywhere in the app. */
export function claimContextMenu(close: () => void): () => void {
    activeClose?.();
    activeClose = close;
    return () => {
        if (activeClose === close) activeClose = null;
    };
}
