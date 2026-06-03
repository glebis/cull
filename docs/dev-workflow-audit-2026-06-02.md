# Cull Dev Workflow Audit - 2026-06-02

## Scope

This audit reviewed the local Cull developer workflow, not product behavior. I
checked repository instructions, package scripts, CI/release workflows, bd setup,
git hooks, test coverage entry points, release/license policy, and agent-facing
runbooks.

## Evidence Checked

- `AGENTS.md`: project rules, bd workflow, E2E instructions, landing-plane
  protocol, database safety rules.
- `package.json` and `scripts/check-ci.sh`: current Node/Rust quality gates.
- `.github/workflows/ci.yml` and `.github/workflows/release.yml`: hosted gates.
- `.github/pull_request_template.md` and issue templates: contributor workflow.
- `tests/e2e/run-e2e.sh` and `tests/e2e/smoke.py`: browser E2E workflow.
- `scripts/license-audit.mjs` and `docs/OPEN_SOURCE_AUDIT.md`: release license
  and model-policy guardrails.
- bd commands: `bd prime`, `bd hooks list`, `bd preflight --check`, `bd lint`,
  `bd stale`, `bd orphans`, `bd vc status`, and `bd version`.

## Current Strengths

- Quality checks are centralized in `scripts/check-ci.sh` and exposed through
  `npm run ci`, `npm run ci:frontend`, and `npm run ci:rust`.
- GitHub CI splits frontend and Rust work sensibly: frontend on Ubuntu, Rust on
  macOS for the Tauri/macOS surface.
- Release workflow reruns the core frontend and Rust checks before signing and
  building artifacts.
- The E2E runner is practical for this app: it starts Vite with
  `CULL_E2E_MOCK=1`, selects a free port, and reuses a server when requested.
- License and model-download policy have a real scripted audit instead of only
  prose policy.
- AGENTS.md captures several high-value safety rules: protect `cull.db`, do not
  reintroduce frontend mock imports, use Svelte 5 runes, and push before session
  completion.

## Findings

### 1. Local enforcement is mostly memory-based

`bd hooks list` reports that pre-commit, post-merge, pre-push, post-checkout,
and prepare-commit-msg hooks are not installed. `.git/hooks` only contains sample
hooks. The repo relies on agents and humans remembering the landing-plane
protocol.

Recommendation: install bd hooks and add a Cull-specific hook chain. Keep hooks
tiered: quick checks at commit time, heavier checks at push or release time.

Tracked as `imageview-2w6.1` and `imageview-2w6.9`.

### 2. bd preflight is wrong for this repository

`bd preflight --check` runs Go/Nix checks (`go test`, `golangci-lint`, `gofmt`,
`go.sum`, `default.nix`). Cull is a Tauri 2, SvelteKit 5, Rust, SQLite app.
Those checks create false failures and do not verify the real stack.

Recommendation: provide a Cull-specific preflight command with quick, full, and
release tiers. If bd preflight cannot be configured in embedded mode, document
the replacement command explicitly.

Tracked as `imageview-2w6.2`.

### 3. Release policy and release automation are not aligned

AGENTS.md and `docs/OPEN_SOURCE_AUDIT.md` require `npm run audit:licenses`
before publishing and before dependency/model policy changes. The release
workflow currently does not run that audit.

Recommendation: add the license/model-policy audit to release automation before
artifact signing/building, and run it in CI for dependency or model-policy
changes.

Tracked as `imageview-2w6.3`.

### 4. CI misses a production frontend build

`npm run ci:frontend` previously ran `npm ci`, `npm run check`, and `npm test`,
but not `npm run build`. Svelte check and Vitest catch many failures, but they
do not prove the production bundle builds.

Recommendation: add `npm run build` to the frontend CI tier or document a clear
reason to keep it out. Also decide and encode whether Rust checks should use
locked dependency resolution and strict Clippy warning policy.

Status: resolved in `imageview-2w6.4`; frontend CI now runs `npm run build`,
and Rust CI uses locked dependency resolution for Clippy/tests. Clippy warnings
remain report-only until `imageview-2w6.11` cleans up the current warning
backlog.

Tracked as `imageview-2w6.4`.

### 5. E2E coverage exists, but its trigger rule is unclear

Cull has one browser E2E smoke suite plus many documented agent-browser patterns.
The suite is not part of `npm run ci` or GitHub CI, and the PR template does not
say when to run it.

Recommendation: classify E2E as local-only, pre-push, nightly, or CI-on-change.
Name the file areas that require it, such as UI navigation, command palette,
drag/drop, preview display, and Tauri mock behavior.

Tracked as `imageview-2w6.5`.

### 6. bd issue hygiene is not strong enough

`bd lint` reports 32 open issues missing `## Acceptance Criteria`. The issue
graph has no stale or orphaned issues, which is good, but many open tasks remain
underspecified for agent execution.

Recommendation: normalize open issues with acceptance criteria and enable
creation-time validation or templates that make acceptance criteria hard to
forget.

Tracked as `imageview-2w6.6` and `imageview-2w6.10`.

### 7. Agent workflow context is useful but scattered

AGENTS.md is rich, but a Cull-specific workflow skill/runbook would make the
standard agent sequence easier to invoke: `bd prime`, safety rules, test tier
selection, E2E patterns, issue updates, and push verification.

Recommendation: create a Cull dev workflow skill or compact runbook that links
back to AGENTS.md as the source of truth.

Tracked as `imageview-2w6.7`.

### 8. Toolchain setup is underspecified

The docs say Rust 1.78+ and Node 20+, but the repo has no `.node-version`,
`.nvmrc`, or `rust-toolchain.toml`. `bd version` also warns that two bd binaries
exist on PATH (`/usr/local/bin/bd` and `/opt/homebrew/bin/bd`).

Recommendation: pin or deliberately document toolchain versions, resolve the bd
binary ambiguity, and add dependency update automation for npm, Cargo, and
GitHub Actions.

Tracked as `imageview-2w6.8`.

## Recommended Workflow Shape

### Quick local check

Use before small commits and during active iteration.

```bash
npm run check
npm test
cd src-tauri && cargo fmt --all -- --check
```

### Full pre-push check

Use before pushing code changes.

```bash
npm run ci
```

After hardening, this should also include the production frontend build or call
a dedicated `npm run preflight:full`. `npm run ci` now includes the production
frontend build.

### Release check

Use before publishing, signing, or changing dependencies/model policy.

```bash
npm run ci
npm run audit:licenses
npm run test:e2e
```

E2E may remain a separate tier if CI cannot run Chrome Beta reliably, but the
trigger rule should be explicit.

## bd Issues Generated

- `imageview-2w6`: Dev workflow hardening: hooks, preflight, bd hygiene, and
  agent runbook
- `imageview-2w6.1`: Install and document bd/git hooks for Cull workflow
- `imageview-2w6.2`: Replace generic bd preflight with Cull-specific preflight
  tiers
- `imageview-2w6.3`: Add release/license audit gate to CI and release workflow
- `imageview-2w6.4`: Add production build and stricter Rust dependency checks to
  CI
- `imageview-2w6.5`: Define E2E smoke-test tier and trigger policy
- `imageview-2w6.6`: Normalize open bd issues with acceptance criteria
- `imageview-2w6.7`: Create a Cull dev workflow skill or agent runbook
- `imageview-2w6.8`: Pin developer toolchain and dependency update automation
- `imageview-2w6.9`: Create a one-command session landing script
- `imageview-2w6.10`: Align contributor templates with actual Cull checks

## Notes

- `bd doctor` and `bd doctor --check=conventions` are not supported in the
  current embedded bd mode, so they should not be recommended as Cull readiness
  checks without a mode change.
- bd auto-export emitted a warning while creating issues because `.beads` is
  ignored by `.gitignore`, but `.beads/issues.jsonl` is tracked and now shows as
  modified. Session landing should verify both git status and `bd vc status`.
