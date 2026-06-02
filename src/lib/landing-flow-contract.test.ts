import { describe, expect, it } from 'vitest';
import { execFileSync } from 'node:child_process';
import { existsSync, readFileSync } from 'node:fs';
import { join } from 'node:path';

const root = process.cwd();
const scriptPath = join(root, 'scripts/land-feature.sh');
const docsPath = join(root, 'docs/landing-flow.md');
const packageJson = JSON.parse(readFileSync(join(root, 'package.json'), 'utf8'));

describe('feature landing flow', () => {
    it('exposes a package script for landing feature branches into main', () => {
        expect(packageJson.scripts['land:feature']).toBe('bash scripts/land-feature.sh');
    });

    it('provides an executable script that checks, builds, pushes, and monitors main', () => {
        expect(existsSync(scriptPath)).toBe(true);
        expect(execFileSync('git', ['ls-files', '-s', 'scripts/land-feature.sh'], { encoding: 'utf8' }))
            .toContain('100755');

        const source = readFileSync(scriptPath, 'utf8');
        expect(source).toContain('git merge --no-ff');
        expect(source).toContain('npm run check');
        expect(source).toContain('npm test');
        expect(source).toContain('npm run build');
        expect(source).toContain('bd sync');
        expect(source).toContain('bd vc status');
        expect(source).toContain('git push origin "$target_branch"');
        expect(source).toContain('gh run watch');
        expect(source).not.toMatch(/\brm\b/);
    });

    it('documents that main CI is not the signed release build', () => {
        const docs = readFileSync(docsPath, 'utf8');
        expect(docs).toContain('main CI');
        expect(docs).toContain('Release workflow');
        expect(docs).toContain('tag/manual');
    });
});
