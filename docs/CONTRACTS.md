# Contracts & Modes — Cull

How Cull makes compatibility **mechanically true** instead of merely promised —
the third formulation in our release policy (after "The Promise" and "Tiers &
Gates"). Declare a compatibility **mode** per surface, then enforce it with tests
that fail the release gate.

## Vocabulary (from schema registries)

- **Backward** — new code reads old data/messages.
- **Forward** — old code tolerates new data (ignores unknown fields).
- **Full** — both. **`_TRANSITIVE`** — checked against *all* prior versions.

Refs: [Schema-Registry compatibility](https://docs.confluent.io/platform/current/schema-registry/fundamentals/schema-evolution.html) ·
[Pact](https://docs.pact.io/) · [Consumer-Driven Contracts (Fowler)](https://martinfowler.com/articles/consumerDrivenContracts.html).

## The pattern (golden test)

1. **Freeze** an artifact produced by an (older) version — a DB, an export, a recorded API exchange.
2. **Exercise** it with current code.
3. **Assert** it still works (opens / serves / validates).

Each golden test is wired into `release.config.json → extraGate`, so a release
cannot ship if compatibility broke.

## Worked example — DB round-trip (`db`, mode `BACKWARD_TRANSITIVE`)

`src-tauri/tests/compat_golden.rs` discovers every retained `v*.db` fixture,
sorts them by schema number, opens a separate copy of each, and asserts current
code migrates it to the current schema and passes `verify_schema_invariants()`.
The guard fails when the retained set is empty, a `v*.db` name is malformed, an
entry is not a regular file, or two file names encode the same schema number.
Before migration, it reads `PRAGMA user_version` from the copied fixture and
requires it to equal the schema encoded by the file name, so a mislabeled golden
cannot silently weaken the compatibility promise.

```bash
# (re)generate the frozen fixture from current code — run once per schema bump:
cargo test --manifest-path src-tauri/Cargo.toml --features test-support \
  --test compat_golden -- --ignored regenerate_db_fixture
# the actual transitive round-trip guard (runs in the release gate):
cargo test --manifest-path src-tauri/Cargo.toml --features test-support \
  --test compat_golden
```

**Timing matters:** freeze a fixture for version `N` *while the code is still at
`N`* — commit `vN.db`, then add migration `N+1`. Freezing *after* bumping the
schema captures the new state and tests nothing. Keep every old fixture so each
released/reachable stable version stays tested forever. Historical fixtures must
be generated with the matching historical code in an isolated worktree, never
with current code and never against the live application data directory.

Retained fixture provenance:

| Schema | Producer commit | Status |
| --- | --- | --- |
| v21 | `e9bd555e24f28acd2f0f22c2abc739826b30651f` | retained historical fixture |
| v22 | `a0a577ae5f96194d2e6424833399f5fb2308eb0b` | reconstructed with that commit's ignored generator |
| v23 | none | blocked: no commit or release tag produced schema 23 |
| v24 | `84b9630361b236d65bec7c7e2ed7a17c14c7c617` | reconstructed with that commit's ignored generator |

Schema 23 and 24 were introduced atomically by the v24 producer commit. Until
P0 issue `imageview-ua01.5` establishes whether v23 was an unreachable internal
migration boundary or supplies independently verifiable producer evidence,
automatic release publication must remain disabled. A v23 fixture must not be
fabricated from current code.

## Stable static package (`exports`, mode `forward-compatible`)

`src-tauri/tests/export_compat_golden.rs` passes the frozen
`cull.static_publishing.v1` package to the production package reader exposed only
by the `test-support` feature. Its manifest includes an unknown top-level field,
proving the reader tolerates additive fields. Run the release-blocking guard with:

```bash
cargo test --manifest-path src-tauri/Cargo.toml --features test-support \
  --test export_compat_golden
```

## Add the next contract test

- [x] **DB** round-trip — the worked example above.
- [x] **Exports** — validate a frozen `cull.static_publishing.v1` package and assert unknown fields are ignored.
- [ ] **MCP** — adopt `protocolVersion`; record consumer expectations (Pact-style) and verify the provider still satisfies them; add negative-path authz tests for every tool.

Promotion `preview → stable` for a surface requires its contract tests to exist
and pass in the gate (see `docs/COMPATIBILITY.md` → 1.0 readiness gate).
