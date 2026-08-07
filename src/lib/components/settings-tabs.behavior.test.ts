// @vitest-environment jsdom
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import '@testing-library/jest-dom/vitest';
import { cleanup, render, screen, waitFor } from '@testing-library/svelte';
import userEvent from '@testing-library/user-event';
import { settingsTab } from '$lib/settings-navigation';
import McpSettings from './McpSettings.svelte';

vi.mock('$lib/api', async (importOriginal) => ({
    ...(await importOriginal<typeof import('$lib/api')>()),
    applyAppIconVariant: vi.fn().mockResolvedValue(undefined),
    fetchPluginRegistry: vi.fn().mockResolvedValue([]),
    getAppSetting: vi.fn().mockResolvedValue(null),
    listInstalledPluginInfo: vi.fn().mockResolvedValue([]),
    setAppSetting: vi.fn().mockResolvedValue(undefined),
}));

afterEach(() => cleanup());
beforeEach(() => settingsTab.set('general'));

describe('Settings tab keyboard navigation', () => {
    it('moves selection and focus with Left and Right, including wraparound', async () => {
        const user = userEvent.setup();
        render(McpSettings, { onclose: vi.fn() });
        const general = screen.getByRole('tab', { name: 'General' });
        const appearance = screen.getByRole('tab', { name: 'Appearance' });
        const plugins = screen.getByRole('tab', { name: 'Plugins' });

        await waitFor(() => expect(screen.getByRole('dialog', { name: 'Settings' })).toHaveFocus());
        general.focus();
        await user.keyboard('{ArrowRight}');
        expect(appearance).toHaveFocus();
        expect(appearance).toHaveAttribute('aria-selected', 'true');

        await user.keyboard('{ArrowLeft}');
        expect(general).toHaveFocus();
        expect(general).toHaveAttribute('aria-selected', 'true');

        await user.keyboard('{ArrowLeft}');
        expect(plugins).toHaveFocus();
        expect(plugins).toHaveAttribute('aria-selected', 'true');
    });

    it('moves selection and focus to the first or last tab with Home and End', async () => {
        const user = userEvent.setup();
        render(McpSettings, { onclose: vi.fn() });
        const appearance = screen.getByRole('tab', { name: 'Appearance' });
        const general = screen.getByRole('tab', { name: 'General' });
        const plugins = screen.getByRole('tab', { name: 'Plugins' });

        await user.click(appearance);
        await user.keyboard('{End}');
        expect(plugins).toHaveFocus();
        expect(plugins).toHaveAttribute('aria-selected', 'true');

        await user.keyboard('{Home}');
        expect(general).toHaveFocus();
        expect(general).toHaveAttribute('aria-selected', 'true');
    });
});
