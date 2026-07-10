import { readFileSync } from 'node:fs';
import { join } from 'node:path';

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
