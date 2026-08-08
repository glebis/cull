import { readFileSync } from 'node:fs';
import { describe, expect, it } from 'vitest';

const read = (path: string) => readFileSync(path, 'utf8');

describe('CI quality gate contract', () => {
  it('blocks ordinary merge CI on the packaged macOS interaction smoke and retains failure artifacts', () => {
    const ciWorkflow = read('.github/workflows/ci.yml');

    expect(ciWorkflow).toContain('packaged-interaction-smoke:');
    expect(ciWorkflow).toContain('bash tests/native/run-packaged-interaction-smoke.sh');
    expect(ciWorkflow).toContain('CULL_NATIVE_SMOKE_SHOTS: ${{ runner.temp }}/cull-native-interaction-smoke');
    expect(ciWorkflow).toContain('if: always()');
    expect(ciWorkflow).toContain('name: packaged-interaction-smoke-${{ github.run_id }}');
  });

  it('runs the production frontend build in every frontend CI tier', () => {
    const checkCi = read('scripts/check-ci.sh');
    const ciWorkflow = read('.github/workflows/ci.yml');
    const releaseWorkflow = read('.github/workflows/release.yml');
    const contributing = read('CONTRIBUTING.md');

    expect(checkCi).toMatch(/npm run check[\s\S]*npm test[\s\S]*npm run build/);
    expect(ciWorkflow).toContain('bash scripts/check-ci.sh frontend');
    expect(releaseWorkflow).toContain('npm run ci:frontend');
    expect(contributing).toContain(
      '`npm run ci:frontend` runs `npm ci`, `npm run check`, `npm test`, and `npm run build`.',
    );
  });

  it('keeps local helper worktrees out of Vitest discovery', () => {
    const viteConfig = read('vite.config.js');

    expect(viteConfig).toContain('configDefaults.exclude');
    expect(viteConfig).toContain('"**/.worktrees/**"');
  });

  it('keeps optimized Vite dependencies local to each worktree', () => {
    const viteConfig = read('vite.config.js');
    const gitignore = read('.gitignore');

    expect(viteConfig).toContain('const viteCacheVariant = nativeInteractionSmoke');
    expect(viteConfig).toContain('cacheDir: `.vite-cache/${viteCacheVariant}`');
    expect(gitignore).toContain('/.vite-cache/');
  });

  it('keeps the independently installed site package out of root Vitest discovery', () => {
    const viteConfig = read('vite.config.js');

    expect(viteConfig).toContain('"**/site/**"');
  });

  it('uses locked Rust dependencies and encodes Clippy warning policy in CI', () => {
    const checkCi = read('scripts/check-ci.sh');
    const releaseWorkflow = read('.github/workflows/release.yml');
    const contributing = read('CONTRIBUTING.md');

    expect(checkCi).toContain('cargo clippy --locked --all-targets');
    expect(checkCi).not.toContain('-D warnings');
    expect(checkCi).toContain('cargo test --locked --all-targets');
    expect(releaseWorkflow).toContain('npm run ci:rust');
    expect(contributing).toContain(
      '`npm run ci:rust` runs `cargo fmt --all -- --check`, `cargo clippy --locked --all-targets`, and `cargo test --locked --all-targets`. Clippy warnings are reported but not denied until `imageview-2w6.11` cleans up the existing warning backlog.',
    );
  });
});
