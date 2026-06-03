# Design — `release` skill: tiered compatibility & release workflow

- **Date:** 2026-06-03
- **Status:** approved (brainstorming), pending spec review → writing-plans
- **Author:** Gleb + Claude
- **Reference consumer:** [Cull](../../../) (Tauri 2 + SvelteKit 5 + Rust)
- **Companion artifacts:** `docs/release-policy-options.html` (the three formulations), Obsidian MOC `Research/Compatibility & Releases/Software Compatibility & Release Policy (MOC)`

## 1. Problem & goals

Cull (and future projects) need a release cycle that fits solo, agent-driven development and scales when a collaborator joins. A version number only means something once the **public API is declared**; for Cull that is three surfaces — **DB schema, MCP token API, export formats**. We adopt the **Tiers & Gates** formulation now and **grow into Contracts & Modes**, with a living `COMPATIBILITY.md` updated every release.

**Goals**
1. A single, professional, **distributable** skill invoked as `/release <patch|minor|major>` that orchestrates the whole ritual.
2. **Working end-to-end on Cull from day one**, but **config-driven** (no Cull specifics baked into the skill).
3. Maintain `COMPATIBILITY.md` as a tiered, deprecation-aware contract, updated each release.
4. **Teach as it runs** — inline rationale + links to standards and the Obsidian notes; `--explain` mode; a tutorial `CONTRACTS.md`.
5. Ship **one worked golden test** (DB round-trip) as the Contracts & Modes teaching template.

**Non-goals (v1)**
- Universal multi-stack support (claims "tested on Cull's stack; others via config").
- Export/MCP contract tests, MCP `protocolVersion` handshake, cargo-deny/SBOM in the gate — documented as deferred in `CONTRACTS.md`.
- Calendar-based release cadence — releases are cut on demand (`/release`).

## 2. Locked decisions

| # | Decision |
|---|---|
| D1 | One `/release` orchestrator skill (not a set). Tiers tracked as tables in `COMPATIBILITY.md`. |
| D2 | Contracts & Modes: stub + tutorial **plus one worked golden test** (DB round-trip). |
| D3 | Home: skills repo `~/ai_projects/claude-skills` (`glebis/claude-skills`); **config-driven**; Cull ships the first `release.config.json`; publishable via the `publish-skill` pipeline. |
| D4 | Learning layer: inline *why* + links + `--explain` flag; `CONTRACTS.md` is a tutorial. |
| D5 | Genericity is **by config, validated on Cull only** in v1 — honest scope in the README. |

## 3. Architecture

Two deliverable groups, two repos.

### A. The skill (in `claude-skills`)
```
skills/release/
├── SKILL.md                  # workflow instructions (the orchestrator), --explain content
├── README.md                 # what it is, honest scope, install, config reference
├── reference/
│   ├── config-schema.md      # release.config.json field reference
│   ├── compatibility-md.md   # COMPATIBILITY.md structure + how tiers/deprecations work
│   └── standards.md          # the standards map + links (Go1, k8s, schema-registry, Pact, RFCs, SemVer)
├── scripts/
│   ├── release.py            # mechanical engine: read/bump versions, draft changelog,
│   │                         #   surface-tier consistency check, tag/push; supports --dry-run
│   └── test_release.py       # unit tests for the engine (pure functions)
└── templates/
    ├── COMPATIBILITY.md.tmpl
    ├── CONTRACTS.md.tmpl
    └── release.config.json.tmpl
```
The **SKILL.md** is the human/agent-facing orchestration (steps, prompts, gates, teaching lines). The **script** holds deterministic, testable mechanics. Separation keeps the skill reasoning-light and the risky mutations unit-tested.

### B. Cull adoption (in `cull`)
```
release.config.json           # Cull's config (version files, gate cmd, surfaces)
docs/COMPATIBILITY.md         # generated from template, then Cull-specific content
docs/CONTRACTS.md             # tutorial (from template)
docs/RELEASING.md             # short runbook pointing at /release
src-tauri/tests/compat_golden.rs        # DB round-trip golden test (worked example)
src-tauri/tests/fixtures/db/v21.db      # frozen fixture (+ future versions)
```

## 4. `release.config.json` schema

```jsonc
{
  "versionFiles": [                       // files whose version must stay in sync
    { "path": "package.json", "kind": "json", "pointer": "/version" },
    { "path": "src-tauri/tauri.conf.json", "kind": "json", "pointer": "/version" },
    { "path": "src-tauri/Cargo.toml", "kind": "toml", "key": "package.version" }
  ],
  "lockfiles": ["src-tauri/Cargo.lock"],  // refreshed after bump
  "gate": "npm run preflight -- release", // the readiness command (must exit 0)
  "extraGate": ["cargo test --manifest-path src-tauri/Cargo.toml --test compat_golden"],
  "changelog": { "path": "CHANGELOG.md", "style": "keep-a-changelog", "from": "conventional-commits" },
  "compatibility": { "path": "docs/COMPATIBILITY.md" },
  "surfaces": [                            // the declared public API
    { "id": "db",      "name": "Database schema",  "tier": "stable",  "mode": "BACKWARD_TRANSITIVE" },
    { "id": "mcp",     "name": "MCP token API",    "tier": "preview", "mode": "unversioned" },
    { "id": "exports", "name": "Export formats",   "tier": "stable",  "mode": "forward-compatible" }
  ],
  "releaseBranch": "main",                 // branch releases are cut from
  "worktree": "../cull-main-landing",      // optional: path where releaseBranch is checked out (Cull uses a worktree)
  "tag": { "prefix": "v", "push": true },  // tag → release.yml
  "issueTracker": { "kind": "bd", "binEnv": "BD_BIN" }   // optional: release-notes source
}
```
Tiers: `experimental | preview | stable`. Modes mirror [[Schema Registry Compatibility Modes]].

## 5. `/release <patch|minor|major>` flow

Step 0 is run once at setup: **verify `release.yml` triggers on `v*` tags** (it exists but has never fired — no tags yet).

1. **Preconditions** — the configured `releaseBranch` (default `main`, in `worktree` if set) is checked out, clean, and synced with `origin`. Cull's `main` lives in the `cull-main-landing` worktree, so the skill operates there. Abort with a clear message otherwise.
2. **Compute version** — read all `versionFiles`, assert they currently agree, bump per arg, show `old → new`. *(teach: SemVer 0.x semantics)*
3. **Readiness gate** — run `gate` + `extraGate` (incl. the DB golden test). Block on any failure. *(teach: this is a Production-Readiness Review)* CVE/SBOM listed as a TODO checkbox, not enforced yet.
4. **Changelog** — draft a `Keep a Changelog` section from conventional commits since the last tag; open for hand-curation. *(teach: link [[Keep a Changelog]])*
5. **Compatibility review** — print the surfaces + deprecations tables and ask:
   - Did any surface change tier?
   - New deprecations? (record item / deprecated-in / removable-in / replacement)
   - **Does this change break a `stable` surface?** If yes → the skill **refuses anything below `major`** (the Tiers & Gates enforcement). *(teach: link [[Kubernetes API Deprecation Policy]])*
   Update `COMPATIBILITY.md`, stamp version + date.
6. **Bump & commit** — write version files + refresh lockfiles; commit `chore(release): vX.Y.Z` including CHANGELOG + COMPATIBILITY.
7. **Tag & push** — `git tag vX.Y.Z`; push tag (→ `release.yml` signed artifacts) and `main`.
8. **Report** — tag, release-workflow link, and bd issues closed since the last tag (release-notes source).

`--dry-run` runs 1–5 and prints the planned commit/tag without mutating. `--explain` expands every *teach* line into a short lesson.

## 6. `COMPATIBILITY.md` structure

```
# Compatibility Policy
<prose promise — Go 1 style: what 1.x guarantees, what forces a 2.0>

## Surfaces
| Surface | Tier | Since | Mode | Notes |
| Database schema | stable | 0.1.0 | BACKWARD_TRANSITIVE | migrations additive-only |
| MCP token API | preview | — | unversioned | no protocolVersion yet → may change |
| Export formats | stable | 0.1.0 | forward-compatible | unknown fields ignored |

## Deprecations
| Item | Deprecated in | Removable in | Replacement |

## 1.0 readiness gate
- [ ] DB: golden round-trip tests for every prior 1.x schema version
- [ ] MCP: protocolVersion handshake + deprecation policy (or stays `preview`)
- [ ] Exports: every artifact versioned + must-ignore-unknown + golden serve test
- [ ] COMPATIBILITY policy + deprecation process written
```
Updated every release in step 5.

## 7. Worked golden test (Contracts & Modes teaching template)

`src-tauri/tests/compat_golden.rs`: open `tests/fixtures/db/v21.db` via `Database::open`, assert it migrates to `CURRENT_SCHEMA_VERSION` and `verify_schema_invariants()` passes. Heavily commented to show the *pattern* (frozen fixture → exercise → assert compatibility) that export/MCP contract tests will copy. Fixture generated by opening a fresh DB and freezing it; new fixtures added as schema advances.

## 8. Educational layer
- Every step emits one *why* line + a link to the standard and the Obsidian note.
- `--explain` expands these into short lessons inline.
- `reference/standards.md` and `docs/CONTRACTS.md` are tutorials, the latter walking through adding the next contract test.
- Mirrors the Obsidian MOC so terminal ↔ vault reinforce each other.

## 9. Testing the skill itself (professional bar)
- `scripts/release.py` mechanics are **pure functions** (version parse/bump, changelog assembly from a commit list, surface-tier consistency check) with `test_release.py` unit tests.
- `--dry-run` covered by a test that asserts no file mutation and a correct plan.
- Manual acceptance: a real `/release patch` (or a `v0.2.0`) on Cull as the day-one proof.

## 10. Standards mapping
Promise → Go 1 · tiers/deprecation → Kubernetes + SRE PRR · modes/tests → Schema Registry + Pact · changelog → Keep a Changelog · versions → SemVer · wire deprecation → RFC 9745/8594 · MCP → protocolVersion. (See `reference/standards.md` and the Obsidian MOC.)

## 11. Deferred / future
- Export golden test; MCP `protocolVersion` handshake + Pact-style negative tests.
- cargo-deny / cargo-audit / cargo-cyclonedx (SBOM) in the gate (closes the P2 CVE gap).
- True multi-stack genericity (Node-only, Python, etc.) once a second consumer exists.

## 12. Risks
- **Mutating skill** (versions, tags, push) — mitigated by `--dry-run`, unit tests, and preconditions.
- **release.yml trigger unknown** — verified in step 0 before first real tag.
- **bd jsonl churn across branches** (seen this session) — release commits the beads state explicitly; documented in AGENTS.md.
- **Scope creep into multi-stack** — explicitly out of v1.

## 13. Open questions
None blocking. (cargo-deny/SBOM timing and a second consumer are post-v1.)
