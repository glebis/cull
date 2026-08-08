// @vitest-environment jsdom
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import '@testing-library/jest-dom/vitest';
import { cleanup, render, screen, waitFor, within } from '@testing-library/svelte';
import userEvent from '@testing-library/user-event';
import AgentAccessSettings from './AgentAccessSettings.svelte';
import { createMcpToken, getAppSetting, listMcpTokens, setAppSetting } from '$lib/api';

vi.mock('$lib/api', () => ({
    createMcpToken: vi.fn(),
    getAppSetting: vi.fn(),
    listMcpTokens: vi.fn().mockResolvedValue([]),
    revokeMcpToken: vi.fn(),
    rotateMcpToken: vi.fn(),
    setAppSetting: vi.fn(),
}));

afterEach(() => cleanup());
beforeEach(() => {
    vi.clearAllMocks();
    vi.mocked(getAppSetting).mockImplementation(async (key) => {
        if (key === 'mcp_http_enabled') return 'true';
        if (key === 'mcp_http_port') return '9847';
        return null;
    });
    vi.mocked(setAppSetting).mockResolvedValue(undefined);
});

async function renderedPortInput(): Promise<HTMLInputElement> {
    return await screen.findByRole('textbox', { name: 'MCP HTTP port' }) as HTMLInputElement;
}

describe('Agent Access MCP HTTP settings', () => {
    it('does not present default controls as final while initialization is pending', async () => {
        let resolveTokens!: (value: []) => void;
        const pendingTokens = new Promise<[]>((resolve) => { resolveTokens = resolve; });
        vi.mocked(listMcpTokens).mockReturnValueOnce(pendingTokens);

        render(AgentAccessSettings);

        expect(await screen.findByText('Loading agent access settings…')).toBeVisible();
        expect(screen.queryByRole('group', { name: 'Skill installation method' })).not.toBeInTheDocument();
        resolveTokens([]);
        expect(await screen.findByRole('group', { name: 'Skill installation method' })).toBeVisible();
    });

    it('shows a tab-local initialization error and retries successfully', async () => {
        vi.mocked(listMcpTokens).mockRejectedValueOnce(new Error('database unavailable'));
        const user = userEvent.setup();
        render(AgentAccessSettings);

        expect(await screen.findByRole('alert')).toHaveTextContent('Could not load agent access settings.');
        expect(screen.queryByRole('group', { name: 'Skill installation method' })).not.toBeInTheDocument();
        await user.click(screen.getByRole('button', { name: 'Retry' }));

        expect(await screen.findByRole('group', { name: 'Skill installation method' })).toBeVisible();
        expect(await renderedPortInput()).toHaveValue('9847');
        expect(screen.queryByRole('alert')).not.toBeInTheDocument();
    });

    it('rejects a port with trailing junk and restores the last saved value', async () => {
        const user = userEvent.setup();
        render(AgentAccessSettings);
        const input = await renderedPortInput();

        await user.clear(input);
        await user.type(input, '9847abc');
        await user.tab();

        expect(await screen.findByRole('alert')).toHaveTextContent('Enter a whole-number port from 1 to 65535.');
        expect(input).toHaveValue('9847');
        expect(setAppSetting).not.toHaveBeenCalledWith('mcp_http_port', expect.anything());
    });

    it.each(['0', '65536'])('rejects out-of-range port %s and restores the last saved value', async (value) => {
        const user = userEvent.setup();
        render(AgentAccessSettings);
        const input = await renderedPortInput();

        await user.clear(input);
        await user.type(input, value);
        await user.tab();

        expect(await screen.findByRole('alert')).toHaveTextContent('Enter a whole-number port from 1 to 65535.');
        expect(input).toHaveValue('9847');
        expect(setAppSetting).not.toHaveBeenCalledWith('mcp_http_port', expect.anything());
    });

    it('persists a valid whole-number port', async () => {
        const user = userEvent.setup();
        render(AgentAccessSettings);
        const input = await renderedPortInput();

        await user.clear(input);
        await user.type(input, '4242');
        await user.tab();

        await waitFor(() => expect(setAppSetting).toHaveBeenCalledWith('mcp_http_port', '4242'));
        expect(input).toHaveValue('4242');
        expect(screen.queryByRole('alert')).not.toBeInTheDocument();
    });

    it('rolls back the HTTP toggle and announces a persistence failure', async () => {
        vi.mocked(setAppSetting).mockRejectedValue(new Error('disk full'));
        const user = userEvent.setup();
        render(AgentAccessSettings);
        const toggle = await screen.findByRole('button', { name: 'ON' });

        await user.click(toggle);

        await waitFor(() => expect(toggle).toHaveAttribute('aria-pressed', 'true'));
        expect(await screen.findByRole('alert')).toHaveTextContent('Could not update the local HTTP endpoint. The previous setting was kept.');
    });
});

describe('Agent Access installation and token accessibility', () => {
    it('uses a labeled pressed-button group for installation methods', async () => {
        const user = userEvent.setup();
        render(AgentAccessSettings);
        const group = await screen.findByRole('group', { name: 'Skill installation method' });
        const npx = within(group).getByRole('button', { name: 'npx' });
        const claude = within(group).getByRole('button', { name: 'Claude' });

        expect(screen.queryByRole('tablist')).not.toBeInTheDocument();
        expect(npx).toHaveAttribute('aria-pressed', 'true');
        expect(claude).toHaveAttribute('aria-pressed', 'false');
        await user.click(claude);
        expect(npx).toHaveAttribute('aria-pressed', 'false');
        expect(claude).toHaveAttribute('aria-pressed', 'true');
        expect(screen.getByRole('button', { name: 'Copy Claude installation instructions' })).toBeVisible();
    });

    it('labels token name and role fields', async () => {
        const user = userEvent.setup();
        render(AgentAccessSettings);
        await user.click(await screen.findByRole('button', { name: '+ Create' }));

        expect(screen.getByLabelText('Token name')).toBeVisible();
        expect(screen.getByLabelText('Token role')).toBeVisible();
        expect(screen.getByLabelText('Token expiry')).toBeVisible();
    });

    it('gives the token-secret and config Copy actions distinct names', async () => {
        vi.mocked(createMcpToken).mockResolvedValue([
            {
                id: 'token-1', name: 'Review bot', role: 'admin', scope_json: null,
                created_at: '2026-08-07T10:00:00.000Z', expires_at: null,
                last_used_at: null, revoked: false,
            },
            'cull_secret',
        ]);
        const user = userEvent.setup();
        render(AgentAccessSettings);
        await user.click(await screen.findByRole('button', { name: '+ Create' }));
        await user.type(screen.getByLabelText('Token name'), 'Review bot');
        await user.click(screen.getByRole('button', { name: 'Create Token' }));

        expect(await screen.findByRole('button', { name: 'Copy token secret' })).toBeVisible();
        expect(screen.getByRole('button', { name: 'Copy Claude Code MCP config' })).toBeVisible();
    });

    it('explains the CLI-first path and fresh-session discovery', async () => {
        render(AgentAccessSettings);

        expect(await screen.findByText(/Start with the Cull skill and CLI; MCP is not required\./)).toBeVisible();
        expect(screen.getByText(/After installation, start a new agent turn or session if the skill is not discovered immediately\./)).toBeVisible();
    });
});
