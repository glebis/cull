import { describe, expect, it } from 'vitest';
import { existsSync, readFileSync } from 'node:fs';
import { join } from 'node:path';

const root = process.cwd();
const helpBookFolder = 'Cull.help';
const helpBookName = 'com.glebkalinin.cull.help';
const helpBookRoot = 'src-tauri/help/Cull.help/Contents';

function readProjectFile(path: string): string {
    return readFileSync(join(root, path), 'utf8');
}

function projectFileExists(path: string): boolean {
    return existsSync(join(root, path));
}

function projectFileSize(path: string): number {
    return readProjectFile(path).length;
}

describe('Help menu contract', () => {
    it('labels the Help menu action as the Cull User Guide', () => {
        const menuSource = readProjectFile('src-tauri/src/menu.rs');

        expect(menuSource).toContain('"help"');
        expect(menuSource).toContain('"Cull User Guide"');
        expect(menuSource).not.toContain('"Cull Help"');
    });

    it('registers the Help submenu with Tauri so macOS adds the native search field', () => {
        const menuSource = readProjectFile('src-tauri/src/menu.rs');

        expect(menuSource).toContain('HELP_SUBMENU_ID');
        expect(menuSource).toContain('Submenu::with_id(app, HELP_SUBMENU_ID, "Help", true)');
    });

    it('adds the project wiki as a native Help menu link', () => {
        const menuSource = readProjectFile('src-tauri/src/menu.rs');
        const frontendMenuSource = readProjectFile('src/lib/menu.ts');

        expect(menuSource).toContain('"github_wiki"');
        expect(menuSource).toContain('"GitHub Wiki"');
        expect(frontendMenuSource).toContain('https://github.com/glebis/cull/wiki');
    });

    it('adds a native Help menu item for agent skill installation instructions', () => {
        const menuSource = readProjectFile('src-tauri/src/menu.rs');
        const frontendMenuSource = readProjectFile('src/lib/menu.ts');
        const pageSource = readProjectFile('src/routes/+page.svelte');
        const dialogSource = readProjectFile('src/lib/components/AgentSkillsDialog.svelte');

        expect(menuSource).toContain('"agent_skills"');
        expect(menuSource).toContain('"Install Agent Skills..."');
        expect(frontendMenuSource).toContain("case 'agent_skills'");
        expect(pageSource).toContain('AgentSkillsDialog');
        // The dialog renders the shared canonical config (see mcp-config.ts,
        // which owns the "command"/"args" contract).
        expect(dialogSource).toContain('MCP_CONFIG_SNIPPET');
        expect(dialogSource).toContain('docs/agents.md');
    });

    it('registers a native macOS Help Book in the app plist', () => {
        const plist = readProjectFile('src-tauri/Info.plist');

        expect(plist).toContain('<key>CFBundleHelpBookFolder</key>');
        expect(plist).toContain(`<string>${helpBookFolder}</string>`);
        expect(plist).toContain('<key>CFBundleHelpBookName</key>');
        expect(plist).toContain(`<string>${helpBookName}</string>`);
        expect(plist).toContain('<key>CFAppleHelpAnchor</key>');
        expect(plist).toContain('<string>index</string>');
    });

    it('bundles the native macOS Help Book into app resources', () => {
        const config = JSON.parse(readProjectFile('src-tauri/tauri.conf.json'));

        expect(config.bundle.macOS.files['Resources/Cull.help']).toBe('help/Cull.help');
    });

    it('bundles the Claude Agent SDK runtime into app resources', () => {
        const config = JSON.parse(readProjectFile('src-tauri/tauri.conf.json'));

        expect(config.bundle.resources['../scripts/claude-agent-sdk-runner.mjs']).toBe('claude-agent-sdk-runner.mjs');
        expect(config.bundle.resources['../node_modules/@anthropic-ai/claude-agent-sdk']).toBe(
            'node_modules/@anthropic-ai/claude-agent-sdk'
        );
        expect(projectFileExists('scripts/claude-agent-sdk-runner.mjs')).toBe(true);
        expect(projectFileExists('node_modules/@anthropic-ai/claude-agent-sdk/sdk.mjs')).toBe(true);
        expect(projectFileExists('node_modules/@anthropic-ai/claude-agent-sdk/package.json')).toBe(true);
    });

    it('ships the user guide as an indexed Apple Help Book', () => {
        const helpInfo = readProjectFile(`${helpBookRoot}/Info.plist`);
        const index = readProjectFile(`${helpBookRoot}/Resources/English.lproj/index.html`);

        expect(helpInfo).toContain('<key>CFBundleIdentifier</key>');
        expect(helpInfo).toContain(`<string>${helpBookName}</string>`);
        expect(helpInfo).toContain('<key>HPDBookAccessPath</key>');
        expect(helpInfo).toContain('<string>index.html</string>');
        expect(helpInfo).toContain('<key>HPDBookIndexPath</key>');
        expect(helpInfo).toContain('<string>Cull.helpindex</string>');
        expect(helpInfo).toContain('<key>HPDBookCSIndexPath</key>');
        expect(helpInfo).toContain('<string>Cull.cshelpindex</string>');

        expect(index).toContain(`name="AppleTitle" content="${helpBookName}"`);
        expect(projectFileExists(`${helpBookRoot}/Resources/English.lproj/Cull.helpindex`)).toBe(true);
        expect(projectFileExists(`${helpBookRoot}/Resources/English.lproj/Cull.cshelpindex`)).toBe(true);
        expect(projectFileSize(`${helpBookRoot}/Resources/English.lproj/Cull.helpindex`)).toBeGreaterThan(0);
        expect(projectFileSize(`${helpBookRoot}/Resources/English.lproj/Cull.cshelpindex`)).toBeGreaterThan(0);
    });

    it('opens the registered Apple Help Book in Tips instead of a repository README', () => {
        const menuSource = readProjectFile('src-tauri/src/menu.rs');
        const frontendMenuSource = readProjectFile('src/lib/menu.ts');

        expect(menuSource).toContain('open_cull_help_book');
        expect(menuSource).toContain('AHRegisterHelpBookWithURL');
        expect(menuSource).toContain('AHGotoPage');
        expect(menuSource).toContain('CULL_HELP_BOOK_ID');
        expect(menuSource).toContain(helpBookName);
        expect(menuSource).toContain('index.html');
        expect(menuSource).not.toContain('tauri_plugin_opener::open_path');
        expect(menuSource).not.toContain('Some("Safari")');
        expect(menuSource).not.toContain('showHelp(None)');
        expect(menuSource).not.toContain('openHelpAnchor_inBook');
        expect(menuSource).not.toContain('help:openbook');
        expect(frontendMenuSource).not.toMatch(/github\.com\/glebis\/(imageview|cull).*readme/i);
        expect(frontendMenuSource).not.toContain('docs/USER_GUIDE.md');
    });

    it('keeps the user guide covering the core user workflows', () => {
        const guide = readProjectFile('docs/USER_GUIDE.md');
        const helpIndex = readProjectFile(`${helpBookRoot}/Resources/English.lproj/index.html`);

        for (const heading of [
            '## Install And Run From Source',
            '## Import Images',
            '## Navigate Views',
            '## Review And Curate',
            '## Collections',
            '## Clipboard Monitor',
            '## Static Publishing',
            '## Embeddings And Search',
            '## Agent And MCP Workflows',
            '## Export Images',
            '## Privacy Defaults',
            '## CLI',
        ]) {
            expect(guide).toContain(heading);
        }

        for (const helpTopic of [
            'Clipboard Monitor',
            'Static publishing',
            'Agent and MCP workflows',
            'Monitor Clipboard',
            'Publish clipboard collection',
        ]) {
            expect(helpIndex).toContain(helpTopic);
        }
    });
});
