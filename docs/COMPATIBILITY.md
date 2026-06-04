# Compatibility Policy — Cull

Cull follows [Semantic Versioning](https://semver.org/spec/v2.0.0.html). This
document declares Cull's **public API** — the surfaces we promise to keep
working — and is updated on every release (by the `release` skill).

**The promise:** within a major version, anything written by an earlier `X.y.z`
keeps working under every later `X.*` — your database, your remote tokens, and
exports you've published. A change that breaks a **stable** surface requires a
new major version. (Modeled on the
[Go 1 Compatibility Promise](https://go.dev/doc/go1compat).)

Pre-1.0 (`0.y.z`): minors may still change `preview` surfaces; `stable` surfaces
are already held to the promise above.

Tiers: `experimental` (no promise) → `preview` (may change, with notice) →
`stable` (the promise applies).

## Surfaces

| Surface | Tier | Since | Mode | Notes |
|---|---|---|---|---|
| **Database schema** (`cull.db`, `user_version`) | stable | 0.1.0 | `BACKWARD_TRANSITIVE` | New code opens every older DB. Migrations are additive; a pre-migration backup is taken and post-migration schema invariants are verified. Never destructive to user data. |
| **MCP token API** (roles, scopes, tools) | preview | — | `unversioned` | Hardened (collection/tag scopes, per-image authorization), but **no protocol-version handshake or deprecation policy yet** — may change within a minor until promoted. |
| **Export formats** (static-publish package, etc.) | stable | 0.1.0 | `forward-compatible` | Static-publish manifests carry a `schema` marker (`cull.static_publishing.v1`); readers ignore unknown fields. Breaking changes bump the marker (`…v2`) with an old-format reader. |

## Deprecations

| Item | Deprecated in | Removable in | Replacement |
|---|---|---|---|
| — | — | — | — |

When a `stable` element is deprecated: keep it ≥1 minor, signal with
[RFC 9745](https://www.rfc-editor.org/rfc/rfc9745.html) /
[RFC 8594](https://www.rfc-editor.org/rfc/rfc8594.html) where applicable, remove
only at a major.

## 1.0 readiness gate

Cull reaches 1.0 when all three surfaces are `stable` and:

- [ ] **DB** — golden round-trip tests for every prior 1.x schema version (the
  v21 golden test is the first; see `docs/CONTRACTS.md`).
- [ ] **MCP** — a `protocolVersion` handshake + a written deprecation policy, OR
  1.0 ships with MCP still `preview` (not promised).
- [ ] **Exports** — every emitted artifact carries a version marker, tolerates
  unknown fields, and has a golden serve/validate test.
- [ ] This policy + a deprecation process are written and followed.

---

Last updated: 0.2.1 (2026-06-04)
