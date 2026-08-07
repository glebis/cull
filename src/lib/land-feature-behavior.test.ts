import { beforeAll, describe, expect, it } from 'vitest';
import { chmod, mkdir, mkdtemp, readFile, writeFile } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { execFile } from 'node:child_process';

const script = join(process.cwd(), 'scripts/land-feature.sh');
let fixture: string;
let fakeBin: string;
let fakeRepo: string;

async function writeExecutable(path: string, source: string) {
    await writeFile(path, source);
    await chmod(path, 0o755);
}

beforeAll(async () => {
    fixture = await mkdtemp(join(tmpdir(), 'cull-land-feature-'));
    fakeBin = join(fixture, 'bin');
    fakeRepo = join(fixture, 'repo');
    await mkdir(fakeBin);
    await mkdir(fakeRepo);

    await writeExecutable(join(fakeBin, 'git'), `#!/usr/bin/env bash
set -euo pipefail
printf 'git %s\\n' "$*" >> "$LAND_LOG"
case "\${1:-} \${2:-}" in
  "rev-parse --show-toplevel") printf '%s\\n' "$LAND_FAKE_REPO" ;;
  "rev-parse codex/feature") printf '%s\\n' 'feature-sha' ;;
  "status --porcelain") ;;
  "fetch origin")
    if [[ "\${LAND_FEATURE_LOCAL_ONLY:-0}" == '1' && "$*" == *'codex/feature'* ]]; then
      exit 1
    fi
    ;;
  "show-ref --verify") exit 0 ;;
  "merge-base --is-ancestor")
    if [[ "\${LAND_TARGET_DIVERGED:-0}" == '1' ]]; then exit 1; fi
    ;;
  "rev-list --count") printf '%s\\n' '1' ;;
esac
`);

    await writeExecutable(join(fakeBin, 'gh'), `#!/usr/bin/env bash
set -euo pipefail
printf 'gh %s\\n' "$*" >> "$LAND_LOG"
case "\${1:-} \${2:-}" in
  "repo view") printf '%s\\n' 'glebis/cull' ;;
  "pr list")
    if [[ "\${LAND_EXISTING_PR:-0}" == '1' ]]; then printf '%s\\n' '42'; fi
    ;;
  "pr create") printf '%s\\n' 'https://github.com/glebis/cull/pull/42' ;;
  "pr view")
    if [[ "$*" == *'--json number'* ]]; then
      printf '%s\\n' '42'
    elif [[ "$*" == *'--json headRefOid,baseRefName'* ]]; then
      printf '%s\\n' 'feature-sha main'
    else
      if [[ "\${LAND_MERGE_DELAYED:-0}" == '1' && ! -f "$LAND_LOG.merge-polled" ]]; then
        : > "$LAND_LOG.merge-polled"
        printf 'OPEN\\t\\n'
      else
        printf '%s\\n' 'MERGED merge-sha'
      fi
    fi
    ;;
  "pr checks")
    if [[ "$*" == *'--json name'* ]]; then
      if [[ "\${LAND_NO_REQUIRED_CHECKS:-0}" == '1' ]]; then
        printf '%s\\n' '0'
      else
        printf '%s\\n' '4'
      fi
    elif [[ "\${LAND_CHECKS_FAIL:-0}" == '1' ]]; then
      exit 1
    fi
    ;;
  "pr merge") ;;
esac
`);

    await writeExecutable(join(fakeBin, 'npm'), `#!/usr/bin/env bash
set -euo pipefail
printf 'npm %s\\n' "$*" >> "$LAND_LOG"
if [[ "\${LAND_PREFLIGHT_FAIL:-0}" == '1' ]]; then exit 1; fi
`);
});

async function runLanding(name: string, extraEnv: Record<string, string> = {}) {
    const log = join(fixture, `${name}.log`);
    const result = await new Promise<{ status: number; stderr: string }>((resolve) => {
        execFile('bash', [script, 'codex/feature'], {
            cwd: fakeRepo,
            env: {
                ...process.env,
                PATH: `${fakeBin}:${process.env.PATH ?? ''}`,
                LAND_LOG: log,
                LAND_FAKE_REPO: fakeRepo,
                ...extraEnv,
            },
        }, (error, _stdout, stderr) => {
            const status = error ? (error as Error & { code?: number }).code ?? 1 : 0;
            resolve({ status, stderr });
        });
    });
    return { result, log: await readFile(log, 'utf8') };
}

describe('land-feature PR-only behavior', () => {
    it('pushes the feature, creates a PR, waits for required checks, merges, then fast-forwards main', async () => {
        const { result, log } = await runLanding('create');
        expect(result.status, result.stderr).toBe(0);

        const push = log.indexOf('git push --set-upstream origin codex/feature');
        const preflight = log.indexOf('npm run preflight -- full');
        const create = log.indexOf('gh pr create');
        const checks = log.indexOf('gh pr checks 42 --repo glebis/cull --required --watch');
        const merge = log.indexOf('gh pr merge 42 --repo glebis/cull --merge --match-head-commit feature-sha');
        const switchMain = log.indexOf('git switch main');
        const fastForward = log.indexOf('git merge --ff-only origin/main');

        expect(preflight).toBeGreaterThan(-1);
        expect(push).toBeGreaterThan(preflight);
        expect(create).toBeGreaterThan(push);
        expect(checks).toBeGreaterThan(create);
        expect(merge).toBeGreaterThan(checks);
        expect(switchMain).toBeGreaterThan(merge);
        expect(fastForward).toBeGreaterThan(switchMain);
        expect(log).toContain('gh api --method DELETE repos/glebis/cull/git/refs/heads/codex/feature');
        expect(log).not.toContain('git merge --no-ff');
        expect(log).not.toContain('git push origin main');
    });

    it('updates an existing PR by pushing the branch instead of creating another PR', async () => {
        const { result, log } = await runLanding('update', { LAND_EXISTING_PR: '1' });
        expect(result.status, result.stderr).toBe(0);
        expect(log).toContain('gh pr list');
        expect(log).not.toContain('gh pr create');
        expect(log).toContain('gh pr checks 42 --repo glebis/cull --required --watch');
        expect(log).toContain('gh pr merge 42 --repo glebis/cull --merge --match-head-commit feature-sha');
    });

    it('creates a PR for a fresh local feature branch that is not on the remote yet', async () => {
        const { result, log } = await runLanding('local-only', { LAND_FEATURE_LOCAL_ONLY: '1' });
        expect(result.status, result.stderr).toBe(0);
        expect(log).toContain('git fetch origin --prune');
        expect(log).toContain('git push --set-upstream origin codex/feature');
        expect(log).toContain('gh pr create');
    });

    it('waits for a queued pull request to be merged before moving local main', async () => {
        const { result, log } = await runLanding('merge-queued', {
            LAND_MERGE_DELAYED: '1',
            CULL_MERGE_WAIT_INTERVAL: '0',
        });
        expect(result.status, result.stderr).toBe(0);
        const mergeStateQueries = log.match(/gh pr view 42 .*--json state,mergeCommit/g) ?? [];
        expect(mergeStateQueries).toHaveLength(2);
        expect(log.indexOf('git switch main')).toBeGreaterThan(
            log.lastIndexOf('gh pr view 42 --repo glebis/cull --json state,mergeCommit'),
        );
    });

    it('fails closed without merging or moving local main when a required check fails', async () => {
        const { result, log } = await runLanding('failed-checks', { LAND_CHECKS_FAIL: '1' });
        expect(result.status).not.toBe(0);
        expect(log).toContain('gh pr checks 42 --repo glebis/cull --required --watch');
        expect(log).not.toContain('gh pr merge');
        expect(log).not.toContain('git switch main');
        expect(log).not.toContain('git merge --ff-only origin/main');
    });

    it('fails before publishing when local main contains unpreserved commits', async () => {
        const { result, log } = await runLanding('diverged-main', { LAND_TARGET_DIVERGED: '1' });
        expect(result.status).not.toBe(0);
        expect(log).toContain('git merge-base --is-ancestor main origin/main');
        expect(log).not.toContain('git push');
        expect(log).not.toContain('gh pr');
    });

    it('fails before pushing when the full local preflight fails', async () => {
        const { result, log } = await runLanding('failed-preflight', { LAND_PREFLIGHT_FAIL: '1' });
        expect(result.status).not.toBe(0);
        expect(log).toContain('npm run preflight -- full');
        expect(log).not.toContain('git push');
        expect(log).not.toContain('gh pr');
    });

    it('fails closed when GitHub exposes no required checks', async () => {
        const { result, log } = await runLanding('no-required-checks', {
            LAND_NO_REQUIRED_CHECKS: '1',
            CULL_REQUIRED_CHECK_DISCOVERY_ATTEMPTS: '1',
        });
        expect(result.status).not.toBe(0);
        expect(log).toContain('gh pr checks 42 --repo glebis/cull --required --json name');
        expect(log).not.toContain('gh pr merge');
        expect(log).not.toContain('git switch main');
    });
});
