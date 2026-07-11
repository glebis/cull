# Schema 23 Reconstruction Report

## Result

`src-tauri/tests/fixtures/db/v23.db` is the exact persistent boundary produced
after historical migration 23 commits and before migration 24 starts. Current
code opens the fixture, migrates it to the current schema, and satisfies the
database invariants.

## Producer and harness

- Producer commit: `84b9630361b236d65bec7c7e2ed7a17c14c7c617`
- Historical worktree: detached, temporary, and removed after fixture extraction
- Generator: the producer commit's ignored `regenerate_db_fixture` test
- Fault injection: one source line added to that worktree only:

```diff
 self.run_migration_step(23, "media_assets", || self.migrate_media_catalog())?;
+std::process::exit(23);
 self.run_migration_step(24, "catalog_schema", || self.migrate_catalog_schema())?;
```

The generator process exited with status 23. No current migration source or SQL
was changed, no SQL was hand-authored to construct the fixture, and the live Cull
database was not accessed.

## Frozen artifact evidence

- Path: `src-tauri/tests/fixtures/db/v23.db`
- SHA-256: `d645eeaf688027d8abcc5a36e07dcd7b9ca497788876d49019a1f4f4a3a17368`
- Size: 610304 bytes
- `PRAGMA user_version`: 23
- `PRAGMA integrity_check`: `ok`
- Latest `schema_migrations` row: `23 | media_assets | 85df75f2b29a4877`
- `schema_migration_steps` version 23: `media_assets | succeeded`
- Migration version 24 rows: 0
- Migration-23 tables present: `media_assets`, `media_files`, `pdf_pages`
- `catalog_%` table count: 0, showing migration 24 had not begun

## TDD and verification

RED was established by enumerating the required retained schemas in
`retained_db_fixtures_open_and_satisfy_invariants`: the guard failed because the
discovered set was `[21, 22, 24]` instead of `[21, 22, 23, 24]`.

After adding only the reconstructed fixture, the focused guard passed and
current `Database::open` migrated the copied fixture successfully. Full
verification commands and results are recorded in the commit handoff; review is
still required before closing P0 `imageview-ua01.5`.
