import { describe, it, expect, beforeEach } from 'vitest';
import { clearPluginTabs, registerCoreTabs, registerPluginTab } from './plugins/tab-registry';
import { viewModeCycleForTest } from './keys';

describe('keys view cycle derives from tabRegistry', () => {
    beforeEach(() => { clearPluginTabs(); registerCoreTabs(); });

    it('includes a plugin-registered tab in the cycle', () => {
        registerPluginTab({ id: 'publish', label: 'Publish View', mountView: () => {} });
        expect(viewModeCycleForTest()).toContain('publish');
    });

    it('excludes publish when no plugin registered it', () => {
        expect(viewModeCycleForTest()).not.toContain('publish');
    });
});
