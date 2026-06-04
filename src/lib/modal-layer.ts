interface ShellInertState {
    inert: boolean;
    ariaHidden: string | null;
}

let activeModals = 0;
let shellState: ShellInertState | null = null;

function getShell(): (HTMLElement & { inert?: boolean }) | null {
    return document.querySelector('.app-shell') as (HTMLElement & { inert?: boolean }) | null;
}

export function claimModalShellInert() {
    const shell = getShell();
    if (!shell) return;

    if (activeModals === 0) {
        shellState = {
            inert: shell.inert,
            ariaHidden: shell.getAttribute('aria-hidden'),
        };
        shell.inert = true;
        shell.setAttribute('aria-hidden', 'true');
    }

    activeModals += 1;
}

export function releaseModalShellInert() {
    const shell = getShell();
    if (!shell || activeModals <= 0) return;

    activeModals = Math.max(0, activeModals - 1);
    if (activeModals !== 0) return;

    if (!shellState) return;
    shell.inert = shellState.inert;
    if (shellState.ariaHidden === null) {
        shell.removeAttribute('aria-hidden');
    } else {
        shell.setAttribute('aria-hidden', shellState.ariaHidden);
    }
    shellState = null;
}
