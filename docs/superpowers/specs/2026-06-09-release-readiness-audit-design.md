# Release Readiness Audit — Design

**Date:** 2026-06-09
**Status:** Draft for review
**Owner:** Gleb Kalinin

## Goal

Audit Cull for public release within ~2 days, covering both technical quality
and UX/product focus, and produce a sequenced release plan. The audit decides
which product identity to be bullish about, which features to cut or demote,
and which to extract into plugins backed by a minimal plugin runtime and a
JSON registry on GitHub.

## Locked decisions

| Decision | Choice |
|---|---|
| Executor | Claude Code multi-agent workflow in this repo |
| Prior audit (`docs/cull-audit-2026-06-03.md`) | Ignored — fresh eyes; agents must NOT read it |
| Release definition | Public GitHub repo + installable signed macOS app + soft launch to friends |
| Product identity | Audit argues and recommends; user decides at the gate |
| Plugin scope in v1 | Minimal runtime + JSON registry + one proof plugin, on a non-blocking track |
| Plugin mechanism | Audit proposes based on actual extraction candidates |
| Sacred (cannot cut or extract) | MCP server, CLIP semantic search, smart collections |

## Architecture: two stages with a human decision gate

```
Stage 1 (workflow): inventory → parallel audits → identity panel →
  feature triage → plugin spec → synthesis + decision sheet
        ↓
Decision gate: user approves/overrides identity, triage verdicts, plugin scope
        ↓
Stage 2 (workflow): sequenced 2-day plan as bd issues in three tracks
```

Rationale: identity and cut/plugin verdicts are taste decisions; packaging and
license alignment are fact-finding. Agents are authoritative on the second,
advisory on the first. The gate keeps the taste calls with the user before any
plan is generated, so no downstream work is built on a verdict the user would
overturn.

## Stage 1 — `release-readiness-audit` workflow

### Phase 0 — Inventory (parallel, no opinions)

Two Explore-type agents produce a structured inventory that all later phases
must cite:

- **UI inventory**: every user-facing feature — routes, panels, modals,
  command-palette commands, keyboard shortcuts, import/export/publish flows.
- **Agent-surface inventory**: every MCP tool (~60), grouped by capability.

Per item: `{ id, name, surface: "ui" | "mcp" | "both", entry_points: [files],
approx_loc, test_coverage: "tested" | "partial" | "none", dependencies,
coupling_to_core: "low" | "medium" | "high", notes }`.

### Phase 1 — Parallel audit lenses

Five auditors run concurrently. Each is explicitly forbidden from reading
`docs/cull-audit-2026-06-03.md` and the bd epics derived from it
(`imageview-hqf`, `imageview-dtj`, `imageview-9fz`).

1. **Code quality** — Rust: `unwrap()`/panic paths, mutex poisoning,
   migration safety, error propagation in Tauri commands. Svelte 5: the
   known `$effect`-reads-state-it-mutates trap class, store misuse, dead
   components. IPC: error handling across the boundary.
2. **Security & privacy** — secrets or personal paths in repo history and
   working tree, path traversal in import/export, MCP token lifecycle,
   Tauri capabilities/CSP config, all outbound network calls (model
   downloads), audit-log coverage.
3. **Release hygiene** — license metadata alignment across `LICENSE.md`,
   `NOTICE` (currently has uncommitted changes), `package.json`,
   `src-tauri/Cargo.toml`, README, About dialog; `npm run audit:licenses`
   result; repo-going-public cleanup (`.beads`, personal references, docs
   leakage); signing/notarization/DMG packaging; README + onboarding docs
   adequacy for a stranger.
4. **UX — stranger's first 10 minutes** — empty states, first-run import
   flow, error states, keyboard discoverability, visual consistency against
   the Tokyo Night token system, terminology coherence. Method: drive the
   real UI via agent-browser against `localhost:1420` where possible
   (per `AGENTS.md` E2E conventions), code-trace where not.
5. **Performance & scale** — thumbnail pipeline, embedding job throughput,
   query patterns at 10k+ images, startup time, memory behavior in loupe.

**Finding schema:** `{ id, lens, title, severity: "release-blocker" |
"pre-launch" | "post-launch", evidence: [file:line refs or repro steps],
proposed_fix, est_effort: "S" | "M" | "L" }`.

**Adversarial verification:** every `release-blocker` is handed to a second
agent prompted to refute it (wrong, already handled, not reachable,
overstated). Only findings that survive keep blocker severity; refuted ones
are downgraded or dropped with the refutation recorded.

### Phase 2 — Identity panel

Three advocate agents each argue one identity at full strength, citing only
Phase 0/1 evidence (feature strength, test coverage, differentiation):

- **Fast culling tool** — keyboard-driven triage of large AI-generation batches.
- **AI art library** — browse, semantic search, collections, generation metadata.
- **Agent-native image tool** — the MCP surface as the headline differentiator.

A judge agent scores the three arguments (differentiation, evidence of code
maturity, size of audience for soft launch, fit with the 2-day budget) and
recommends one identity, explicitly preserving the strongest claims of the
losing arguments as "keep anyway" notes.

### Phase 3 — Feature triage

One agent (with the inventory, findings, and identity verdict in context)
assigns every inventory item a verdict:

| Verdict | Meaning |
|---|---|
| CORE | Bullish — gets polish effort in the plan |
| KEEP | Ships as-is, no investment |
| DEMOTE | Hidden behind settings/flag for v1 |
| PLUGIN | Extraction candidate — include effort estimate |
| CUT | Removed before release |

Constraints: MCP server, CLIP search, smart collections may not receive
PLUGIN or CUT. Every verdict must cite inventory evidence (coupling,
maintenance cost, UX surface cost). Verdicts are recommendations — the user
overrides at the gate.

### Phase 4 — Plugin mechanism + store spec

Input: the PLUGIN-verdict features. The agent works backwards from them:
what is the simplest runtime that can host these specific candidates?

Must produce:

- **Mechanism choice** with rationale: frontend JS modules vs. MCP tool packs
  vs. external process plugins (or a hybrid), chosen to fit the actual
  candidates, Tauri 2 constraints, and the 2-day budget.
- **Manifest format** (per plugin): `{ id, name, version, description, entry,
  permissions, minAppVersion, checksum, repo }`.
- **Registry v1**: a single `registry.json` in a public `cull-plugins` GitHub
  repo, fetched over HTTPS, checksum-verified, no backend or accounts.
  Include the migration path (static JSON → signed index / API later) so v1
  choices don't block it.
- **Security model**: what plugins can touch; how permissions are surfaced to
  the user; consistency with the existing MCP token/audit-log posture.
- **Honest sizing** in hours for: runtime, registry fetch + install UX, and
  extracting one proof plugin. If the total cannot fit the non-blocking
  track, say so and propose the smallest shippable subset.

### Phase 5 — Synthesis

Outputs to `docs/release-audit-2026-06-09/`:

- `report.md` — verified findings by severity, identity argument summary,
  triage table, plugin spec.
- `decision-sheet.md` — one page: each taste call (identity, every non-KEEP
  triage verdict, plugin scope) as a checkbox with the audit's
  recommendation pre-marked, for the user to approve or override.

A completeness critic reviews the report before it is presented: any lens
that silently narrowed scope, any blocker without verification, any inventory
item missing a triage verdict.

## Decision gate

The user edits `decision-sheet.md` (≈15 minutes). Stage 2 consumes only the
decided sheet. Nothing in Stage 2 runs before this.

## Stage 2 — `release-plan-synthesis` workflow

Input: decided sheet + report. Output: one bd epic with child issues, each
with Jobs-To-Be-Done framing and acceptance criteria (matching existing bd
conventions), sequenced into three tracks:

- **Track A — release-blocking** (Day 1 → Day 2 morning): verified blockers,
  release-hygiene fixes, approved cuts/demotions, packaging + notarization,
  README. Release does not ship until Track A is empty.
- **Track B — bullish polish** (Day 2): the CORE features' rough edges from
  the UX lens.
- **Track C — plugin runtime + registry + one proof plugin**: explicitly
  marked *slips without blocking release*; if Day 2 runs hot it becomes v1.1.

Plan-level gates: `npm run preflight:release` green; DMG install test on a
clean machine (or fresh user account); soft-launch checklist (download link,
two-line pitch, feedback channel).

## Risks

- **Plugin runtime in 2 days**: highest-risk item; mitigated by Track C
  isolation and Phase 4's "smallest shippable subset" requirement.
- **Identity verdict feels wrong**: mitigated by the gate — the user can
  override before any plan exists.
- **UX lens can't drive the real app** (dev server not running, CDP
  unavailable): falls back to code-trace + screenshots; findings marked
  "not runtime-verified".
- **Fresh-eyes duplication**: the new audit may rediscover known issues;
  acceptable cost of avoiding anchoring, and duplicates merge naturally when
  bd issues are created in Stage 2.

## Out of scope

- Show HN / public launch-post readiness (soft launch only).
- Windows/Linux packaging.
- Plugin store backend, accounts, payments, or review pipeline (design note
  on migration path only).
- Executing fixes — this design covers the audit and the plan; execution is
  governed by the resulting bd issues.

## Review

- Reviewed by codex (or `feature-dev:code-reviewer` as codex-substitute if
  codex CLI is unavailable) before Stage 1 runs; review outcome recorded
  below.

### Review log

- _Pending._
