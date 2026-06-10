import { describe, expect, test } from 'vitest';
import { existsSync, readFileSync, readdirSync, statSync } from 'node:fs';
import { join } from 'node:path';

function read(path: string): string {
  return readFileSync(path, 'utf8');
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
    const strategy = read('docs/oss-strategy-explorer.html');
    const aiCopyright = read('docs/ai-code-copyright-research.md');
    const oldCopyleftLabel = `Open source (A${'G'}PL)`;

    expect(strategy).toContain("Historical planning artifact only. Cull's current license is Apache-2.0.");
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
