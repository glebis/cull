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

use std::collections::HashSet;
use std::path::{Path, PathBuf};

fn fixture_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/db")
}

fn retained_fixture_paths(dir: &Path) -> Result<Vec<(i64, PathBuf)>, String> {
    let mut fixtures = Vec::new();
    let mut schemas = HashSet::new();
    for entry in std::fs::read_dir(dir).map_err(|error| error.to_string())? {
        let path = entry.map_err(|error| error.to_string())?.path();
        let Some(file_name) = path.file_name().and_then(|name| name.to_str()) else {
            continue;
        };
        let Some(schema) = file_name
            .strip_prefix('v')
            .and_then(|name| name.strip_suffix(".db"))
            .and_then(|value| value.parse::<i64>().ok())
        else {
            continue;
        };
        if !schemas.insert(schema) {
            return Err(format!("duplicate retained DB schema v{schema}"));
        }
        fixtures.push((schema, path));
    }
    fixtures.sort_by_key(|(schema, _)| *schema);
    if fixtures.is_empty() {
        return Err("no retained v*.db compatibility fixtures found".to_string());
    }
    Ok(fixtures)
}

/// The compatibility guard wired into the release gate.
#[test]
fn retained_db_fixtures_open_and_satisfy_invariants() {
    let fixtures = retained_fixture_paths(&fixture_dir()).expect("discover retained DB fixtures");
    for (schema, src) in fixtures {
        // Work on a separate copy so the test never mutates a committed fixture.
        let tmp = tempfile::tempdir().unwrap();
        let work = tmp.path().join(format!("v{schema}.db"));
        std::fs::copy(&src, &work).expect("copy fixture");

        let db = cull_lib::test_support::Database::open(&work).unwrap_or_else(|error| {
            panic!("frozen DB schema v{schema} must open under current code: {error}")
        });
        db.verify_schema_invariants_for_test()
            .unwrap_or_else(|error| {
                panic!("schema invariants must hold after migrating v{schema}: {error}")
            });
    }
}

/// One-shot helper to (re)create the frozen fixture from current code.
/// Ignored by default; run explicitly when the schema version changes.
#[test]
#[ignore]
fn regenerate_db_fixture() {
    let out = fixture_dir().join("v25.db");
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

#[test]
fn retained_fixture_discovery_is_sorted() {
    let tmp = tempfile::tempdir().unwrap();
    std::fs::write(tmp.path().join("v24.db"), []).unwrap();
    std::fs::write(tmp.path().join("v21.db"), []).unwrap();
    std::fs::write(tmp.path().join("notes.txt"), []).unwrap();

    let schemas = retained_fixture_paths(tmp.path())
        .unwrap()
        .into_iter()
        .map(|(schema, _)| schema)
        .collect::<Vec<_>>();
    assert_eq!(schemas, vec![21, 24]);
}

#[test]
fn retained_fixture_discovery_rejects_empty_sets() {
    let tmp = tempfile::tempdir().unwrap();
    assert!(retained_fixture_paths(tmp.path())
        .unwrap_err()
        .contains("no retained"));
}

#[test]
fn retained_fixture_discovery_rejects_duplicate_schema_numbers() {
    let tmp = tempfile::tempdir().unwrap();
    std::fs::write(tmp.path().join("v21.db"), []).unwrap();
    std::fs::write(tmp.path().join("v021.db"), []).unwrap();
    assert!(retained_fixture_paths(tmp.path())
        .unwrap_err()
        .contains("duplicate retained DB schema v21"));
}
