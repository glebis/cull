# Modular Cull Release Skill Suite — Design

**Date:** 2026-07-10
**Status:** Approved in conversation; written specification awaiting user review
**Owner:** Gleb Kalinin
**Tracking:** `imageview-b4y1`

## Goal

Automate Cull's release cycle from an explicit SemVer request through signed
publication, Homebrew promotion, and post-publish verification. The operator
invokes one public skill with `patch`, `minor`, or `major`; the automation may
publish without another confirmation only after every mandatory gate passes.

The system must remain reproducible without Codex. Deterministic checks and
mutations live in Cull's tracked scripts and GitHub workflows. Codex skills
coordinate those interfaces, explain failures, and resume interrupted releases.

## Locked decisions

| Decision | Choice |
|---|---|
| Version intent | Operator explicitly supplies `patch`, `minor`, or `major` |
| Structure | Modular skill suite with one public orchestrator |
| Publication authority | Automation may publish after all mandatory gates pass |
| Human draft approval | Not required |
| Recovery | Never rewrite or automatically delete a pushed tag or published release |
| Post-publish failure | Block later releases, record an incident, and prepare an explicit patch plan |
| Skill source | `~/ai_projects/claude-skills/` |
| Codex discovery during development | Symlinks in `~/.agents/skills/` |
| Deterministic release logic | Cull repository scripts and GitHub Actions, not skill prose |
| Supported release target | Apple Silicon macOS until the release matrix deliberately expands |

## Non-goals

- Inferring SemVer intent from commits.
- Releasing every merged pull request.
- Automatically fixing product, test, signing, compatibility, or licensing
  failures.
- Reusing or rewriting a pushed tag after a failed build.
- Automatically publishing a recovery patch. Recovery prepares the next release;
  the operator must explicitly invoke `cull-release patch`.
- Expanding Cull to Intel macOS, Windows, or Linux as part of this work.

## Ownership and discovery

The canonical skill directories live in the existing
`~/ai_projects/claude-skills/` Git repository:

```text
~/ai_projects/claude-skills/
  cull-release/
  cull-release-check/
  cull-release-prepare/
  cull-release-publish/
  cull-release-verify/
  cull-release-recover/
```

Each directory contains a concise `SKILL.md` and `agents/openai.yaml`. Reusable
logic belongs in Cull, so the skills need references only when they document a
tool contract that cannot be discovered from command help.

During development, each directory is symlinked into `~/.agents/skills/`. This
makes it discoverable by Codex without maintaining a second copy. Publishing or
installing on another machine uses the skills CLI or plugin marketplace. Merely
committing a directory to `~/ai_projects/claude-skills/` does not install it.

## Skill boundaries

### `cull-release`

Public entry point. Accepts `patch`, `minor`, or `major`, creates or resumes the
release record, and calls the other skills in order. It does not duplicate their
checks. It is the only skill that performs the complete release lifecycle.

Example triggers:

- “Release Cull patch.”
- “Resume Cull release 0.2.6.”
- “What is blocking the current Cull release?”

### `cull-release-check`

Read-only readiness inspection. It verifies repository state, release ancestry,
toolchains, disk space, version consistency, required secrets by presence, open
release blockers, and the commands that the requested release would execute. It
must not bump versions, commit, tag, dispatch a publishing workflow, or expose
secret values.

### `cull-release-prepare`

Runs in a dedicated clean release worktree based on current `origin/main`. It
calculates the explicit next version, updates every version-bearing file and
lockfile, generates the curated changelog section, performs the compatibility
review, executes local gates, commits the release, creates an annotated tag, and
pushes the release commit and tag only after the local gates pass.

### `cull-release-publish`

Watches the tag-triggered GitHub workflow, verifies the signed draft and its exact
assets, publishes the GitHub release, and promotes Homebrew using the verified DMG
SHA-256. It may publish automatically because the operator authorized that action
when invoking the orchestrator and the workflow independently revalidated all
mandatory gates.

### `cull-release-verify`

Read-only verification for a release version. It checks public GitHub assets,
release notes, updater metadata and signature, Homebrew version and checksum,
macOS trust state, installed version, and a launch smoke test. It can be invoked
independently after any release.

### `cull-release-recover`

Handles a failed release state without destructive history operations. It records
the failure, creates or updates a P0 bd incident, marks the release as problematic
when already public, blocks the next release, and prepares a patch-release plan.
It never deletes a release, rewrites a tag, or publishes the patch.

## Release state machine

```text
requested
  -> checked
  -> prepared
  -> tagged
  -> draft-built
  -> artifact-verified
  -> published
  -> homebrew-promoted
  -> post-publish-verified
```

Every transition is monotonic. A rerun resumes from the last state whose evidence
still verifies. A state is not trusted merely because it appears in the record;
the responsible skill rechecks the referenced commit, workflow, artifact, or
public endpoint before advancing.

### Release record

Each release writes a local JSON record under the gitignored
`.release-state/<version>.json` path with this minimum schema:

```json
{
  "schema": "cull.release.v1",
  "version": "0.2.6",
  "bump": "patch",
  "state": "artifact-verified",
  "releaseCommit": "<sha>",
  "tag": "v0.2.6",
  "workflowRunId": 123,
  "requestedAt": "<ISO-8601>",
  "updatedAt": "<ISO-8601>",
  "gates": {},
  "assets": {},
  "failure": null
}
```

Gate entries record command or workflow name, result, timestamp, and evidence
reference. Asset entries record filename, size, SHA-256, updater signature, and
source workflow. The record contains identifiers and results, never secret values.

The local record is a resumability cache, not the source of truth. The release
workflow uploads the same schema as an Actions artifact and attaches a finalized
`release-provenance.json` to the GitHub release. On resume, skills rederive state
from the immutable tag and commit, GitHub workflow run, release assets, and
Homebrew tap commit. Missing or stale local state cannot override that evidence.

## Mandatory gates

### Readiness

- Release worktree is clean and based on current `origin/main`.
- Requested version is greater than the latest published and pushed tag.
- Release commit is reachable from `main`.
- Node and Rust match the repository pins.
- Available disk exceeds a documented conservative threshold before Rust builds.
- Required GitHub, Apple signing/notarization, Tauri updater, and Homebrew
  credentials are present without printing their contents.
- No unresolved P0 release incident blocks publication.

### Source and compatibility

- `package.json`, the root package entry in `package-lock.json`, the root Cull
  package entry in `src-tauri/Cargo.lock`, `src-tauri/Cargo.toml`, and
  `src-tauri/tauri.conf.json` agree with the requested version.
- Changelog contains a curated section for the version.
- Compatibility policy is reviewed and stamped for the version.
- A stable-surface breaking change rejects `patch` or `minor` as appropriate.
- Golden database fixtures cover every retained schema boundary and pass with
  `test-support` enabled.
- Golden contracts for every stable export format pass. Their missing fixtures
  block enabling automatic publication.

### Code and policy

- Frontend static checks and Vitest pass.
- Rust formatting, Clippy, and all targets pass with locked dependencies.
- License and model-download policy audit passes.
- Supply-chain audit passes or matches an explicitly documented accepted advisory.
- Browser E2E runs when the diff from the previous release tag touches paths in
  the E2E policy. The path classifier and result are recorded.
- Production frontend build passes.

### Artifact

- The tag-triggered workflow independently repeats the release-critical gates.
- Tag, requested version, all metadata files, and workflow commit agree.
- The signed build is produced by the recorded workflow run, not a local build.
- DMG and updater archive exist with expected architecture and names.
- SHA-256 is recorded for every public binary artifact.
- Tauri updater signature validates against the configured public key.
- `codesign --verify --deep --strict`, Gatekeeper assessment, and stapler
  validation pass.
- The DMG mounts read-only, contains Cull, installs into an isolated test location,
  launches, and reports the requested version.

### Publication and distribution

- GitHub release body uses the curated changelog section instead of a generic
  sentence.
- Publishing exposes the already-verified assets without replacing them.
- `latest.json` names the published version and verified updater artifacts.
- Homebrew cask version and SHA-256 match the verified DMG; `sha256 :no_check` is
  forbidden.
- A Homebrew install or upgrade smoke test launches the published version.

## GitHub workflow enforcement

The release workflow gains a required `release-gate` job. Packaging and
publication depend on it. The gate checks `main` ancestry, tag/version alignment,
license policy, supply chain, compatibility, and conditional E2E. Manual workflow
dispatch may rebuild an existing immutable tag but may not create a release from
an arbitrary branch or untagged `main` commit.

The workflow first creates or updates a draft release. Post-build jobs download
and verify the exact produced artifacts. Only a successful verification job may
publish. Homebrew promotion runs after publication with the recorded SHA-256.

A separate non-publishing canary path exercises the current signing,
notarization, Tauri action, and verification configuration before automatic
publication is enabled.

## Failure behavior

| Failure point | Required behavior |
|---|---|
| Before preparation | Stop with a read-only report |
| During preparation | Preserve the isolated worktree and diagnostics; do not touch the operator's active branch |
| After release commit, before tag push | Stop; keep the local release state for inspection |
| After pushed tag, before publication | Keep the immutable tag and draft; record failure and require a new explicit version after fixes |
| Artifact verification | Do not publish or promote Homebrew |
| GitHub publication | Record public state before attempting Homebrew promotion |
| Homebrew promotion | Keep the valid GitHub release; record distribution failure and retry only that transition |
| Post-publish verification | Mark problematic, block later releases, file/update a P0 incident, and prepare a patch plan |

No failure handler uses `git reset --hard`, force-pushes, deletes a pushed tag, or
deletes a published release. Cleanup of temporary files uses safe project cleanup
mechanisms and never touches `cull.db`.

## Operator interface

The public commands are intentionally small:

```text
Release Cull patch|minor|major
Check Cull release readiness
Resume Cull release <version>
Verify Cull <version>
Prepare recovery for Cull <version>
```

Before mutations, the orchestrator reports the requested version, source commit,
target version, supported platform, and automatic-publication authority already
granted by the request. It does not ask for another publication confirmation when
all gates pass.

## Testing strategy

### Deterministic script tests

- Unit tests for version parsing, next-version calculation, metadata alignment,
  changelog extraction, E2E path classification, state transitions, and checksum
  generation.
- Fixture repositories for clean, dirty, divergent, stale-tag, mismatched-version,
  failed-gate, and resumable-release cases.
- Contract tests for the JSON release record and command JSON output.
- Shell syntax and static checks for all workflow scripts.

### Workflow tests

- Validate workflow syntax and expression paths in CI.
- Exercise gates on pull requests without signing or publishing.
- Run one non-publishing signed canary that uploads Actions artifacts only.
- Simulate failed artifact verification and prove publication remains blocked.
- Simulate a Homebrew failure and prove resume starts at promotion rather than
  rebuilding the app.

### Skill tests

- Validate every skill with the skill-creator validator.
- Forward-test each skill with minimal context against dry-run fixtures.
- Verify only `cull-release` attempts the whole lifecycle.
- Verify `cull-release-check` and `cull-release-verify` remain read-only.
- Verify recovery never proposes tag deletion, force-push, or database cleanup.

## Rollout

1. Correct workflow and documentation drift that would invalidate automation:
   version alignment, Rust pin documentation, signed-build instructions, active
   architecture, and E2E policy wording.
2. Add deterministic release-state, version, gate, and artifact-verification
   scripts with dry-run support.
3. Add the enforced GitHub release gate and non-publishing canary.
4. Replace Homebrew `:no_check` with verified SHA-256 promotion.
5. Build and validate the six skills in `~/ai_projects/claude-skills/`, then
   symlink them into `~/.agents/skills/`.
6. Run dry-run and failure-path forward tests.
7. Run a signed non-publishing canary.
8. Enable automatic publication only after the canary and publication-blocking
   tests pass.

## Acceptance criteria

- One explicit `Release Cull patch|minor|major` request can reach a verified
  GitHub publication and checksum-pinned Homebrew cask without a second approval.
- The same release can resume after interruption without repeating verified
  expensive work.
- GitHub rejects a direct tag or manual dispatch that bypasses release policy.
- The exact CI-produced artifact passes trust, updater, mount, install, launch,
  version, and checksum verification before publication.
- Conditional browser E2E is enforced from the previous-tag diff.
- A failed post-publish verification blocks later releases and prepares, but does
  not publish, an explicit patch recovery.
- Skills are canonical in `~/ai_projects/claude-skills/` and automatically
  discoverable in Codex through `~/.agents/skills/` symlinks.
- No workflow deletes or resets Cull user data.
