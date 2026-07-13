# Releasing Cull

Cull is released with the modular **`cull-release`** skill suite (coordinator,
check, prepare, publish, verify, and recover; canonical source in
`~/ai_projects/claude-skills/`). Repository config: `release.config.json`.
Policy: `docs/COMPATIBILITY.md`. Contract tests: `docs/CONTRACTS.md`.

## Repository release CLI

```bash
# Read-only readiness report
npm run release:cull -- check --bump patch --json

# Read-only preparation preview (the compatibility review is explicit JSON)
npm run release:cull -- prepare --bump patch --expected-source "$(git rev-parse HEAD)" \
  --expected-version 0.3.2 --request-json '{"version":"0.3.2","requestedBump":"patch","stableBreakingChange":false,"changedSurfaces":[],"reviewedBy":"Gleb Kalinin"}' \
  --notes $'### Fixed\n\n- Curated release note.' --dry-run --json
```

Remove `--dry-run` only after reviewing the plan. Real preparation validates that
HEAD and the expected next version have not moved, requires the configured clean
release worktree, updates the five declared version locations, inserts the curated
changelog notes, stamps the compatibility review date, runs the configured gates,
and creates exactly one `chore(release): vX.Y.Z` commit. Preparation never tags or
pushes. The release state cache is written to `.release-state/X.Y.Z.json` with
owner-only permissions.

Preparation must run on `main` in the configured linked release worktree; an
ordinary checkout, detached worktree, or submodule is rejected. Gate commands are
preferably configured as JSON argument arrays. The legacy string form accepts
only simple whitespace-separated commands and rejects shell syntax.

If a gate, concurrent Git operation, or staging race occurs before the commit,
the CLI restores every release-owned file and the Git index byte-for-byte. After
commit, the CLI verifies the parent, subject, exact seven-file path set, and exact
planned bytes. A state-cache failure after that verified commit is reported as
`INCONSISTENT_RECOVERY`; the valid commit remains the recovery anchor.

The compatibility review is mandatory. Any breaking change to a `stable` surface
requires a major bump.

## Signed non-publishing canary

`.github/workflows/release-canary.yml` is the manual confidence run for Apple
signing, notarization, private artifact transfer, and exact artifact verification.
It executes `gate -> signed-build -> verify` and cannot create a Git tag, GitHub
Release, or Homebrew update. Repository contents are read-only, signing secrets
exist only in `signed-build`, and the secret-free verifier receives the signed
inventory through GitHub Actions artifacts. The signed inventory is retained for
one day; gate evidence and verifier provenance, checksums, and logs are retained
for 14 days.

The `ref` input defaults to `main`, but convenience is not authority. Canary mode
requires the resolved commit to be reachable from `origin/main` and validates its
version files, changelog and compatibility stamps, stable contracts, and changed
path classification. Its canonical diff base is the highest reachable exact
stable SemVer tag at or below the target version. This intentionally permits an
untagged `main` commit after `vX.Y.Z` while the package still reports `X.Y.Z`; the
same-version ancestor tag remains the immutable diff base. A caller-supplied
older or injected tag cannot narrow or widen that diff.

Gate evidence records both `event` and `publishEligible`. Canary evidence is
always `event: canary` and `publishEligible: false`; the signed job checks both
values before any secret-bearing step. Tag and manual-dispatch evidence remains
publish-eligible only after exact tag-to-SHA binding succeeds, so canary evidence
cannot be reused as release approval. After the workflow is present on the
default branch, dispatch it from GitHub Actions and inspect all three jobs plus
the evidence artifact. Canary dispatch is an explicit enablement operation;
repository-local checks do not dispatch it.

Canary verifier provenance records `tagObjectSha: null` because it intentionally
does not represent a publishable annotated tag identity.

## Verified automatic publication

`.github/workflows/release.yml` accepts only a pushed `vX.Y.Z` tag or a manual
`tag` input naming an existing annotated `vX.Y.Z` tag. It resolves that tag to an
immutable commit and runs four ordered jobs:

1. `release-gate` is secret-free and completes licence, supply-chain,
   compatibility, stable-export, conditional browser E2E, and frontend build
   checks.
2. `signed-build` receives Apple and Tauri signing material only after the gate
   record says `publishEligible: true`; it builds and uploads a private exact
   inventory without creating a release.
3. `verify-artifact` is secret-free. It downloads the build by immutable artifact
   ID only after the Actions REST metadata matches the expected name, run,
   workflow invocation commit, expiry state, and upload digest. It verifies the
   signed DMG/updater inventory and uploads the exact verified files, checksums,
   provenance, log, and gate record as a new run-bound artifact.
4. `publish` has the workflow's only `contents: write` permission and is protected
   by the `release-publish` environment. It validates both evidence records,
   hashes, file identities, artifact/run binding, and the matching curated
   `CHANGELOG.md` section before creating or reusing an empty draft. It uploads
   explicit verified files without rebuilding or replacing assets, verifies the
   uploaded digests, rechecks both the remote annotated tag object and its peeled
   commit, publishes the draft without an intervening action, and immediately
   checks both tag identities again.

Automatic publication is fail-closed. The `publish` job requires the repository
variable `CULL_RELEASE_PUBLISH_ENABLED` to equal `true`; absent or false skips
publication after verification. Signed non-publishing canary run `29240223393`
passed the complete protected-main gate, signing, notarization, private transfer,
and secret-free verification on 2026-07-13. Automatic publication was enabled
only after that evidence and the active branch and immutable `v*` tag rules were
verified. Production run `29182947689` previously published v0.3.1 successfully.
The workflow's before/after identity checks detect a race but cannot substitute
for repository protection.

A manual dispatch is launched from the default-branch workflow and may select an
older annotated tag, so the workflow invocation SHA can legitimately differ from
the selected tag's peeled commit. Actions artifact metadata is therefore bound to
`github.sha` (the invocation), while gate evidence, artifact contents, verifier
provenance, and release validation remain bound separately to the selected
release commit. Publishable provenance also records the exact annotated tag
object SHA; the public provenance, remote object, peeled commit, and gate record
must all agree.

A failed build or verification publishes nothing. A publication failure leaves
the existing tag and any draft intact for explicit recovery; automation never
deletes or rewrites a tag/release and never uses asset clobbering. A partial draft
with assets fails closed instead of silently replacing them.

## SHA-pinned Homebrew promotion

After the guarded draft becomes public, `release.yml` dispatches
`update-tap.yml` with the verified version, DMG SHA-256, and public
`release-provenance.json` URL. The promotion workflow also accepts a public
GitHub `release.published` event; that path fetches the provenance asset from the
published release. Manual dispatch requires all three inputs and accepts only the
canonical public asset URL for that version.

The workflow has no tap credential while it fetches and validates the public
release, anonymous remote annotated-tag object and peeled commit, exact
provenance schema/run/identity, four-asset inventory and verification checks.
Every public binary asset's size and GitHub SHA-256 must match provenance. The
downloaded `checksums.txt` must enumerate exactly those four assets and its own
public size and digest must match, as must the downloaded provenance asset. The
workflow then downloads the public DMG and compares its computed SHA-256 with
provenance (and with the manual input when dispatched). Only after those checks
pass does the workflow authenticate `workflowRunId` through GitHub's Actions API
as the exact `glebis/cull` `.github/workflows/release.yml` run. The authenticated
run may be `in_progress` with no conclusion during the legitimate publication
race, or `completed/success`; every other lifecycle state fails closed. Push
runs must have `head_sha` equal to the provenance commit; manually
dispatched release runs must originate from `main` and may have a different
invocation SHA. Only then does the tap checkout receive `HOMEBREW_TAP_TOKEN`.

All Cull promotions share one non-cancelling concurrency group, so an older run
cannot race a newer version. Promotion changes exactly the cask's sole canonical
`version` and quoted `sha256` lines. `sha256 :no_check` (including a trailing
comment), duplicate or noncanonical directives, a SemVer downgrade, an equal
version with a different SHA, an unexpected cask diff, or a digest mismatch fails
closed. Equal version and SHA is the only idempotent no-write case. Before any push, the macOS runner executes
`brew audit --cask cull`, installs the cask into an isolated runner directory,
launches it, waits, proves the installed process remains alive, and rechecks the
bundle version. Only then may the local candidate commit be pushed. When the tap
already has the exact version and SHA, the workflow still validates it but
succeeds without a commit. Every run retains `homebrew-promotion.json`; a failed
run records a stable stage code so promotion can be resumed without rebuilding or
republishing Cull.

## Resume and recovery

```bash
npm run release:cull -- state show --version 0.3.1 --json
npm run release:cull -- resume --version 0.3.1 --json
```

Both commands are read-only. They probe the release commit, tag, workflow,
required release assets, published GitHub release, and Homebrew tap, then report
the derived state. Local state is only a cache: stale state may derive backward,
and read-only commands never rewrite it. `resume` returns the next action but does
not execute it.

State-writing automation may advance one step or record a stable failure:

```bash
npm run release:cull -- state transition --version 0.3.2 --to tagged \
  --evidence-json '{"tag":"v0.3.2"}' --json
npm run release:cull -- state fail --version 0.3.2 --code BUILD_FAILED \
  --evidence-json '{"workflowRunId":123}' --json
```

Failure codes are a stable allowlist rather than free-form text. Resume accepts
public state only when the release tag/draft/prerelease fields, remote annotated
tag, peeled commit, strict provenance, authenticated exact release workflow run,
exact inventory/checks/sizes/digests, checksums, cask version/SHA,
and successful `Promote Cull vX.Y.Z` workflow agree. It does not depend on a
fabricated post-verification field in provenance. A valid public release whose
tap is stale resumes with `promote-homebrew`; it does not rebuild or replace
release assets. `POST_PUBLISH_VERIFY_FAILED` is different because users
may already have received a bad release: `state fail` creates or updates one P0
bd incident through `npm run bd --`, stores its identifier and evidence, and
blocks later readiness checks. Readiness independently queries all non-closed P0
issues in the stable `cull-release-X.Y.Z-post-publish` external-reference
namespace, even if local release state is missing or stale, and fails closed when
that lookup is unavailable. `resume` then returns `prepare-patch-plan`.
Recovery never deletes a release, rewrites a tag, force-pushes, or silently
publishes the patch; a new explicit release request is required for that patch.

## Legacy manual flow

```bash
npm run preflight -- release                                           # deterministic local gate
$EDITOR CHANGELOG.md docs/COMPATIBILITY.md                              # curate + stamp
```

Prefer the repository CLI for preparation because it guards source/version races
and records resumable state. Tagging and publication remain separate, explicit
steps after preparation and artifact verification.

## Release artifact gate checks

Before publishing a macOS release build, run the clean-machine gate:

```bash
npm run clean-machine-dmg-gate -- --build                          # verify + checksums only
npm run clean-machine-dmg-gate:build-install                       # builds, checks, installs from DMG on a clean macOS machine
```

The gate has no trust bypass. It snapshots the exact DMG, updater archive,
base64-wrapped signature, and `latest.json`, validates the mounted app and
updater signature, and only then publishes checksums, logs, and provenance.
`--install` copies the verified app beneath `$RUNNER_TEMP/install`; it never
modifies the system app directory.

Private verification and staging directories are retired with the user's
`trash` command when available. Minimal CI runners do not need that nonstandard
tool: the safe-cleanup helper first atomically claims the exact validated inode
inside a unique mode-0700 sibling container on the same filesystem. Without
`trash`, that uniquely named `.cull-cleanup-claim.*` container is retained for
runner/worktree cleanup. The helper never recursively deletes, acts on a
replaced basename, follows symlinks, crosses filesystems, or moves an active
DMG mount.

## Notes

- `main` lives in the `cull-main-landing` worktree; release from there.
- Releases are **on demand** (ship-when-meaningful), not on a calendar.
- `release.yml` triggers on `v*` tags (and a required `workflow_dispatch` tag),
  but publication remains disabled while `CULL_RELEASE_PUBLISH_ENABLED` is absent
  or false.
- Disk: a full Rust rebuild is large; move only an idle worktree's generated
  `target/` to Trash if low on space (see AGENTS.md). Never touch `cull.db`.
