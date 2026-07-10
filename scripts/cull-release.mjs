#!/usr/bin/env node
import { execFileSync, spawnSync } from 'node:child_process';
import { statfsSync } from 'node:fs';
import {
  buildReadinessReport,
  loadReleaseConfig,
  nextVersion,
  readVersionSnapshot,
  validateVersionAlignment,
} from './cull-release-core.mjs';

const repoRoot = process.cwd();
const argv = process.argv.slice(2);
const command = argv[0] ?? null;

function inputError(message) {
  const error = new Error(message);
  error.code = 'INPUT_INVALID';
  return error;
}

function externalFailure(message, details) {
  const error = new Error(message);
  error.code = 'EXTERNAL_FAILURE';
  error.details = details;
  return error;
}

function parseArgs(args) {
  const parsed = {};
  for (let index = 0; index < args.length; index += 1) {
    const token = args[index];
    if (token === '--json') {
      parsed.json = true;
    } else if (token === '--bump') {
      parsed.bump = args[index += 1];
      if (!parsed.bump) throw inputError('Missing value for --bump');
    } else {
      throw inputError(`Unknown argument ${token}`);
    }
  }
  if (!parsed.json) throw inputError('--json is required');
  if (!parsed.bump) throw inputError('--bump is required');
  if (!['patch', 'minor', 'major'].includes(parsed.bump)) {
    throw inputError(`Unsupported bump ${parsed.bump}`);
  }
  return parsed;
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
    const error = new Error(`Git command failed: git ${args.join(' ')}`);
    error.code = 'EXTERNAL_FAILURE';
    error.details = { status: cause.status };
    throw error;
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
  if (result.error) {
    throw externalFailure('Rust toolchain probe failed', { code: result.error.code });
  }
  if (result.status !== 0) {
    throw externalFailure('Rust toolchain probe failed', {
      status: result.status,
      signal: result.signal,
    });
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

function errorExitCode(error) {
  if (error.code === 'BLOCKED') return 3;
  if (error.code === 'EXTERNAL_FAILURE') return 4;
  if (error.code === 'INCONSISTENT_RECOVERY') return 5;
  return 2;
}

try {
  if (command !== 'check') throw inputError(`Unknown command ${command}`);
  const args = parseArgs(argv.slice(1));
  const result = runCheck(args);
  const ok = result.blockers.length === 0;
  process.stdout.write(`${JSON.stringify({
    schema: 'cull.release.command.v1',
    event: 'result',
    ok,
    command,
    result,
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
