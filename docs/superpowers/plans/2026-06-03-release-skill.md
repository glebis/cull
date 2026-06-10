# Release + Compatibility Skill — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Build a distributable, config-driven `/release` orchestrator skill (validated on Cull) that bumps versions, gates on a readiness check, maintains a tiered `COMPATIBILITY.md`, tags→`release.yml`, and teaches as it runs.

**Architecture:** A skill in `~/ai_projects/claude-skills/skills/release/` whose risky mechanics live in a unit-tested Python engine (`release.py`, stdlib-only). `SKILL.md` orchestrates and teaches. Cull is the first consumer via `release.config.json` plus a worked DB golden test. Spec: `cull/docs/superpowers/specs/2026-06-03-release-compatibility-skill-design.md`.

**Tech Stack:** Python 3.11+ (stdlib: `json`, `re`, `subprocess`, `argparse`, `pathlib`, `unittest`); Bash; Rust (Cull golden test); Markdown.

**Repos:** `claude-skills` (CS) and `cull` (CULL). Each task tags its repo.

---

## File structure (locked)

```
CS  skills/release/SKILL.md                 # orchestration + --explain lessons
CS  skills/release/README.md                # honest scope, install, config ref
CS  skills/release/scripts/release.py       # engine (pure fns + CLI)
CS  skills/release/scripts/test_release.py  # unittest suite
CS  skills/release/reference/{config-schema,compatibility-md,standards}.md
CS  skills/release/templates/{COMPATIBILITY.md.tmpl,CONTRACTS.md.tmpl,release.config.json.tmpl}
CULL release.config.json
CULL docs/COMPATIBILITY.md
CULL docs/CONTRACTS.md
CULL docs/RELEASING.md
CULL src-tauri/tests/compat_golden.rs
CULL src-tauri/tests/fixtures/db/v21.db
```

---

### Task 1: Scaffold the skill + engine module (CS)

**Files:**
- Create: `~/ai_projects/claude-skills/skills/release/scripts/release.py`
- Create: `~/ai_projects/claude-skills/skills/release/scripts/test_release.py`

- [ ] **Step 1: Create the engine skeleton**

```python
# release.py — config-driven release engine (stdlib only). Pure fns are unit-tested;
# side-effecting CLI wraps them. See SKILL.md for orchestration.
from __future__ import annotations
import json, re, argparse
from dataclasses import dataclass
from pathlib import Path

class ReleaseError(Exception):
    """User-facing, recoverable error (printed, non-zero exit)."""

@dataclass(frozen=True)
class Version:
    major: int; minor: int; patch: int
    def __str__(self) -> str: return f"{self.major}.{self.minor}.{self.patch}"
```

- [ ] **Step 2: Create the test file header**

```python
import unittest
from release import Version  # add more imports as tasks land
```

- [ ] **Step 3: Run the (empty) suite to verify wiring**

Run: `cd ~/ai_projects/claude-skills/skills/release/scripts && python3 -m unittest test_release -v`
Expected: `Ran 0 tests` (or "no tests") — imports succeed, no error.

- [ ] **Step 4: Commit (CS)**

```bash
cd ~/ai_projects/claude-skills && git add skills/release/scripts && \
git commit -m "feat(release): scaffold release engine module"
```

---

### Task 2: SemVer parse + bump (CS)

**Files:** Modify `scripts/release.py`, `scripts/test_release.py`

- [ ] **Step 1: Write failing tests**

```python
from release import parse_version, bump, Version

class TestVersion(unittest.TestCase):
    def test_parse(self):
        self.assertEqual(parse_version("1.2.3"), Version(1,2,3))
    def test_parse_rejects_junk(self):
        with self.assertRaises(Exception): parse_version("1.2")
    def test_bump_patch(self):
        self.assertEqual(str(bump(Version(0,1,0),"patch")), "0.1.1")
    def test_bump_minor_resets_patch(self):
        self.assertEqual(str(bump(Version(0,1,4),"minor")), "0.2.0")
    def test_bump_major_resets(self):
        self.assertEqual(str(bump(Version(0,9,3),"major")), "1.0.0")
```

- [ ] **Step 2: Run to verify fail**

Run: `python3 -m unittest test_release.TestVersion -v`
Expected: FAIL — `cannot import name 'parse_version'`.

- [ ] **Step 3: Implement**

```python
_SEMVER = re.compile(r"^(\d+)\.(\d+)\.(\d+)$")
def parse_version(s: str) -> Version:
    m = _SEMVER.match(s.strip())
    if not m: raise ReleaseError(f"not a SemVer x.y.z: {s!r}")
    return Version(int(m[1]), int(m[2]), int(m[3]))

def bump(v: Version, kind: str) -> Version:
    if kind == "major": return Version(v.major+1, 0, 0)
    if kind == "minor": return Version(v.major, v.minor+1, 0)
    if kind == "patch": return Version(v.major, v.minor, v.patch+1)
    raise ReleaseError(f"bump kind must be major|minor|patch, got {kind!r}")
```

- [ ] **Step 4: Run to verify pass**

Run: `python3 -m unittest test_release.TestVersion -v` → Expected: PASS (5 tests).

- [ ] **Step 5: Commit (CS)**

```bash
git add skills/release/scripts && git commit -m "feat(release): SemVer parse + bump"
```

---

### Task 3: Read/write version files (json pointer + toml key) (CS)

**Files:** Modify `scripts/release.py`, `scripts/test_release.py`

- [ ] **Step 1: Write failing tests** (use tmp files)

```python
import tempfile, os
from release import read_version_file, write_version_file

class TestVersionFiles(unittest.TestCase):
    def _tmp(self, name, content):
        d = tempfile.mkdtemp(); p = Path(d)/name; p.write_text(content); return p
    def test_json_pointer(self):
        p = self._tmp("package.json", '{"name":"x","version":"0.1.0"}')
        self.assertEqual(read_version_file(p, "json", pointer="/version"), "0.1.0")
        write_version_file(p, "json", "0.2.0", pointer="/version")
        self.assertEqual(json.loads(p.read_text())["version"], "0.2.0")
    def test_toml_key(self):
        p = self._tmp("Cargo.toml", '[package]\nname = "x"\nversion = "0.1.0"\n')
        self.assertEqual(read_version_file(p, "toml", key="package.version"), "0.1.0")
        write_version_file(p, "toml", "0.2.0", key="package.version")
        self.assertIn('version = "0.2.0"', p.read_text())
```

- [ ] **Step 2: Run to verify fail** — Expected: ImportError.

- [ ] **Step 3: Implement**

```python
try:
    import tomllib  # 3.11+
except ModuleNotFoundError:  # pragma: no cover
    tomllib = None

def read_version_file(path: Path, kind: str, *, pointer=None, key=None) -> str:
    text = Path(path).read_text()
    if kind == "json":
        node = json.loads(text)
        for part in [p for p in pointer.split("/") if p]:
            node = node[part]
        return str(node)
    if kind == "toml":
        # read with tomllib if available, else regex the [package] version line
        sect, name = key.split(".", 1)
        if tomllib:
            data = tomllib.loads(text)
            return str(data[sect][name])
        m = re.search(rf'(?ms)^\[{re.escape(sect)}\].*?^{re.escape(name)}\s*=\s*"([^"]+)"', text)
        if not m: raise ReleaseError(f"no {key} in {path}")
        return m[1]
    raise ReleaseError(f"unknown version-file kind {kind!r}")

def write_version_file(path: Path, kind: str, new: str, *, pointer=None, key=None) -> None:
    path = Path(path); text = path.read_text()
    if kind == "json":
        # targeted replace to preserve formatting/key order
        parts = [p for p in pointer.split("/") if p]
        leaf = parts[-1]
        new_text, n = re.subn(rf'("{re.escape(leaf)}"\s*:\s*)"[^"]*"', rf'\g<1>"{new}"', text, count=1)
        if n != 1: raise ReleaseError(f"could not rewrite {pointer} in {path}")
        path.write_text(new_text); return
    if kind == "toml":
        sect, name = key.split(".", 1)
        new_text, n = re.subn(
            rf'(?ms)(^\[{re.escape(sect)}\].*?^{re.escape(name)}\s*=\s*)"[^"]*"',
            rf'\g<1>"{new}"', text, count=1)
        if n != 1: raise ReleaseError(f"could not rewrite {key} in {path}")
        path.write_text(new_text); return
    raise ReleaseError(f"unknown version-file kind {kind!r}")
```

- [ ] **Step 4: Run to verify pass** → PASS (2 tests).

- [ ] **Step 5: Commit (CS)**

```bash
git add skills/release/scripts && git commit -m "feat(release): read/write JSON-pointer and TOML-key version fields"
```

---

### Task 4: Changelog draft from conventional commits (CS)

**Files:** Modify `scripts/release.py`, `scripts/test_release.py`

- [ ] **Step 1: Write failing test** (pure fn over a list of commit subjects)

```python
from release import draft_changelog

class TestChangelog(unittest.TestCase):
    def test_buckets_by_type(self):
        commits = ["feat(mcp): add tag scopes", "fix: stop key leak",
                   "perf(db): sql folders", "chore: bump dep", "docs: tweak"]
        md = draft_changelog("0.2.0", "2026-06-03", commits)
        self.assertIn("## [0.2.0] - 2026-06-03", md)
        self.assertIn("### Added", md);  self.assertIn("add tag scopes", md)
        self.assertIn("### Fixed", md);  self.assertIn("stop key leak", md)
        self.assertIn("### Changed", md) # perf+others
        self.assertNotIn("chore", md.lower().split("###")[0])  # chores not headline
```

- [ ] **Step 2: Run to verify fail** — ImportError.

- [ ] **Step 3: Implement**

```python
_CC = re.compile(r"^(?P<type>\w+)(?:\([^)]*\))?(?P<bang>!)?:\s*(?P<desc>.+)$")
_BUCKETS = {"feat": "Added", "fix": "Fixed", "perf": "Changed", "refactor": "Changed"}

def draft_changelog(version: str, date: str, commit_subjects: list[str]) -> str:
    groups: dict[str, list[str]] = {}
    for s in commit_subjects:
        m = _CC.match(s.strip())
        if not m: continue
        t = m["type"]
        if t in ("chore", "docs", "test", "ci", "style", "build"): continue
        section = "Changed" if m["bang"] else _BUCKETS.get(t, "Changed")
        groups.setdefault(section, []).append(m["desc"].strip())
    out = [f"## [{version}] - {date}", ""]
    for section in ("Added", "Changed", "Fixed"):
        items = groups.get(section)
        if not items: continue
        out.append(f"### {section}")
        out += [f"- {i}" for i in items]
        out.append("")
    return "\n".join(out).rstrip() + "\n"
```

- [ ] **Step 4: Run to verify pass** → PASS.

- [ ] **Step 5: Commit (CS)**

```bash
git add skills/release/scripts && git commit -m "feat(release): draft Keep-a-Changelog section from conventional commits"
```

---

### Task 5: Surface-tier consistency check — the Gate teeth (CS)

**Files:** Modify `scripts/release.py`, `scripts/test_release.py`

- [ ] **Step 1: Write failing tests**

```python
from release import required_bump, ReleaseError

class TestSurfaceGate(unittest.TestCase):
    # surfaces_changed = list of {id, tier, breaking: bool}
    def test_breaking_stable_forces_major(self):
        self.assertEqual(required_bump([{"id":"db","tier":"stable","breaking":True}]), "major")
    def test_breaking_preview_is_minor(self):
        self.assertEqual(required_bump([{"id":"mcp","tier":"preview","breaking":True}]), "minor")
    def test_additive_is_minor(self):
        self.assertEqual(required_bump([{"id":"db","tier":"stable","breaking":False}]), "minor")
    def test_nothing_is_patch(self):
        self.assertEqual(required_bump([]), "patch")

class TestGateEnforcement(unittest.TestCase):
    def test_rejects_too_small_bump(self):
        from release import enforce_bump
        with self.assertRaises(ReleaseError):
            enforce_bump(requested="patch", required="major")
    def test_allows_equal_or_larger(self):
        from release import enforce_bump
        enforce_bump(requested="major", required="minor")  # no raise
```

- [ ] **Step 2: Run to verify fail** — ImportError.

- [ ] **Step 3: Implement**

```python
_ORDER = {"patch": 0, "minor": 1, "major": 2}

def required_bump(surfaces_changed: list[dict]) -> str:
    req = "patch"
    for s in surfaces_changed:
        if s.get("breaking") and s.get("tier") == "stable":
            return "major"
        if s.get("breaking") or not s.get("breaking"):  # any declared change ≥ minor
            req = "minor" if _ORDER[req] < _ORDER["minor"] else req
    return req

def enforce_bump(requested: str, required: str) -> None:
    if _ORDER[requested] < _ORDER[required]:
        raise ReleaseError(
            f"requested '{requested}' but a {required} bump is required "
            f"(a stable surface changed incompatibly). See COMPATIBILITY.md.")
```

- [ ] **Step 4: Run to verify pass** → PASS (6 tests).

- [ ] **Step 5: Commit (CS)**

```bash
git add skills/release/scripts && git commit -m "feat(release): surface-tier gate (breaking stable surface forces major)"
```

---

### Task 6: Config loader + validation (CS)

**Files:** Modify `scripts/release.py`, `scripts/test_release.py`

- [ ] **Step 1: Write failing tests**

```python
from release import load_config

class TestConfig(unittest.TestCase):
    def test_loads_and_validates(self):
        d = tempfile.mkdtemp()
        (Path(d)/"release.config.json").write_text(json.dumps({
            "versionFiles":[{"path":"package.json","kind":"json","pointer":"/version"}],
            "gate":"true","compatibility":{"path":"docs/COMPATIBILITY.md"},
            "surfaces":[{"id":"db","name":"DB","tier":"stable","mode":"BACKWARD_TRANSITIVE"}],
            "tag":{"prefix":"v","push":True}}))
        cfg = load_config(Path(d)/"release.config.json")
        self.assertEqual(cfg["gate"], "true")
        self.assertEqual(cfg["surfaces"][0]["tier"], "stable")
    def test_rejects_bad_tier(self):
        d = tempfile.mkdtemp()
        (Path(d)/"c.json").write_text(json.dumps({
            "versionFiles":[{"path":"p","kind":"json","pointer":"/version"}],
            "gate":"true","compatibility":{"path":"x"},
            "surfaces":[{"id":"db","name":"DB","tier":"GA","mode":"x"}],"tag":{"prefix":"v"}}))
        with self.assertRaises(Exception): load_config(Path(d)/"c.json")
```

- [ ] **Step 2: Run to verify fail** — ImportError.

- [ ] **Step 3: Implement**

```python
_TIERS = {"experimental", "preview", "stable"}
def load_config(path: Path) -> dict:
    cfg = json.loads(Path(path).read_text())
    for req in ("versionFiles", "gate", "compatibility", "surfaces", "tag"):
        if req not in cfg: raise ReleaseError(f"release.config.json missing '{req}'")
    if not cfg["versionFiles"]: raise ReleaseError("versionFiles must be non-empty")
    for s in cfg["surfaces"]:
        if s.get("tier") not in _TIERS:
            raise ReleaseError(f"surface {s.get('id')}: tier must be one of {_TIERS}")
    return cfg
```

- [ ] **Step 4: Run to verify pass** → PASS.

- [ ] **Step 5: Commit (CS)**

```bash
git add skills/release/scripts && git commit -m "feat(release): config loader + validation"
```

---

### Task 7: CLI + `--dry-run` plan assembly (CS)

**Files:** Modify `scripts/release.py`, `scripts/test_release.py`

- [ ] **Step 1: Write failing test** (dry-run mutates nothing, returns a plan)

```python
from release import build_plan

class TestPlan(unittest.TestCase):
    def test_build_plan_dry(self):
        cfg = {"versionFiles":[{"path":"package.json","kind":"json","pointer":"/version"}],
               "tag":{"prefix":"v"}}
        plan = build_plan(cfg, current="0.1.0", kind="minor")
        self.assertEqual(plan["new_version"], "0.2.0")
        self.assertEqual(plan["tag"], "v0.2.0")
        self.assertIn("package.json", plan["files"])
```

- [ ] **Step 2: Run to verify fail** — ImportError.

- [ ] **Step 3: Implement plan + argparse CLI**

```python
def build_plan(cfg: dict, current: str, kind: str) -> dict:
    new = str(bump(parse_version(current), kind))
    prefix = cfg.get("tag", {}).get("prefix", "v")
    return {"new_version": new, "tag": f"{prefix}{new}",
            "files": [vf["path"] for vf in cfg["versionFiles"]]}

def _cmd_plan(args):
    cfg = load_config(Path(args.config))
    cur = read_version_file(Path(cfg["versionFiles"][0]["path"]),
                            cfg["versionFiles"][0]["kind"],
                            pointer=cfg["versionFiles"][0].get("pointer"),
                            key=cfg["versionFiles"][0].get("key"))
    print(json.dumps(build_plan(cfg, cur, args.kind), indent=2))

def main(argv=None):
    ap = argparse.ArgumentParser(prog="release")
    ap.add_argument("--config", default="release.config.json")
    sub = ap.add_subparsers(dest="cmd", required=True)
    p = sub.add_parser("plan"); p.add_argument("kind", choices=["patch","minor","major"])
    p.set_defaults(func=_cmd_plan)
    args = ap.parse_args(argv)
    try: args.func(args)
    except ReleaseError as e: print(f"error: {e}"); raise SystemExit(2)

if __name__ == "__main__": main()
```

- [ ] **Step 4: Run to verify pass + smoke the CLI**

Run: `python3 -m unittest test_release -v` → all PASS.
Run (smoke): `cd /tmp && echo '{"name":"x","version":"0.1.0"}' > package.json && echo '{"versionFiles":[{"path":"package.json","kind":"json","pointer":"/version"}],"gate":"true","compatibility":{"path":"x"},"surfaces":[],"tag":{"prefix":"v"}}' > release.config.json && python3 ~/ai_projects/claude-skills/skills/release/scripts/release.py plan minor`
Expected: JSON plan with `"new_version": "0.2.0"`, `"tag": "v0.2.0"`.

- [ ] **Step 5: Commit (CS)**

```bash
git add skills/release/scripts && git commit -m "feat(release): plan command + dry-run CLI"
```

---

### Task 8: SKILL.md + README + reference docs + templates (CS)

**Files:** Create `SKILL.md`, `README.md`, `reference/*.md`, `templates/*.tmpl`

- [ ] **Step 1: Write `SKILL.md`** — frontmatter (`name: release`, `description:` covering "release, version bump, changelog, compatibility, tag"), then the 8-step `/release <patch|minor|major>` flow from spec §5, with each step carrying a one-line *why* + a standards link, and an `## --explain` section expanding each lesson. Include the precondition checks and the manual fallbacks (call `release.py plan`, run the gate, etc.).

- [ ] **Step 2: Write `README.md`** — honest scope ("reference-tested on a Tauri+Rust+SvelteKit repo; other stacks via config"), install (clone/symlink into skills path), and a pointer to `reference/config-schema.md`.

- [ ] **Step 3: Write `reference/config-schema.md`** — the full `release.config.json` field reference from spec §4 (including `releaseBranch`/`worktree`).

- [ ] **Step 4: Write `reference/standards.md`** — the standards map + links (mirror the Obsidian MOC): Go 1, Kubernetes deprecation, SRE PRR, Schema-Registry modes, Pact, Keep a Changelog, SemVer, RFC 9745/8594, MCP protocolVersion.

- [ ] **Step 5: Write `reference/compatibility-md.md`** — how `COMPATIBILITY.md` is structured and how tiers/deprecations are updated each release.

- [ ] **Step 6: Write `templates/COMPATIBILITY.md.tmpl`, `CONTRACTS.md.tmpl`, `release.config.json.tmpl`** — generic placeholders (`{{PROJECT}}`, `{{SURFACES}}`).

- [ ] **Step 7: Validate skill metadata**

Run: `python3 -c "import pathlib,re,sys; t=pathlib.Path('~/ai_projects/claude-skills/skills/release/SKILL.md').expanduser().read_text(); assert t.startswith('---') and 'name: release' in t and 'description:' in t, 'frontmatter'; print('SKILL.md ok')"`
Expected: `SKILL.md ok`.

- [ ] **Step 8: Commit (CS)**

```bash
cd ~/ai_projects/claude-skills && git add skills/release && \
git commit -m "docs(release): SKILL.md orchestration, README, reference docs, templates"
```

---

### Task 9: Cull `release.config.json` (CULL)

**Files:** Create `release.config.json` (repo root)

- [ ] **Step 1: Write the config** — exactly the spec §4 example (3 version files, `lockfiles: ["src-tauri/Cargo.lock"]`, `gate: "npm run preflight -- release"`, `extraGate: ["cargo test --manifest-path src-tauri/Cargo.toml --test compat_golden"]`, the 3 surfaces with tiers db=stable/mcp=preview/exports=stable, `releaseBranch: "main"`, `worktree: "../cull-main-landing"`, `tag.prefix: "v"`, `issueTracker.kind: "bd"`).

- [ ] **Step 2: Validate it loads**

Run: `cd "$CULL_REPO" && python3 ~/ai_projects/claude-skills/skills/release/scripts/release.py plan patch`
Expected: JSON plan, `new_version` = current patch+1, no error.

- [ ] **Step 3: Commit (CULL, on a branch)**

```bash
cd "$CULL_REPO" && git add release.config.json && \
git commit -m "chore(release): add release.config.json for the release skill"
```

---

### Task 10: Cull `docs/COMPATIBILITY.md` (CULL)

**Files:** Create `docs/COMPATIBILITY.md`

- [ ] **Step 1: Write it** from spec §6: Go-1-style prose promise; Surfaces table (DB stable/BACKWARD_TRANSITIVE/since 0.1.0; MCP preview/unversioned; Exports stable/forward-compatible/since 0.1.0); empty Deprecations table with headers; the 1.0 readiness gate checklist (the four items from spec §6). Stamp "Last updated: 0.1.0 (2026-06-03)".

- [ ] **Step 2: Sanity check links** — ensure it references `docs/CONTRACTS.md` and the standards.

- [ ] **Step 3: Commit (CULL)**

```bash
git add docs/COMPATIBILITY.md && git commit -m "docs: add COMPATIBILITY.md (tiered surfaces + 1.0 gate)"
```

---

### Task 11: Cull `docs/CONTRACTS.md` tutorial + `docs/RELEASING.md` (CULL)

**Files:** Create `docs/CONTRACTS.md`, `docs/RELEASING.md`

- [ ] **Step 1: Write `docs/CONTRACTS.md`** — a tutorial: what Contracts & Modes means ([[Schema Registry Compatibility Modes]], [[Pact]]), the worked DB golden test as the template, and a step-by-step "add the next contract test" (export serve test, MCP protocolVersion negative tests) marked as the next exercises. Link the standards.

- [ ] **Step 2: Write `docs/RELEASING.md`** — short runbook: "`/release <patch|minor|major>` does X; to do it by hand run `release.py plan`, then the gate, then edit COMPATIBILITY, then tag." Point to the skill.

- [ ] **Step 3: Commit (CULL)**

```bash
git add docs/CONTRACTS.md docs/RELEASING.md && \
git commit -m "docs: add CONTRACTS tutorial + RELEASING runbook"
```

---

### Task 12: DB golden test + frozen fixture (CULL) — the worked example

**Files:** Create `src-tauri/tests/compat_golden.rs`, `src-tauri/tests/fixtures/db/v21.db`

- [ ] **Step 1: Generate the frozen v21 fixture**

Run:
```bash
cd "$CULL_REPO"/src-tauri
mkdir -p tests/fixtures/db
cat > /tmp/gen_fixture.rs <<'EOF'
// one-off: open a fresh DB at current schema, copy it to the fixture path
EOF
# Use a tiny Rust binary OR sqlite: open via the app once, then copy.
python3 - <<'PY'
import sqlite3, pathlib
p = pathlib.Path("tests/fixtures/db/v21.db")
con = sqlite3.connect(p)
con.execute("PRAGMA user_version=21")
con.execute("CREATE TABLE app_settings(key TEXT PRIMARY KEY, value TEXT NOT NULL)")
con.commit(); con.close()
print("seed fixture written (migrations will complete the schema on open)")
PY
```
Note: the fixture intentionally carries `user_version=21` + a marker table; `Database::open` runs remaining migrations and `verify_schema_invariants` must pass. (This mirrors the realistic-fixture pattern in `db.rs` migration_safety_tests.)

- [ ] **Step 2: Write the failing test**

```rust
// src-tauri/tests/compat_golden.rs
// CONTRACTS & MODES — worked example (template for export/MCP contract tests).
// Pattern: freeze an artifact from an older version -> exercise current code ->
// assert it still works (backward compatibility). See docs/CONTRACTS.md.
use std::path::Path;

#[test]
fn db_v21_fixture_opens_and_satisfies_invariants() {
    let src = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/db/v21.db");
    let tmp = tempfile::tempdir().unwrap();
    let work = tmp.path().join("cull.db");
    std::fs::copy(&src, &work).expect("fixture must exist");

    // Opening must migrate to current + pass the schema-invariant check.
    let db = cull_lib::db_core::db::Database::open(&work)
        .expect("a frozen older DB must open cleanly under current code");
    db.verify_schema_invariants_for_test()
        .expect("schema invariants must hold after migrating an older DB");
}
```

- [ ] **Step 3: Expose a test hook** (Modify `src-tauri/src/db_core/db.rs`)

`verify_schema_invariants` is private. Add a crate-visible test shim:
```rust
#[cfg(any(test, feature = "test-support"))]
impl Database {
    pub fn verify_schema_invariants_for_test(&self) -> rusqlite::Result<()> {
        self.verify_schema_invariants()
    }
}
```
Add to `src-tauri/Cargo.toml` `[features]`: `test-support = []`, and ensure the integration test enables it (or gate the shim on `#[cfg(test)]` won't cover integration tests — use the `test-support` feature and run with `--features test-support`). Confirm `cull_lib` is the lib name (`[lib] name`).

- [ ] **Step 4: Run to verify fail**

Run: `cargo test --manifest-path src-tauri/Cargo.toml --features test-support --test compat_golden -v`
Expected: FAIL first (shim/feature missing) → then compiles.

- [ ] **Step 5: Make it pass** — adjust lib name/feature until green.

Run: same command → Expected: PASS (1 test).

- [ ] **Step 6: Wire into config gate** — confirm `extraGate` in `release.config.json` uses `--features test-support`.

- [ ] **Step 7: Commit (CULL)**

```bash
git add src-tauri/tests/compat_golden.rs src-tauri/tests/fixtures/db/v21.db src-tauri/src/db_core/db.rs src-tauri/Cargo.toml release.config.json && \
git commit -m "test(compat): DB golden round-trip fixture (Contracts & Modes worked example)"
```

---

### Task 13: Verify `release.yml` trigger + dry-run on Cull (CULL/CS)

**Files:** Read `.github/workflows/release.yml`

- [ ] **Step 1: Confirm the trigger**

Run: `grep -nA4 '^on:' .github/workflows/release.yml`
Expected: a `push: tags: ['v*']` (or similar). If absent, add it (Modify `release.yml`) and commit `ci: trigger release.yml on v* tags`.

- [ ] **Step 2: Full dry-run of the engine on Cull**

Run: `python3 ~/ai_projects/claude-skills/skills/release/scripts/release.py plan minor`
Expected: plan to `0.2.0`, tag `v0.2.0`, the 3 version files listed.

- [ ] **Step 3: Run the gate once manually (no mutation)**

Run: `cd "$CULL_LANDING_WORKTREE" && CULL_PREFLIGHT_SKIP_E2E=1 npm run preflight -- release` then `cargo test --manifest-path src-tauri/Cargo.toml --features test-support --test compat_golden`
Expected: both exit 0. (If disk-constrained, `cargo clean` the main worktree target first — see AGENTS.md.)

- [ ] **Step 4: Commit any workflow fix (CULL)** — only if `release.yml` was edited.

---

### Task 14: Day-one proof — cut `v0.2.0` via the skill (CULL)

**Files:** (mutates version files, CHANGELOG.md, docs/COMPATIBILITY.md)

- [ ] **Step 1: Merge the doc/config/test branch into main** (use the established merge flow into the `cull-main-landing` worktree, `--no-ff`).

- [ ] **Step 2: Invoke `/release minor`** (the skill) on Cull. Walk its steps: it bumps `0.1.0 → 0.2.0`, runs the gate, drafts the CHANGELOG from this session's audit/P0/P1 commits, prompts the compatibility review (no stable breakage this batch → `minor` is allowed), stamps COMPATIBILITY.md, commits `chore(release): v0.2.0`, tags `v0.2.0`, pushes.

- [ ] **Step 3: Verify the release**

Run: `git ls-remote --tags origin | grep v0.2.0` and check the Actions run for `release.yml`.
Expected: tag on origin; release workflow running/green.

- [ ] **Step 4: Confirm docs updated** — `CHANGELOG.md` has `[0.2.0]`; `COMPATIBILITY.md` stamped `0.2.0`.

---

## Self-review

- **Spec coverage:** §3 architecture → Tasks 1–12; §4 config → Tasks 6, 9; §5 flow → Task 8 (SKILL.md) + engine Tasks 2–7 + run Tasks 13–14; §6 COMPATIBILITY → Task 10; §7 golden test → Task 12; §8 teaching → Task 8; §9 skill tests → Tasks 2–7 (`test_release.py`); §10 standards → Task 8 (`reference/standards.md`); §11 deferred → Task 11 (`CONTRACTS.md`). All covered.
- **Placeholder scan:** code provided for every engine step; docs tasks describe exact content sourced from spec sections (acceptable — content is fully specified in the spec they reference). No "TODO/handle errors" hand-waves.
- **Type consistency:** `Version`, `parse_version`, `bump`, `read/write_version_file`, `draft_changelog`, `required_bump`, `enforce_bump`, `load_config`, `build_plan`, `main` — names consistent across tasks and tests.
- **Known follow-up:** Task 12 Step 3 must confirm the actual `[lib] name` (likely `cull_lib`) and that `verify_schema_invariants` exists (it does — added this session); adjust the shim/feature accordingly during execution.
