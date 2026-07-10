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

The compatibility review is mandatory. Any breaking change to a `stable` surface
requires a major bump.

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
CULL_PREFLIGHT_SKIP_E2E=1 npm run preflight -- release                  # gate
cargo test --manifest-path src-tauri/Cargo.toml --features test-support --test compat_golden
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

Use `-- --allow-local-dev` for local ad-hoc builds where notarization checks are
expected to fail.

## Notes

- `main` lives in the `cull-main-landing` worktree; release from there.
- Releases are **on demand** (ship-when-meaningful), not on a calendar.
- `release.yml` triggers on `v*` tags (and `workflow_dispatch`).
- Disk: a full Rust rebuild is large; `cargo clean` an idle worktree's `target/`
  if low on space (see AGENTS.md).
