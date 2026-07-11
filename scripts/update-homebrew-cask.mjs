#!/usr/bin/env node
import {
  chmodSync,
  closeSync,
  constants,
  fsyncSync,
  lstatSync,
  openSync,
  readFileSync,
  renameSync,
  unlinkSync,
  writeFileSync,
} from 'node:fs';
import { randomBytes } from 'node:crypto';

const args = process.argv.slice(2);

function failure(code, message) {
  const error = new Error(message);
  error.code = code;
  return error;
}

function parseArgs() {
  const parsed = {};
  const values = new Set(['--cask', '--version', '--sha256']);
  for (let index = 0; index < args.length; index += 1) {
    const token = args[index];
    if (token === '--json') parsed.json = true;
    else if (values.has(token)) {
      if (parsed[token]) throw failure('INPUT_INVALID', `Duplicate option ${token}`);
      const value = args[index += 1];
      if (!value || value.startsWith('--')) throw failure('INPUT_INVALID', `Missing value for ${token}`);
      parsed[token] = value;
    } else throw failure('INPUT_INVALID', `Unknown argument ${token}`);
  }
  if (!parsed.json) throw failure('INPUT_INVALID', '--json is required');
  for (const option of values) {
    if (!parsed[option]) throw failure('INPUT_INVALID', `${option} is required`);
  }
  if (!/^(0|[1-9]\d*)\.(0|[1-9]\d*)\.(0|[1-9]\d*)$/.test(parsed['--version'])) {
    throw failure('INPUT_INVALID', 'Version must be exact SemVer');
  }
  if (!/^[0-9a-f]{64}$/.test(parsed['--sha256'])) {
    throw failure('INPUT_INVALID', 'SHA-256 must be 64 lowercase hex characters');
  }
  return parsed;
}

function compareSemver(left, right) {
  const a = left.split('.').map(Number);
  const b = right.split('.').map(Number);
  for (let index = 0; index < 3; index += 1) {
    if (a[index] !== b[index]) return a[index] - b[index];
  }
  return 0;
}

function updateCask(path, targetVersion, targetSha) {
  const stat = lstatSync(path);
  if (!stat.isFile() || stat.isSymbolicLink() || stat.nlink !== 1) {
    throw failure('CASK_INVALID', 'Cask must be a singly linked regular file');
  }
  const source = readFileSync(path, 'utf8');
  const lines = source.split('\n');
  const versionLines = lines.map((line, index) => ({ line, index }))
    .filter(({ line }) => /^[ \t]*version\b/.test(line));
  const shaLines = lines.map((line, index) => ({ line, index }))
    .filter(({ line }) => /^[ \t]*sha256\b/.test(line));
  if (shaLines.some(({ line }) => /^[ \t]*sha256[ \t]+:no_check(?:[ \t]+#.*)?[ \t]*$/.test(line))) {
    throw failure('CASK_NO_CHECK', 'sha256 :no_check is forbidden');
  }
  if (versionLines.length !== 1 || shaLines.length !== 1) {
    throw failure('CASK_INVALID', 'Cask must contain one active version and one active sha256 directive');
  }
  const version = /^([ \t]*)version "((?:0|[1-9]\d*)\.(?:0|[1-9]\d*)\.(?:0|[1-9]\d*))"[ \t]*$/.exec(versionLines[0].line);
  const sha = /^([ \t]*)sha256 "([0-9a-f]{64})"[ \t]*$/.exec(shaLines[0].line);
  if (!version || !sha) throw failure('CASK_INVALID', 'Cask directives must use canonical quoted syntax');
  const comparison = compareSemver(targetVersion, version[2]);
  if (comparison < 0) throw failure('CASK_DOWNGRADE', `Refusing downgrade ${version[2]} -> ${targetVersion}`);
  if (comparison === 0 && sha[2] !== targetSha) {
    throw failure('CASK_IMMUTABLE_SHA_MISMATCH', 'Equal cask version already has a different SHA-256');
  }
  if (comparison === 0) return { changed: false, previousVersion: version[2] };

  lines[versionLines[0].index] = `${version[1]}version "${targetVersion}"`;
  lines[shaLines[0].index] = `${sha[1]}sha256 "${targetSha}"`;
  const updated = lines.join('\n');
  const temporary = `${path}.tmp-${process.pid}-${randomBytes(8).toString('hex')}`;
  let fd;
  try {
    fd = openSync(temporary, constants.O_WRONLY | constants.O_CREAT | constants.O_EXCL | constants.O_NOFOLLOW, stat.mode & 0o777);
    writeFileSync(fd, updated, { encoding: 'utf8' });
    fsyncSync(fd);
    closeSync(fd);
    fd = undefined;
    chmodSync(temporary, stat.mode & 0o777);
    renameSync(temporary, path);
  } catch (cause) {
    if (fd !== undefined) closeSync(fd);
    try { unlinkSync(temporary); } catch { /* unique temporary may already be renamed */ }
    throw cause;
  }
  return { changed: true, previousVersion: version[2] };
}

try {
  const parsed = parseArgs();
  const result = updateCask(parsed['--cask'], parsed['--version'], parsed['--sha256']);
  process.stdout.write(`${JSON.stringify({
    schema: 'cull.release.cask-edit.v1', event: 'result', ok: true, result,
  })}\n`);
} catch (error) {
  process.stdout.write(`${JSON.stringify({
    schema: 'cull.release.cask-edit.v1', event: 'error', ok: false,
    code: error.code ?? 'CASK_INVALID', message: error.message,
  })}\n`);
  process.exitCode = 2;
}
