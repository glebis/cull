#!/usr/bin/env node
import { execFileSync, spawnSync } from 'node:child_process';
import {
  closeSync,
  constants,
  fchmodSync,
  fsyncSync,
  lstatSync,
  openSync,
  readFileSync,
  readlinkSync,
  statfsSync,
  symlinkSync,
  unlinkSync,
  writeFileSync,
  writeSync,
} from 'node:fs';
import { resolve } from 'node:path';
import {
  buildReadinessReport,
  applyVersionEdits,
  buildResumeAction,
  createReleaseRecord,
  deriveReleaseState,
  loadReleaseConfig,
  nextVersion,
  prepareRelease,
  readReleaseRecord,
  readVersionSnapshot,
  recordFailure,
  transitionReleaseRecord,
  validateVersionAlignment,
  writeReleaseRecordAtomic,
} from './cull-release-core.mjs';

const repoRoot = process.cwd();
const argv = process.argv.slice(2);
const command = argv[0] ?? null;

function commandError(code, message, details) {
  const error = new Error(message);
  error.code = code;
  error.details = details;
  return error;
}

function inputError(message) {
  return commandError('INPUT_INVALID', message);
}

function externalFailure(message, details) {
  return commandError('EXTERNAL_FAILURE', message, details);
}

const VALUE_OPTIONS = new Set([
  '--bump', '--expected-source', '--expected-version', '--request-json', '--notes',
  '--version', '--to', '--evidence-json', '--code',
]);

function parseArgs(args) {
  const parsed = {};
  for (let index = 0; index < args.length; index += 1) {
    const token = args[index];
    const key = token.slice(2).replace(/-([a-z])/g, (_match, letter) => letter.toUpperCase());
    if (Object.hasOwn(parsed, key)) throw inputError(`Duplicate option ${token}`);
    if (token === '--json') parsed.json = true;
    else if (token === '--dry-run') parsed.dryRun = true;
    else if (VALUE_OPTIONS.has(token)) {
      const value = args[index += 1];
      if (value === undefined || value.startsWith('--')) throw inputError(`Missing value for ${token}`);
      parsed[key] = value;
    } else throw inputError(`Unknown argument ${token}`);
  }
  if (!parsed.json) throw inputError('--json is required');
  return parsed;
}

function requireArgs(args, names) {
  for (const name of names) {
    if (args[name] === undefined) throw inputError(`--${name.replace(/[A-Z]/g, (c) => `-${c.toLowerCase()}`)} is required`);
  }
}

function git(...args) {
  try {
    return execFileSync('git', args, {
      cwd: repoRoot,
      encoding: 'utf8',
      env: { ...process.env, GIT_OPTIONAL_LOCKS: '0' },
      stdio: ['ignore', 'pipe', 'pipe'],
    }).trim();
  } catch (cause) {
    throw externalFailure(`Git command failed: git ${args.join(' ')}`, { status: cause.status });
  }
}

function gitBytes(...args) {
  try {
    return execFileSync('git', args, {
      cwd: repoRoot,
      env: { ...process.env, GIT_OPTIONAL_LOCKS: '0' },
      stdio: ['ignore', 'pipe', 'pipe'],
    });
  } catch (cause) {
    throw externalFailure(`Git command failed: git ${args.join(' ')}`, { status: cause.status });
  }
}

function rustVersion() {
  if (process.env.CULL_RELEASE_TEST_MODE === '1') {
    if (process.env.CULL_RELEASE_TEST_FAIL_PROBE === 'rust-missing') return null;
    if (process.env.CULL_RELEASE_TEST_FAIL_PROBE === 'rust-failure') {
      throw externalFailure('Rust toolchain probe failed', { code: 'RUSTC_FAILED' });
    }
  }
  const result = spawnSync('rustc', ['--version'], { encoding: 'utf8' });
  if (result.error?.code === 'ENOENT') return null;
  if (result.error) throw externalFailure('Rust toolchain probe failed', { code: result.error.code });
  if (result.status !== 0) {
    throw externalFailure('Rust toolchain probe failed', { status: result.status, signal: result.signal });
  }
  return result.stdout.trim();
}

function availableDiskGiB(path) {
  try {
    if (process.env.CULL_RELEASE_TEST_MODE === '1'
      && process.env.CULL_RELEASE_TEST_FAIL_PROBE === 'statfs') {
      throw new Error('Injected statfs failure');
    }
    const stats = statfsSync(path, { bigint: true });
    return Number(stats.bavail * stats.bsize) / (1024 ** 3);
  } catch (cause) {
    throw externalFailure('Free disk space probe failed', { code: cause.code ?? 'STATFS_FAILED' });
  }
}

function runCheck(args) {
  requireArgs(args, ['bump']);
  if (!['patch', 'minor', 'major'].includes(args.bump)) throw inputError(`Unsupported bump ${args.bump}`);
  const config = loadReleaseConfig(repoRoot);
  const currentVersion = validateVersionAlignment(readVersionSnapshot(repoRoot, config));
  const source = git('rev-parse', 'HEAD');
  const clean = git('status', '--porcelain').length === 0;
  const syncedWithOriginMain = source === git('rev-parse', 'origin/main');
  return buildReadinessReport({
    currentVersion,
    targetVersion: nextVersion(currentVersion, args.bump),
    source,
    branch: git('branch', '--show-current'),
    clean,
    syncedWithOriginMain,
    minimumFreeDiskGiB: config.minimumFreeDiskGiB,
    availableGiB: availableDiskGiB(repoRoot),
    nodeVersion: process.version,
    rustVersion: rustVersion(),
  });
}

function normalizeCommand(commandValue) {
  if (Array.isArray(commandValue)) {
    if (commandValue.length === 0
      || commandValue.some((part) => typeof part !== 'string' || part.length === 0 || part.includes('\0'))
      || commandValue[0].startsWith('-')) {
      throw commandError('CONFIG_INVALID', 'Release gate command array contains invalid argv');
    }
    return commandValue;
  }
  if (typeof commandValue === 'string') {
    if (/[;&|<>$`()"'\\\n\r\0]/.test(commandValue)) {
      throw commandError('CONFIG_INVALID', 'Legacy release gate contains unsafe shell-like syntax');
    }
    const parts = commandValue.trim().split(/\s+/);
    if (parts.length > 0 && parts[0] && !parts[0].startsWith('-')) return parts;
  }
  throw commandError('CONFIG_INVALID', 'Release gates must be non-empty command arrays or strings');
}

function runGate(commandValue) {
  const [executable, ...args] = normalizeCommand(commandValue);
  const result = spawnSync(executable, args, { cwd: repoRoot, encoding: 'utf8', stdio: 'pipe' });
  if (result.error || result.status !== 0) {
    throw commandError('BLOCKED', `Release gate failed: ${executable}`, {
      status: result.status,
      signal: result.signal,
    });
  }
}

function now() {
  return process.env.CULL_RELEASE_NOW ?? new Date().toISOString();
}

function assertDedicatedWorktree(config) {
  const gitDir = resolve(repoRoot, git('rev-parse', '--git-dir'));
  const commonDir = resolve(repoRoot, git('rev-parse', '--git-common-dir'));
  const topLevel = resolve(git('rev-parse', '--show-toplevel'));
  const superproject = git('rev-parse', '--show-superproject-working-tree');
  const branch = git('branch', '--show-current');
  if (gitDir === commonDir
    || topLevel !== resolve(repoRoot)
    || superproject !== ''
    || branch !== config.releaseBranch
    || resolve(repoRoot, config.worktree ?? '.') !== resolve(repoRoot)) {
    throw commandError('BLOCKED', 'Prepare must run on the configured branch in a dedicated linked release worktree');
  }
}

function snapshotPath(relativePath, regularOnly = false) {
  const absolutePath = resolve(repoRoot, relativePath);
  const stat = lstatSync(absolutePath);
  if (stat.isSymbolicLink()) {
    if (regularOnly) throw commandError('PLAN_INVALID', `Release-owned path must be a regular file: ${relativePath}`);
    return { relativePath, absolutePath, type: 'symlink', target: readlinkSync(absolutePath) };
  }
  if (!stat.isFile()) throw commandError('PLAN_INVALID', `Tracked path must be a regular file or symlink: ${relativePath}`);
  const fd = openSync(absolutePath, constants.O_RDONLY | constants.O_NOFOLLOW);
  try {
    return {
      relativePath,
      absolutePath,
      type: 'file',
      mode: stat.mode & 0o7777,
      bytes: readFileSync(fd),
    };
  } finally {
    closeSync(fd);
  }
}

function readRegularBytesNoFollow(absolutePath) {
  const fd = openSync(absolutePath, constants.O_RDONLY | constants.O_NOFOLLOW);
  try {
    return readFileSync(fd);
  } finally {
    closeSync(fd);
  }
}

function captureOwnedPaths(config) {
  const paths = [
    ...config.versionFiles.map((entry) => entry.path),
    config.changelog.path,
    config.compatibility.path,
  ];
  if (new Set(paths).size !== paths.length) {
    throw commandError('CONFIG_INVALID', 'Release-owned paths must be unique');
  }
  return paths.map((path) => snapshotPath(path, true));
}

function capturePrepareSnapshot(ownedFiles) {
  const indexPath = resolve(repoRoot, git('rev-parse', '--git-path', 'index'));
  const trackedPaths = gitBytes('ls-files', '-z').toString('utf8').split('\0').filter(Boolean);
  const owned = new Set(ownedFiles.map((file) => file.relativePath));
  return {
    files: ownedFiles,
    tracked: trackedPaths.filter((path) => !owned.has(path)).map((path) => snapshotPath(path)),
    untracked: new Set(gitBytes('ls-files', '--others', '--exclude-standard', '-z')
      .toString('utf8').split('\0').filter(Boolean)),
    indexPath,
    index: readFileSync(indexPath),
  };
}

function pathMatches(snapshot) {
  try {
    const stat = lstatSync(snapshot.absolutePath);
    if (snapshot.type === 'symlink') {
      return stat.isSymbolicLink() && readlinkSync(snapshot.absolutePath) === snapshot.target;
    }
    return stat.isFile()
      && !stat.isSymbolicLink()
      && (stat.mode & 0o7777) === snapshot.mode
      && readRegularBytesNoFollow(snapshot.absolutePath).equals(snapshot.bytes);
  } catch {
    return false;
  }
}

function safelyRemovePath(absolutePath) {
  if (process.env.CULL_RELEASE_TEST_MODE === '1') {
    const stat = lstatSync(absolutePath);
    if (stat.isDirectory() && !stat.isSymbolicLink()) {
      execFileSync('trash', [absolutePath], { stdio: 'ignore' });
    } else unlinkSync(absolutePath);
    return;
  }
  execFileSync('trash', [absolutePath], { stdio: 'ignore' });
}

function restorePath(snapshot) {
  if (pathMatches(snapshot)) return;
  let exists = true;
  try { lstatSync(snapshot.absolutePath); } catch (cause) {
    if (cause.code === 'ENOENT') exists = false;
    else throw cause;
  }
  if (snapshot.type === 'symlink') {
    if (exists) safelyRemovePath(snapshot.absolutePath);
    symlinkSync(snapshot.target, snapshot.absolutePath);
    return;
  }
  if (exists) {
    const current = lstatSync(snapshot.absolutePath);
    if (!current.isFile() || current.isSymbolicLink()) safelyRemovePath(snapshot.absolutePath);
  }
  const flags = exists && (() => {
    try {
      const current = lstatSync(snapshot.absolutePath);
      return current.isFile() && !current.isSymbolicLink();
    } catch { return false; }
  })()
    ? constants.O_WRONLY | constants.O_TRUNC | constants.O_NOFOLLOW
    : constants.O_WRONLY | constants.O_CREAT | constants.O_EXCL | constants.O_NOFOLLOW;
  const fd = openSync(snapshot.absolutePath, flags, snapshot.mode);
  try {
    fchmodSync(fd, snapshot.mode);
    let offset = 0;
    while (offset < snapshot.bytes.length) {
      offset += writeSync(fd, snapshot.bytes, offset, snapshot.bytes.length - offset);
    }
    fsyncSync(fd);
    fchmodSync(fd, snapshot.mode);
  } finally {
    closeSync(fd);
  }
}

function restorePrepareSnapshot(snapshot, source) {
  const failures = [];
  const unsafeSideEffects = [];
  for (const file of snapshot.files) {
    try { restorePath(file); } catch { failures.push(file.relativePath); }
  }
  try { writeFileSync(snapshot.indexPath, snapshot.index); } catch { failures.push('.git/index'); }
  if (git('rev-parse', 'HEAD') === source) {
    for (const file of snapshot.tracked) {
      if (!pathMatches(file)) {
        try { restorePath(file); } catch { failures.push(file.relativePath); }
      }
    }
    const currentUntracked = gitBytes('ls-files', '--others', '--exclude-standard', '-z')
      .toString('utf8').split('\0').filter(Boolean);
    for (const path of currentUntracked) {
      if (!snapshot.untracked.has(path)) {
        try { safelyRemovePath(resolve(repoRoot, path)); } catch { failures.push(path); }
      }
    }
  } else {
    unsafeSideEffects.push(...gitBytes('ls-files', '--others', '--exclude-standard', '-z')
      .toString('utf8').split('\0').filter((path) => path && !snapshot.untracked.has(path)));
  }
  if (failures.length > 0) {
    throw commandError('PREPARE_SIDE_EFFECT', 'Preparation side effects require manual cleanup', {
      paths: [...new Set(failures)].sort(),
    });
  }
  return [...new Set(unsafeSideEffects)].sort();
}

function assertPreCommitPlan(plan, source, snapshot) {
  if (git('rev-parse', 'HEAD') !== source) {
    throw commandError('SOURCE_MOVED', 'HEAD moved while release gates were running');
  }
  if (!readFileSync(snapshot.indexPath).equals(snapshot.index)) {
    throw commandError('PREPARE_RACE', 'Release gates modified the Git index');
  }
  const originals = new Map(snapshot.files.map((file) => [file.relativePath, file]));
  for (const edit of plan.edits) {
    const original = originals.get(edit.path);
    let valid = false;
    try {
      const stat = lstatSync(edit.absolutePath);
      valid = stat.isFile()
        && !stat.isSymbolicLink()
        && (stat.mode & 0o7777) === original.mode
        && readRegularBytesNoFollow(edit.absolutePath).equals(Buffer.from(edit.after));
    } catch { valid = false; }
    if (!valid) {
      throw commandError('PLAN_MUTATED', `Release gate changed planned bytes for ${edit.path}`);
    }
  }
  const entries = gitBytes('status', '--porcelain=v1', '-z', '--untracked-files=all')
    .toString('utf8').split('\0').filter(Boolean);
  const expected = new Set(plan.edits.map((edit) => edit.path));
  const actual = new Set();
  for (const entry of entries) {
    if (!entry.startsWith(' M ')) {
      throw commandError('PREPARE_RACE', `Unexpected repository change during preparation: ${entry}`);
    }
    actual.add(entry.slice(3));
  }
  if (actual.size !== expected.size || [...actual].some((path) => !expected.has(path))) {
    throw commandError('PREPARE_RACE', 'Repository changes no longer match the release plan');
  }
}

function verifyReleaseCommit(plan, source, subject, snapshot) {
  const releaseCommit = git('rev-parse', 'HEAD');
  const parent = git('rev-parse', `${releaseCommit}^`);
  const actualSubject = git('show', '-s', '--format=%s', releaseCommit);
  const changedPaths = git('diff-tree', '--no-commit-id', '--name-only', '-r', releaseCommit)
    .split('\n').filter(Boolean).sort();
  const expectedPaths = plan.edits.map((edit) => edit.path).sort();
  const originals = new Map(snapshot.files.map((file) => [file.relativePath, file]));
  const exactTree = plan.edits.every((edit) => {
    const tree = git('ls-tree', releaseCommit, '--', edit.path);
    const mode = tree.split(/\s/, 1)[0];
    const original = originals.get(edit.path);
    const expectedMode = original.mode & 0o111 ? '100755' : '100644';
    return mode === expectedMode
      && gitBytes('show', `${releaseCommit}:${edit.path}`).equals(Buffer.from(edit.after));
  });
  if (parent !== source
    || actualSubject !== subject
    || JSON.stringify(changedPaths) !== JSON.stringify(expectedPaths)
    || !exactTree) {
    throw commandError('INCONSISTENT_RECOVERY', 'Created release commit did not match the verified plan', {
      releaseCommit,
    });
  }
  return releaseCommit;
}

function runPrepare(args) {
  requireArgs(args, ['bump', 'expectedSource', 'expectedVersion', 'requestJson', 'notes']);
  if (!['patch', 'minor', 'major'].includes(args.bump)) throw inputError(`Unsupported bump ${args.bump}`);
  let request;
  try { request = JSON.parse(args.requestJson); } catch (cause) {
    throw inputError(`Invalid --request-json: ${cause.message}`);
  }
  const config = loadReleaseConfig(repoRoot);
  const source = git('rev-parse', 'HEAD');
  if (args.expectedSource !== source) {
    throw commandError('SOURCE_MOVED', 'HEAD moved after release planning', {
      expected: args.expectedSource, actual: source,
    });
  }
  if (git('status', '--porcelain').length !== 0) {
    throw commandError('BLOCKED', 'Prepare requires a clean worktree');
  }
  assertDedicatedWorktree(config);
  const ownedFiles = captureOwnedPaths(config);
  const currentVersion = validateVersionAlignment(readVersionSnapshot(repoRoot, config));
  const targetVersion = nextVersion(currentVersion, args.bump);
  if (args.expectedVersion !== targetVersion || request.version !== targetVersion) {
    throw commandError('VERSION_MOVED', 'Next release version changed after planning', {
      expected: args.expectedVersion, actual: targetVersion,
    });
  }
  if (request.requestedBump !== args.bump) throw inputError('Request bump does not match --bump');
  const timestamp = now();
  const plan = prepareRelease({
    repoRoot,
    config,
    request,
    notes: args.notes.replaceAll('\\n', '\n'),
    date: timestamp.slice(0, 10),
    dryRun: true,
  });
  if (args.dryRun) {
    return { version: plan.version, files: plan.edits.map((edit) => edit.path), diff: '' };
  }
  const snapshot = capturePrepareSnapshot(ownedFiles);
  let committed = false;
  let releaseCommit;
  try {
    applyVersionEdits(plan.edits);
    runGate(config.gate);
    for (const gate of config.extraGate ?? []) runGate(gate);
    assertPreCommitPlan(plan, source, snapshot);
    const subject = `chore(release): v${plan.version}`;
    git('add', '--', ...plan.edits.map((edit) => edit.path));
    const hookIsolation = process.env.CULL_RELEASE_TEST_MODE === '1'
      && process.env.CULL_RELEASE_TEST_ALLOW_POST_COMMIT_HOOK === '1'
      ? []
      : ['-c', 'core.hooksPath=/dev/null'];
    git(...hookIsolation, 'commit', '--no-verify', '--only', '-m', subject, '--', ...plan.edits.map((edit) => edit.path));
    committed = true;
    releaseCommit = verifyReleaseCommit(plan, source, subject, snapshot);
  } catch (error) {
    if (!committed) {
      const sideEffects = restorePrepareSnapshot(snapshot, source);
      if (sideEffects.length > 0) {
        error.details = { ...(error.details ?? {}), sideEffects };
      }
    }
    throw error;
  }
  let record = createReleaseRecord({
    version: plan.version,
    bump: args.bump,
    source: releaseCommit,
    now: timestamp,
  });
  record = transitionReleaseRecord(record, 'checked', { readiness: 'passed' }, timestamp);
  record = transitionReleaseRecord(record, 'prepared', { preparation: 'committed' }, timestamp);
  try {
    writeReleaseRecordAtomic(repoRoot, config, record);
  } catch (cause) {
    throw commandError(
      'INCONSISTENT_RECOVERY',
      'Release commit succeeded but the local state cache could not be written',
      { version: plan.version, releaseCommit, cause: cause.message },
    );
  }
  return { version: plan.version, state: record.state, releaseCommit, files: plan.edits.map((edit) => edit.path) };
}

function tryGit(...args) {
  try { return git(...args); } catch { return null; }
}

function tryGh(...args) {
  try {
    return JSON.parse(execFileSync('gh', args, {
      cwd: repoRoot, encoding: 'utf8', stdio: ['ignore', 'pipe', 'ignore'],
    }));
  } catch { return null; }
}

function probeCommit(record) {
  return tryGit('show', '-s', '--format=%s', '--end-of-options', record.releaseCommit)
    === `chore(release): v${record.version}`;
}

function probeTag(record) {
  return tryGit('rev-parse', '--verify', '--end-of-options', `${record.tag}^{commit}`) === record.releaseCommit;
}

function probeWorkflow(record) {
  if (!record.workflowRunId) return false;
  return tryGh('run', 'view', '--json', 'conclusion', '--', String(record.workflowRunId))?.conclusion === 'success';
}

function probeRelease(record) {
  return tryGh('release', 'view', '--json', 'isDraft,assets', '--', record.tag);
}

function probeTapCommit(record, config) {
  const repo = config.homebrew?.repo;
  const cask = config.homebrew?.cask;
  if (!/^[A-Za-z0-9_.-]+\/[A-Za-z0-9_.-]+$/.test(repo ?? '')
    || !/^[A-Za-z0-9_./-]+$/.test(cask ?? '')
    || cask.startsWith('-')
    || cask.split('/').includes('..')) return false;
  const response = tryGh('api', `repos/${repo}/contents/${cask}`);
  if (!response?.content) return false;
  const contents = Buffer.from(response.content, 'base64').toString('utf8');
  return new RegExp(`^version "${record.version.replaceAll('.', '\\.')}"$`, 'm').test(contents);
}

function probePostPublishProvenance(record) {
  try {
    const raw = execFileSync('gh', [
      'release', 'download', '--pattern', 'release-provenance.json', '--output', '-',
      '--', record.tag,
    ], { cwd: repoRoot, encoding: 'utf8', stdio: ['ignore', 'pipe', 'ignore'] });
    const provenance = JSON.parse(raw);
    return provenance.schema === 'cull.release.provenance.v1'
      && provenance.version === record.version
      && provenance.tag === record.tag
      && provenance.releaseCommit === record.releaseCommit
      && provenance.postPublishVerified === true;
  } catch {
    return false;
  }
}

function probeEvidence(record, config) {
  if (process.env.CULL_RELEASE_TEST_MODE === '1' && process.env.CULL_RELEASE_TEST_EVIDENCE) {
    try { return JSON.parse(process.env.CULL_RELEASE_TEST_EVIDENCE); } catch (cause) {
      throw inputError(`Invalid CULL_RELEASE_TEST_EVIDENCE: ${cause.message}`);
    }
  }
  const release = probeRelease(record);
  const required = config.artifacts?.required?.map((name) => name.replace('{version}', record.version)) ?? [];
  const names = new Set(release?.assets?.map((asset) => asset.name) ?? []);
  const publishedRelease = release !== null && release.isDraft === false;
  const tapCommit = probeTapCommit(record, config);
  return {
    commit: probeCommit(record),
    tag: probeTag(record),
    workflow: probeWorkflow(record),
    releaseAsset: required.length > 0 && required.every((name) => names.has(name)),
    publishedRelease,
    tapCommit,
    postPublishVerified: publishedRelease && tapCommit && probePostPublishProvenance(record),
  };
}

function parseJsonOption(value, option) {
  try {
    const parsed = JSON.parse(value);
    if (parsed === null || typeof parsed !== 'object' || Array.isArray(parsed)) {
      throw new Error('expected a JSON object');
    }
    return parsed;
  } catch (cause) {
    throw inputError(`Invalid ${option}: ${cause.message}`);
  }
}

function readDerived(version) {
  const config = loadReleaseConfig(repoRoot);
  const record = readReleaseRecord(repoRoot, config, version);
  const evidence = probeEvidence(record, config);
  const derivedState = deriveReleaseState(evidence);
  return { config, record, evidence, derivedState };
}

function runState(subcommand, args) {
  requireArgs(args, ['version']);
  if (subcommand === 'show') {
    const { record, evidence, derivedState } = readDerived(args.version);
    return { record, derivedState, evidence };
  }
  const config = loadReleaseConfig(repoRoot);
  const record = readReleaseRecord(repoRoot, config, args.version);
  let updated;
  if (subcommand === 'transition') {
    requireArgs(args, ['to', 'evidenceJson']);
    updated = transitionReleaseRecord(record, args.to, parseJsonOption(args.evidenceJson, '--evidence-json'), now());
  } else if (subcommand === 'fail') {
    requireArgs(args, ['code', 'evidenceJson']);
    updated = recordFailure(record, {
      code: args.code,
      evidence: parseJsonOption(args.evidenceJson, '--evidence-json'),
    }, now());
  } else throw inputError(`Unknown state command ${subcommand}`);
  writeReleaseRecordAtomic(repoRoot, config, updated);
  return updated;
}

function runResume(args) {
  requireArgs(args, ['version']);
  const { evidence, derivedState } = readDerived(args.version);
  return { ...buildResumeAction(derivedState), evidence };
}

function errorExitCode(error) {
  if (error.code === 'BLOCKED') return 3;
  if (error.code === 'EXTERNAL_FAILURE') return 4;
  if (error.code === 'INCONSISTENT_RECOVERY') return 5;
  return 2;
}

try {
  let result;
  let args;
  if (command === 'state') {
    const subcommand = argv[1] ?? null;
    args = parseArgs(argv.slice(2));
    result = runState(subcommand, args);
  } else {
    args = parseArgs(argv.slice(1));
    if (command === 'check') result = runCheck(args);
    else if (command === 'prepare') result = runPrepare(args);
    else if (command === 'resume') result = runResume(args);
    else throw inputError(`Unknown command ${command}`);
  }
  const ok = command === 'check' ? result.blockers.length === 0 : true;
  process.stdout.write(`${JSON.stringify({
    schema: 'cull.release.command.v1', event: 'result', ok, command, result,
  })}\n`);
  if (!ok) process.exitCode = 3;
} catch (error) {
  process.stdout.write(`${JSON.stringify({
    schema: 'cull.release.command.v1',
    event: 'error',
    ok: false,
    command,
    code: error.code ?? 'INPUT_INVALID',
    message: error.message,
    ...(error.details === undefined ? {} : { details: error.details }),
  })}\n`);
  process.exitCode = errorExitCode(error);
}
