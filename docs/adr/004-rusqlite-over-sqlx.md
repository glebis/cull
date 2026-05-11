# ADR-004: rusqlite Over SQLx for Database Layer

**Status:** Accepted
**Date:** 2025-04
**Author:** Gleb Kalinin

## Context

Need an embedded database for image metadata, ratings, collections, embeddings, and settings. SQLite is the obvious choice for a desktop app. Two main Rust crates: rusqlite (synchronous, thin wrapper) and SQLx (async, compile-time query checking).

## Options Considered

1. **SQLx** — Async, compile-time SQL verification, but heavyweight for embedded use
2. **rusqlite** — Synchronous, thin C wrapper, simple API, battle-tested
3. **sled/redb** — Embedded key-value stores, but no SQL, poor for relational queries

## Decision

rusqlite with Mutex-wrapped Connection, manual migrations.

## Rationale

- Desktop app doesn't need async DB — all queries are fast local I/O
- rusqlite's API is simpler: `conn.lock().unwrap()` + raw SQL
- Compile-time SQL checking (SQLx) adds build complexity for marginal benefit in a single-developer project
- Manual migrations give full control over schema evolution
- Mutex<Connection> is sufficient — no concurrent write pressure in a desktop app
- Easier to debug: raw SQL in code, no ORM abstraction hiding queries

## Consequences

- No compile-time SQL verification — runtime errors possible
- Synchronous API means DB calls block (acceptable for fast local queries)
- Must write migration SQL by hand
