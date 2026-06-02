import { describe, expect, it } from 'vitest';
import { existsSync, readFileSync } from 'node:fs';
import { join } from 'node:path';

const root = process.cwd();
const scriptPath = join(root, 'scripts/preflight.sh');
const packageJson = JSON.parse(readFileSync(join(root, 'package.json'), 'utf8'));

describe('Cull preflight command', () => {
    it('exposes a package script for tiered project preflight checks', () => {
        expect(packageJson.scripts.preflight).toBe('bash scripts/preflight.sh');
    });

    it('provides quick, full, and release tiers for the real Cull checks', () => {
        expect(existsSync(scriptPath)).toBe(true);

        const source = readFileSync(scriptPath, 'utf8');
        expect(source).toMatch(/^#!\/usr\/bin\/env bash/);
        expect(source).toContain('quick)');
        expect(source).toContain('full)');
        expect(source).toContain('release)');
        expect(source).toContain('npm run check');
        expect(source).toContain('npm test');
        expect(source).toContain('cargo fmt --all -- --check');
        expect(source).toContain('cargo clippy --all-targets');
        expect(source).toContain('cargo test --all-targets');
        expect(source).toContain('npm run audit:licenses');
        expect(source).toContain('npm run build');
        expect(source).not.toMatch(/\bgo test\b/);
        expect(source).not.toContain('golangci-lint');
        expect(source).not.toContain('gofmt');
        expect(source).not.toContain('default.nix');
        expect(source).not.toContain('go.sum');
    });

    it('documents npm preflight as the replacement for generic bd preflight', () => {
        const contributing = readFileSync(join(root, 'CONTRIBUTING.md'), 'utf8');
        const agents = readFileSync(join(root, 'AGENTS.md'), 'utf8');

        expect(contributing).toContain('npm run preflight -- quick');
        expect(contributing).toContain('npm run preflight -- full');
        expect(contributing).toContain('npm run preflight -- release');
        expect(agents).toContain('Do not use `bd preflight --check`');
        expect(agents).toContain('embedded bd preflight cannot be configured');
        expect(agents).toContain('npm run preflight -- <quick|full|release>');
    });
});
