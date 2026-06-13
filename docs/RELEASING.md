# Releasing Cull

Cull is released with the **`release` skill** (config-driven; lives in
`glebis/claude-skills`). Config: `release.config.json`. Policy:
`docs/COMPATIBILITY.md`. Contract tests: `docs/CONTRACTS.md`.

## Normal release

```
/release <patch|minor|major>
```

The skill: checks preconditions → bumps the 3 version files → runs the readiness
gate (`npm run preflight -- release` + the golden contract tests) → drafts the
`CHANGELOG.md` section from conventional commits → walks the **compatibility
review** (tiers / deprecations; a breaking change to a `stable` surface forces a
`major`) → commits `chore(release): vX.Y.Z` → tags `vX.Y.Z` (→ `release.yml`
signed artifacts) → reports closed bd issues since the last tag.

Run `/release --dry-run <kind>` first if unsure — it previews without mutating.

## By hand (no skill)

```bash
python3 ../claude-skills/skills/release/scripts/release.py plan minor   # preview
CULL_PREFLIGHT_SKIP_E2E=1 npm run preflight -- release                  # gate
cargo test --manifest-path src-tauri/Cargo.toml --features test-support --test compat_golden
$EDITOR CHANGELOG.md docs/COMPATIBILITY.md                              # curate + stamp
python3 ../claude-skills/skills/release/scripts/release.py bump minor   # write versions
git commit -am "chore(release): vX.Y.Z" && git tag vX.Y.Z && git push --follow-tags
```

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
