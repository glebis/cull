# Task 9 implementation report

Status: DONE

Commits:

- `ffb9ac95e feat(release): pin and resume Homebrew promotion`
- `a18a1e366 fix(release): audit committed tap candidate`
- `6de2d8c01 fix(release): harden public promotion recovery`

## Scope

- Replaced the unpinned tap bump with provenance/schema/check validation, exact public DMG download hashing, delayed token use, exact two-field cask editing, audit/install/version/launch smoke, idempotent no-commit success, and retained promotion evidence.
- Added a post-publication dispatch from `release.yml` using the verified provenance digest and canonical public provenance URL.
- Added stable CLI failure codes, idempotent P0 incident creation/update via `npm run bd --`, readiness blocking, patch-plan recovery, and exact version+SHA tap recovery probes.
- Documented promotion, resume, and immutable recovery behavior.

## TDD evidence

Initial red run:

`npx vitest run scripts/cull-release-cli.test.ts scripts/release-gate.test.ts`

- 5 expected failures: unstable failure code accepted; no P0 incident; missing provenance/SHA tap contract; no post-publish promotion dispatch.
- Added wrong-tap-SHA regression separately; it failed because resume returned `complete` rather than `promote-homebrew`.
- Self-review added an ordering regression: it failed while the local cask commit occurred after `brew audit`. The fix commits the candidate locally before audit/install, while retaining push until after the smoke test.

Final green evidence:

- `npm run test:release`: 97/97 passed after commit.
- `npm test`: 1,048/1,048 passed before the final narrow SHA-probe addition; the final release suite covers that addition.
- `actionlint .github/workflows/update-tap.yml .github/workflows/release.yml`: passed.
- `git diff --check`: passed.
- `npm run build`: passed.
- Commit hook shell syntax/preflight: passed.

## Diagnostic outside Task 9

`npm run check` fails while loading the unchanged nested `site/vite.config.ts`: svelte-check reports that it has no Svelte plugin. Task 9 does not touch root/site configuration. The production build and all Vitest suites pass. Per controller direction, no out-of-scope site/config change was made.

## Risks / review attention

- The public GitHub asset API must expose the same `sha256:<digest>` value already required by the publish workflow; promotion fails closed if missing or different.
- The current tap must contain exactly one quoted `version` and one quoted 64-hex `sha256`; `:no_check` intentionally fails closed.
- A closed P0 incident unblocks readiness; an unreadable/malformed incident remains blocking.

## Independent-review fix wave

The first independent review was not approved and returned six blocking findings.
All six were converted to failing tests before implementation:

1. Public promotion now resolves the anonymous remote annotated tag object and
   peeled commit, binds them to provenance, compares all four public asset
   sizes/digests, and validates downloaded provenance/checksum asset digests and
   exact checksum contents before tap-token use.
2. Tap promotion uses one global concurrency group. The executable cask editor
   rejects downgrades and equal-version/different-SHA attempts; only exact
   version+SHA equality is idempotent.
3. Readiness independently queries bd's stable release-incident external-ref
   namespace and fails closed even when local state is missing or stale.
4. Resume uses strict remote tag, public provenance/inventory/checksums, release
   workflow, exact tap, and successful promotion-workflow truth. The fabricated
   `postPublishVerified` provenance test field was removed; a test proves stale
   cached commit/run data cannot override public truth.
5. The new built-ins-only cask editor has executable adversarial tests for
   `:no_check` with comments, duplicate active directives, noncanonical forms,
   downgrade, immutable equal version, idempotency, and exact two-field upgrade.
6. The macOS smoke test waits after launch, finds the isolated app process,
   verifies it is still alive, and rechecks its bundle version before push.

Final evidence after the fix wave:

- `npm run test:release`: 107/107 passed.
- `npm test`: 1,059/1,059 passed.
- `npm run build`: passed.
- `actionlint .github/workflows/update-tap.yml .github/workflows/release.yml`: passed.
- `node --check` for both release CLIs: passed.
- `git diff --check` and the commit hook: passed.

## Final provenance-authentication blockers

Implemented the two remaining independent-review blockers with red-green TDD.

Red evidence:

- `npx vitest run scripts/release-gate.test.ts -t 'validates public provenance'`
  failed because `update-tap.yml` did not fetch or authenticate
  `repos/glebis/cull/actions/runs/$WORKFLOW_RUN_ID` before exposing the tap token.
- `npx vitest run scripts/cull-release-cli.test.ts -t 'wrong DMG SHA'` failed
  because malformed public provenance combined with cached `workflowRunId: 42`
  still derived `promote-homebrew`.
- The recovery regression also covers a successful but unrelated `ci.yml` run;
  it must not authenticate release provenance or derive promotion.

Green implementation:

- `.github/workflows/update-tap.yml` now requires the current repository to be
  exactly `glebis/cull`, fetches the provenance run by exact ID from the Actions
  API, and requires exact ID, repository, `.github/workflows/release.yml` path,
  completed/success status, and allowed event. Push binds `head_sha` to the
  provenance commit; `workflow_dispatch` binds origin to `main` while allowing
  its invocation SHA to differ. This occurs before the first tap-token reference.
- `scripts/cull-release.mjs` no longer falls back to cached
  `record.workflowRunId`. Recovery binds release `tag_name`, `draft`, and
  `prerelease`, strict provenance, and the same authenticated exact workflow run
  before either `publishedRelease` or `releaseAsset` can become true.
- Tests and release documentation were updated in
  `scripts/cull-release-cli.test.ts`, `scripts/release-gate.test.ts`, and
  `docs/RELEASING.md`.

Green evidence:

- Focused workflow test: 1/1 passed.
- Focused recovery tests: 2/2 passed.
- `npm run test:release`: 107/107 passed.
- `actionlint .github/workflows/update-tap.yml`: passed.
- `node --check scripts/cull-release.mjs`: passed.
- `git diff --check`: passed.
- Full `npm test` was attempted repeatedly, but the repository test harness
  terminated the invoking shell after printing only its startup/fixture reset,
  without a final Vitest summary or exit-status line. It is therefore not
  represented as passing; the complete release-specific suite is green.

Files changed:

- `.github/workflows/update-tap.yml`
- `scripts/cull-release.mjs`
- `scripts/cull-release-cli.test.ts`
- `scripts/release-gate.test.ts`
- `docs/RELEASING.md`

Commit: `90ae91f532e615aeb5a477e6fd758e3c0110a206`
(`fix(release): authenticate release workflow provenance`).

Concern: the full repository Vitest aggregate could not be conclusively observed
because its harness terminated the parent shell. No external writes, workflows,
tags, releases, tap changes, database access, or pushes were performed.

Controller verification resolved this concern after the commit:

- `npm test -- --reporter=dot`: 134 files, 1,059 tests passed in 41.01s.

Final publication-race correction:

- Added an executable regression that extracts and runs the authentication
  program embedded in `update-tap.yml`.
- The authenticated exact release run is accepted while it is `in_progress`
  with an unset conclusion, as well as after `completed/success`; queued and
  failed runs remain rejected.
- Red evidence: focused release-gate suite failed 1/40 because the legitimate
  in-progress run exited 2.
- Green evidence: focused release-gate suite passed 40/40.
- `npm run test:release`: 108/108 passed.
- `actionlint .github/workflows/*.yml`: passed.
- `node --check scripts/cull-release.mjs`: passed.
- `git diff --check`: passed.
