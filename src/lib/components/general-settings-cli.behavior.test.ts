// @vitest-environment jsdom
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import '@testing-library/jest-dom/vitest';
import { cleanup, render, screen, waitFor } from '@testing-library/svelte';
import userEvent from '@testing-library/user-event';
import { get } from 'svelte/store';
import GeneralSettings from './GeneralSettings.svelte';
import { cliToolStatus, getAppSetting, installCliTool, uninstallCliTool } from '$lib/api';
import type { CliToolStatus } from '$lib/api';
import { toasts } from '$lib/stores';

vi.mock('$lib/api', async (importOriginal) => ({
    ...(await importOriginal<typeof import('$lib/api')>()),
    backfillRawPreviews: vi.fn().mockResolvedValue(0),
    getAppSetting: vi.fn().mockResolvedValue(null),
    setAppSetting: vi.fn().mockResolvedValue(undefined),
    cliToolStatus: vi.fn(),
    installCliTool: vi.fn(),
    uninstallCliTool: vi.fn(),
}));

function status(overrides: Partial<CliToolStatus> = {}): CliToolStatus {
    return {
        installed: false, link_path: null, target_path: null, stale: false,
        candidate_dir: '/Users/test/.local/bin', path_hint: null,
        ...overrides,
    };
}

beforeEach(() => {
    vi.clearAllMocks();
    vi.mocked(getAppSetting).mockResolvedValue(null);
    toasts.set([]);
});

afterEach(() => cleanup());

describe('General Settings: command line tool', () => {
    it('shows an explicit unavailable state with retry when the status check fails', async () => {
        vi.mocked(cliToolStatus).mockRejectedValue(new Error('shell unavailable'));
        render(GeneralSettings);

        const retry = await screen.findByRole('button', { name: 'Retry command line tool status check' });
        expect(retry).toBeEnabled();
        expect(screen.queryByRole('button', { name: 'Install command line tool' })).not.toBeInTheDocument();
        expect(screen.getByRole('alert')).toHaveTextContent('Cull could not check the command line tool just now. Press Retry to check again.');
    });

    it('does not present the row as ON or OFF while the first check is pending', async () => {
        let resolveStatus!: (value: CliToolStatus) => void;
        vi.mocked(cliToolStatus).mockReturnValueOnce(new Promise<CliToolStatus>((resolve) => { resolveStatus = resolve; }));
        render(GeneralSettings);

        const checking = await screen.findByRole('button', { name: 'Checking command line tool status' });
        expect(checking).toBeDisabled();
        expect(screen.getByRole('status')).toHaveTextContent('Checking the command line tool…');

        resolveStatus(status());
        expect(await screen.findByRole('button', { name: 'Install command line tool' })).toBeEnabled();
        expect(screen.queryByRole('status')).not.toBeInTheDocument();
    });

    it('recovers to the install action after a successful retry', async () => {
        const user = userEvent.setup();
        vi.mocked(cliToolStatus)
            .mockRejectedValueOnce(new Error('shell unavailable'))
            .mockResolvedValue(status({ candidate_dir: '/Users/test/.local/bin' }));
        render(GeneralSettings);

        await user.click(await screen.findByRole('button', { name: 'Retry command line tool status check' }));

        expect(await screen.findByRole('button', { name: 'Install command line tool' })).toBeEnabled();
        expect(screen.queryByRole('alert')).not.toBeInTheDocument();
        const note = await screen.findByText(
            (_, element) => element?.classList.contains('note') === true
                && (element.textContent ?? '').includes('Links cull into'),
        );
        expect(note).toHaveTextContent('/Users/test/.local/bin');
    });

    it('names the install action exactly and shows the destination after success', async () => {
        const user = userEvent.setup();
        vi.mocked(cliToolStatus).mockResolvedValue(status());
        vi.mocked(installCliTool).mockResolvedValue(status({
            installed: true,
            link_path: '/Users/test/.local/bin/cull',
            target_path: '/Applications/Cull.app/Contents/MacOS/cull',
        }));
        render(GeneralSettings);

        await user.click(await screen.findByRole('button', { name: 'Install command line tool' }));

        expect(await screen.findByRole('button', { name: 'Remove command line tool' })).toBeEnabled();
        expect(screen.getByText(/is available at/)).toHaveTextContent('/Users/test/.local/bin/cull');
        await waitFor(() => {
            const toast = get(toasts).find(t => t.type === 'success');
            expect(toast?.message).toContain('Command line tool installed');
        });
    });

    it('passes the shell profile line through when the install directory is off PATH', async () => {
        const user = userEvent.setup();
        vi.mocked(cliToolStatus).mockResolvedValue(status());
        vi.mocked(installCliTool).mockResolvedValue(status({
            installed: true,
            link_path: '/Users/test/.local/bin/cull',
            path_hint: "export PATH='/Users/test/.local/bin':$PATH",
        }));
        render(GeneralSettings);

        await user.click(await screen.findByRole('button', { name: 'Install command line tool' }));

        expect(await screen.findByRole('button', { name: 'Remove command line tool' })).toBeEnabled();
        await waitFor(() => {
            const toast = get(toasts).find(t => t.type === 'success');
            expect(toast?.detail).toContain("export PATH='/Users/test/.local/bin':$PATH");
        });
    });

    it('explains a failed install inline and lets the user try again', async () => {
        const user = userEvent.setup();
        vi.mocked(cliToolStatus).mockResolvedValue(status());
        vi.mocked(installCliTool)
            .mockRejectedValueOnce(new Error('/usr/local/bin is not writable'))
            .mockResolvedValue(status({ installed: true, link_path: '/Users/test/.local/bin/cull' }));
        render(GeneralSettings);

        await user.click(await screen.findByRole('button', { name: 'Install command line tool' }));

        expect(await screen.findByRole('alert')).toHaveTextContent('Cull could not install the command line tool just now. Press Install to try again.');
        await waitFor(() => {
            const toast = get(toasts).find(t => t.type === 'error');
            expect(toast?.message).toBe('Could not install command line tool');
            expect(toast?.detail).toContain('/usr/local/bin is not writable');
        });

        await user.click(screen.getByRole('button', { name: 'Install command line tool' }));
        expect(await screen.findByRole('button', { name: 'Remove command line tool' })).toBeEnabled();
        expect(screen.queryByRole('alert')).not.toBeInTheDocument();
    });

    it('offers Repair for a stale link and installs in its place', async () => {
        const user = userEvent.setup();
        vi.mocked(cliToolStatus).mockResolvedValue(status({
            installed: true,
            link_path: '/opt/homebrew/bin/cull',
            target_path: '/Applications/Old Cull.app/Contents/MacOS/cull',
            stale: true,
        }));
        vi.mocked(installCliTool).mockResolvedValue(status({
            installed: true,
            link_path: '/opt/homebrew/bin/cull',
            target_path: '/Applications/Cull.app/Contents/MacOS/cull',
        }));
        render(GeneralSettings);

        expect(await screen.findByRole('button', { name: 'Repair command line tool' })).toBeEnabled();
        expect(screen.getByText(/points at a different copy of Cull/)).toBeVisible();

        await user.click(screen.getByRole('button', { name: 'Repair command line tool' }));
        expect(await screen.findByRole('button', { name: 'Remove command line tool' })).toBeEnabled();
        expect(installCliTool).toHaveBeenCalledTimes(1);
        expect(uninstallCliTool).not.toHaveBeenCalled();
    });

    it('removes the tool with a named Remove action', async () => {
        const user = userEvent.setup();
        vi.mocked(cliToolStatus).mockResolvedValue(status({
            installed: true,
            link_path: '/Users/test/.local/bin/cull',
        }));
        vi.mocked(uninstallCliTool).mockResolvedValue(status());
        render(GeneralSettings);

        await user.click(await screen.findByRole('button', { name: 'Remove command line tool' }));

        expect(await screen.findByRole('button', { name: 'Install command line tool' })).toBeEnabled();
        expect(uninstallCliTool).toHaveBeenCalledTimes(1);
        expect(installCliTool).not.toHaveBeenCalled();
        await waitFor(() => {
            const toast = get(toasts).find(t => t.type === 'success');
            expect(toast?.message).toBe('Command line tool removed');
        });
    });

    it('keeps the CLI row loading independently when other settings fail to load', async () => {
        vi.mocked(getAppSetting).mockRejectedValue(new Error('database unavailable'));
        vi.mocked(cliToolStatus).mockResolvedValue(status());
        render(GeneralSettings);

        expect(await screen.findByRole('button', { name: 'Install command line tool' })).toBeEnabled();
    });
});