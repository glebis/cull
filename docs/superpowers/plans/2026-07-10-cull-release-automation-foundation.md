# Cull Release Automation Foundation Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Build repository-enforced, resumable Cull release automation from explicit SemVer preparation through verified signed publication and SHA-pinned Homebrew promotion.

**Architecture:** A Node 20, built-ins-only release CLI owns pure version, state, readiness, and resume contracts inside Cull. GitHub Actions independently enforces the same release policy, builds a signed artifact without publishing, verifies the exact artifact, then publishes and promotes Homebrew. Local state is a cache; Git tags, workflow runs, release assets, provenance, and the tap commit remain authoritative.

**Tech Stack:** Node.js 20 ESM, Vitest, Bash, Rust integration tests, GitHub Actions, `gh`, Tauri 2, macOS signing/notarization tools, Homebrew.

## Global Constraints

- The release kind is always an explicit `patch`, `minor`, or `major`.
- Automatic publication is disabled until stable export contracts and retained database-schema fixtures pass.
- Never rewrite or automatically delete a pushed tag or published release.
- Never use `git reset --hard`, force-push, or discard unrelated work.
- Never delete, reset, migrate manually, or test against the user's real `cull.db`.
- JSON commands write exactly one envelope to stdout; subprocess logs go to stderr.
- Use Node built-ins only for the release CLI; add no runtime dependency.
- PR/gate/verifier jobs receive no signing or Homebrew secrets.
- Signed-build jobs receive Apple and Tauri signing secrets only.
- Publish receives only `GITHUB_TOKEN`; Homebrew promotion receives only `HOMEBREW_TAP_TOKEN`.
- Apple Silicon macOS is the only release target in this plan.
- A local release record is a resumability cache, never the source of truth.

---

## File Structure

### New files

- `scripts/cull-release-core.mjs` — pure SemVer, configuration, version snapshot, E2E classification, state transition, and resume logic.
- `scripts/cull-release.mjs` — non-interactive CLI with stable JSON envelopes and injected command execution.
- `scripts/cull-release-core.test.ts` — pure unit tests.
- `scripts/cull-release-cli.test.ts` — temporary Git repository and mutation-contract tests.
- `scripts/release-gate.mjs` — CI entry point that binds a tag and SHA to release policy and emits `release-gate.json`.
- `scripts/release-gate.test.ts` — CI-gate fixtures and path-classifier tests.
- `scripts/verify-release-artifacts.sh` — exact-artifact trust, version, architecture, updater, and provenance verification.
- `scripts/verify-release-artifacts.test.sh` — fake-tool failure tests that cannot touch real releases.
- `.github/workflows/release-canary.yml` — signed, non-publishing workflow.
- `src-tauri/tests/export_compat_golden.rs` — frozen stable-export reader contract.
- `src-tauri/tests/fixtures/static-publish/v1/index.html` — frozen package entry point.
- `src-tauri/tests/fixtures/static-publish/v1/data/canvas.json` — frozen v1 manifest with unknown-field coverage.

### Modified files

- `package.json` — release CLI and focused test scripts.
- `.gitignore` — local `.release-state/` cache.
- `release.config.json` — version pointers, machine-readable E2E policy, gates, artifacts, and distribution contract.
- `scripts/preflight.sh` — release tier calls named release-policy gates; no fake E2E skip flag.
- `scripts/check-ci.sh` — validate root and `site/` independently without duplicate local installs.
- `scripts/clean-machine-dmg-gate.sh` — delegate artifact checks to the exact-artifact verifier and stop mutating `/Applications` in CI mode.
- `.github/workflows/ci.yml` — concurrency, Rust cache, site checks, and npm security coverage.
- `.github/workflows/release.yml` — gate → signed build → verify → publish.
- `.github/workflows/update-tap.yml` — public artifact/provenance verification and literal SHA update.
- `.github/dependabot.yml` — `site/` npm updates.
- `src-tauri/src/lib.rs` — test-support export for the stable static-package reader.
- `src-tauri/src/commands/static_publishing.rs` — make package validation accessible only through `test-support`.
- `src-tauri/tests/compat_golden.rs` — enumerate all retained DB fixtures.
- `docs/RELEASING.md` — repository CLI, canary, automatic publish, resume, and recovery.
- `docs/e2e-testing-policy.md` — machine matcher is authoritative for release diffs.
- `docs/toolchain.md`, `README.md`, `docs/cross-platform-distribution.md` — correct Rust, signing, and architecture drift.

## Interfaces Shared Across Tasks

### CLI

```bash
npm run release:cull -- check --bump patch --json
SOURCE_SHA=0123456789abcdef0123456789abcdef01234567
NOTES_FILE=/tmp/cull-release-notes.md
COMPAT_FILE=/tmp/cull-compat-review.json
npm run release:cull -- prepare --bump patch --expected-version 0.2.6 --expected-source "$SOURCE_SHA" --notes-file "$NOTES_FILE" --compat-review-file "$COMPAT_FILE" --dry-run --json
npm run release:cull -- state show --version 0.2.6 --json
npm run release:cull -- state transition --version 0.2.6 --to checked --evidence-file /tmp/release-check-evidence.json --json
npm run release:cull -- state fail --version 0.2.6 --code ARTIFACT_INVALID --evidence-file /tmp/artifact-failure-evidence.json --json
npm run release:cull -- resume --version 0.2.6 --json
```

### Success envelope

```json
{
  "schema": "cull.release.command.v1",
  "event": "result",
  "ok": true,
  "command": "check",
  "result": {}
}
```

### Error envelope

```json
{
  "schema": "cull.release.command.v1",
  "event": "error",
  "ok": false,
  "command": "prepare",
  "code": "VERSION_MISMATCH",
  "message": "Release metadata does not match the expected version.",
  "details": {}
}
```

### Release states

```js
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
```

---

### Task 1: Pure Release Model and State Machine

**Files:**
- Create: `scripts/cull-release-core.mjs`
- Create: `scripts/cull-release-core.test.ts`
- Modify: `package.json`
- Modify: `.gitignore`

**Interfaces:**
- Produces: `parseSemver(value)`, `nextVersion(current, bump)`, `classifyE2EPaths(paths, rules)`, `createReleaseRecord(input)`, `transitionReleaseRecord(record, nextState, evidence, now)`, `recordFailure(record, failure, now)`, `buildResumeAction(state)`.
- Consumes: no earlier task.

- [ ] **Step 1: Add the focused test command and ignored state directory**

Add to `package.json` scripts:

```json
"test:release": "vitest run scripts/cull-release-core.test.ts scripts/cull-release-cli.test.ts scripts/release-gate.test.ts"
```

Add to `.gitignore`:

```gitignore
# Local release resumability cache; authoritative evidence lives in Git/GitHub.
.release-state/
```

- [ ] **Step 2: Write failing SemVer and transition tests**

Create `scripts/cull-release-core.test.ts`:

```ts
import { describe, expect, it } from 'vitest';
import {
  RELEASE_STATES,
  buildResumeAction,
  classifyE2EPaths,
  createReleaseRecord,
  nextVersion,
  parseSemver,
  transitionReleaseRecord,
} from './cull-release-core.mjs';

describe('Cull release core', () => {
  it.each([
    ['0.2.5', 'patch', '0.2.6'],
    ['0.2.5', 'minor', '0.3.0'],
    ['0.2.5', 'major', '1.0.0'],
  ])('bumps %s as %s', (current, bump, expected) => {
    expect(nextVersion(current, bump)).toBe(expected);
  });

  it('rejects non-explicit SemVer', () => {
    expect(() => parseSemver('v0.2')).toThrow('Expected SemVer x.y.z');
  });

  it('classifies release E2E paths deterministically', () => {
    const rules = { exact: ['src/routes/+page.svelte'], prefixes: ['src/lib/components/'] };
    expect(classifyE2EPaths(['README.md'], rules)).toEqual([]);
    expect(classifyE2EPaths(['src/lib/components/Grid.svelte'], rules))
      .toEqual(['src/lib/components/Grid.svelte']);
  });

  it('allows only the next monotonic state', () => {
    const record = createReleaseRecord({
      version: '0.2.6', bump: 'patch', source: 'a'.repeat(40), now: '2026-07-10T12:00:00Z',
    });
    const checked = transitionReleaseRecord(
      record, 'checked', { check: 'release-gate.json' }, '2026-07-10T12:01:00Z',
    );
    expect(checked.state).toBe('checked');
    expect(() => transitionReleaseRecord(checked, 'published', {}, '2026-07-10T12:02:00Z'))
      .toThrow('Illegal release transition');
  });

  it('resumes without repeating an expensive verified build', () => {
    expect(buildResumeAction('artifact-verified')).toEqual({
      nextState: 'published', nextAction: 'publish-verified-artifacts',
    });
  });

  it('keeps the state list stable', () => {
    expect(RELEASE_STATES).toHaveLength(9);
  });
});
```

- [ ] **Step 3: Run the tests and verify failure**

Run: `npx vitest run scripts/cull-release-core.test.ts`

Expected: FAIL because `scripts/cull-release-core.mjs` does not exist.

- [ ] **Step 4: Implement the minimal pure model**

Create `scripts/cull-release-core.mjs` with the exported state list above and these contracts:

```js
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
```

- [ ] **Step 5: Run focused tests**

Run: `npx vitest run scripts/cull-release-core.test.ts`

Expected: PASS.

- [ ] **Step 6: Commit**

```bash
git add package.json .gitignore scripts/cull-release-core.mjs scripts/cull-release-core.test.ts
git commit -m "feat(release): add deterministic release state model"
```

---

### Task 2: Version Metadata and Readiness CLI

**Files:**
- Modify: `scripts/cull-release-core.mjs`
- Create: `scripts/cull-release.mjs`
- Create: `scripts/cull-release-cli.test.ts`
- Modify: `release.config.json`
- Modify: `package.json`

**Interfaces:**
- Consumes: Task 1 pure model.
- Produces: `loadReleaseConfig(repoRoot)`, `readVersionSnapshot(repoRoot, config)`, `validateVersionAlignment(snapshot)`, `buildReadinessReport(input)` and `release:cull check`.

- [ ] **Step 1: Extend machine-readable configuration**

Add these top-level keys to `release.config.json` while retaining existing compatibility surfaces:

```json
"schemaVersion": 1,
"stateDir": ".release-state",
"minimumFreeDiskGiB": 15,
"e2e": {
  "exact": ["src/routes/+page.svelte", "src/lib/keys.ts", "src/lib/view-tabs.ts", "src/lib/api.ts", "src/lib/tauri-mock.ts", "vite.config.js"],
  "prefixes": ["src/lib/components/", "tests/e2e/"]
},
"artifacts": {
  "target": "aarch64-apple-darwin",
  "required": ["Cull_{version}_aarch64.dmg", "Cull_aarch64.app.tar.gz", "Cull_aarch64.app.tar.gz.sig", "latest.json"]
},
"homebrew": { "repo": "glebis/homebrew-tap", "cask": "Casks/cull.rb" }
```

Change `versionFiles` so it declares named values instead of ambiguous generic rewrites:

```json
[
  { "id": "package", "path": "package.json", "kind": "json", "pointers": ["/version"] },
  { "id": "package-lock", "path": "package-lock.json", "kind": "json", "pointers": ["/version", "/packages//version"] },
  { "id": "tauri", "path": "src-tauri/tauri.conf.json", "kind": "json", "pointers": ["/version"] },
  { "id": "cargo", "path": "src-tauri/Cargo.toml", "kind": "toml-package-version", "package": "cull" },
  { "id": "cargo-lock", "path": "src-tauri/Cargo.lock", "kind": "cargo-lock-package-version", "package": "cull" }
]
```

- [ ] **Step 2: Write failing temporary-repository CLI tests**

Add tests that create a temporary Git repository with the five version locations, then execute:

```ts
const result = spawnSync(process.execPath, [cli, 'check', '--bump', 'patch', '--json'], {
  cwd: fixture, encoding: 'utf8', env: { ...process.env, CULL_RELEASE_TEST_MODE: '1' },
});
expect(result.status).toBe(0);
expect(JSON.parse(result.stdout)).toMatchObject({
  schema: 'cull.release.command.v1', event: 'result', ok: true, command: 'check',
});
expect(result.stderr).not.toContain('TAURI_SIGNING_PRIVATE_KEY');
expect(readFileSync(join(fixture, 'package.json'), 'utf8')).toBe(before);
```

Add a mismatched `package-lock.json` case expecting exit code `2`, code
`VERSION_MISMATCH`, and exactly one JSON value on stdout.

- [ ] **Step 3: Run tests and verify failure**

Run: `npx vitest run scripts/cull-release-cli.test.ts`

Expected: FAIL because the CLI does not exist.

- [ ] **Step 4: Implement exact pointer and package readers**

Implement JSON pointer decoding including the empty key in `/packages//version`,
section-bounded Cargo TOML matching, and package-bounded Cargo lock matching.
Export:

```js
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
```

The Cargo lock reader must isolate the block beginning with `[[package]]`, followed
by `name = "cull"`, and rewrite only that block.

- [ ] **Step 5: Implement a thin JSON CLI**

`scripts/cull-release.mjs` must parse argument arrays without a shell, call core
functions, print one envelope, and map input/config failures to exit `2`, blocked
gates to `3`, external failures to `4`, and inconsistent recovery to `5`.

For `check`, return:

```js
{
  currentVersion,
  targetVersion: nextVersion(currentVersion, args.bump),
  source: git.revParse('HEAD'),
  branch: git.branch(),
  clean: git.isClean(),
  syncedWithOriginMain: git.revParse('HEAD') === git.revParse('origin/main'),
  disk: { minimumGiB: config.minimumFreeDiskGiB, availableGiB },
  toolchains: { node: nodeVersion, rust: rustVersion },
  blockers,
}
```

`check` exits `3` when `blockers` is non-empty and performs zero writes.

- [ ] **Step 6: Add the package script and run tests**

Add:

```json
"release:cull": "node scripts/cull-release.mjs"
```

Run: `npm run test:release`

Expected: PASS.

- [ ] **Step 7: Commit**

```bash
git add package.json release.config.json scripts/cull-release-core.mjs scripts/cull-release.mjs scripts/cull-release-cli.test.ts
git commit -m "feat(release): add metadata and readiness CLI"
```

---

### Task 3: Preparation, Atomic State, and Resume Contracts

**Files:**
- Modify: `scripts/cull-release-core.mjs`
- Modify: `scripts/cull-release.mjs`
- Modify: `scripts/cull-release-cli.test.ts`
- Modify: `docs/RELEASING.md`

**Interfaces:**
- Consumes: Task 2 metadata readers and Task 1 state model.
- Produces: `planVersionEdits`, `applyVersionEdits`, `validateCompatibilityReview`, `prepareRelease`, atomic `state` commands, and read-only `resume`.

- [ ] **Step 1: Write failing dry-run, race, and resume tests**

Cover these exact contracts:

```ts
expect(run('prepare', ['--dry-run', '--expected-source', source]).diff).toBe('');
expect(run('prepare', ['--expected-source', 'b'.repeat(40)]).error.code)
  .toBe('SOURCE_MOVED');
expect(run('resume', ['--version', '0.2.6']).result.nextAction)
  .toBe('publish-verified-artifacts');
```

Assert real prepare changes only the five version locations, `CHANGELOG.md`,
`docs/COMPATIBILITY.md`, and `.release-state/0.2.6.json`. It must not create a tag,
push, or touch files outside the fixture.

- [ ] **Step 2: Run tests and verify failure**

Run: `npx vitest run scripts/cull-release-cli.test.ts -t "prepare|resume|state"`

Expected: FAIL with unsupported commands.

- [ ] **Step 3: Implement atomic state storage**

Write JSON to `${path}.tmp`, `fsync`, rename to `.release-state/${version}.json`, and
set mode `0600`. `state show` and `resume` rederive evidence through injected probes
before returning. Stale local state may move backward in the derived result but the
file is not silently rewritten by read-only commands.

- [ ] **Step 4: Implement guarded preparation**

`prepare` must require:

```json
{
  "version": "0.2.6",
  "requestedBump": "patch",
  "stableBreakingChange": false,
  "changedSurfaces": [],
  "reviewedBy": "Gleb Kalinin"
}
```

It must validate `expectedSource === HEAD`, `expectedVersion === nextVersion`, a
clean dedicated release worktree, curated non-empty notes, and a compatible bump.
It applies exact version edits, inserts notes below `Unreleased`, stamps
`COMPATIBILITY.md`, runs configured gates through argument arrays, and creates one
`chore(release): vX.Y.Z` commit. It never tags or pushes.

- [ ] **Step 5: Make resume evidence-driven**

Implement probes for commit, tag, workflow, release asset, published release, and
tap commit. Return only `nextState`, `nextAction`, and evidence. Never execute the
returned action from `resume`.

- [ ] **Step 6: Run focused and full release tests**

Run: `npm run test:release`

Expected: PASS, including zero-write checks for `check`, `resume`, and dry-run.

- [ ] **Step 7: Commit**

```bash
git add scripts/cull-release-core.mjs scripts/cull-release.mjs scripts/cull-release-cli.test.ts docs/RELEASING.md
git commit -m "feat(release): prepare and resume releases safely"
```

---

### Task 4: Make Stable Compatibility Promises Mechanically True

**Files:**
- Modify: `src-tauri/src/commands/static_publishing.rs`
- Modify: `src-tauri/src/lib.rs`
- Create: `src-tauri/tests/export_compat_golden.rs`
- Create: `src-tauri/tests/fixtures/static-publish/v1/index.html`
- Create: `src-tauri/tests/fixtures/static-publish/v1/data/canvas.json`
- Modify: `src-tauri/tests/compat_golden.rs`
- Add: retained `src-tauri/tests/fixtures/db/v22.db`, `v23.db`, and `v24.db` when their historical schema commits can be reproduced
- Modify: `docs/CONTRACTS.md`
- Modify: `release.config.json`

**Interfaces:**
- Consumes: current static-package validator and database `test-support` export.
- Produces: `test_support::validate_cull_static_package_for_test(path)` and release-blocking golden commands.

- [ ] **Step 1: Write the failing frozen export integration test**

```rust
#[test]
fn frozen_v1_package_is_still_readable() {
    let fixture = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/static-publish/v1");
    cull_lib::test_support::validate_cull_static_package_for_test(&fixture)
        .expect("current Cull must read the frozen stable v1 package");
}
```

The frozen manifest must include `"schema": "cull.static_publishing.v1"` and an
unknown top-level field to prove forward-compatible readers ignore additions.

- [ ] **Step 2: Run and verify failure**

Run: `cargo test --manifest-path src-tauri/Cargo.toml --features test-support --test export_compat_golden`

Expected: FAIL because the test-support function does not exist.

- [ ] **Step 3: Expose only the reader through test support**

Change `validate_cull_static_package` visibility to `pub(crate)` and add:

```rust
#[cfg(feature = "test-support")]
pub fn validate_cull_static_package_for_test(path: &std::path::Path) -> Result<(), String> {
    crate::commands::static_publishing::validate_cull_static_package(path)
}
```

Re-export it from `lib.rs::test_support`. Do not expose it in normal builds.

- [ ] **Step 4: Make the DB golden test enumerate retained fixtures**

Replace the single `v21.db` path with sorted `tests/fixtures/db/v*.db` discovery,
copy each fixture to its own temp directory, open it, migrate it, and verify schema
invariants. Fail if the fixture list is empty or duplicate schema numbers exist.

- [ ] **Step 5: Reconstruct missing retained fixtures safely**

For each missing schema, locate the historical commit with:

```bash
git log -S 'CURRENT_SCHEMA_VERSION: i32 = 22' -- src-tauri/src/db_core/db.rs
```

Create an isolated historical worktree, run that commit's ignored fixture generator
against a temp path, copy only the resulting frozen DB to the current fixture
directory, and remove the worktree. Never point historical code at the real app
data directory. If a historical schema cannot be reproduced, keep automatic
publication disabled and file a P0 compatibility issue rather than fabricating a
fixture from current code or weakening the backward-transitive promise.

- [ ] **Step 6: Add both commands to `extraGate`**

```json
[
  "cargo test --manifest-path src-tauri/Cargo.toml --features test-support --test compat_golden",
  "cargo test --manifest-path src-tauri/Cargo.toml --features test-support --test export_compat_golden"
]
```

- [ ] **Step 7: Run compatibility tests and formatting**

```bash
cd src-tauri
cargo fmt
cargo test --features test-support --test compat_golden
cargo test --features test-support --test export_compat_golden
```

Expected: PASS.

- [ ] **Step 8: Commit**

```bash
git add src-tauri/src/commands/static_publishing.rs src-tauri/src/lib.rs src-tauri/tests docs/CONTRACTS.md release.config.json
git commit -m "test(release): enforce stable compatibility contracts"
```

---

### Task 5: CI Release Gate and Conditional E2E

**Files:**
- Create: `scripts/release-gate.mjs`
- Create: `scripts/release-gate.test.ts`
- Modify: `scripts/preflight.sh`
- Modify: `.github/workflows/ci.yml`
- Modify: `.github/dependabot.yml`
- Modify: `scripts/check-ci.sh`

**Interfaces:**
- Consumes: Tasks 1–4 CLI/config/contracts.
- Produces: `release-gate.json` with `{schema, version, tag, sha, baseTag, mainAncestor, versions, e2e, commands}` and gate outputs for workflows.

- [ ] **Step 1: Write failing gate tests**

Test exact rejection of malformed tags, tag/SHA mismatch, SHA not reachable from
`origin/main`, mismatched version metadata, missing changelog stamp, missing stable
contract command, and unrecorded E2E hits.

- [ ] **Step 2: Implement the gate CLI**

Accept only:

```text
--tag vX.Y.Z --sha SHA40 --base-tag vA.B.C --event tag|dispatch --json-out /tmp/release-gate.json
```

Use `git diff --name-only "$BASE_TAG..$SHA"` and Task 1's classifier. For manual
dispatch, require the supplied tag already exists and resolves to the supplied SHA.
Write the gate JSON atomically and print the same object to stdout.

- [ ] **Step 3: Make release preflight truthful**

Remove documentation and script handling for the nonexistent
`CULL_PREFLIGHT_SKIP_E2E`. Keep local release preflight deterministic:

```bash
run npm run audit:licenses
run bash scripts/supply-chain-audit.sh check
run cargo test --manifest-path src-tauri/Cargo.toml --features test-support --test compat_golden
run cargo test --manifest-path src-tauri/Cargo.toml --features test-support --test export_compat_golden
run npm run build
```

Conditional browser E2E remains in the tag release gate, where changed paths are
known and evidence can be persisted.

- [ ] **Step 4: Add CI concurrency and Rust caching**

At workflow top level:

```yaml
permissions:
  contents: read
concurrency:
  group: ci-${{ github.workflow }}-${{ github.event.pull_request.number || github.ref }}
  cancel-in-progress: true
```

Add `Swatinem/rust-cache` to Rust jobs with `workspaces: src-tauri -> target`.
Add an explicit `site` job running `npm ci`, `npm run check`, `npm test`, and
`npm run build` inside `site/`. Add `/site` to Dependabot.

- [ ] **Step 5: Verify**

```bash
npm run test:release
actionlint .github/workflows/ci.yml
npm run preflight:hook
```

Expected: PASS.

- [ ] **Step 6: Commit**

```bash
git add scripts/release-gate.mjs scripts/release-gate.test.ts scripts/preflight.sh scripts/check-ci.sh .github/workflows/ci.yml .github/dependabot.yml
git commit -m "ci(release): enforce release policy before packaging"
```

---

### Task 6: Exact Signed-Artifact Verification

**Files:**
- Create: `scripts/verify-release-artifacts.sh`
- Create: `scripts/verify-release-artifacts.test.sh`
- Modify: `scripts/clean-machine-dmg-gate.sh`

**Interfaces:**
- Consumes: directory containing exactly the workflow-produced DMG, updater archive, signature, and `latest.json`.
- Produces: `release-provenance.json`, `checksums.txt`, logs, and exit status.

- [ ] **Step 1: Write fake-tool failure tests**

Use a temporary `PATH` with shims for `codesign`, `spctl`, `xcrun`, `hdiutil`,
`plutil`, and `lipo`. Cover valid inventory, missing `.sig`, stale `latest.json`,
wrong embedded version, non-arm64 app, and failed stapler validation.

- [ ] **Step 2: Run tests and verify failure**

Run: `bash scripts/verify-release-artifacts.test.sh`

Expected: FAIL because the verifier does not exist.

- [ ] **Step 3: Implement the verifier contract**

Accept:

```text
--artifact-dir DIR --version X.Y.Z --tag vX.Y.Z --commit SHA --run-id ID --out DIR [--launch]
```

Reject missing or extra public artifact names. Compute SHA-256, validate updater
signature with the configured Tauri public key, mount the DMG read-only, verify the
app from the mounted image, extract `CFBundleShortVersionString`, require arm64,
and optionally copy to `$RUNNER_TEMP/install/Cull.app` for launch. Never write to
`/Applications`.

Write provenance with this top-level shape:

```json
{
  "schema": "cull.release.provenance.v1",
  "version": "0.2.6",
  "tag": "v0.2.6",
  "commit": "0123456789abcdef0123456789abcdef01234567",
  "workflowRunId": "123",
  "assets": {},
  "checks": {}
}
```

- [ ] **Step 4: Refactor the local DMG gate**

Keep local `--build` as a wrapper, but delegate trust and inventory checks to the
new verifier. Replace `/Applications` backup/restore with an isolated temp install
location. Remove the current `rm -rf` restore path.

- [ ] **Step 5: Verify**

```bash
bash scripts/verify-release-artifacts.test.sh
bash -n scripts/verify-release-artifacts.sh scripts/clean-machine-dmg-gate.sh
```

Expected: PASS.

- [ ] **Step 6: Commit**

```bash
git add scripts/verify-release-artifacts.sh scripts/verify-release-artifacts.test.sh scripts/clean-machine-dmg-gate.sh
git commit -m "feat(release): verify exact signed artifacts"
```

---

### Task 7: Signed Non-Publishing Canary

**Files:**
- Create: `.github/workflows/release-canary.yml`
- Modify: `docs/RELEASING.md`

**Interfaces:**
- Consumes: release gate and artifact verifier.
- Produces: signed workflow artifact plus provenance; never a GitHub release.

- [ ] **Step 1: Create the canary workflow**

Use `workflow_dispatch` input `ref` defaulting to `main`, concurrency group
`release-canary`, `cancel-in-progress: true`, and top-level `contents: read`.

Jobs must be:

```text
gate -> signed-build -> verify
```

The signed build receives Apple/Tauri secrets and invokes `tauri-action` without
`tagName`, `releaseName`, or release upload settings. Upload named artifact
`cull-canary-${{ github.run_id }}`. The verify job has no secrets and runs the exact
artifact verifier.

- [ ] **Step 2: Assert the workflow cannot publish**

Add a contract test in `scripts/release-gate.test.ts` that reads the YAML as text
and asserts it contains no `contents: write`, `gh release`, `tagName`,
`HOMEBREW_TAP_TOKEN`, or `releaseDraft`.

- [ ] **Step 3: Validate locally**

```bash
actionlint .github/workflows/release-canary.yml
npm run test:release
```

Expected: PASS.

- [ ] **Step 4: Dispatch and inspect one canary**

```bash
gh workflow run release-canary.yml -f ref=main
gh run list --workflow release-canary.yml --limit 1
```

Expected: signed-build and verify succeed; no GitHub release or tag is created.

- [ ] **Step 5: Commit**

```bash
git add .github/workflows/release-canary.yml docs/RELEASING.md scripts/release-gate.test.ts
git commit -m "ci(release): add signed non-publishing canary"
```

---

### Task 8: Gate, Build, Verify, and Automatic Publish Workflow

**Files:**
- Modify: `.github/workflows/release.yml`
- Modify: `scripts/release-gate.test.ts`
- Modify: `docs/RELEASING.md`

**Interfaces:**
- Consumes: immutable existing tag, gate JSON, signed workflow artifact, provenance.
- Produces: published GitHub release with curated notes and verified assets.

- [ ] **Step 1: Write workflow contract tests**

Assert `release.yml` has distinct jobs `release-gate`, `signed-build`,
`verify-artifact`, and `publish`; only publish has `contents: write`; signed-build
contains signing secrets but no tap token; publish has no signing secrets.

- [ ] **Step 2: Restrict manual dispatch**

Require a typed `tag` input. Both tag push and dispatch resolve an existing
annotated `v*` tag and its immutable SHA. Set concurrency to
`release-${{ inputs.tag || github.ref_name }}` with cancellation disabled.

- [ ] **Step 3: Add the zero-secret release gate**

Checkout full history, run `release-gate.mjs`, install locked dependencies, execute
license, supply-chain, compatibility, stable export, and conditional E2E gates,
then upload `release-gate.json`. A failure prevents the secret-bearing build job.

- [ ] **Step 4: Build without publishing**

Run Tauri action in build-only mode, upload exactly one named workflow artifact,
and expose its name and run ID to downstream jobs. Do not create a draft here.

- [ ] **Step 5: Verify without secrets**

Download the exact named artifact, run the verifier, and upload the verified
artifact plus `release-provenance.json` and checksums. Failure prevents publish.

- [ ] **Step 6: Publish automatically**

The publish job downloads only the verified artifact, creates a draft bound to the
existing tag/SHA, uploads assets and provenance, sets the release body from the
matching `CHANGELOG.md` section, then executes:

```bash
gh release edit "$TAG" --draft=false
```

Record published state before dispatching Homebrew promotion. Never rebuild or
replace assets in this job.

- [ ] **Step 7: Validate**

```bash
actionlint .github/workflows/release.yml
npm run test:release
```

Expected: PASS.

- [ ] **Step 8: Commit**

```bash
git add .github/workflows/release.yml scripts/release-gate.test.ts docs/RELEASING.md
git commit -m "ci(release): publish only verified signed artifacts"
```

---

### Task 9: SHA-Pinned Homebrew Promotion and Recovery

**Files:**
- Modify: `.github/workflows/update-tap.yml`
- Modify: `scripts/cull-release.mjs`
- Modify: `scripts/cull-release-cli.test.ts`
- Modify: `docs/RELEASING.md`

**Interfaces:**
- Consumes: published release and `release-provenance.json`.
- Produces: tap commit with exact version and SHA; resumable promotion evidence.

- [ ] **Step 1: Write failure and idempotency tests**

Cover wrong SHA, missing provenance, `sha256 :no_check`, already-equal version/SHA,
and tap-behind recovery. Ensure recovery returns a plan and never deletes a tag or
release.

- [ ] **Step 2: Require provenance-backed inputs**

Manual dispatch requires `version`, `dmg_sha256`, and `provenance_url`. Release
events fetch the public provenance asset. Download the DMG, compute SHA-256, and
require exact equality before checking out the tap with its token.

- [ ] **Step 3: Update both cask fields**

Replace only:

```ruby
version "0.2.6"
sha256 "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef"
```

Reject `sha256 :no_check`. Run `brew audit --cask cull` and an isolated cask
install/launch smoke before pushing. If version and SHA already agree, succeed
without a commit.

- [ ] **Step 4: Implement recovery classification**

`state fail` stores a stable code and evidence. `resume` reconstructs truth and
returns `promote-homebrew` for a valid published release with a stale tap. A failed
post-publish verify returns `prepare-patch-plan`, creates/updates a P0 bd issue via
the repository wrapper, and blocks a later release check.

- [ ] **Step 5: Validate**

```bash
actionlint .github/workflows/update-tap.yml
npm run test:release
```

Expected: PASS.

- [ ] **Step 6: Commit**

```bash
git add .github/workflows/update-tap.yml scripts/cull-release.mjs scripts/cull-release-cli.test.ts docs/RELEASING.md
git commit -m "feat(release): pin and resume Homebrew promotion"
```

---

### Task 10: Documentation, Repository Protection, and Enablement Gate

**Files:**
- Modify: `README.md`
- Modify: `docs/toolchain.md`
- Modify: `docs/cross-platform-distribution.md`
- Modify: `docs/e2e-testing-policy.md`
- Modify: `docs/RELEASING.md`
- Modify: `docs/OPEN_SOURCE_AUDIT.md`
- Modify: `AGENTS.md`

**Interfaces:**
- Consumes: all previous tasks.
- Produces: truthful operator documentation and a deliberate automatic-publication enablement decision.

- [ ] **Step 1: Correct documented facts**

Document Rust 1.95, signed/notarized current releases, Apple Silicon-only artifacts,
machine-classified release E2E, repository-local release commands, canary behavior,
automatic publication, immutable-tag recovery, and checksum-pinned Homebrew.
Correct the stale skills repository path from `~/ai_projects/Codex-skills/` to
`~/ai_projects/claude-skills/`.

- [ ] **Step 2: Protect `main` and release tags**

Create GitHub rules requiring Frontend, Site, Rust, and Supply-chain checks on
`main`, blocking force pushes and deletion. Add a `v*` tag ruleset blocking update
and deletion while allowing the release automation identity to create new tags.
Record the ruleset IDs in the release readiness output, not in source secrets.

- [ ] **Step 3: Run the complete local gate**

```bash
npm run test:release
npm run preflight:release
actionlint .github/workflows/*.yml
```

Expected: PASS. If disk is below the configured threshold, stop before Rust builds
and clean only explicitly approved inactive caches.

- [ ] **Step 4: Run a signed non-publishing canary**

Dispatch `release-canary.yml`, download its artifact and provenance, and rerun the
verifier locally. Confirm no tag, release, or tap commit was created.

- [ ] **Step 5: Prove publication blocking**

In a test fork or fixture repository, intentionally fail artifact verification and
confirm publish is skipped. Intentionally fail tap SHA verification and confirm the
GitHub release remains valid while the tap is unchanged.

- [ ] **Step 6: Enable automatic publication**

Enable the publish job only after Tasks 4–10 are green and the canary evidence is
attached to the implementation issue. Do not enable it merely because the YAML
parses.

- [ ] **Step 7: Commit**

```bash
git add README.md AGENTS.md docs
git commit -m "docs(release): document automated verified publication"
```

- [ ] **Step 8: Run Cull landing flow**

```bash
npm run land
```

Expected: full gate passes, bd state is reported, branch rebases cleanly, and push
succeeds.
