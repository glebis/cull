import { describe, expect, test } from 'vitest';
import { existsSync, readFileSync } from 'node:fs';

function read(path: string): string {
  return readFileSync(path, 'utf8');
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

  test('historical licensing artifacts cannot be mistaken for current licensing', () => {
    const strategy = read('docs/oss-strategy-explorer.html');
    const aiCopyright = read('docs/ai-code-copyright-research.md');
    const oldCopyleftLabel = `Open source (A${'G'}PL)`;

    expect(strategy).toContain("Historical planning artifact only. Cull's current license is Apache-2.0.");
    expect(aiCopyright).toContain('Open source (Apache-2.0)');
    expect(aiCopyright).not.toContain(oldCopyleftLabel);
  });
});
