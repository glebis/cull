import { execFileSync, spawnSync } from 'node:child_process';
import {
  chmodSync,
  existsSync,
  lstatSync,
  mkdtempSync,
  mkdirSync,
  readFileSync,
  readdirSync,
  statSync,
  symlinkSync,
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

function createFixture(options: {
  gateCode?: string;
  changelog?: string;
  packageJson?: string;
  packageLock?: string;
  tauriJson?: string;
  gate?: string | string[];
} = {}) {
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
    worktree: '.',
    stateDir: '.release-state',
    gate: options.gate ?? [process.execPath, '-e', options.gateCode ?? 'process.exit(0)'],
    extraGate: [],
    changelog: { path: 'CHANGELOG.md' },
    compatibility: { path: 'docs/COMPATIBILITY.md' },
    artifacts: { required: ['Cull_{version}.dmg'] },
    homebrew: { repo: 'glebis/homebrew-tap', cask: 'Casks/cull.rb' },
    versionFiles,
  }, null, 2));
  writeFixtureFile(root, 'package.json', options.packageJson ?? '{\n  "name": "fixture",\n  "version": "1.2.3"\n}\n');
  writeFixtureFile(root, 'package-lock.json', options.packageLock ?? JSON.stringify({
    name: 'fixture', version: '1.2.3', packages: { '': { name: 'fixture', version: '1.2.3' } },
  }, null, 2));
  writeFixtureFile(root, 'src-tauri/tauri.conf.json', options.tauriJson ?? '{ "version": "1.2.3" }\n');
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
  writeFixtureFile(root, 'CHANGELOG.md', options.changelog ?? '# Changelog\n\n## [Unreleased]\n\nNo changes yet.\n\n## [1.2.3] - 2026-07-01\n');
  writeFixtureFile(root, 'docs/COMPATIBILITY.md', '# Compatibility\n\nLast updated: 1.2.3 (2026-07-01)\n');
  writeFixtureFile(root, 'untouched.txt', 'must stay untouched\n');
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
  execFileSync('git', ['config', 'user.name', 'Cull Test'], { cwd: root });
  execFileSync('git', ['config', 'user.email', 'cull@example.test'], { cwd: root });
  execFileSync('git', ['add', '.'], { cwd: root });
  execFileSync('git', ['-c', 'user.name=Cull Test', '-c', 'user.email=cull@example.test', 'commit', '-m', 'fixture'], {
    cwd: root,
    stdio: 'ignore',
  });
  execFileSync('git', ['update-ref', 'refs/remotes/origin/main', 'HEAD'], { cwd: root });
  return root;
}

function createReleaseFixture(options: Parameters<typeof createFixture>[0] = {}) {
  const checkout = createFixture(options);
  execFileSync('git', ['branch', 'fixture-holder'], { cwd: checkout });
  execFileSync('git', ['switch', 'fixture-holder'], { cwd: checkout, stdio: 'ignore' });
  const releaseWorktree = `${checkout}-release`;
  execFileSync('git', ['worktree', 'add', releaseWorktree, 'main'], { cwd: checkout, stdio: 'ignore' });
  return releaseWorktree;
}

function runCheck(fixture: string, options: { bump?: string; env?: NodeJS.ProcessEnv } = {}) {
  return spawnSync(process.execPath, [cli, 'check', '--bump', options.bump ?? 'patch', '--json'], {
    cwd: fixture,
    encoding: 'utf8',
    env: { ...process.env, CULL_RELEASE_TEST_MODE: '1', ...options.env },
  });
}

function run(
  fixture: string,
  command: string,
  args: string[] = [],
  env: NodeJS.ProcessEnv = {},
) {
  const execution = spawnSync(process.execPath, [cli, command, ...args, '--json'], {
    cwd: fixture,
    encoding: 'utf8',
    env: { ...process.env, CULL_RELEASE_TEST_MODE: '1', ...env },
  });
  return { execution, output: JSON.parse(execution.stdout) };
}

function head(fixture: string) {
  return execFileSync('git', ['rev-parse', 'HEAD'], { cwd: fixture, encoding: 'utf8' }).trim();
}

function prepareArgs(fixture: string, source = head(fixture)) {
  return [
    '--bump', 'patch',
    '--expected-source', source,
    '--expected-version', '1.2.4',
    '--request-json', JSON.stringify({
      version: '1.2.4',
      requestedBump: 'patch',
      stableBreakingChange: false,
      changedSurfaces: [],
      reviewedBy: 'Gleb Kalinin',
    }),
    '--notes', '### Fixed\\n\\n- Release preparation is now guarded.',
  ];
}

const PREPARE_TRACKED_PATHS = [
  'package.json',
  'package-lock.json',
  'src-tauri/tauri.conf.json',
  'src-tauri/Cargo.toml',
  'src-tauri/Cargo.lock',
  'CHANGELOG.md',
  'docs/COMPATIBILITY.md',
];

function prepareSafetySnapshot(fixture: string) {
  const indexPath = execFileSync('git', ['rev-parse', '--git-path', 'index'], {
    cwd: fixture, encoding: 'utf8',
  }).trim();
  return {
    files: Object.fromEntries(PREPARE_TRACKED_PATHS.map((path) => {
      const absolutePath = join(fixture, path);
      const stat = lstatSync(absolutePath);
      return [path, {
        bytes: readFileSync(absolutePath),
        mode: stat.mode & 0o777,
        regular: stat.isFile(),
        symlink: stat.isSymbolicLink(),
      }];
    })),
    index: readFileSync(resolve(fixture, indexPath)),
  };
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
  const indexPath = execFileSync('git', ['rev-parse', '--git-path', 'index'], gitOptions).trim();
  const index = statSync(resolve(fixture, indexPath), { bigint: true });
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

describe('Cull release prepare, resume, and state CLI', () => {
  it('keeps prepare dry-run truly zero-write', () => {
    const fixture = createReleaseFixture();
    const before = repositorySnapshot(fixture);

    const { execution, output } = run(fixture, 'prepare', [
      ...prepareArgs(fixture), '--dry-run',
    ]);

    expect(execution.status).toBe(0);
    expect(output.result.diff).toBe('');
    expect(repositorySnapshot(fixture)).toEqual(before);
  });

  it('rejects source and version races before writing', () => {
    const fixture = createReleaseFixture();
    const before = repositorySnapshot(fixture);

    const sourceMoved = run(fixture, 'prepare', prepareArgs(fixture, 'b'.repeat(40)));
    expect(sourceMoved.execution.status).toBe(2);
    expect(sourceMoved.output.code).toBe('SOURCE_MOVED');

    const versionMoved = run(fixture, 'prepare', prepareArgs(fixture).map((value) => (
      value === '1.2.4' ? '1.2.5' : value
    )));
    expect(versionMoved.execution.status).toBe(2);
    expect(versionMoved.output.code).toBe('VERSION_MOVED');
    expect(repositorySnapshot(fixture)).toEqual(before);
  });

  it('requires the named reviewer and a major bump for stable breaking changes', () => {
    const fixture = createReleaseFixture();
    const args = prepareArgs(fixture);
    const requestIndex = args.indexOf('--request-json') + 1;
    const review = JSON.parse(args[requestIndex]);
    review.reviewedBy = 'Someone Else';
    args[requestIndex] = JSON.stringify(review);

    const wrongReviewer = run(fixture, 'prepare', [...args, '--dry-run']);
    expect(wrongReviewer.execution.status).toBe(2);
    expect(wrongReviewer.output.code).toBe('REVIEW_INVALID');

    review.reviewedBy = 'Gleb Kalinin';
    review.stableBreakingChange = true;
    args[requestIndex] = JSON.stringify(review);
    const breakingPatch = run(fixture, 'prepare', [...args, '--dry-run']);
    expect(breakingPatch.execution.status).toBe(2);
    expect(breakingPatch.output.code).toBe('INCOMPATIBLE_BUMP');
  });

  it('prepares exactly the declared files in one commit without tagging or pushing', () => {
    const fixture = createReleaseFixture();
    const expectedModes = Object.fromEntries(PREPARE_TRACKED_PATHS.map((path) => {
      const mode = lstatSync(join(fixture, path)).mode;
      return [path, mode & 0o111 ? '100755' : '100644'];
    }));
    const oldHead = head(fixture);
    const oldRemote = execFileSync('git', ['rev-parse', 'origin/main'], { cwd: fixture, encoding: 'utf8' }).trim();

    const { execution, output } = run(fixture, 'prepare', prepareArgs(fixture), {
      CULL_RELEASE_NOW: '2026-07-11T12:00:00.000Z',
    });

    expect(execution.status).toBe(0);
    expect(output.result).toMatchObject({ version: '1.2.4', state: 'prepared' });
    expect(head(fixture)).not.toBe(oldHead);
    expect(execFileSync('git', ['show', '--format=', '--name-only', 'HEAD'], {
      cwd: fixture, encoding: 'utf8',
    }).trim().split('\n').sort()).toEqual([
      'CHANGELOG.md',
      'docs/COMPATIBILITY.md',
      'package-lock.json',
      'package.json',
      'src-tauri/Cargo.lock',
      'src-tauri/Cargo.toml',
      'src-tauri/tauri.conf.json',
    ].sort());
    expect(execFileSync('git', ['log', '-1', '--pretty=%s'], { cwd: fixture, encoding: 'utf8' }).trim())
      .toBe('chore(release): v1.2.4');
    expect(execFileSync('git', ['tag', '--list'], { cwd: fixture, encoding: 'utf8' }).trim()).toBe('');
    expect(execFileSync('git', ['rev-parse', 'origin/main'], { cwd: fixture, encoding: 'utf8' }).trim())
      .toBe(oldRemote);
    for (const [path, expectedMode] of Object.entries(expectedModes)) {
      expect(execFileSync('git', ['ls-tree', 'HEAD', '--', path], {
        cwd: fixture, encoding: 'utf8',
      }).split(/\s/, 1)[0]).toBe(expectedMode);
    }
    expect(readFileSync(join(fixture, 'untouched.txt'), 'utf8')).toBe('must stay untouched\n');
    expect(readFileSync(join(fixture, 'CHANGELOG.md'), 'utf8'))
      .toContain('## [1.2.4] - 2026-07-11\n\n### Fixed');
    expect(readFileSync(join(fixture, 'docs/COMPATIBILITY.md'), 'utf8'))
      .toContain('Last updated: 1.2.4 (2026-07-11)');
    const statePath = join(fixture, '.release-state/1.2.4.json');
    expect(JSON.parse(readFileSync(statePath, 'utf8'))).toMatchObject({
      version: '1.2.4', state: 'prepared', releaseCommit: head(fixture),
    });
    expect(statSync(statePath).mode & 0o777).toBe(0o600);
    expect(existsSync(`${statePath}.tmp`)).toBe(false);
  });

  it('rejects an ordinary checkout and accepts only a linked main release worktree', () => {
    const ordinary = createFixture();
    const rejected = run(ordinary, 'prepare', [...prepareArgs(ordinary), '--dry-run']);
    expect(rejected.execution.status).toBe(3);
    expect(rejected.output.code).toBe('BLOCKED');

    const linked = createReleaseFixture();
    const accepted = run(linked, 'prepare', [...prepareArgs(linked), '--dry-run']);
    expect(accepted.execution.status).toBe(0);
  });

  it.each([
    ['gate mutation', "require('fs').appendFileSync('package.json', ' ')", 'PLAN_MUTATED'],
    ['gate failure', 'process.exit(17)', 'BLOCKED'],
    ['unrelated staged file', [
      "require('fs').writeFileSync('gate-extra.txt', 'extra')",
      "require('child_process').execFileSync('git', ['add', '--', 'gate-extra.txt'])",
    ].join(';'), 'PREPARE_RACE'],
  ])('restores exact task files and index after %s', (_name, gateCode, errorCode) => {
    const fixture = createReleaseFixture({ gateCode });
    const before = prepareSafetySnapshot(fixture);

    const result = run(fixture, 'prepare', prepareArgs(fixture));

    expect(result.execution.status).not.toBe(0);
    expect(result.output.code).toBe(errorCode);
    expect(prepareSafetySnapshot(fixture)).toEqual(before);
    expect(head(fixture)).toBe(execFileSync('git', ['rev-parse', 'origin/main'], {
      cwd: fixture, encoding: 'utf8',
    }).trim());
    if (_name === 'unrelated staged file') expect(existsSync(join(fixture, 'gate-extra.txt'))).toBe(false);
  });

  it('rejects a gate mode change and restores the original regular file mode and bytes', () => {
    const fixture = createReleaseFixture({
      gateCode: "require('fs').chmodSync('package.json', 0o755)",
    });
    const before = prepareSafetySnapshot(fixture);

    const result = run(fixture, 'prepare', prepareArgs(fixture));

    expect(result.execution.status).not.toBe(0);
    expect(result.output.code).toBe('PLAN_MUTATED');
    expect(prepareSafetySnapshot(fixture)).toEqual(before);
  });

  it('rejects an owned-file symlink replacement and restores without writing through it', () => {
    const victim = join(mkdtempSync(join(tmpdir(), 'cull-release-symlink-victim-')), 'victim.json');
    writeFileSync(victim, 'victim must not change\n');
    const gateCode = [
      "require('fs').renameSync('package.json', 'gate-backup.json')",
      `require('fs').symlinkSync(${JSON.stringify(victim)}, 'package.json')`,
    ].join(';');
    const fixture = createReleaseFixture({ gateCode });
    const before = prepareSafetySnapshot(fixture);

    const result = run(fixture, 'prepare', prepareArgs(fixture));

    expect(result.execution.status).not.toBe(0);
    expect(result.output.code).toBe('PLAN_MUTATED');
    expect(readFileSync(victim, 'utf8')).toBe('victim must not change\n');
    expect(existsSync(join(fixture, 'gate-backup.json'))).toBe(false);
    expect(prepareSafetySnapshot(fixture)).toEqual(before);
  });

  it('restores an owned hard-link replacement without changing the victim inode', () => {
    const victim = join(mkdtempSync(join(tmpdir(), 'cull-release-hardlink-victim-')), 'victim.json');
    writeFileSync(victim, 'hard-link victim must not change\n');
    const gateCode = [
      "require('fs').renameSync('package.json', 'gate-backup.json')",
      `require('fs').linkSync(${JSON.stringify(victim)}, 'package.json')`,
    ].join(';');
    const fixture = createReleaseFixture({ gateCode });
    const before = prepareSafetySnapshot(fixture);

    const result = run(fixture, 'prepare', prepareArgs(fixture));

    expect(result.execution.status).not.toBe(0);
    expect(result.output.code).toBe('PLAN_MUTATED');
    expect(readFileSync(victim, 'utf8')).toBe('hard-link victim must not change\n');
    expect(existsSync(join(fixture, 'gate-backup.json'))).toBe(false);
    expect(prepareSafetySnapshot(fixture)).toEqual(before);
  });

  it.each([
    ['tracked modification', "require('fs').appendFileSync('untouched.txt', 'gate change\\n')", 'untouched.txt'],
    ['new untracked path', "require('fs').writeFileSync('gate-extra.txt', 'gate extra\\n')", 'gate-extra.txt'],
  ])('rejects and safely cleans an unrelated gate %s', (_name, gateCode, path) => {
    const fixture = createReleaseFixture({ gateCode });
    const original = path === 'untouched.txt' ? readFileSync(join(fixture, path)) : null;

    const result = run(fixture, 'prepare', prepareArgs(fixture));

    expect(result.execution.status).not.toBe(0);
    expect(result.output.code).toBe('PREPARE_RACE');
    if (original) expect(readFileSync(join(fixture, path))).toEqual(original);
    else expect(existsSync(join(fixture, path))).toBe(false);
  });

  it('removes gate-created empty directories while leaving pre-existing directories untouched', () => {
    const fixture = createReleaseFixture({
      gateCode: "require('fs').mkdirSync('gate-empty/a/b', {recursive:true})",
    });

    const result = run(fixture, 'prepare', prepareArgs(fixture));

    expect(result.execution.status).not.toBe(0);
    expect(result.output.code).toBe('PREPARE_RACE');
    expect(existsSync(join(fixture, 'gate-empty'))).toBe(false);
    expect(existsSync(join(fixture, 'docs'))).toBe(true);
    expect(existsSync(join(fixture, 'src-tauri'))).toBe(true);
  });

  it('restores exact task files and index when a gate moves HEAD', () => {
    const gateCode = [
      "require('fs').writeFileSync('gate-commit.txt', 'gate commit')",
      "require('child_process').execFileSync('git', ['add', '--', 'gate-commit.txt'])",
      "require('child_process').execFileSync('git', ['commit', '-m', 'gate moved head'], {stdio:'ignore'})",
    ].join(';');
    const fixture = createReleaseFixture({ gateCode });
    const before = prepareSafetySnapshot(fixture);
    const source = head(fixture);

    const result = run(fixture, 'prepare', prepareArgs(fixture, source));

    expect(result.execution.status).not.toBe(0);
    expect(result.output.code).toBe('SOURCE_MOVED');
    expect(result.output.details.sideEffects).toContain('gate-commit.txt');
    expect(prepareSafetySnapshot(fixture)).toEqual(before);
    expect(head(fixture)).not.toBe(source);
  });

  it('reports every moved-HEAD tracked divergence and new path without creating a release commit', () => {
    const gateCode = [
      "require('fs').appendFileSync('untouched.txt', 'committed gate change\\n')",
      "require('fs').unlinkSync('docs/COMPATIBILITY.md')",
      "require('fs').writeFileSync('gate-commit.txt', 'gate commit')",
      "require('child_process').execFileSync('git', ['add', '-A'])",
      "require('child_process').execFileSync('git', ['commit', '-m', 'gate moved head with side effects'], {stdio:'ignore'})",
    ].join(';');
    const fixture = createReleaseFixture({ gateCode });
    const source = head(fixture);

    const result = run(fixture, 'prepare', prepareArgs(fixture, source));

    expect(result.execution.status).not.toBe(0);
    expect(result.output.code).toBe('SOURCE_MOVED');
    expect(result.output.details.sideEffects).toEqual(expect.arrayContaining([
      'docs/COMPATIBILITY.md', 'gate-commit.txt', 'untouched.txt',
    ]));
    expect(execFileSync('git', ['log', '-1', '--pretty=%s'], { cwd: fixture, encoding: 'utf8' }).trim())
      .toBe('gate moved head with side effects');
    expect(head(fixture)).not.toBe(source);
  });

  it('preserves all existing Unreleased content when inserting curated notes', () => {
    const existing = [
      '# Changelog', '', '## [Unreleased]', '', '### Added', '',
      '- Existing unreleased feature.', '', '### Fixed', '', '- Existing unreleased fix.', '',
      '## [1.2.3] - 2026-07-01', '',
    ].join('\n');
    const fixture = createReleaseFixture({ changelog: existing });

    const result = run(fixture, 'prepare', prepareArgs(fixture), {
      CULL_RELEASE_NOW: '2026-07-11T12:00:00.000Z',
    });

    expect(result.execution.status).toBe(0);
    const changelog = readFileSync(join(fixture, 'CHANGELOG.md'), 'utf8');
    expect(changelog).toContain('- Existing unreleased feature.');
    expect(changelog).toContain('- Existing unreleased fix.');
    expect(changelog).toContain('- Release preparation is now guarded.');
  });

  it('preserves JSON bytes outside the three declared pointer values', () => {
    const packageJson = '{\n\t"name" : "fixture",\n\t"note": "1.2.3\\u0020",\n\t"version" : "1.2.3"\n}';
    const packageLock = '{"name":"fixture","version" : "1.2.3","note":"1.2.3","packages":{"":{"name":"fixture","version":"1.2.3"}}}';
    const tauriJson = '{\r\n  "identifier": "fixture",\r\n  "version"  :  "1.2.3"\r\n}\r\n';
    const fixture = createReleaseFixture({ packageJson, packageLock, tauriJson });

    const result = run(fixture, 'prepare', prepareArgs(fixture), {
      CULL_RELEASE_NOW: '2026-07-11T12:00:00.000Z',
    });

    expect(result.execution.status).toBe(0);
    expect(readFileSync(join(fixture, 'package.json'), 'utf8'))
      .toBe(packageJson.replace('"version" : "1.2.3"', '"version" : "1.2.4"'));
    expect(readFileSync(join(fixture, 'package-lock.json'), 'utf8')).toBe(
      packageLock
        .replace('"version" : "1.2.3"', '"version" : "1.2.4"')
        .replace('"version":"1.2.3"', '"version":"1.2.4"'),
    );
    expect(readFileSync(join(fixture, 'src-tauri/tauri.conf.json'), 'utf8'))
      .toBe(tauriJson.replace('"version"  :  "1.2.3"', '"version"  :  "1.2.4"'));
  });

  it('creates an exact focused commit even when a hook would stage an extra file', () => {
    const fixture = createReleaseFixture();
    const hookPath = resolve(fixture, execFileSync('git', ['rev-parse', '--git-path', 'hooks/pre-commit'], {
      cwd: fixture, encoding: 'utf8',
    }).trim());
    writeFixtureFile(dirname(hookPath), 'pre-commit', [
      '#!/bin/sh',
      'echo hook > hook-extra.txt',
      'git add -- hook-extra.txt',
      '',
    ].join('\n'));
    chmodSync(hookPath, 0o755);

    const result = run(fixture, 'prepare', prepareArgs(fixture), {
      CULL_RELEASE_NOW: '2026-07-11T12:00:00.000Z',
    });

    expect(result.execution.status).toBe(0);
    expect(existsSync(join(fixture, 'hook-extra.txt'))).toBe(false);
    expect(execFileSync('git', ['show', '--format=', '--name-only', 'HEAD'], {
      cwd: fixture, encoding: 'utf8',
    }).trim().split('\n').sort()).toEqual([...PREPARE_TRACKED_PATHS].sort());
  });

  it('preserves a verified commit and reports recovery when state-cache creation fails', () => {
    const fixture = createReleaseFixture();
    const victim = mkdtempSync(join(tmpdir(), 'cull-release-state-victim-'));
    const hookPath = resolve(fixture, execFileSync('git', ['rev-parse', '--git-path', 'hooks/post-commit'], {
      cwd: fixture, encoding: 'utf8',
    }).trim());
    writeFixtureFile(dirname(hookPath), 'post-commit', [
      '#!/bin/sh',
      `ln -s '${victim}' .release-state`,
      '',
    ].join('\n'));
    chmodSync(hookPath, 0o755);
    const source = head(fixture);

    const result = run(fixture, 'prepare', prepareArgs(fixture), {
      CULL_RELEASE_NOW: '2026-07-11T12:00:00.000Z',
      CULL_RELEASE_TEST_ALLOW_POST_COMMIT_HOOK: '1',
    });

    expect(result.execution.status).toBe(5);
    expect(result.output.code).toBe('INCONSISTENT_RECOVERY');
    expect(result.output.details).toMatchObject({ version: '1.2.4', releaseCommit: head(fixture) });
    expect(head(fixture)).not.toBe(source);
    expect(execFileSync('git', ['show', '--format=', '--name-only', 'HEAD'], {
      cwd: fixture, encoding: 'utf8',
    }).trim().split('\n').sort()).toEqual([...PREPARE_TRACKED_PATHS].sort());
  });

  it('accepts a simple legacy gate string and rejects shell-like legacy syntax', () => {
    const safe = createReleaseFixture({ gate: `${process.execPath} --version` });
    expect(run(safe, 'prepare', prepareArgs(safe), {
      CULL_RELEASE_NOW: '2026-07-11T12:00:00.000Z',
    }).execution.status).toBe(0);

    const unsafe = createReleaseFixture({ gate: `${process.execPath} --version && touch owned.txt` });
    const before = prepareSafetySnapshot(unsafe);
    const rejected = run(unsafe, 'prepare', prepareArgs(unsafe));
    expect(rejected.execution.status).toBe(2);
    expect(rejected.output.code).toBe('CONFIG_INVALID');
    expect(existsSync(join(unsafe, 'owned.txt'))).toBe(false);
    expect(prepareSafetySnapshot(unsafe)).toEqual(before);
  });

  it('atomically transitions and records failures in state', () => {
    const fixture = createFixture();
    const stateDir = join(fixture, '.release-state');
    mkdirSync(stateDir);
    writeFileSync(join(stateDir, '1.2.4.json'), JSON.stringify({
      schema: 'cull.release.v1', version: '1.2.4', bump: 'patch', state: 'requested',
      releaseCommit: head(fixture), tag: 'v1.2.4', workflowRunId: null,
      requestedAt: '2026-07-11T12:00:00.000Z', updatedAt: '2026-07-11T12:00:00.000Z',
      gates: {}, assets: {}, failure: null,
    }));

    const transitioned = run(fixture, 'state', [
      'transition', '--version', '1.2.4', '--to', 'checked',
      '--evidence-json', '{"readiness":"passed"}',
    ], { CULL_RELEASE_NOW: '2026-07-11T12:01:00.000Z' });
    expect(transitioned.execution.status).toBe(0);
    const statePath = join(stateDir, '1.2.4.json');
    expect(JSON.parse(readFileSync(statePath, 'utf8'))).toMatchObject({
      state: 'checked', gates: { readiness: 'passed' },
    });
    expect(statSync(statePath).mode & 0o777).toBe(0o600);
    expect(statSync(stateDir).mode & 0o777).toBe(0o700);
    expect(existsSync(`${statePath}.tmp`)).toBe(false);

    const failed = run(fixture, 'state', [
      'fail', '--version', '1.2.4', '--code', 'GATE_FAILED',
      '--evidence-json', '{"gate":"preflight"}',
    ], { CULL_RELEASE_NOW: '2026-07-11T12:02:00.000Z' });
    expect(failed.execution.status).toBe(0);
    expect(JSON.parse(readFileSync(statePath, 'utf8')).failure).toMatchObject({
      code: 'GATE_FAILED', evidence: { gate: 'preflight' },
    });
  });

  it('rejects unstable release failure codes', () => {
    const fixture = createFixture();
    const stateDir = join(fixture, '.release-state');
    mkdirSync(stateDir);
    writeFileSync(join(stateDir, '1.2.4.json'), JSON.stringify({
      schema: 'cull.release.v1', version: '1.2.4', bump: 'patch', state: 'published',
      releaseCommit: head(fixture), tag: 'v1.2.4', workflowRunId: 42,
      requestedAt: '2026-07-11T12:00:00.000Z', updatedAt: '2026-07-11T12:00:00.000Z',
      gates: {}, assets: {}, failure: null,
    }));

    const failed = run(fixture, 'state', [
      'fail', '--version', '1.2.4', '--code', 'whatever happened',
      '--evidence-json', '{"stage":"verify"}',
    ]);

    expect(failed.execution.status).toBe(2);
    expect(failed.output.code).toBe('INPUT_INVALID');
  });

  it('files one P0 incident, updates it idempotently, and prepares a patch plan after public verification fails', () => {
    const fixture = createFixture();
    const stateDir = join(fixture, '.release-state');
    mkdirSync(stateDir);
    const statePath = join(stateDir, '1.2.4.json');
    writeFileSync(statePath, JSON.stringify({
      schema: 'cull.release.v1', version: '1.2.4', bump: 'patch', state: 'homebrew-promoted',
      releaseCommit: head(fixture), tag: 'v1.2.4', workflowRunId: 42,
      requestedAt: '2026-07-11T12:00:00.000Z', updatedAt: '2026-07-11T12:00:00.000Z',
      gates: {}, assets: {}, failure: null,
    }));
    const bin = mkdtempSync(join(tmpdir(), 'cull-release-npm-wrapper-'));
    const log = join(bin, 'npm.jsonl');
    const fakeNpm = join(bin, 'npm');
    writeFileSync(fakeNpm, [
      '#!/usr/bin/env node',
      "const fs = require('node:fs');",
      "fs.appendFileSync(process.env.CULL_TEST_BD_LOG, JSON.stringify(process.argv.slice(2)) + '\\n');",
      "if (process.argv.includes('list')) process.stdout.write('[]\\n');",
      "else if (process.argv.includes('show')) process.stdout.write(JSON.stringify({id:'imageview-release-p0',status:'open',priority:0}) + '\\n');",
      "if (process.argv.includes('create')) process.stdout.write('imageview-release-p0\\n');",
      '',
    ].join('\n'));
    chmodSync(fakeNpm, 0o755);
    const env = { PATH: `${bin}:${process.env.PATH}`, CULL_TEST_BD_LOG: log };

    const first = run(fixture, 'state', [
      'fail', '--version', '1.2.4', '--code', 'POST_PUBLISH_VERIFY_FAILED',
      '--evidence-json', '{"check":"homebrew-launch","runId":42}',
    ], env);
    expect(first.execution.status).toBe(0);
    expect(first.output.result.failure).toMatchObject({
      code: 'POST_PUBLISH_VERIFY_FAILED',
      incidentId: 'imageview-release-p0',
      evidence: { check: 'homebrew-launch', runId: 42 },
    });

    const second = run(fixture, 'state', [
      'fail', '--version', '1.2.4', '--code', 'POST_PUBLISH_VERIFY_FAILED',
      '--evidence-json', '{"check":"homebrew-launch","runId":43}',
    ], env);
    expect(second.execution.status).toBe(0);
    expect(second.output.result.failure.incidentId).toBe('imageview-release-p0');
    expect(readFileSync(log, 'utf8').trim().split('\n').map((line) => JSON.parse(line))).toEqual([
      expect.arrayContaining(['run', 'bd', '--', 'list', '--json']),
      expect.arrayContaining(['run', 'bd', '--', 'create', '--type', 'task', '-p', 'P0']),
      expect.arrayContaining(['run', 'bd', '--', 'update', 'imageview-release-p0', '--status', 'open']),
    ]);

    const evidence = {
      commit: true, tag: true, workflow: true, releaseAsset: true,
      publishedRelease: true, tapCommit: true, postPublishVerified: false,
    };
    const resumed = run(fixture, 'resume', ['--version', '1.2.4'], {
      ...env, CULL_RELEASE_TEST_EVIDENCE: JSON.stringify(evidence),
    });
    expect(resumed.execution.status).toBe(0);
    expect(resumed.output.result).toMatchObject({
      nextState: null,
      nextAction: 'prepare-patch-plan',
      evidence,
      failure: { code: 'POST_PUBLISH_VERIFY_FAILED', incidentId: 'imageview-release-p0' },
    });

    const checked = runCheck(fixture, { env });
    expect(checked.status).toBe(3);
    expect(JSON.parse(checked.stdout).result.blockers).toContain(
      'Unresolved P0 release incident imageview-release-p0 blocks later releases',
    );
    expect(JSON.parse(readFileSync(log, 'utf8').trim().split('\n').at(-1)!)).toEqual(
      expect.arrayContaining(['run', 'bd', '--', 'show', 'imageview-release-p0', '--json']),
    );
    expect(readFileSync(statePath, 'utf8')).toContain('POST_PUBLISH_VERIFY_FAILED');
  });

  it('resumes at Homebrew promotion when a valid public release is ahead of the tap', () => {
    const fixture = createFixture();
    const stateDir = join(fixture, '.release-state');
    mkdirSync(stateDir);
    writeFileSync(join(stateDir, '1.2.4.json'), JSON.stringify({
      schema: 'cull.release.v1', version: '1.2.4', bump: 'patch', state: 'requested',
      releaseCommit: head(fixture), tag: 'v1.2.4', workflowRunId: 42,
      requestedAt: '2026-07-11T12:00:00.000Z', updatedAt: '2026-07-11T12:00:00.000Z',
      gates: {}, assets: {}, failure: null,
    }));
    const evidence = {
      commit: true, tag: true, workflow: true, releaseAsset: true,
      publishedRelease: true, tapCommit: false, postPublishVerified: false,
    };

    const resumed = run(fixture, 'resume', ['--version', '1.2.4'], {
      CULL_RELEASE_TEST_EVIDENCE: JSON.stringify(evidence),
    });

    expect(resumed.execution.status).toBe(0);
    expect(resumed.output.result).toEqual({
      nextState: 'homebrew-promoted', nextAction: 'promote-homebrew', evidence,
    });
  });

  it('derives resume from evidence without rewriting stale local state', () => {
    const fixture = createFixture();
    const stateDir = join(fixture, '.release-state');
    mkdirSync(stateDir);
    const statePath = join(stateDir, '1.2.4.json');
    writeFileSync(statePath, JSON.stringify({
      schema: 'cull.release.v1', version: '1.2.4', bump: 'patch', state: 'post-publish-verified',
      releaseCommit: head(fixture), tag: 'v1.2.4', workflowRunId: 42,
      requestedAt: '2026-07-11T12:00:00.000Z', updatedAt: '2026-07-11T12:00:00.000Z',
      gates: {}, assets: {}, failure: null,
    }));
    const before = readFileSync(statePath, 'utf8');
    const evidence = {
      commit: true, tag: true, workflow: true, releaseAsset: true,
      publishedRelease: false, tapCommit: false,
    };

    const resumed = run(fixture, 'resume', ['--version', '1.2.4'], {
      CULL_RELEASE_TEST_EVIDENCE: JSON.stringify(evidence),
    });
    expect(resumed.execution.status).toBe(0);
    expect(resumed.output.result).toEqual({
      nextState: 'published', nextAction: 'publish-verified-artifacts', evidence,
    });
    expect(readFileSync(statePath, 'utf8')).toBe(before);

    const shown = run(fixture, 'state', ['show', '--version', '1.2.4'], {
      CULL_RELEASE_TEST_EVIDENCE: JSON.stringify(evidence),
    });
    expect(shown.execution.status).toBe(0);
    expect(shown.output.result.derivedState).toBe('artifact-verified');
    expect(readFileSync(statePath, 'utf8')).toBe(before);
  });

  it('uses unique no-follow temp files and never follows a malicious fixed temp symlink', () => {
    const fixture = createFixture();
    const stateDir = join(fixture, '.release-state');
    mkdirSync(stateDir);
    const statePath = join(stateDir, '1.2.4.json');
    writeFileSync(statePath, JSON.stringify({
      schema: 'cull.release.v1', version: '1.2.4', bump: 'patch', state: 'requested',
      releaseCommit: head(fixture), tag: 'v1.2.4', workflowRunId: null,
      requestedAt: '2026-07-11T12:00:00.000Z', updatedAt: '2026-07-11T12:00:00.000Z',
      gates: {}, assets: {}, failure: null,
    }));
    const victim = join(fixture, 'victim.txt');
    writeFileSync(victim, 'do not follow\n');
    symlinkSync(victim, `${statePath}.tmp`);

    const result = run(fixture, 'state', [
      'transition', '--version', '1.2.4', '--to', 'checked', '--evidence-json', '{}',
    ]);

    expect(result.execution.status).toBe(0);
    expect(readFileSync(victim, 'utf8')).toBe('do not follow\n');
    expect(statSync(statePath).mode & 0o777).toBe(0o600);
  });

  it('removes only its unique temp and leaves no final state after an injected pre-fsync failure', () => {
    const fixture = createReleaseFixture();

    const result = run(fixture, 'prepare', prepareArgs(fixture), {
      CULL_RELEASE_NOW: '2026-07-11T12:00:00.000Z',
      CULL_RELEASE_TEST_FAIL_STATE_WRITE: 'before-fsync',
    });

    expect(result.execution.status).toBe(5);
    expect(result.output.code).toBe('INCONSISTENT_RECOVERY');
    const stateDir = join(fixture, '.release-state');
    expect(existsSync(join(stateDir, '1.2.4.json'))).toBe(false);
    expect(readdirSync(stateDir)).toEqual([]);
  });

  it('removes its unique temp and leaves no final state after an injected rename failure', () => {
    const fixture = createReleaseFixture();

    const result = run(fixture, 'prepare', prepareArgs(fixture), {
      CULL_RELEASE_NOW: '2026-07-11T12:00:00.000Z',
      CULL_RELEASE_TEST_FAIL_STATE_WRITE: 'rename',
    });

    expect(result.execution.status).toBe(5);
    expect(result.output.code).toBe('INCONSISTENT_RECOVERY');
    const stateDir = join(fixture, '.release-state');
    expect(existsSync(join(stateDir, '1.2.4.json'))).toBe(false);
    expect(readdirSync(stateDir)).toEqual([]);
  });

  it('rejects a symlinked state record instead of following it', () => {
    const fixture = createFixture();
    const stateDir = join(fixture, '.release-state');
    mkdirSync(stateDir);
    const victim = join(fixture, 'victim-state.json');
    writeFileSync(victim, JSON.stringify({
      schema: 'cull.release.v1', version: '1.2.4', bump: 'patch', state: 'requested',
      releaseCommit: head(fixture), tag: 'v1.2.4', workflowRunId: null,
      requestedAt: '2026-07-11T12:00:00.000Z', updatedAt: '2026-07-11T12:00:00.000Z',
      gates: {}, assets: {}, failure: null,
    }));
    symlinkSync(victim, join(stateDir, '1.2.4.json'));

    const result = run(fixture, 'resume', ['--version', '1.2.4'], {
      CULL_RELEASE_TEST_EVIDENCE: JSON.stringify({ commit: true }),
    });

    expect(result.execution.status).toBe(2);
    expect(result.output.code).toBe('STATE_INVALID');
    expect(readFileSync(victim, 'utf8')).toContain('"state":"requested"');
  });

  it.each([
    ['tag', { tag: '--upload-pack=evil' }],
    ['release SHA', { releaseCommit: '--help' }],
    ['state', { state: 'teleported' }],
    ['version', { version: '../1.2.4' }],
  ])('validates cached record %s before running probes', (_name, override) => {
    const fixture = createFixture();
    const stateDir = join(fixture, '.release-state');
    mkdirSync(stateDir);
    writeFileSync(join(stateDir, '1.2.4.json'), JSON.stringify({
      schema: 'cull.release.v1', version: '1.2.4', bump: 'patch', state: 'requested',
      releaseCommit: head(fixture), tag: 'v1.2.4', workflowRunId: null,
      requestedAt: '2026-07-11T12:00:00.000Z', updatedAt: '2026-07-11T12:00:00.000Z',
      gates: {}, assets: {}, failure: null, ...override,
    }));

    const result = run(fixture, 'resume', ['--version', '1.2.4'], {
      CULL_RELEASE_TEST_EVIDENCE: '{"postPublishVerified":true}',
    });

    expect(result.execution.status).toBe(2);
    expect(result.output.code).toBe('STATE_INVALID');
  });

  it('reaches the terminal state from production commit, tag, public, tap, and provenance probes', () => {
    const fixture = createFixture();
    execFileSync('git', ['commit', '--allow-empty', '-m', 'chore(release): v1.2.4'], {
      cwd: fixture, stdio: 'ignore',
    });
    const releaseCommit = head(fixture);
    execFileSync('git', ['tag', 'v1.2.4'], { cwd: fixture });
    const stateDir = join(fixture, '.release-state');
    mkdirSync(stateDir);
    writeFileSync(join(stateDir, '1.2.4.json'), JSON.stringify({
      schema: 'cull.release.v1', version: '1.2.4', bump: 'patch', state: 'requested',
      releaseCommit, tag: 'v1.2.4', workflowRunId: 42,
      requestedAt: '2026-07-11T12:00:00.000Z', updatedAt: '2026-07-11T12:00:00.000Z',
      gates: {}, assets: {}, failure: null,
    }));
    const bin = mkdtempSync(join(tmpdir(), 'cull-release-gh-probes-'));
    const fakeGh = join(bin, 'gh');
    const dmgSha256 = 'a'.repeat(64);
    const cask = Buffer.from(`version "1.2.4"\nsha256 "${dmgSha256}"\n`).toString('base64');
    writeFileSync(fakeGh, [
      '#!/usr/bin/env node',
      `const releaseCommit = ${JSON.stringify(releaseCommit)};`,
      `const cask = ${JSON.stringify(cask)};`,
      "const args = process.argv.slice(2);",
      "if (args[0] === 'run') console.log(JSON.stringify({conclusion:'success'}));",
      "else if (args[0] === 'api') console.log(JSON.stringify({content:cask}));",
      "else if (args[0] === 'release' && args[1] === 'view') console.log(JSON.stringify({isDraft:false,assets:[{name:'Cull_1.2.4.dmg'}]}));",
      "else if (args[0] === 'release' && args[1] === 'download') console.log(JSON.stringify({schema:'cull.release.provenance.v1',version:'1.2.4',tag:'v1.2.4',commit:releaseCommit,postPublishVerified:true,assets:{'Cull_1.2.4.dmg':{sha256:'a'.repeat(64)}}}));",
      'else process.exit(9);',
      '',
    ].join('\n'));
    chmodSync(fakeGh, 0o755);
    const evidence = {
      commit: true, tag: true, workflow: true, releaseAsset: true,
      publishedRelease: true, tapCommit: true, postPublishVerified: true,
    };

    const result = run(fixture, 'resume', ['--version', '1.2.4'], {
      PATH: `${bin}:${process.env.PATH}`,
    });

    expect(result.execution.status).toBe(0);
    expect(result.output.result).toEqual({ nextState: null, nextAction: 'complete', evidence });
  });

  it('treats a same-version tap with the wrong DMG SHA as behind the published release', () => {
    const fixture = createFixture();
    execFileSync('git', ['commit', '--allow-empty', '-m', 'chore(release): v1.2.4'], {
      cwd: fixture, stdio: 'ignore',
    });
    const releaseCommit = head(fixture);
    execFileSync('git', ['tag', 'v1.2.4'], { cwd: fixture });
    const stateDir = join(fixture, '.release-state');
    mkdirSync(stateDir);
    writeFileSync(join(stateDir, '1.2.4.json'), JSON.stringify({
      schema: 'cull.release.v1', version: '1.2.4', bump: 'patch', state: 'requested',
      releaseCommit, tag: 'v1.2.4', workflowRunId: 42,
      requestedAt: '2026-07-11T12:00:00.000Z', updatedAt: '2026-07-11T12:00:00.000Z',
      gates: {}, assets: {}, failure: null,
    }));
    const bin = mkdtempSync(join(tmpdir(), 'cull-release-wrong-tap-sha-'));
    const fakeGh = join(bin, 'gh');
    const cask = Buffer.from(`version "1.2.4"\nsha256 "${'b'.repeat(64)}"\n`).toString('base64');
    const provenance = {
      schema: 'cull.release.provenance.v1', version: '1.2.4', tag: 'v1.2.4',
      commit: releaseCommit, postPublishVerified: true,
      assets: { 'Cull_1.2.4.dmg': { sha256: 'a'.repeat(64) } },
    };
    writeFileSync(fakeGh, [
      '#!/usr/bin/env node',
      `const cask = ${JSON.stringify(cask)};`,
      `const provenance = ${JSON.stringify(provenance)};`,
      "const args = process.argv.slice(2);",
      "if (args[0] === 'run') console.log(JSON.stringify({conclusion:'success'}));",
      "else if (args[0] === 'api') console.log(JSON.stringify({content:cask}));",
      "else if (args[0] === 'release' && args[1] === 'view') console.log(JSON.stringify({isDraft:false,assets:[{name:'Cull_1.2.4.dmg'}]}));",
      "else if (args[0] === 'release' && args[1] === 'download') console.log(JSON.stringify(provenance));",
      'else process.exit(9);',
      '',
    ].join('\n'));
    chmodSync(fakeGh, 0o755);

    const result = run(fixture, 'resume', ['--version', '1.2.4'], {
      PATH: `${bin}:${process.env.PATH}`,
    });

    expect(result.execution.status).toBe(0);
    expect(result.output.result.nextAction).toBe('promote-homebrew');
    expect(result.output.result.evidence).toMatchObject({ publishedRelease: true, tapCommit: false });
  });

  it('rejects duplicate options, option-looking values, and invalid evidence JSON', () => {
    const fixture = createFixture();
    const duplicate = run(fixture, 'check', ['--bump', 'patch', '--bump', 'minor']);
    expect(duplicate.execution.status).toBe(2);
    expect(duplicate.output.code).toBe('INPUT_INVALID');
    expect(duplicate.output.message).toContain('Duplicate option');

    const missing = run(fixture, 'check', ['--bump', '--version']);
    expect(missing.execution.status).toBe(2);
    expect(missing.output.code).toBe('INPUT_INVALID');
    expect(missing.output.message).toContain('Missing value for --bump');

    const stateDir = join(fixture, '.release-state');
    mkdirSync(stateDir);
    writeFileSync(join(stateDir, '1.2.4.json'), JSON.stringify({
      schema: 'cull.release.v1', version: '1.2.4', bump: 'patch', state: 'requested',
      releaseCommit: head(fixture), tag: 'v1.2.4', workflowRunId: null,
      requestedAt: '2026-07-11T12:00:00.000Z', updatedAt: '2026-07-11T12:00:00.000Z',
      gates: {}, assets: {}, failure: null,
    }));
    const invalidJson = run(fixture, 'state', [
      'transition', '--version', '1.2.4', '--to', 'checked', '--evidence-json', '{oops',
    ]);
    expect(invalidJson.execution.status).toBe(2);
    expect(invalidJson.output.code).toBe('INPUT_INVALID');
    expect(invalidJson.output.message).toContain('Invalid --evidence-json');
  });
});
