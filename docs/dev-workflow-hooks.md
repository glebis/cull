# Dev Workflow Hooks

Cull uses bd git hooks plus a tracked Cull hook chain.

Install or refresh local hooks:

```bash
npm run hooks:install
```

The installer runs `bd hooks install --chain`, then appends Cull-managed sections
to `.git/hooks/pre-commit` and `.git/hooks/pre-push`. Both bd and Cull sections
use markers, so reinstalling updates managed content while preserving user hook
content outside those markers.

## Hook Tiers

Pre-commit runs the hook tier:

```bash
npm run preflight:hook
```

Hook checks are intentionally cheap:

- shell syntax for tracked workflow scripts
- no `tauri-mock` imports from app source

Quick checks add the frontend readiness checks:

```bash
npm run preflight:quick
```

Pre-push runs the full tier:

```bash
npm run preflight:full
```

Full checks run hook checks, frontend checks/tests, and Rust fmt/clippy/tests:

```bash
npm run preflight:full
```

Release checks are explicit and heavier:

```bash
npm run preflight:release
```

Release checks run the full tier, license/model policy audit, and production
build.

## Overrides

Use overrides sparingly and mention them in handoff when used:

- `CULL_HOOK_SKIP_CHECKS=1`: skip Cull hook checks for the current git command.
- `CULL_PRE_COMMIT_TIER=<tier>`: override the pre-commit tier.
- `CULL_PRE_PUSH_TIER=<tier>`: override the pre-push tier.
- `CULL_PREFLIGHT_DRY_RUN=1`: print preflight commands without running them.
- `CULL_PREFLIGHT_SKIP_E2E=1`: skip E2E inside the release tier.

bd hook shims still run separately unless bd itself is disabled.

## Landing A Session

Use the landing script after changes are committed:

```bash
npm run land
```

It requires a clean worktree, runs the full preflight tier, reports bd
version-control state, uses `bd sync` when available, falls back to bd vc/Dolt
status when unavailable, pulls with rebase, pushes, and prints final git/bd
status.

Useful variants:

```bash
npm run land -- --release
npm run land -- --skip-checks
npm run land -- --dry-run
```

The script does not delete, reset, or clean user files.
