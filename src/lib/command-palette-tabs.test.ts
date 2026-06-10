import { describe, it, expect, beforeEach } from 'vitest';
import { clearPluginTabs, registerCoreTabs, registerPluginTab } from './plugins/tab-registry';
import { viewCommandsForTest } from './command-palette';

describe('palette view commands derive from tabRegistry', () => {
    beforeEach(() => { clearPluginTabs(); registerCoreTabs(); });

    it('lists a plugin tab as a view command with its label', () => {
        registerPluginTab({ id: 'publish', label: 'Publish View', subtitle: 'Build a static site package', mountView: () => {} });
        const cmds = viewCommandsForTest();
        expect(cmds.find(c => c.mode === 'publish')?.title).toBe('Publish View');
    });

    it('omits publish when no plugin registered it', () => {
        expect(viewCommandsForTest().find(c => c.mode === 'publish')).toBeUndefined();
    });

    it('preserves core Cmd+digit shortcuts', () => {
        const cmds = viewCommandsForTest();
        expect(cmds.find(c => c.mode === 'grid')?.shortcut).toBe('Cmd+1');
        expect(cmds.find(c => c.mode === 'export')?.shortcut).toBe('Cmd+7');
    });
});
