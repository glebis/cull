import {
  chmodSync,
  closeSync,
  constants,
  fchmodSync,
  fsyncSync,
  lstatSync,
  mkdirSync,
  openSync,
  readFileSync,
  renameSync,
  unlinkSync,
  writeFileSync,
  writeSync,
} from 'node:fs';
import { randomBytes } from 'node:crypto';
import { dirname, join } from 'node:path';

export const RELEASE_STATES = [
  'requested',
  'checked',
  'prepared',
  'tagged',
  'draft-built',
  'artifact-verified',
  'published',
  'homebrew-promoted',
  'post-publish-verified',
];

const BUMPS = new Set(['patch', 'minor', 'major']);

export function parseSemver(value) {
  const match = /^(\d+)\.(\d+)\.(\d+)$/.exec(String(value));
  if (!match) throw new Error(`Expected SemVer x.y.z, got ${JSON.stringify(value)}`);
  return { major: Number(match[1]), minor: Number(match[2]), patch: Number(match[3]) };
}

export function nextVersion(current, bump) {
  if (!BUMPS.has(bump)) throw new Error(`Expected bump patch|minor|major, got ${bump}`);
  const version = parseSemver(current);
  if (bump === 'major') return `${version.major + 1}.0.0`;
  if (bump === 'minor') return `${version.major}.${version.minor + 1}.0`;
  return `${version.major}.${version.minor}.${version.patch + 1}`;
}

export function classifyE2EPaths(paths, rules) {
  return [...new Set(paths)].sort().filter((path) =>
    rules.exact.includes(path) || rules.prefixes.some((prefix) => path.startsWith(prefix))
  );
}

export function createReleaseRecord({ version, bump, source, now }) {
  parseSemver(version);
  if (!BUMPS.has(bump)) throw new Error(`Expected bump patch|minor|major, got ${bump}`);
  if (!/^[0-9a-f]{40}$/.test(source)) throw new Error('Expected a 40-character source SHA');
  return {
    schema: 'cull.release.v1', version, bump, state: 'requested', releaseCommit: source,
    tag: `v${version}`, workflowRunId: null, requestedAt: now, updatedAt: now,
    gates: {}, assets: {}, failure: null,
  };
}

export function transitionReleaseRecord(record, nextState, evidence, now) {
  const current = RELEASE_STATES.indexOf(record.state);
  const next = RELEASE_STATES.indexOf(nextState);
  if (current < 0 || next !== current + 1) {
    throw new Error(`Illegal release transition ${record.state} -> ${nextState}`);
  }
  return { ...record, state: nextState, updatedAt: now, gates: { ...record.gates, ...evidence } };
}

export function recordFailure(record, failure, now) {
  return { ...record, failure: { ...failure, at: now }, updatedAt: now };
}

export function buildResumeAction(state) {
  const actions = {
    requested: ['checked', 'run-readiness-check'], checked: ['prepared', 'prepare-release'],
    prepared: ['tagged', 'push-annotated-tag'], tagged: ['draft-built', 'watch-signed-build'],
    'draft-built': ['artifact-verified', 'verify-workflow-artifact'],
    'artifact-verified': ['published', 'publish-verified-artifacts'],
    published: ['homebrew-promoted', 'promote-homebrew'],
    'homebrew-promoted': ['post-publish-verified', 'verify-public-release'],
    'post-publish-verified': [null, 'complete'],
  };
  const action = actions[state];
  if (!action) throw new Error(`Unknown release state ${state}`);
  return { nextState: action[0], nextAction: action[1] };
}

function releaseError(code, message, details) {
  const error = new Error(message);
  error.code = code;
  error.details = details;
  return error;
}

function editJson(contents, entry, version) {
  JSON.parse(contents);
  const strings = new Map();
  let cursor = 0;
  const whitespace = () => {
    while (/\s/.test(contents[cursor] ?? '')) cursor += 1;
  };
  const stringToken = () => {
    const start = cursor;
    if (contents[cursor] !== '"') throw configurationError(`Expected JSON string in ${entry.path}`);
    cursor += 1;
    while (cursor < contents.length) {
      if (contents[cursor] === '\\') cursor += 2;
      else if (contents[cursor++] === '"') break;
    }
    const raw = contents.slice(start, cursor);
    return { start, end: cursor, value: JSON.parse(raw) };
  };
  const parseValue = (path) => {
    whitespace();
    if (contents[cursor] === '"') {
      const token = stringToken();
      strings.set(`/${path.map((part) => part.replaceAll('~', '~0').replaceAll('/', '~1')).join('/')}`, token);
      return;
    }
    if (contents[cursor] === '{') {
      cursor += 1;
      whitespace();
      while (contents[cursor] !== '}') {
        const key = stringToken().value;
        whitespace();
        if (contents[cursor++] !== ':') throw configurationError(`Invalid JSON object in ${entry.path}`);
        parseValue([...path, key]);
        whitespace();
        if (contents[cursor] === ',') {
          cursor += 1;
          whitespace();
        } else break;
      }
      if (contents[cursor++] !== '}') throw configurationError(`Invalid JSON object in ${entry.path}`);
      return;
    }
    if (contents[cursor] === '[') {
      cursor += 1;
      whitespace();
      let index = 0;
      while (contents[cursor] !== ']') {
        parseValue([...path, String(index++)]);
        whitespace();
        if (contents[cursor] === ',') {
          cursor += 1;
          whitespace();
        } else break;
      }
      if (contents[cursor++] !== ']') throw configurationError(`Invalid JSON array in ${entry.path}`);
      return;
    }
    while (cursor < contents.length && !/[\s,}\]]/.test(contents[cursor])) cursor += 1;
  };
  parseValue([]);
  const replacements = entry.pointers.map((pointer) => {
    const token = strings.get(pointer);
    if (!token) throw configurationError(`Missing JSON pointer ${pointer} in ${entry.path}`);
    return token;
  }).sort((left, right) => right.start - left.start);
  let edited = contents;
  for (const token of replacements) {
    edited = `${edited.slice(0, token.start)}${JSON.stringify(version)}${edited.slice(token.end)}`;
  }
  return edited;
}

function editTomlPackage(contents, entry, version, lockfile = false) {
  const sections = lockfile
    ? contents.split(/(?=^\[\[package\]\]\s*$)/m)
    : contents.split(/(?=^\s*\[)/m);
  const index = sections.findIndex((section) => {
    const expectedHeader = lockfile ? /^\s*\[\[package\]\]\s*$/m : /^\s*\[package\]\s*$/m;
    return expectedHeader.test(section)
      && /^\s*name\s*=\s*"([^"]+)"\s*$/m.exec(section)?.[1] === entry.package;
  });
  if (index < 0) throw configurationError(`Missing package ${entry.package} in ${entry.path}`);
  let replacements = 0;
  sections[index] = sections[index].replace(
    /^([ \t]*version[ \t]*=[ \t]*)"[^"]+"([ \t]*)$/m,
    (_match, prefix, suffix) => {
      replacements += 1;
      return `${prefix}"${version}"${suffix}`;
    },
  );
  if (replacements !== 1) throw configurationError(`Missing package version in ${entry.path}`);
  return sections.join('');
}

export function planVersionEdits(repoRoot, config, version) {
  parseSemver(version);
  return config.versionFiles.map((entry) => {
    const path = join(repoRoot, entry.path);
    const before = readFileSync(path, 'utf8');
    let after;
    if (entry.kind === 'json') after = editJson(before, entry, version);
    else if (entry.kind === 'toml-package-version') {
      after = editTomlPackage(before, entry, version, false);
    } else if (entry.kind === 'cargo-lock-package-version') {
      after = editTomlPackage(before, entry, version, true);
    } else throw configurationError(`Unsupported version file kind ${entry.kind}`);
    return { id: entry.id, path: entry.path, absolutePath: path, before, after };
  });
}

export function applyVersionEdits(edits) {
  for (const edit of edits) writeFileSync(edit.absolutePath, edit.after);
}

export function validateCompatibilityReview(review, currentVersion) {
  if (!review || typeof review !== 'object'
    || !isNonEmptyString(review.version)
    || !BUMPS.has(review.requestedBump)
    || typeof review.stableBreakingChange !== 'boolean'
    || !Array.isArray(review.changedSurfaces)
    || review.changedSurfaces.some((surface) => !isNonEmptyString(surface))
    || review.reviewedBy !== 'Gleb Kalinin') {
    throw releaseError('REVIEW_INVALID', 'A complete compatibility review is required');
  }
  const expected = nextVersion(currentVersion, review.requestedBump);
  if (review.version !== expected) {
    throw releaseError('VERSION_MOVED', `Expected next version ${expected}, got ${review.version}`);
  }
  if (review.stableBreakingChange && review.requestedBump !== 'major') {
    throw releaseError('INCOMPATIBLE_BUMP', 'A stable breaking change requires a major bump');
  }
  return review;
}

function planChangelog(contents, version, notes, date) {
  const marker = '## [Unreleased]';
  const markerIndex = contents.indexOf(marker);
  if (markerIndex < 0) throw releaseError('CHANGELOG_INVALID', 'Missing Unreleased changelog section');
  const nextHeading = contents.indexOf('\n## [', markerIndex + marker.length);
  const prefix = contents.slice(0, markerIndex + marker.length);
  const unreleased = contents.slice(markerIndex + marker.length, nextHeading < 0 ? contents.length : nextHeading).trim();
  const preserved = unreleased === 'No changes yet.' ? '' : unreleased;
  const suffix = nextHeading < 0 ? '\n' : contents.slice(nextHeading);
  return `${prefix}\n\n## [${version}] - ${date}\n\n${notes.trim()}${preserved ? `\n\n${preserved}` : ''}\n${suffix}`;
}

function planCompatibility(contents, version, date) {
  const pattern = /^Last updated: .*$/m;
  if (!pattern.test(contents)) {
    throw releaseError('COMPATIBILITY_INVALID', 'Missing compatibility Last updated stamp');
  }
  return contents.replace(pattern, `Last updated: ${version} (${date})`);
}

export function prepareRelease({ repoRoot, config, request, notes, date, dryRun = false }) {
  if (!isNonEmptyString(notes) || !/^- |^### /m.test(notes)) {
    throw releaseError('NOTES_INVALID', 'Curated non-empty release notes are required');
  }
  const currentVersion = validateVersionAlignment(readVersionSnapshot(repoRoot, config));
  validateCompatibilityReview(request, currentVersion);
  const versionEdits = planVersionEdits(repoRoot, config, request.version);
  const changelogPath = join(repoRoot, config.changelog.path);
  const compatibilityPath = join(repoRoot, config.compatibility.path);
  const changelogBefore = readFileSync(changelogPath, 'utf8');
  const compatibilityBefore = readFileSync(compatibilityPath, 'utf8');
  const edits = [
    ...versionEdits,
    {
      path: config.changelog.path,
      absolutePath: changelogPath,
      before: changelogBefore,
      after: planChangelog(changelogBefore, request.version, notes, date),
    },
    {
      path: config.compatibility.path,
      absolutePath: compatibilityPath,
      before: compatibilityBefore,
      after: planCompatibility(compatibilityBefore, request.version, date),
    },
  ];
  if (!dryRun) applyVersionEdits(edits);
  return { currentVersion, version: request.version, edits };
}

export function releaseStatePath(repoRoot, config, version) {
  parseSemver(version);
  return join(repoRoot, config.stateDir ?? '.release-state', `${version}.json`);
}

export function readReleaseRecord(repoRoot, config, version) {
  const path = releaseStatePath(repoRoot, config, version);
  try {
    const stat = lstatSync(path);
    if (stat.isSymbolicLink() || !stat.isFile()) throw new Error('State path is not a regular file');
    const record = JSON.parse(readFileSync(path, {
      encoding: 'utf8', flag: constants.O_RDONLY | constants.O_NOFOLLOW,
    }));
    return validateReleaseRecord(record, version);
  } catch (cause) {
    if (cause.code === 'STATE_INVALID') throw cause;
    throw releaseError('STATE_INVALID', `Unable to read release state for ${version}`, {
      cause: cause.message,
    });
  }
}

export function validateReleaseRecord(record, expectedVersion = record?.version) {
  const object = (value) => value !== null && typeof value === 'object' && !Array.isArray(value);
  const validTimestamp = (value) => typeof value === 'string' && Number.isFinite(Date.parse(value));
  if (!object(record)
    || record.schema !== 'cull.release.v1'
    || typeof record.version !== 'string'
    || record.version !== expectedVersion
    || !/^\d+\.\d+\.\d+$/.test(record.version)
    || !BUMPS.has(record.bump)
    || !RELEASE_STATES.includes(record.state)
    || !/^[0-9a-f]{40}$/.test(record.releaseCommit)
    || record.tag !== `v${record.version}`
    || !(record.workflowRunId === null
      || (Number.isSafeInteger(record.workflowRunId) && record.workflowRunId > 0))
    || !validTimestamp(record.requestedAt)
    || !validTimestamp(record.updatedAt)
    || !object(record.gates)
    || !object(record.assets)
    || !(record.failure === null || object(record.failure))) {
    throw releaseError('STATE_INVALID', 'Release state record failed schema validation');
  }
  return record;
}

export function writeReleaseRecordAtomic(repoRoot, config, record) {
  validateReleaseRecord(record);
  const path = releaseStatePath(repoRoot, config, record.version);
  const stateDir = dirname(path);
  mkdirSync(stateDir, { recursive: true, mode: 0o700 });
  const directoryStat = lstatSync(stateDir);
  if (directoryStat.isSymbolicLink() || !directoryStat.isDirectory()) {
    throw releaseError('STATE_INVALID', 'Release state directory must be a regular directory');
  }
  chmodSync(stateDir, 0o700);
  const temporary = `${path}.tmp-${process.pid}-${randomBytes(12).toString('hex')}`;
  const fd = openSync(
    temporary,
    constants.O_WRONLY | constants.O_CREAT | constants.O_EXCL | constants.O_NOFOLLOW,
    0o600,
  );
  let writeFailure;
  try {
    fchmodSync(fd, 0o600);
    const bytes = Buffer.from(`${JSON.stringify(record, null, 2)}\n`);
    let offset = 0;
    while (offset < bytes.length) offset += writeSync(fd, bytes, offset, bytes.length - offset);
    if (process.env.CULL_RELEASE_TEST_MODE === '1'
      && process.env.CULL_RELEASE_TEST_FAIL_STATE_WRITE === 'before-fsync') {
      throw new Error('Injected state write failure before fsync');
    }
    fsyncSync(fd);
  } catch (cause) {
    writeFailure = cause;
  } finally {
    closeSync(fd);
  }
  if (writeFailure) {
    try {
      unlinkSync(temporary);
    } catch (cleanupCause) {
      throw releaseError('STATE_INVALID', 'State write failed and its unique temp could not be removed', {
        cause: writeFailure.message,
        cleanupCause: cleanupCause.message,
        temporary,
      });
    }
    throw writeFailure;
  }
  chmodSync(temporary, 0o600);
  renameSync(temporary, path);
  chmodSync(path, 0o600);
  const directoryFd = openSync(stateDir, constants.O_RDONLY | constants.O_NOFOLLOW);
  try {
    fsyncSync(directoryFd);
  } finally {
    closeSync(directoryFd);
  }
  return path;
}

export function deriveReleaseState(evidence) {
  if (!evidence.commit) return 'requested';
  if (!evidence.tag) return 'prepared';
  if (!evidence.workflow) return 'tagged';
  if (!evidence.releaseAsset) return 'draft-built';
  if (!evidence.publishedRelease) return 'artifact-verified';
  if (!evidence.tapCommit) return 'published';
  if (!evidence.postPublishVerified) return 'homebrew-promoted';
  return 'post-publish-verified';
}

function configurationError(message, details) {
  const error = new Error(message);
  error.code = 'CONFIG_INVALID';
  error.details = details;
  return error;
}

function isNonEmptyString(value) {
  return typeof value === 'string' && value.length > 0;
}

function validateVersionFile(entry) {
  if (!entry || typeof entry !== 'object'
    || !isNonEmptyString(entry.id)
    || !isNonEmptyString(entry.path)) {
    throw configurationError('Malformed version file declaration');
  }
  if (entry.kind === 'json') {
    if (!Array.isArray(entry.pointers) || entry.pointers.length === 0
      || entry.pointers.some((pointer) => typeof pointer !== 'string'
        || (pointer !== '' && !pointer.startsWith('/')))) {
      throw configurationError('Malformed version file declaration', { id: entry.id });
    }
    return;
  }
  if ((entry.kind === 'toml-package-version'
      || entry.kind === 'cargo-lock-package-version')
    && isNonEmptyString(entry.package)) {
    return;
  }
  throw configurationError('Malformed version file declaration', { id: entry.id });
}

export function loadReleaseConfig(repoRoot) {
  const path = join(repoRoot, 'release.config.json');
  let config;
  try {
    config = JSON.parse(readFileSync(path, 'utf8'));
  } catch (cause) {
    throw configurationError(`Unable to load ${path}`, { cause: cause.message });
  }
  if (config.schemaVersion !== 1 || !Array.isArray(config.versionFiles)
    || config.versionFiles.length === 0) {
    throw configurationError('Unsupported release configuration', {
      schemaVersion: config.schemaVersion,
    });
  }
  if (!Number.isFinite(config.minimumFreeDiskGiB) || config.minimumFreeDiskGiB <= 0) {
    throw configurationError('minimumFreeDiskGiB must be a positive finite number');
  }
  config.versionFiles.forEach(validateVersionFile);
  const ids = config.versionFiles.map((entry) => entry.id);
  if (new Set(ids).size !== ids.length) {
    throw configurationError('Version file IDs must be unique', { ids });
  }
  return config;
}

function decodeJsonPointer(pointer) {
  if (pointer === '') return [];
  if (!pointer.startsWith('/')) throw configurationError(`Invalid JSON pointer ${pointer}`);
  return pointer.slice(1).split('/').map((part) => part.replace(/~1/g, '/').replace(/~0/g, '~'));
}

function readJsonPointers(contents, entry) {
  const document = JSON.parse(contents);
  return entry.pointers.map((pointer) => {
    let value = document;
    for (const part of decodeJsonPointer(pointer)) {
      if (value === null || typeof value !== 'object' || !Object.hasOwn(value, part)) {
        throw configurationError(`Missing JSON pointer ${pointer} in ${entry.path}`);
      }
      value = value[part];
    }
    if (typeof value !== 'string') {
      throw configurationError(`Expected a string at ${pointer} in ${entry.path}`);
    }
    return value;
  });
}

function readTomlPackageVersion(contents, entry) {
  const sections = contents.split(/(?=^\s*\[)/m);
  const section = sections.find((candidate) => /^\s*\[package\]\s*$/m.test(candidate));
  if (!section) throw configurationError(`Missing [package] in ${entry.path}`);
  const name = /^\s*name\s*=\s*"([^"]+)"\s*$/m.exec(section)?.[1];
  const version = /^\s*version\s*=\s*"([^"]+)"\s*$/m.exec(section)?.[1];
  if (name !== entry.package) {
    throw configurationError(`Expected package ${entry.package} in ${entry.path}`);
  }
  if (!version) throw configurationError(`Missing package version in ${entry.path}`);
  return [version];
}

function readCargoLockPackageVersion(contents, entry) {
  const blocks = contents.split(/(?=^\[\[package\]\]\s*$)/m);
  const block = blocks.find((candidate) => {
    if (!/^\[\[package\]\]\s*$/m.test(candidate)) return false;
    return /^\s*name\s*=\s*"([^"]+)"\s*$/m.exec(candidate)?.[1] === entry.package;
  });
  const version = block && /^\s*version\s*=\s*"([^"]+)"\s*$/m.exec(block)?.[1];
  if (!version) throw configurationError(`Missing package ${entry.package} in ${entry.path}`);
  return [version];
}

function readDeclaredVersions(repoRoot, entry) {
  let contents;
  try {
    contents = readFileSync(join(repoRoot, entry.path), 'utf8');
  } catch (cause) {
    throw configurationError(`Unable to read ${entry.path}`, { cause: cause.message });
  }
  try {
    if (entry.kind === 'json') return readJsonPointers(contents, entry);
    if (entry.kind === 'toml-package-version') return readTomlPackageVersion(contents, entry);
    if (entry.kind === 'cargo-lock-package-version') return readCargoLockPackageVersion(contents, entry);
  } catch (error) {
    if (error.code) throw error;
    throw configurationError(`Unable to parse ${entry.path}`, { cause: error.message });
  }
  throw configurationError(`Unsupported version file kind ${entry.kind}`);
}

export function readVersionSnapshot(repoRoot, config) {
  return Object.fromEntries(config.versionFiles.map((entry) => [
    entry.id,
    readDeclaredVersions(repoRoot, entry),
  ]));
}

export function validateVersionAlignment(snapshot) {
  const values = Object.values(snapshot).flat();
  if (new Set(values).size !== 1) {
    const error = new Error('Release metadata versions disagree');
    error.code = 'VERSION_MISMATCH';
    error.details = snapshot;
    throw error;
  }
  return values[0];
}

export function buildReadinessReport(input) {
  const blockers = [];
  if (!input.clean) blockers.push({ code: 'WORKTREE_DIRTY', message: 'Git worktree is not clean' });
  if (!input.syncedWithOriginMain) {
    blockers.push({ code: 'NOT_SYNCED_WITH_ORIGIN_MAIN', message: 'HEAD is not origin/main' });
  }
  if (input.availableGiB < input.minimumFreeDiskGiB) {
    blockers.push({ code: 'INSUFFICIENT_DISK', message: 'Insufficient free disk space' });
  }
  if (!input.rustVersion) {
    blockers.push({ code: 'RUST_UNAVAILABLE', message: 'Rust toolchain is unavailable' });
  }
  return {
    currentVersion: input.currentVersion,
    targetVersion: input.targetVersion,
    source: input.source,
    branch: input.branch,
    clean: input.clean,
    syncedWithOriginMain: input.syncedWithOriginMain,
    disk: { minimumGiB: input.minimumFreeDiskGiB, availableGiB: input.availableGiB },
    toolchains: { node: input.nodeVersion, rust: input.rustVersion },
    blockers,
  };
}
