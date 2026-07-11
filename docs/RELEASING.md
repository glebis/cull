# Releasing Cull

Cull is released with the **`release` skill** (config-driven; lives in
`glebis/claude-skills`). Config: `release.config.json`. Policy:
`docs/COMPATIBILITY.md`. Contract tests: `docs/CONTRACTS.md`.

## Repository release CLI

```bash
# Read-only readiness report
npm run release:cull -- check --bump patch --json

# Read-only preparation preview (the compatibility review is explicit JSON)
npm run release:cull -- prepare --bump patch --expected-source "$(git rev-parse HEAD)" \
  --expected-version 0.2.6 --request-json '{"version":"0.2.6","requestedBump":"patch","stableBreakingChange":false,"changedSurfaces":[],"reviewedBy":"Gleb Kalinin"}' \
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

## Resume and recovery

```bash
npm run release:cull -- state show --version 0.2.6 --json
npm run release:cull -- resume --version 0.2.6 --json
```

Both commands are read-only. They probe the release commit, tag, workflow,
required release assets, published GitHub release, and Homebrew tap, then report
the derived state. Local state is only a cache: stale state may derive backward,
and read-only commands never rewrite it. `resume` returns the next action but does
not execute it.

State-writing automation may advance one step or record a stable failure:

```bash
npm run release:cull -- state transition --version 0.2.6 --to tagged \
  --evidence-json '{"tag":"v0.2.6"}' --json
npm run release:cull -- state fail --version 0.2.6 --code BUILD_FAILED \
  --evidence-json '{"workflowRunId":123}' --json
```

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
- `release.yml` triggers on `v*` tags (and `workflow_dispatch`).
- Disk: a full Rust rebuild is large; `cargo clean` an idle worktree's `target/`
  if low on space (see AGENTS.md).
