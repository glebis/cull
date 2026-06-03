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

`src-tauri/tests/compat_golden.rs` opens a frozen `cull.db` fixture and asserts
current code migrates it to the current schema and passes
`verify_schema_invariants()`. Generate/refresh the fixture with the ignored
generator test, then run the guard:

```bash
# (re)generate the frozen fixture from current code — run once per schema bump:
cargo test --manifest-path src-tauri/Cargo.toml --features test-support \
  --test compat_golden -- --ignored regenerate_db_fixture
# the actual round-trip guard (runs in the release gate):
cargo test --manifest-path src-tauri/Cargo.toml --features test-support \
  --test compat_golden db_fixture_opens_and_satisfies_invariants
```

Freeze a new fixture (`v22.db`, …) whenever `CURRENT_SCHEMA_VERSION` advances;
keep older fixtures so each is tested forever.

## Add the next contract test

- [x] **DB** round-trip — the worked example above.
- [ ] **Exports** — serve a frozen `cull.static_publishing.v1` package; assert it renders and unknown fields are ignored.
- [ ] **MCP** — adopt `protocolVersion`; record consumer expectations (Pact-style) and verify the provider still satisfies them; add negative-path authz tests for every tool.

Promotion `preview → stable` for a surface requires its contract tests to exist
and pass in the gate (see `docs/COMPATIBILITY.md` → 1.0 readiness gate).
