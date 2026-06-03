//! Contracts & Modes — worked example (the template for export/MCP contract tests).
//!
//! Pattern (see docs/CONTRACTS.md):
//!   1. FREEZE an artifact produced by an (older) version — here a `cull.db`.
//!   2. EXERCISE it with current code — `Database::open` runs migrations.
//!   3. ASSERT it still works — schema invariants hold (backward compatibility).
//!
//! Surface: `db`, declared mode `BACKWARD_TRANSITIVE` in release.config.json.
//! Requires the `test-support` feature:
//!   cargo test --features test-support --test compat_golden
//!
//! Freeze a fixture for the CURRENT schema, then add it to the suite. Crucially,
//! freeze BEFORE advancing CURRENT_SCHEMA_VERSION: commit `v<N>.db` while the code
//! is still at v<N>, *then* add migration v<N+1>. That way each committed fixture
//! is a genuine OLDER-version DB, and the guard proves new code opens it. Keep
//! every old fixture so each version stays tested forever.
//!   cargo test --features test-support --test compat_golden -- --ignored regenerate_db_fixture

// Requires the `test-support` feature (for the gated `test_support::Database`
// re-export). Without it this file compiles to nothing, so a plain
// `cargo test --all-targets` does not fail to build.
#![cfg(feature = "test-support")]

use std::path::{Path, PathBuf};

fn fixture_path() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/db/v21.db")
}

/// The compatibility guard wired into the release gate.
#[test]
fn db_fixture_opens_and_satisfies_invariants() {
    let src = fixture_path();
    assert!(
        src.exists(),
        "missing frozen DB fixture {} — generate it with: \
         cargo test --features test-support --test compat_golden -- --ignored regenerate_db_fixture",
        src.display()
    );

    // Work on a copy so the test never mutates the committed fixture.
    let tmp = tempfile::tempdir().unwrap();
    let work = tmp.path().join("cull.db");
    std::fs::copy(&src, &work).expect("copy fixture");

    // Opening an older DB under current code must migrate it cleanly...
    let db = cull_lib::test_support::Database::open(&work)
        .expect("a frozen older DB must open cleanly under current code");
    // ...and the resulting schema must satisfy all invariants.
    db.verify_schema_invariants_for_test()
        .expect("schema invariants must hold after migrating an older DB");
}

/// One-shot helper to (re)create the frozen fixture from current code.
/// Ignored by default; run explicitly when the schema version changes.
#[test]
#[ignore]
fn regenerate_db_fixture() {
    let out = fixture_path();
    std::fs::create_dir_all(out.parent().unwrap()).unwrap();
    let _ = std::fs::remove_file(&out);
    // Opening a fresh path runs the full migration chain at the current schema.
    let db = cull_lib::test_support::Database::open(&out).expect("open fresh db");
    drop(db);
    assert!(
        out.exists(),
        "fixture should have been written to {}",
        out.display()
    );
}
