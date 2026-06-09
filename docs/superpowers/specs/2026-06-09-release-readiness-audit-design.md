# Release Readiness Audit — Design

**Date:** 2026-06-09
**Status:** Codex-reviewed (APPROVE WITH CHANGES, incorporated) — awaiting user approval
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

**Inventory completeness gate:** before any Phase 1 lens consumes the
inventory, the completeness critic cross-checks it against routes, the
command-palette registry, keyboard-shortcut map, and the MCP tool list; the
lenses start only once gaps are filled.

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
   `NOTICE`, `package.json`, `src-tauri/Cargo.toml`, README, About dialog
   (include uncommitted drift in any of these files); `npm run
   audit:licenses` result; supply-chain: `npm audit`, `cargo audit`,
   lockfiles present and committed; repo-going-public cleanup (`.beads`,
   personal references, docs leakage); `SECURITY.md` + release-notes
   presence; macOS trust chain on the built artifact — `codesign --verify
   --deep --strict`, `spctl --assess --type execute`, `xcrun stapler
   validate`; upgrade path — a `cull.db` from a previous app version opens
   and migrates cleanly (leverage existing `migration_safety_tests`);
   README + onboarding docs adequacy for a stranger.
4. **UX — stranger's first 10 minutes** — empty states, first-run import
   flow, error states, keyboard discoverability, visual consistency against
   the Tokyo Night token system, terminology coherence. Method: drive the
   real UI via agent-browser against `localhost:1420` where possible
   (per `AGENTS.md` E2E conventions), code-trace where not.
5. **Performance & scale** — thumbnail pipeline, embedding job throughput,
   query patterns, startup time, memory behavior in loupe. Pass/fail
   thresholds at a 10k-image library (findings must state measured vs.
   threshold):
   - cold start to interactive grid < 3 s
   - grid scroll: thumbnail load p95 < 200 ms, no unbounded memory growth
   - smart-collection / NL query p95 < 500 ms
   - CLIP find-similar < 2 s
   - loupe open < 1 s full-res (cached thumbnail < 300 ms)
   - resident memory after browsing the full library < 1.5 GB

   A 100k corpus is explicitly out of scope for this release (deferred).

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
  extracting one proof plugin.
- **Kill criteria (hard):** if runtime + registry + proof plugin sizes above
  8 hours, Phase 4 must propose the smallest shippable subset instead; if
  even a registry-fetch-plus-manual-install subset sizes above 4 hours,
  Track C ships as a design doc only (`docs/plugins-design.md`) and the
  runtime moves to v1.1. The plan must never let Track C borrow time from
  Track A.

### Phase 5 — Synthesis

Outputs to `docs/release-audit-2026-06-09/`:

- `report.md` — verified findings by severity, identity argument summary,
  triage table, plugin spec.
- `decision-sheet.md` — machine-parseable markdown table, one row per taste
  call (identity, every non-KEEP triage verdict, plugin scope). Columns:
  `item_id` (unique), `type` (`identity` | `triage` | `plugin-scope`),
  `audit_recommendation`, `user_decision` (pre-filled with the
  recommendation; the user edits this column), `override_reason` (required
  when decision ≠ recommendation), `evidence_ids` (required unless the
  source finding is marked not-runtime-verified). Stage 2 consumes only
  this table.

A completeness critic reviews the report before it is presented: any lens
that silently narrowed scope, any blocker without verification, any inventory
item missing a triage verdict.

## Decision gate

The user edits the `user_decision` column of `decision-sheet.md`
(≈15 minutes). Stage 2 consumes only the decided sheet. Nothing in Stage 2
runs before this.

**Override rework rule:** if the user overrides the identity verdict, only
Phase 3 (feature triage) re-runs against the chosen identity — the
inventory, audit findings, and plugin mechanism analysis stand. Individual
triage overrides require no re-run; Stage 2 takes them as-is.

## Time budgets

Wall-clock budgets so drift is visible (agents run in parallel within a
phase):

| Phase | Budget |
|---|---|
| Stage 1 total | ≤ half a day |
| Phase 0 inventory + completeness gate | 45 min |
| Phase 1 lenses + blocker verification | 90 min |
| Phases 2–4 (identity, triage, plugin spec) | 90 min |
| Phase 5 synthesis | 30 min |
| Decision gate (user) | 15–30 min |
| Stage 2 plan synthesis | 45 min |

That leaves ≥ 1.5 days of the 2-day budget for executing Track A and B
issues, which is the point of the exercise.

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

Plan-level gates: `npm run preflight:release` green; `npm audit` and
`cargo audit` reviewed (no unaddressed high/critical advisories);
`codesign --verify --deep --strict`, `spctl --assess --type execute`, and
`xcrun stapler validate` pass on the shipped DMG; DMG install test on a
clean machine (or fresh user account); soft-launch checklist (download link,
two-line pitch, feedback channel = GitHub issues + direct chat).

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

## Deferred by budget (reviewed and consciously rejected for this release)

Raised in the codex review; deferred with rationale so later "was this
considered?" questions have an answer:

- **Reproducible builds / artifact provenance** — post-launch; signing +
  notarization + checksum on the release asset is the v1 integrity story.
- **Crash reporting / telemetry infrastructure** — conflicts with Cull's
  local-first posture; soft-launch feedback channel is GitHub issues +
  direct chat.
- **Full accessibility (WCAG) audit** — keyboard-only end-to-end flow IS in
  scope via the UX lens; contrast/screen-reader hardening is post-launch.
- **CODEOWNERS / branch protection** — ceremony for a solo-maintainer repo
  with no external contributors yet; revisit at first outside PR.
- **100k-image performance benchmark** — cannot be built and run inside the
  budget; 10k thresholds above are the release bar.

## Review

- Reviewed by codex (or `feature-dev:code-reviewer` as codex-substitute if
  codex CLI is unavailable) before Stage 1 runs; review outcome recorded
  below.

### Review log

- **2026-06-09 — codex (read-only sandbox), round 1: REWORK.** Seven
  required changes: macOS trust-chain verification, repo governance,
  supply-chain audits, hard performance thresholds, data-migration checks,
  plugin-track kill criteria, machine-parseable decision sheet. Plus
  ambiguity findings: NOTICE foreknowledge vs. fresh-eyes, missing per-phase
  budgets, no inventory completeness gate, undefined override-rework rule.
- **2026-06-09 — codex, round 2 (negotiated): APPROVE WITH CHANGES.**
  Accepted the triaged subset (full: trust chain, supply chain, kill
  criteria, decision-sheet schema; reduced: governance → SECURITY.md +
  release notes, thresholds → 10k only, migration → upgrade-path check).
  Rejections (reproducible builds, telemetry, full WCAG, CODEOWNERS,
  100k benchmark) recorded in "Deferred by budget". Codex's three closing
  conditions — deferred-by-budget section, locked 10k numbers, `item_id`
  uniqueness + `evidence_ids` optional only when not runtime-verified —
  are all incorporated above.
