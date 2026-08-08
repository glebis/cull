import { describe, expect, it } from 'vitest';
import { existsSync, readFileSync } from 'node:fs';
import { join } from 'node:path';

const root = process.cwd();
const scriptPath = join(root, 'scripts/land-feature.sh');
const docsPath = join(root, 'docs/landing-flow.md');
const historicalPlanPath = join(root, 'docs/superpowers/plans/2026-06-03-release-skill.md');
const packageJson = JSON.parse(readFileSync(join(root, 'package.json'), 'utf8'));

describe('feature landing flow', () => {
    it('exposes a package script for landing feature branches into main', () => {
        expect(packageJson.scripts['land:feature']).toBe('bash scripts/land-feature.sh');
    });

    it('provides a PR-only script that waits for required checks and fast-forwards main', () => {
        expect(existsSync(scriptPath)).toBe(true);

        const source = readFileSync(scriptPath, 'utf8');
        expect(source).toMatch(/^#!\/usr\/bin\/env bash/);
        expect(source).toContain('gh pr list');
        expect(source).toContain('gh pr create');
        expect(source).toContain('gh pr checks');
        expect(source).toContain('--required');
        expect(source).toContain('--watch');
        expect(source).toContain('gh pr merge');
        expect(source).toContain('--match-head-commit');
        expect(source).toContain('npm run preflight -- full');
        expect(source).toContain('git log "$remote/$target_branch..$feature_branch" --oneline');
        expect(source).toContain('git diff --stat "$remote/$target_branch...$feature_branch"');
        expect(source).toContain('git merge --ff-only');
        expect(source).toContain('gh api');
        expect(source).toContain('--method DELETE');
        expect(source).not.toContain('git merge --no-ff');
        expect(source).not.toContain('git push origin "$target_branch"');
        expect(source).not.toMatch(/\brm\b/);
    });

    it('documents that main CI is not the signed release build', () => {
        const docs = readFileSync(docsPath, 'utf8');
        expect(docs).toContain('main CI');
        expect(docs).toContain('Release workflow');
        expect(docs).toContain('tag/manual');
    });

    it('documents pull-request-only landing instead of a local main merge', () => {
        const docs = readFileSync(docsPath, 'utf8');
        const historicalPlan = readFileSync(historicalPlanPath, 'utf8');
        expect(docs).toContain('pull request');
        expect(docs).toContain('required checks');
        expect(docs).toContain('fast-forward');
        expect(docs).not.toContain('merges the feature branch with `--no-ff`');
        expect(docs).not.toContain('pushes `main`');
        expect(historicalPlan).not.toContain('`--no-ff`');
    });
});
