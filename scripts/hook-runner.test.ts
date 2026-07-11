import { execFileSync, spawnSync } from 'node:child_process';
import { chmodSync, mkdtempSync, mkdirSync, readFileSync, realpathSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join, resolve } from 'node:path';
import { describe, expect, it } from 'vitest';

const repoRoot = resolve(import.meta.dirname, '..');

function git(...args: string[]): string {
  return execFileSync('/usr/bin/git', args, { cwd: repoRoot, encoding: 'utf8' }).trimEnd();
}

describe('hook runner Git environment isolation', () => {
  it('scrubs repository-local Git variables before preflight commands', () => {
    const sandbox = mkdtempSync(join(tmpdir(), 'cull-hook-env-'));
    const bin = join(sandbox, 'bin');
    const fixture = join(sandbox, 'fixture');
    const report = join(sandbox, 'report');
    mkdirSync(bin);

    const fakeBash = join(bin, 'bash');
    writeFileSync(fakeBash, `#!/bin/sh
set -eu

[ "$1" = "scripts/preflight.sh" ]
[ "$2" = "hook" ]
[ -z "\${GIT_DIR+x}" ]
[ -z "\${GIT_WORK_TREE+x}" ]
[ -z "\${GIT_CONFIG+x}" ]
[ "\${GH_TOKEN:-}" = "auth-sentinel" ]
[ "\${GIT_AUTHOR_NAME:-}" = "author-sentinel" ]
[ "\${GIT_CONFIG_GLOBAL:-}" = "/dev/null" ]
for variable in $(/usr/bin/git rev-parse --local-env-vars); do
  if /usr/bin/env | /usr/bin/grep -q "^$variable="; then
    exit 1
  fi
done

/usr/bin/git init -q "$CULL_HOOK_FIXTURE"
/usr/bin/git -C "$CULL_HOOK_FIXTURE" config user.name Fixture
/usr/bin/git -C "$CULL_HOOK_FIXTURE" config user.email fixture@example.invalid
printf 'fixture\n' > "$CULL_HOOK_FIXTURE/file.txt"
/usr/bin/git -C "$CULL_HOOK_FIXTURE" add file.txt
/usr/bin/git -C "$CULL_HOOK_FIXTURE" commit -q -m fixture
/usr/bin/git -C "$CULL_HOOK_FIXTURE" rev-parse --show-toplevel > "$CULL_HOOK_REPORT"
`);
    chmodSync(fakeBash, 0o755);

    const before = {
      branch: git('branch', '--show-current'),
      head: git('rev-parse', 'HEAD'),
      status: git('status', '--porcelain=v1', '--untracked-files=all'),
    };
    const result = spawnSync('/bin/bash', ['scripts/hook-runner.sh', 'pre-commit', 'preserved-hook-arg'], {
      cwd: repoRoot,
      encoding: 'utf8',
      env: {
        ...process.env,
        PATH: `${bin}:${process.env.PATH ?? ''}`,
        GIT_DIR: git('rev-parse', '--absolute-git-dir'),
        GIT_WORK_TREE: repoRoot,
        GIT_CONFIG: '/dev/null',
        GH_TOKEN: 'auth-sentinel',
        GIT_AUTHOR_NAME: 'author-sentinel',
        GIT_CONFIG_GLOBAL: '/dev/null',
        CULL_HOOK_FIXTURE: fixture,
        CULL_HOOK_REPORT: report,
      },
    });

    expect(result.status, `${result.stdout}${result.stderr}`).toBe(0);
    expect(readFileSync(report, 'utf8').trim()).toBe(realpathSync(fixture));
    expect({
      branch: git('branch', '--show-current'),
      head: git('rev-parse', 'HEAD'),
      status: git('status', '--porcelain=v1', '--untracked-files=all'),
    }).toEqual(before);
  });
});
