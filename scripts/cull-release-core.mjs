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
