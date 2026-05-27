# Migration Recovery

Cull records schema migration progress in two places:

- `schema_migrations`: committed migrations and checksums.
- `schema_migration_steps`: per-step `started`, `succeeded`, or `failed` recovery state.

Each migration step runs in a SQLite transaction. On success, the step records its
`schema_migrations` row and updates `PRAGMA user_version` in the same transaction.
On failure, the transaction is rolled back and `schema_migration_steps` records the
failed version, name, finish time, and error text.

Recovery checklist:

1. Do not delete `cull.db`.
2. Inspect the failed row:
   `SELECT version, name, status, error FROM schema_migration_steps WHERE status = 'failed';`
3. Confirm `PRAGMA user_version` matches `SELECT MAX(version) FROM schema_migrations;`.
4. Restore from the matching `*.backups` file only if the failed step affected data
   outside SQLite rollback guarantees.
5. After fixing the migration bug, reopen Cull. The runner skips versions at or below
   `PRAGMA user_version` and retries the failed or next unapplied step.
