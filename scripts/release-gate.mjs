#!/usr/bin/env node
import { spawnSync } from 'node:child_process';
import {
  appendFileSync,
  closeSync,
  constants,
  fsyncSync,
  mkdirSync,
  openSync,
  renameSync,
  unlinkSync,
  writeSync,
} from 'node:fs';
import { randomBytes } from 'node:crypto';
import { dirname, isAbsolute, resolve } from 'node:path';
import { classifyE2EPaths } from './cull-release-core.mjs';

const DB_CONTRACT = 'cargo test --manifest-path src-tauri/Cargo.toml --features test-support --test compat_golden';
const EXPORT_CONTRACT = 'cargo test --manifest-path src-tauri/Cargo.toml --features test-support --test export_compat_golden';
const STATIC_COMMANDS = [
  'npm run audit:licenses',
  'bash scripts/supply-chain-audit.sh check',
  DB_CONTRACT,
  EXPORT_CONTRACT,
];
const E2E_COMMAND = 'bash tests/e2e/run-e2e.sh';
const BUILD_COMMAND = 'npm run build';
const VALUE_OPTIONS = new Set(['--tag', '--sha', '--base-tag', '--event', '--json-out']);
const SEMVER_TAG = /^v(0|[1-9]\d*)\.(0|[1-9]\d*)\.(0|[1-9]\d*)$/;
const SHA40 = /^[0-9a-f]{40}$/;

function gateError(code, message, details) {
  const error = new Error(message);
  error.code = code;
  error.details = details;
  return error;
}

function parseArgs(argv) {
  const parsed = {};
  for (let index = 0; index < argv.length; index += 1) {
    const option = argv[index];
    if (!VALUE_OPTIONS.has(option)) throw gateError('INPUT_INVALID', `Unknown argument ${option}`);
    if (Object.hasOwn(parsed, option)) throw gateError('INPUT_INVALID', `Duplicate option ${option}`);
    const value = argv[index += 1];
    if (value === undefined || value.startsWith('--')) {
      throw gateError('INPUT_INVALID', `Missing value for ${option}`);
    }
    parsed[option] = value;
  }
  for (const option of VALUE_OPTIONS) {
    if (!Object.hasOwn(parsed, option)) throw gateError('INPUT_INVALID', `${option} is required`);
  }
  for (const [option, value] of Object.entries(parsed)) {
    if (/[\0\r\n]/.test(value)) throw gateError('INPUT_INVALID', `${option} contains control characters`);
  }
  if (!SEMVER_TAG.test(parsed['--tag'])) throw gateError('INPUT_INVALID', 'Expected --tag vX.Y.Z');
  if (!SEMVER_TAG.test(parsed['--base-tag'])) throw gateError('INPUT_INVALID', 'Expected --base-tag vX.Y.Z');
  if (!SHA40.test(parsed['--sha'])) throw gateError('INPUT_INVALID', 'Expected --sha as 40 lowercase hexadecimal characters');
  if (!['tag', 'dispatch'].includes(parsed['--event'])) {
    throw gateError('INPUT_INVALID', 'Expected --event tag|dispatch');
  }
  if (!isAbsolute(parsed['--json-out'])) throw gateError('INPUT_INVALID', '--json-out must be an absolute path');
  return {
    tag: parsed['--tag'],
    sha: parsed['--sha'],
    baseTag: parsed['--base-tag'],
    event: parsed['--event'],
    jsonOut: parsed['--json-out'],
  };
}

function git(repoRoot, args, options = {}) {
  const result = spawnSync('git', args, {
    cwd: repoRoot,
    encoding: options.encoding ?? 'utf8',
    env: { ...process.env, GIT_OPTIONAL_LOCKS: '0' },
    stdio: ['ignore', 'pipe', 'pipe'],
  });
  if (result.error) throw gateError('GIT_FAILED', `Unable to run git ${args[0]}`, { code: result.error.code });
  if (result.status !== 0 && !options.allowFailure) {
    throw gateError('GIT_FAILED', `Git command failed: git ${args.join(' ')}`, { status: result.status });
  }
  return result;
}

function gitText(repoRoot, ...args) {
  return git(repoRoot, args).stdout.trim();
}

function resolveTag(repoRoot, tag, role) {
  const result = git(repoRoot, ['rev-parse', '--verify', `refs/tags/${tag}^{commit}`], { allowFailure: true });
  if (result.status !== 0) {
    throw gateError('TAG_NOT_FOUND', `${role} tag ${tag} does not exist`);
  }
  return result.stdout.trim();
}

function requireAncestor(repoRoot, ancestor, descendant, code, message) {
  const result = git(repoRoot, ['merge-base', '--is-ancestor', ancestor, descendant], { allowFailure: true });
  if (result.status === 1) throw gateError(code, message);
  if (result.status !== 0) throw gateError('GIT_FAILED', 'Git ancestry check failed', { status: result.status });
}

function objectFile(repoRoot, sha, path) {
  if (typeof path !== 'string' || path.length === 0 || path.startsWith('/')
    || path.split('/').includes('..') || path.includes('\0')) {
    throw gateError('CONFIG_INVALID', `Unsafe release path ${JSON.stringify(path)}`);
  }
  const result = git(repoRoot, ['show', `${sha}:${path}`], { allowFailure: true });
  if (result.status !== 0) throw gateError('CONFIG_INVALID', `Unable to read ${path} at release SHA`);
  return result.stdout;
}

function loadConfigAt(repoRoot, sha) {
  let config;
  try {
    config = JSON.parse(objectFile(repoRoot, sha, 'release.config.json'));
  } catch (cause) {
    if (cause.code) throw cause;
    throw gateError('CONFIG_INVALID', 'Unable to parse release.config.json', { cause: cause.message });
  }
  if (config.schemaVersion !== 1 || !Array.isArray(config.versionFiles)
    || config.versionFiles.length === 0 || !Array.isArray(config.extraGate)
    || !config.changelog?.path || !config.compatibility?.path
    || !Array.isArray(config.e2e?.exact) || !Array.isArray(config.e2e?.prefixes)) {
    throw gateError('CONFIG_INVALID', 'Release configuration is incomplete');
  }
  for (const command of [DB_CONTRACT, EXPORT_CONTRACT]) {
    if (!config.extraGate.includes(command)) {
      throw gateError('STABLE_CONTRACT_MISSING', `Missing stable contract command: ${command}`);
    }
  }
  return config;
}

function decodePointer(pointer) {
  if (pointer === '') return [];
  if (typeof pointer !== 'string' || !pointer.startsWith('/')) {
    throw gateError('CONFIG_INVALID', `Invalid JSON pointer ${JSON.stringify(pointer)}`);
  }
  return pointer.slice(1).split('/').map((part) => part.replace(/~1/g, '/').replace(/~0/g, '~'));
}

function jsonVersions(contents, entry) {
  let document;
  try { document = JSON.parse(contents); } catch (cause) {
    throw gateError('CONFIG_INVALID', `Unable to parse ${entry.path}`, { cause: cause.message });
  }
  if (!Array.isArray(entry.pointers) || entry.pointers.length === 0) {
    throw gateError('CONFIG_INVALID', `Missing JSON pointers for ${entry.id}`);
  }
  return entry.pointers.map((pointer) => {
    let value = document;
    for (const part of decodePointer(pointer)) {
      if (value === null || typeof value !== 'object' || !Object.hasOwn(value, part)) {
        throw gateError('CONFIG_INVALID', `Missing JSON pointer ${pointer} in ${entry.path}`);
      }
      value = value[part];
    }
    if (typeof value !== 'string') throw gateError('CONFIG_INVALID', `Version at ${pointer} is not a string`);
    return value;
  });
}

function tomlPackageVersion(contents, entry, lockfile) {
  if (typeof entry.package !== 'string' || entry.package.length === 0) {
    throw gateError('CONFIG_INVALID', `Missing package name for ${entry.id}`);
  }
  const sections = lockfile
    ? contents.split(/(?=^\[\[package\]\]\s*$)/m)
    : contents.split(/(?=^\s*\[)/m);
  const header = lockfile ? /^\s*\[\[package\]\]\s*$/m : /^\s*\[package\]\s*$/m;
  const section = sections.find((candidate) => header.test(candidate)
    && /^\s*name\s*=\s*"([^"]+)"\s*$/m.exec(candidate)?.[1] === entry.package);
  const version = section && /^\s*version\s*=\s*"([^"]+)"\s*$/m.exec(section)?.[1];
  if (!version) throw gateError('CONFIG_INVALID', `Missing package ${entry.package} in ${entry.path}`);
  return [version];
}

function versionSnapshotAt(repoRoot, sha, config) {
  const snapshot = {};
  for (const entry of config.versionFiles) {
    if (!entry || typeof entry.id !== 'string' || !entry.id || Object.hasOwn(snapshot, entry.id)) {
      throw gateError('CONFIG_INVALID', 'Version file IDs must be present and unique');
    }
    const contents = objectFile(repoRoot, sha, entry.path);
    if (entry.kind === 'json') snapshot[entry.id] = jsonVersions(contents, entry);
    else if (entry.kind === 'toml-package-version') {
      snapshot[entry.id] = tomlPackageVersion(contents, entry, false);
    } else if (entry.kind === 'cargo-lock-package-version') {
      snapshot[entry.id] = tomlPackageVersion(contents, entry, true);
    } else throw gateError('CONFIG_INVALID', `Unsupported version file kind ${entry.kind}`);
  }
  return snapshot;
}

function assertVersions(snapshot, expectedVersion) {
  const values = Object.values(snapshot).flat();
  if (values.length === 0 || new Set(values).size !== 1 || values[0] !== expectedVersion) {
    throw gateError('VERSION_MISMATCH', 'Release metadata versions do not match the tag', snapshot);
  }
}

function assertReleaseStamps(repoRoot, sha, config, version) {
  const escaped = version.replaceAll('.', '\\.');
  const changelog = objectFile(repoRoot, sha, config.changelog.path);
  if (!new RegExp(`^## \\[${escaped}\\] - \\d{4}-\\d{2}-\\d{2}(?:\\r)?$`, 'm').test(changelog)) {
    throw gateError('CHANGELOG_INVALID', `Missing changelog stamp for ${version}`);
  }
  const compatibility = objectFile(repoRoot, sha, config.compatibility.path);
  if (!new RegExp(`^Last updated: ${escaped} \\(\\d{4}-\\d{2}-\\d{2}\\)(?:\\r)?$`, 'm').test(compatibility)) {
    throw gateError('COMPATIBILITY_INVALID', `Missing compatibility stamp for ${version}`);
  }
}

export function assertE2ERecorded(classifiedPaths, evidence) {
  const expected = [...new Set(classifiedPaths)].sort();
  const recorded = Array.isArray(evidence?.matchedPaths) ? [...evidence.matchedPaths] : [];
  if (evidence?.required !== (expected.length > 0)
    || JSON.stringify(recorded) !== JSON.stringify(expected)) {
    throw gateError('E2E_EVIDENCE_INVALID', 'Every classified E2E path must be recorded', {
      expected,
      recorded,
    });
  }
}

function changedPaths(repoRoot, baseTag, sha) {
  const result = git(repoRoot, ['diff', '--name-only', '--diff-filter=ACDMRTUXB', '-z', `${baseTag}..${sha}`], {
    encoding: 'buffer',
  });
  return result.stdout.toString('utf8').split('\0').filter(Boolean);
}

function writeJsonAtomic(path, record) {
  const directory = dirname(path);
  mkdirSync(directory, { recursive: true });
  const temporary = `${path}.tmp-${process.pid}-${randomBytes(12).toString('hex')}`;
  let fd;
  try {
    fd = openSync(temporary, constants.O_WRONLY | constants.O_CREAT | constants.O_EXCL | constants.O_NOFOLLOW, 0o600);
    const bytes = Buffer.from(`${JSON.stringify(record, null, 2)}\n`);
    let offset = 0;
    while (offset < bytes.length) offset += writeSync(fd, bytes, offset, bytes.length - offset);
    fsyncSync(fd);
    closeSync(fd);
    fd = undefined;
    renameSync(temporary, path);
    const directoryFd = openSync(directory, constants.O_RDONLY | constants.O_NOFOLLOW);
    try { fsyncSync(directoryFd); } finally { closeSync(directoryFd); }
  } catch (cause) {
    if (fd !== undefined) {
      try { closeSync(fd); } catch { /* preserve the original failure */ }
    }
    try { unlinkSync(temporary); } catch { /* unique temp may not exist */ }
    throw gateError('OUTPUT_WRITE_FAILED', `Unable to write ${path}`, { cause: cause.message });
  }
}

export function buildGateRecord(repoRoot, input) {
  const tagSha = resolveTag(repoRoot, input.tag, 'Release');
  if (tagSha !== input.sha) {
    throw gateError('TAG_SHA_MISMATCH', `Tag ${input.tag} does not resolve to ${input.sha}`, { tagSha });
  }
  const baseSha = resolveTag(repoRoot, input.baseTag, 'Base');
  if (input.baseTag === input.tag) throw gateError('INPUT_INVALID', 'Base and release tags must differ');
  requireAncestor(repoRoot, baseSha, input.sha, 'BASE_NOT_ANCESTOR', 'Base tag is not an ancestor of the release SHA');
  const originMain = gitText(repoRoot, 'rev-parse', '--verify', 'origin/main^{commit}');
  requireAncestor(repoRoot, input.sha, originMain, 'NOT_ON_ORIGIN_MAIN', 'Release SHA is not reachable from origin/main');

  const version = input.tag.slice(1);
  const config = loadConfigAt(repoRoot, input.sha);
  const versions = versionSnapshotAt(repoRoot, input.sha, config);
  assertVersions(versions, version);
  assertReleaseStamps(repoRoot, input.sha, config, version);
  const paths = changedPaths(repoRoot, input.baseTag, input.sha);
  const matchedPaths = classifyE2EPaths(paths, config.e2e);
  const e2e = { required: matchedPaths.length > 0, matchedPaths };
  assertE2ERecorded(matchedPaths, e2e);
  const commands = [...STATIC_COMMANDS, ...(e2e.required ? [E2E_COMMAND] : []), BUILD_COMMAND];
  return {
    schema: 'cull.release.gate.v1',
    version,
    tag: input.tag,
    sha: input.sha,
    baseTag: input.baseTag,
    mainAncestor: true,
    versions,
    e2e,
    commands,
  };
}

function appendWorkflowOutputs(path, record, jsonOut) {
  if (!path) return;
  const lines = [
    `version=${record.version}`,
    `tag=${record.tag}`,
    `sha=${record.sha}`,
    `base_tag=${record.baseTag}`,
    `e2e_required=${record.e2e.required}`,
    `json_out=${jsonOut}`,
  ];
  appendFileSync(path, `${lines.join('\n')}\n`, { encoding: 'utf8' });
}

function main() {
  const repoRoot = resolve(process.cwd());
  try {
    const input = parseArgs(process.argv.slice(2));
    const record = buildGateRecord(repoRoot, input);
    writeJsonAtomic(input.jsonOut, record);
    appendWorkflowOutputs(process.env.GITHUB_OUTPUT, record, input.jsonOut);
    process.stdout.write(`${JSON.stringify(record)}\n`);
  } catch (cause) {
    const error = {
      ok: false,
      code: cause.code ?? 'INTERNAL_ERROR',
      message: cause.message,
      ...(cause.details === undefined ? {} : { details: cause.details }),
    };
    process.stderr.write(`${JSON.stringify(error)}\n`);
    process.exitCode = cause.code ? 2 : 1;
  }
}

if (process.argv[1] && resolve(process.argv[1]) === resolve(import.meta.filename)) main();
