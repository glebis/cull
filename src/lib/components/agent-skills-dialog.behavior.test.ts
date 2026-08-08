// @vitest-environment jsdom

import { afterEach, describe, expect, it, vi } from 'vitest';
import '@testing-library/jest-dom/vitest';
import { cleanup, render, screen } from '@testing-library/svelte';
import AgentSkillsDialog from './AgentSkillsDialog.svelte';

afterEach(() => cleanup());

describe('AgentSkillsDialog inline code', () => {
    it('renders command names as semantic code without visible Markdown backticks', () => {
        render(AgentSkillsDialog, { onclose: vi.fn() });

        const dialog = screen.getByRole('dialog', { name: 'Install Agent Skills' });
        expect(screen.getByText('cull', { selector: 'code' })).toBeInTheDocument();
        expect(screen.getByText('cull --json', { selector: 'code' })).toBeInTheDocument();
        expect(dialog).not.toHaveTextContent('`cull`');
        expect(dialog).not.toHaveTextContent('`cull --json`');
    });
});
