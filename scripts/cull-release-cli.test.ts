import { execFileSync, spawnSync } from 'node:child_process';
import {
  chmodSync,
  existsSync,
  mkdtempSync,
  mkdirSync,
  readFileSync,
  statSync,
  writeFileSync,
} from 'node:fs';
import { tmpdir } from 'node:os';
import { dirname, join, resolve } from 'node:path';
import { describe, expect, it } from 'vitest';
import { loadReleaseConfig } from './cull-release-core.mjs';

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
    minimumFreeDiskGiB: 0.000001,
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

function runCheck(fixture: string, options: { bump?: string; env?: NodeJS.ProcessEnv } = {}) {
  return spawnSync(process.execPath, [cli, 'check', '--bump', options.bump ?? 'patch', '--json'], {
    cwd: fixture,
    encoding: 'utf8',
    env: { ...process.env, CULL_RELEASE_TEST_MODE: '1', ...options.env },
  });
}

function expectConfigInvalid(result: ReturnType<typeof runCheck>) {
  expect(result.status).toBe(2);
  expect(JSON.parse(result.stdout)).toMatchObject({
    event: 'error',
    ok: false,
    command: 'check',
    code: 'CONFIG_INVALID',
  });
  expect(result.stdout.trim().split('\n')).toHaveLength(1);
}

function repositorySnapshot(fixture: string) {
  const metadataPaths = [
    'package.json',
    'package-lock.json',
    'src-tauri/tauri.conf.json',
    'src-tauri/Cargo.toml',
    'src-tauri/Cargo.lock',
  ];
  const gitOptions = {
    cwd: fixture,
    encoding: 'utf8',
    env: { ...process.env, GIT_OPTIONAL_LOCKS: '0' },
  } as const;
  const head = execFileSync('git', ['rev-parse', 'HEAD'], gitOptions);
  const porcelain = execFileSync('git', ['status', '--porcelain'], gitOptions);
  const index = statSync(join(fixture, '.git/index'), { bigint: true });
  return {
    metadata: Object.fromEntries(metadataPaths.map((path) => [
      path,
      readFileSync(join(fixture, path), 'utf8'),
    ])),
    stateDirExists: existsSync(join(fixture, '.release-state')),
    head,
    porcelain,
    index: { size: index.size, mtimeNs: index.mtimeNs, ctimeNs: index.ctimeNs },
  };
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

  it('disables optional Git locks and leaves repository and index state byte-for-byte unchanged', () => {
    const fixture = createFixture();
    const bin = mkdtempSync(join(tmpdir(), 'cull-release-git-wrapper-'));
    const wrapper = join(bin, 'git');
    writeFileSync(wrapper, [
      '#!/usr/bin/env node',
      "const { spawnSync } = require('node:child_process');",
      "if (process.env.GIT_OPTIONAL_LOCKS !== '0') process.exit(91);",
      "const result = spawnSync(process.env.CULL_TEST_REAL_GIT, process.argv.slice(2), { stdio: 'inherit' });",
      'process.exit(result.status ?? 92);',
      '',
    ].join('\n'));
    chmodSync(wrapper, 0o755);
    const before = repositorySnapshot(fixture);

    const result = runCheck(fixture, {
      env: {
        PATH: `${bin}:${process.env.PATH}`,
        CULL_TEST_REAL_GIT: execFileSync('which', ['git'], { encoding: 'utf8' }).trim(),
      },
    });

    expect(result.status).toBe(0);
    expect(repositorySnapshot(fixture)).toEqual(before);
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

  it('classifies an operational disk probe failure as external', () => {
    const fixture = createFixture();

    const result = runCheck(fixture, { env: { CULL_RELEASE_TEST_FAIL_PROBE: 'statfs' } });

    expect(result.status).toBe(4);
    expect(JSON.parse(result.stdout)).toMatchObject({
      event: 'error',
      ok: false,
      command: 'check',
      code: 'EXTERNAL_FAILURE',
    });
    expect(result.stdout.trim().split('\n')).toHaveLength(1);
  });

  it.each([
    ['rust-missing', 3, 'result', 'RUST_UNAVAILABLE'],
    ['rust-failure', 4, 'error', 'EXTERNAL_FAILURE'],
  ])('distinguishes the %s system-probe outcome', (probe, status, event, code) => {
    const fixture = createFixture();

    const result = runCheck(fixture, { env: { CULL_RELEASE_TEST_FAIL_PROBE: probe } });
    const output = JSON.parse(result.stdout);

    expect(result.status).toBe(status);
    expect(output.event).toBe(event);
    if (event === 'result') {
      expect(output.result.blockers).toContainEqual(expect.objectContaining({ code }));
    } else {
      expect(output.code).toBe(code);
    }
  });

  it.each([
    ['missing', undefined],
    ['negative', -1],
    ['zero', 0],
    ['non-finite', '1e400'],
  ])('rejects a %s minimumFreeDiskGiB', (_name, value) => {
    const fixture = createFixture();
    const configPath = join(fixture, 'release.config.json');
    const config = JSON.parse(readFileSync(configPath, 'utf8'));
    if (value === undefined) {
      delete config.minimumFreeDiskGiB;
      writeFileSync(configPath, JSON.stringify(config));
    } else if (typeof value === 'string') {
      writeFileSync(configPath, JSON.stringify(config).replace(
        /"minimumFreeDiskGiB":[^,}]+/,
        `"minimumFreeDiskGiB":${value}`,
      ));
    } else {
      config.minimumFreeDiskGiB = value;
      writeFileSync(configPath, JSON.stringify(config));
    }

    expectConfigInvalid(runCheck(fixture));
  });

  it('rejects malformed version-file declarations', () => {
    const fixture = createFixture();
    const configPath = join(fixture, 'release.config.json');
    const config = JSON.parse(readFileSync(configPath, 'utf8'));
    delete config.versionFiles[0].pointers;
    writeFileSync(configPath, JSON.stringify(config));

    expect(() => loadReleaseConfig(fixture)).toThrow('Malformed version file declaration');
    expectConfigInvalid(runCheck(fixture));
  });

  it('rejects duplicate version-file IDs', () => {
    const fixture = createFixture();
    const configPath = join(fixture, 'release.config.json');
    const config = JSON.parse(readFileSync(configPath, 'utf8'));
    config.versionFiles[1].id = config.versionFiles[0].id;
    writeFileSync(configPath, JSON.stringify(config));

    expectConfigInvalid(runCheck(fixture));
  });

  it('returns one input-error envelope for an unsupported bump before repository access', () => {
    const emptyDirectory = mkdtempSync(join(tmpdir(), 'cull-release-invalid-bump-'));

    const result = runCheck(emptyDirectory, { bump: 'banana' });

    expect(result.status).toBe(2);
    expect(JSON.parse(result.stdout)).toMatchObject({
      event: 'error',
      ok: false,
      command: 'check',
      code: 'INPUT_INVALID',
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
