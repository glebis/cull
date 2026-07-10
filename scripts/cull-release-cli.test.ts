import { execFileSync, spawnSync } from 'node:child_process';
import { mkdtempSync, mkdirSync, readFileSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { dirname, join, resolve } from 'node:path';
import { describe, expect, it } from 'vitest';

const cli = resolve(import.meta.dirname, 'cull-release.mjs');

function writeFixtureFile(root: string, path: string, contents: string) {
  const destination = join(root, path);
  mkdirSync(dirname(destination), { recursive: true });
  writeFileSync(destination, contents);
}

function createFixture() {
  const root = mkdtempSync(join(tmpdir(), 'cull-release-cli-'));
  const versionFiles = [
    { id: 'package', path: 'package.json', kind: 'json', pointers: ['/version'] },
    { id: 'package-lock', path: 'package-lock.json', kind: 'json', pointers: ['/version', '/packages//version'] },
    { id: 'tauri', path: 'src-tauri/tauri.conf.json', kind: 'json', pointers: ['/version'] },
    { id: 'cargo', path: 'src-tauri/Cargo.toml', kind: 'toml-package-version', package: 'cull' },
    { id: 'cargo-lock', path: 'src-tauri/Cargo.lock', kind: 'cargo-lock-package-version', package: 'cull' },
  ];

  writeFixtureFile(root, 'release.config.json', JSON.stringify({
    schemaVersion: 1,
    minimumFreeDiskGiB: 0,
    releaseBranch: 'main',
    versionFiles,
  }, null, 2));
  writeFixtureFile(root, 'package.json', '{\n  "name": "fixture",\n  "version": "1.2.3"\n}\n');
  writeFixtureFile(root, 'package-lock.json', JSON.stringify({
    name: 'fixture', version: '1.2.3', packages: { '': { name: 'fixture', version: '1.2.3' } },
  }, null, 2));
  writeFixtureFile(root, 'src-tauri/tauri.conf.json', '{ "version": "1.2.3" }\n');
  writeFixtureFile(root, 'src-tauri/Cargo.toml', [
    '[workspace.package]',
    'version = "9.9.9"',
    '',
    '[package]',
    'name = "cull"',
    'version = "1.2.3"',
    '',
    '[dependencies]',
    'version = "8.8.8"',
    '',
  ].join('\n'));
  writeFixtureFile(root, 'src-tauri/Cargo.lock', [
    'version = 4',
    '',
    '[[package]]',
    'name = "other"',
    'version = "7.7.7"',
    '',
    '[[package]]',
    'name = "cull"',
    'version = "1.2.3"',
    'dependencies = []',
    '',
  ].join('\n'));

  execFileSync('git', ['init', '-b', 'main'], { cwd: root, stdio: 'ignore' });
  execFileSync('git', ['add', '.'], { cwd: root });
  execFileSync('git', ['-c', 'user.name=Cull Test', '-c', 'user.email=cull@example.test', 'commit', '-m', 'fixture'], {
    cwd: root,
    stdio: 'ignore',
  });
  execFileSync('git', ['update-ref', 'refs/remotes/origin/main', 'HEAD'], { cwd: root });
  return root;
}

function runCheck(fixture: string) {
  return spawnSync(process.execPath, [cli, 'check', '--bump', 'patch', '--json'], {
    cwd: fixture,
    encoding: 'utf8',
    env: { ...process.env, CULL_RELEASE_TEST_MODE: '1' },
  });
}

describe('Cull release readiness CLI', () => {
  it('checks all five version locations without writing to the repository', () => {
    const fixture = createFixture();
    const before = readFileSync(join(fixture, 'package.json'), 'utf8');

    const result = runCheck(fixture);

    expect(result.status).toBe(0);
    expect(JSON.parse(result.stdout)).toMatchObject({
      schema: 'cull.release.command.v1',
      event: 'result',
      ok: true,
      command: 'check',
      result: {
        currentVersion: '1.2.3',
        targetVersion: '1.2.4',
        branch: 'main',
        clean: true,
        syncedWithOriginMain: true,
        blockers: [],
      },
    });
    expect(result.stderr).not.toContain('TAURI_SIGNING_PRIVATE_KEY');
    expect(readFileSync(join(fixture, 'package.json'), 'utf8')).toBe(before);
  });

  it('returns one classified JSON error for mismatched package-lock metadata', () => {
    const fixture = createFixture();
    const lockPath = join(fixture, 'package-lock.json');
    const lock = JSON.parse(readFileSync(lockPath, 'utf8'));
    lock.packages[''].version = '1.2.2';
    writeFileSync(lockPath, `${JSON.stringify(lock, null, 2)}\n`);

    const result = runCheck(fixture);
    const output = JSON.parse(result.stdout);

    expect(result.status).toBe(2);
    expect(output).toMatchObject({
      schema: 'cull.release.command.v1',
      event: 'error',
      ok: false,
      command: 'check',
      code: 'VERSION_MISMATCH',
    });
    expect(result.stdout.trim().split('\n')).toHaveLength(1);
  });

  it('exits three without writes when a readiness gate is blocked', () => {
    const fixture = createFixture();
    const before = readFileSync(join(fixture, 'package.json'), 'utf8');
    writeFileSync(join(fixture, 'uncommitted.txt'), 'block release\n');

    const result = runCheck(fixture);

    expect(result.status).toBe(3);
    expect(JSON.parse(result.stdout)).toMatchObject({
      event: 'result',
      ok: false,
      result: { blockers: [{ code: 'WORKTREE_DIRTY' }] },
    });
    expect(readFileSync(join(fixture, 'package.json'), 'utf8')).toBe(before);
  });
});
