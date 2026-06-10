import { describe, expect, test } from 'vitest';
import { existsSync, readFileSync, readdirSync, statSync } from 'node:fs';
import { join } from 'node:path';
import { execFileSync } from 'node:child_process';

function read(path: string): string {
  return readFileSync(path, 'utf8');
}

function trackedFiles(...paths: string[]): string[] {
  return execFileSync('git', ['ls-files', '-z', '--', ...paths], { encoding: 'utf8' })
    .split('\0')
    .filter(Boolean);
}

function sourceFiles(dir: string): string[] {
  if (!existsSync(dir)) return [];
  return readdirSync(dir).flatMap((entry) => {
    const path = join(dir, entry);
    if (statSync(path).isDirectory()) return sourceFiles(path);
    return /\.(ts|svelte|rs)$/.test(entry) ? [path] : [];
  });
}

describe('open-source release legal and privacy contract', () => {
  test('authorship record quotes provider output-rights claims with official sources', () => {
    const authorship = read('AUTHORSHIP.md');
    const audit = read('docs/OPEN_SOURCE_AUDIT.md');

    for (const text of [authorship, audit]) {
      expect(text).toContain('"own the Output"');
      expect(text).toContain('"retain ownership rights"');
      expect(text).toContain('https://openai.com/policies/row-terms-of-use/');
      expect(text).toContain('https://www.anthropic.com/news/expanded-legal-protections-api-improvements');
    }
  });

  test('public privacy claims distinguish local core behavior from opt-in cloud features', () => {
    const presentation = read('docs/imageview-presentation.html');
    const privacyDashboard = read('src/lib/components/PrivacyDashboard.svelte');

    expect(presentation).not.toContain('Every pixel stays');
    expect(presentation).not.toContain('0</span>\n      cloud uploads');
    expect(presentation).toContain('Core review stays');
    expect(presentation).toContain('Cloud is opt-in');

    expect(privacyDashboard).toContain('Verify current provider terms before regulated or sensitive use.');
    expect(privacyDashboard).not.toContain('Free tier: Yes — images used for training');
    expect(privacyDashboard).not.toContain('30 days. Zero Data Retention available.');
    expect(privacyDashboard).not.toContain('≤30 days for debugging');
  });

  test('asset policy records concrete provenance categories for bundled public assets', () => {
    const assets = read('docs/ASSETS.md');
    const notice = read('NOTICE');

    expect(assets).toContain('Asset Attribution Inventory');
    expect(assets).toContain('JetBrains Mono');
    expect(assets).toContain('EB Garamond');
    expect(assets).toContain('Cull app icons');
    expect(assets).toContain('Product screenshots and generated mockups');
    expect(notice).toContain('Bundled fonts and visual assets are documented in docs/ASSETS.md.');
  });

  test('repo exposes conventional license and release supply-chain audit commands', () => {
    const packageJson = JSON.parse(read('package.json'));
    const audit = read('docs/OPEN_SOURCE_AUDIT.md');

    expect(existsSync('LICENSE')).toBe(true);
    expect(packageJson.scripts['audit:supply-chain']).toBe('bash scripts/supply-chain-audit.sh');
    expect(packageJson.scripts['audit:sbom']).toBe('bash scripts/supply-chain-audit.sh sbom');
    expect(audit).toContain('cargo-deny');
    expect(audit).toContain('CycloneDX');
    expect(audit).toContain('src-tauri/tests/fixtures/db/v21.db');
    expect(audit).toContain('contains no image, path, token, audit-log, or user-content rows');
  });

  test('supply-chain audit is executable and enforced in CI (HYG-003)', () => {
    const ci = read('.github/workflows/ci.yml');

    expect(existsSync('src-tauri/deny.toml')).toBe(true);
    const denyToml = read('src-tauri/deny.toml');
    expect(denyToml).toContain('[licenses]');
    expect(denyToml).toContain('"Apache-2.0"');
    expect(denyToml).toContain('[advisories]');

    expect(ci).toContain('scripts/supply-chain-audit.sh');
  });

  test('historical licensing artifacts cannot be mistaken for current licensing', () => {
    const aiCopyright = read('docs/ai-code-copyright-research.md');
    const oldCopyleftLabel = `Open source (A${'G'}PL)`;

    // The strategy explorer is an untracked internal artifact; if a local copy
    // exists it must still carry the historical-artifact disclaimer.
    const strategyPath = 'docs/internal/oss-strategy-explorer.html';
    if (existsSync(strategyPath)) {
      expect(read(strategyPath)).toContain(
        "Historical planning artifact only. Cull's current license is Apache-2.0."
      );
    }
    expect(aiCopyright).toContain('Open source (Apache-2.0)');
    expect(aiCopyright).not.toContain(oldCopyleftLabel);
  });

  test('copyright year agrees across NOTICE, About dialog, and source headers (HYG-002)', () => {
    const notice = read('NOTICE');
    const about = read('src/lib/components/AboutDialog.svelte');
    const authorship = read('AUTHORSHIP.md');

    const noticeYear = notice.match(/Copyright (\d{4})-present Gleb Kalinin/)?.[1];
    const aboutYear = about.match(/\(c\) (\d{4})-present Gleb Kalinin/)?.[1];
    const authorshipYear = authorship.match(/\(c\) (\d{4})-present Gleb Kalinin/)?.[1];

    // First commit is 2026-05-07, so the copyright term starts at 2026.
    expect(noticeYear).toBe('2026');
    expect(aboutYear).toBe(noticeYear);
    expect(authorshipYear).toBe(noticeYear);

    const staleCopyright = ['2025', 'present'].join('-');
    for (const file of [...sourceFiles('src'), ...sourceFiles('src-tauri/src')]) {
      expect(read(file), `${file} carries a stale copyright year`).not.toContain(staleCopyright);
    }
  });

  test('README and SECURITY describe the shipping app, not v0.1.0 (HYG-006)', () => {
    const readme = read('README.md');
    const security = read('SECURITY.md');
    const packageVersion = JSON.parse(read('package.json')).version as string;

    // Status heading must not pin a stale version; if it pins one, it must match package.json.
    const statusHeading = readme.match(/^## Current Status.*$/m)?.[0] ?? '';
    expect(statusHeading).not.toBe('');
    const pinnedVersion = statusHeading.match(/v(\d+\.\d+\.\d+)/)?.[1];
    if (pinnedVersion) expect(pinnedVersion).toBe(packageVersion);

    // Non-developers get a Download/Install path pointing at GitHub Releases.
    expect(readme).toMatch(/^## (Download|Install)/m);
    expect(readme).toContain('https://github.com/glebis/cull/releases');
    expect(readme.toLowerCase()).toContain('applications');

    // Security policy covers the shipping 0.2.x line.
    const major = packageVersion.split('.').slice(0, 2).join('.');
    expect(security).toContain(`${major}.x`);
    expect(security).not.toMatch(/\|\s*0\.1\.x\s*\|\s*Yes/);
  });

  test('the dead SessionTimeline component stays cut from the release (CQ-5)', () => {
    expect(existsSync('src/lib/components/SessionTimeline.svelte')).toBe(false);
    const api = read('src/lib/api.ts');
    expect(api).not.toContain('listSessionEvents');
  });
});

describe('repo-going-public content pass (HYG-004/SEC-005)', () => {
  const TEXT_FILE = /\.(md|json|html|ts|js|svelte|rs|ya?ml|toml|sh|txt|css|svg)$/;
  // Assembled so this test file never contains the literal it forbids.
  const PERSONAL_PATH = ['/Users', 'glebkalinin'].join('/');

  test('internal working artifacts are not tracked and docs/internal/ is gitignored', () => {
    const internalOnly = [
      'docs/cull-audit-2026-06-03.md',
      'docs/2026-05-10-vision-brainstorm-raw.md',
      'docs/dev-workflow-audit-2026-06-02.md',
      'docs/tooling-research-2026-06-03.md',
      'docs/settings-mockup-draft.json',
      'docs/settings-mockup-v2.json',
      'docs/oss-strategy-explorer.html'
    ];

    const tracked = new Set(trackedFiles('docs/'));
    for (const file of internalOnly) {
      expect(tracked.has(file), `${file} must not ship in the public repo`).toBe(false);
    }
    expect([...tracked].some((f) => f.startsWith('docs/internal/'))).toBe(false);
    expect(read('.gitignore')).toContain('docs/internal/');
  });

  test('.beads interaction traces are not tracked; only the issue export ships', () => {
    const tracked = trackedFiles('.beads/');
    expect(tracked).not.toContain('.beads/interactions.jsonl');
    // The public tracker mirror stays tracked deliberately.
    expect(tracked).toContain('.beads/issues.jsonl');
  });

  test('no personal absolute paths in tracked docs or AGENTS.md', () => {
    for (const file of trackedFiles('docs/', 'AGENTS.md')) {
      if (!TEXT_FILE.test(file)) continue;
      expect(read(file), `${file} leaks a personal absolute path`).not.toContain(PERSONAL_PATH);
    }
  });

  test('no personal absolute paths in source or shipped Tauri config', () => {
    for (const file of trackedFiles(
      'src/',
      'src-tauri/src/',
      'src-tauri/tauri.conf.json',
      'src-tauri/capabilities/'
    )) {
      if (!TEXT_FILE.test(file)) continue;
      expect(read(file), `${file} leaks a personal absolute path`).not.toContain(PERSONAL_PATH);
    }
  });

  test('the agent-surface doc names the real CLI flags and MCP tools and is linked from README (dkz.22)', () => {
    expect(existsSync('docs/agents.md')).toBe(true);
    const doc = read('docs/agents.md');
    const readme = read('README.md');
    const cli = read('src-tauri/src/cli/mod.rs');
    const headlessTools = read('src-tauri/src/cli/tools/mod.rs');
    const mcpTools = read('src-tauri/src/mcp/tools.rs');

    // README links the doc so a stranger can find it.
    expect(readme).toContain('docs/agents.md');

    // CLI flags the doc relies on must exist in the clap parser, so docs can't
    // drift from the real argument surface.
    for (const flag of ['mcp_stdio', 'mcp_http', 'mcp_http_allow_remote']) {
      expect(cli).toContain(flag);
    }
    for (const flag of ['--mcp-stdio', '--mcp-http', 'call_tool', '--json']) {
      expect(doc).toContain(flag);
    }

    // Headless-CLI tools cited in the doc must be in the real SUPPORTED_TOOLS slice.
    for (const tool of ['get_library_stats', 'import_folder', 'export_images']) {
      expect(headlessTools).toContain(`"${tool}"`);
      expect(doc).toContain(tool);
    }

    // MCP tool names in the demo loop must match the registered #[tool] fns.
    for (const tool of [
      'create_token',
      'capture_current_view_snapshot',
      'get_last_view_snapshot',
      'select_images_in_view',
      'get_audit_log',
    ]) {
      expect(mcpTools).toMatch(new RegExp(`(?:async )?fn ${tool}\\(`));
      expect(doc).toContain(tool);
    }

    // The doc must be honest that snapshot/selection tools are local stdio only
    // and that the headless CLI is a slice, not the full MCP surface.
    expect(doc.toLowerCase()).toContain('local stdio');
  });

  test('AGENTS.md carries no personal-machine reference paths', () => {
    const agents = read('AGENTS.md');
    expect(agents).not.toContain('~/Brains/brain');
    expect(agents).not.toContain('.Codex/refs');
    expect(agents).not.toContain('Codex-skills');
    expect(agents).not.toContain('Codex-ref-twitter');
  });
});
