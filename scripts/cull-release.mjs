#!/usr/bin/env node
import { execFileSync, spawnSync } from 'node:child_process';
import { statfsSync } from 'node:fs';
import { resolve } from 'node:path';
import {
  buildReadinessReport,
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
    if (token === '--json') parsed.json = true;
    else if (token === '--dry-run') parsed.dryRun = true;
    else if (VALUE_OPTIONS.has(token)) {
      const value = args[index += 1];
      if (value === undefined) throw inputError(`Missing value for ${token}`);
      parsed[token.slice(2).replace(/-([a-z])/g, (_match, letter) => letter.toUpperCase())] = value;
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
  if (Array.isArray(commandValue)
    && commandValue.length > 0
    && commandValue.every((part) => typeof part === 'string')) return commandValue;
  if (typeof commandValue === 'string') {
    const parts = commandValue.match(/"[^"]*"|'[^']*'|\S+/g)?.map((part) => (
      (/^(['"]).*\1$/.test(part) ? part.slice(1, -1) : part)
    ));
    if (parts?.length) return parts;
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
  const currentVersion = validateVersionAlignment(readVersionSnapshot(repoRoot, config));
  const targetVersion = nextVersion(currentVersion, args.bump);
  if (args.expectedVersion !== targetVersion || request.version !== targetVersion) {
    throw commandError('VERSION_MOVED', 'Next release version changed after planning', {
      expected: args.expectedVersion, actual: targetVersion,
    });
  }
  if (request.requestedBump !== args.bump) throw inputError('Request bump does not match --bump');
  if (git('status', '--porcelain').length !== 0) {
    throw commandError('BLOCKED', 'Prepare requires a clean worktree');
  }
  if (resolve(repoRoot, config.worktree ?? '.') !== resolve(repoRoot)) {
    throw commandError('BLOCKED', 'Prepare must run in the configured dedicated release worktree');
  }
  const timestamp = now();
  const plan = prepareRelease({
    repoRoot,
    config,
    request,
    notes: args.notes.replaceAll('\\n', '\n'),
    date: timestamp.slice(0, 10),
    dryRun: args.dryRun,
  });
  if (args.dryRun) {
    return { version: plan.version, files: plan.edits.map((edit) => edit.path), diff: '' };
  }
  runGate(config.gate);
  for (const gate of config.extraGate ?? []) runGate(gate);
  git('add', '--', ...plan.edits.map((edit) => edit.path));
  git('commit', '-m', `chore(release): v${plan.version}`);
  const releaseCommit = git('rev-parse', 'HEAD');
  let record = createReleaseRecord({
    version: plan.version,
    bump: args.bump,
    source: releaseCommit,
    now: timestamp,
  });
  record = transitionReleaseRecord(record, 'checked', { readiness: 'passed' }, timestamp);
  record = transitionReleaseRecord(record, 'prepared', { preparation: 'committed' }, timestamp);
  writeReleaseRecordAtomic(repoRoot, config, record);
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
  return tryGit('show', '-s', '--format=%s', record.releaseCommit) === `chore(release): v${record.version}`;
}

function probeTag(record) {
  return tryGit('rev-list', '-n', '1', record.tag) === record.releaseCommit;
}

function probeWorkflow(record) {
  if (!record.workflowRunId) return false;
  return tryGh('run', 'view', String(record.workflowRunId), '--json', 'conclusion')?.conclusion === 'success';
}

function probeRelease(record) {
  return tryGh('release', 'view', record.tag, '--json', 'isDraft,assets');
}

function probeTapCommit(record, config) {
  const repo = config.homebrew?.repo;
  const cask = config.homebrew?.cask;
  if (!repo || !cask) return false;
  const response = tryGh('api', `repos/${repo}/contents/${cask}`);
  if (!response?.content) return false;
  const contents = Buffer.from(response.content, 'base64').toString('utf8');
  return new RegExp(`^version "${record.version.replaceAll('.', '\\.')}"$`, 'm').test(contents);
}

function probeEvidence(record, config) {
  if (process.env.CULL_RELEASE_TEST_EVIDENCE) {
    try { return JSON.parse(process.env.CULL_RELEASE_TEST_EVIDENCE); } catch (cause) {
      throw inputError(`Invalid CULL_RELEASE_TEST_EVIDENCE: ${cause.message}`);
    }
  }
  const release = probeRelease(record);
  const required = config.artifacts?.required?.map((name) => name.replace('{version}', record.version)) ?? [];
  const names = new Set(release?.assets?.map((asset) => asset.name) ?? []);
  return {
    commit: probeCommit(record),
    tag: probeTag(record),
    workflow: probeWorkflow(record),
    releaseAsset: required.length > 0 && required.every((name) => names.has(name)),
    publishedRelease: release !== null && release.isDraft === false,
    tapCommit: probeTapCommit(record, config),
  };
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
    updated = transitionReleaseRecord(record, args.to, JSON.parse(args.evidenceJson), now());
  } else if (subcommand === 'fail') {
    requireArgs(args, ['code', 'evidenceJson']);
    updated = recordFailure(record, { code: args.code, evidence: JSON.parse(args.evidenceJson) }, now());
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
