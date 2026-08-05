# Cull Release Skill Suite Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Create six automatically discoverable Codex skills that orchestrate Cull’s repository-enforced release CLI without duplicating release logic or weakening automatic-publication gates.

**Architecture:** The canonical skills live in `~/ai_projects/claude-skills/` and symlink into `~/.agents/skills/`. The public `cull-release` skill routes the lifecycle; five focused skills expose readiness, preparation, publication, verification, and recovery independently. Every skill treats Cull’s JSON CLI and GitHub evidence as authoritative and contains no replacement release engine.

**Tech Stack:** Codex skills, Markdown, `agents/openai.yaml`, Bash symlink installer, Python skill-creator validation tools, Cull’s `npm run release:cull` CLI, GitHub CLI.

## Global Constraints

- Implement this plan only after the Cull release automation foundation plan exposes the documented JSON CLI.
- Canonical source is `$HOME/ai_projects/claude-skills/`.
- Codex discovery is `$HOME/.agents/skills/` through symlinks.
- The skills repository is currently dirty; implementation must use a new worktree from `origin/main` and preserve all existing changes.
- The complete release always requires an explicit `patch`, `minor`, or `major` request.
- That complete request authorizes automatic publication after every mandatory gate passes; do not ask for a second publication confirmation.
- Mutating skills set `policy.allow_implicit_invocation: false`.
- Read-only check and verify skills may allow implicit invocation.
- Never copy the release engine into a skill.
- Never print secrets, rewrite tags, delete releases, force-push, or touch the real `cull.db`.
- Recovery prepares a patch plan but never silently publishes it.
- Keep the existing generic `skills/release` skill functional for other repositories.

---

## File Structure

### New suite

```text
cull-release/
  SKILL.md
  agents/openai.yaml
  references/phase-contracts.md
  scripts/install-suite.sh
  scripts/install-suite.test.sh
cull-release-check/
  SKILL.md
  agents/openai.yaml
cull-release-prepare/
  SKILL.md
  agents/openai.yaml
cull-release-publish/
  SKILL.md
  agents/openai.yaml
cull-release-verify/
  SKILL.md
  agents/openai.yaml
cull-release-recover/
  SKILL.md
  agents/openai.yaml
```

### Modified existing files

- `skills/release/SKILL.md` — route Cull-specific full releases to `$cull-release` while retaining generic behavior.
- Root skills catalog/README only if the repository’s generated catalog requires manual registration.

## Shared Runtime Contract

All phase skills locate Cull in this order:

1. Current Git repository when `git remote get-url origin` matches `glebis/cull`.
2. `CULL_REPO`, when set to an absolute Cull checkout.
3. `$HOME/ai_projects/cull`.

Before mutating, require the repository CLI:

```bash
npm run release:cull -- --help
```

Every invocation reads one JSON envelope from stdout and treats stderr as operator
logs. Exit codes are `2` input/config, `3` blocked gate, `4` external action, and
`5` inconsistent/recovery-needed.

---

### Task 1: Isolated Skills Worktree and Generated Skeletons

**Files:**
- Create: six skill directories and `agents/openai.yaml` files listed above.

**Interfaces:**
- Consumes: approved design and installed system skill-creator.
- Produces: valid generated skill skeletons on a clean `codex/cull-release-skills` branch.

- [ ] **Step 1: Create an isolated worktree without touching dirty `main`**

```bash
git -C "$HOME/ai_projects/claude-skills" fetch origin main
git -C "$HOME/ai_projects/claude-skills" worktree add \
  /tmp/claude-skills-cull-release -b codex/cull-release-skills origin/main
```

Expected: the existing dirty checkout remains unchanged; the temp worktree is clean.

- [ ] **Step 2: Initialize the public orchestrator**

```bash
SC="$HOME/.codex/skills/.system/skill-creator/scripts"
python3 "$SC/init_skill.py" cull-release \
  --path /tmp/claude-skills-cull-release \
  --resources scripts,references \
  --interface display_name="Cull Release" \
  --interface short_description="Orchestrate Cull's guarded release cycle" \
  --interface default_prompt='Use $cull-release to release Cull safely from checks through verification.'
```

- [ ] **Step 3: Initialize the five phase skills**

Run `init_skill.py` once per skill without resource directories:

```bash
python3 "$SC/init_skill.py" cull-release-check --path /tmp/claude-skills-cull-release \
  --interface display_name="Cull Release Check" \
  --interface short_description="Audit Cull release readiness safely" \
  --interface default_prompt='Use $cull-release-check to report every release blocker without changing the repository.'
python3 "$SC/init_skill.py" cull-release-prepare --path /tmp/claude-skills-cull-release \
  --interface display_name="Cull Release Prepare" \
  --interface short_description="Prepare Cull release metadata and commit" \
  --interface default_prompt='Use $cull-release-prepare to prepare a Cull patch release without tagging or pushing.'
python3 "$SC/init_skill.py" cull-release-publish --path /tmp/claude-skills-cull-release \
  --interface display_name="Cull Release Publish" \
  --interface short_description="Publish verified Cull release artifacts" \
  --interface default_prompt='Use $cull-release-publish to publish an already prepared Cull release through verified distribution.'
python3 "$SC/init_skill.py" cull-release-verify --path /tmp/claude-skills-cull-release \
  --interface display_name="Cull Release Verify" \
  --interface short_description="Verify Cull artifacts and distribution" \
  --interface default_prompt='Use $cull-release-verify to verify a Cull release and all distribution channels.'
python3 "$SC/init_skill.py" cull-release-recover --path /tmp/claude-skills-cull-release \
  --interface display_name="Cull Release Recover" \
  --interface short_description="Recover a failed Cull release safely" \
  --interface default_prompt='Use $cull-release-recover to diagnose a failed Cull release and prepare the safest resumable action.'
```

The generated interfaces must match:

| Skill | Display name | Short description | Default prompt |
|---|---|---|---|
| `cull-release-check` | Cull Release Check | Audit Cull release readiness safely | Use `$cull-release-check` to report every release blocker without changing the repository. |
| `cull-release-prepare` | Cull Release Prepare | Prepare Cull release metadata and commit | Use `$cull-release-prepare` to prepare a Cull patch release without tagging or pushing. |
| `cull-release-publish` | Cull Release Publish | Publish verified Cull release artifacts | Use `$cull-release-publish` to publish an already prepared Cull release through verified distribution. |
| `cull-release-verify` | Cull Release Verify | Verify Cull artifacts and distribution | Use `$cull-release-verify` to verify a Cull release and all distribution channels. |
| `cull-release-recover` | Cull Release Recover | Recover a failed Cull release safely | Use `$cull-release-recover` to diagnose a failed Cull release and prepare the safest resumable action. |

- [ ] **Step 4: Set invocation policy**

Use this exact `agents/openai.yaml` shape for mutating skills and the orchestrator:

```yaml
interface:
  display_name: "Cull Release"
  short_description: "Orchestrate Cull's guarded release cycle"
  default_prompt: "Use $cull-release to release Cull safely from checks through verification."
policy:
  allow_implicit_invocation: false
```

Set `allow_implicit_invocation: true` only for `cull-release-check` and
`cull-release-verify`.

- [ ] **Step 5: Validate generated skeletons**

```bash
SC="$HOME/.codex/skills/.system/skill-creator/scripts"
for skill in cull-release cull-release-check cull-release-prepare cull-release-publish cull-release-verify cull-release-recover; do
  python3 "$SC/quick_validate.py" "/tmp/claude-skills-cull-release/$skill"
done
```

Expected: each skeleton validates before customization.

- [ ] **Step 6: Commit**

```bash
git -C /tmp/claude-skills-cull-release add cull-release*
git -C /tmp/claude-skills-cull-release commit -m "chore: scaffold Cull release skill suite"
```

---

### Task 2: Read-Only Release Check Skill

**Files:**
- Modify: `cull-release-check/SKILL.md`
- Regenerate: `cull-release-check/agents/openai.yaml`

**Interfaces:**
- Consumes: `npm run release:cull -- check --bump KIND --json`.
- Produces: concise blocker report and no mutations.

- [ ] **Step 1: Replace the generated SKILL with this contract**

Use frontmatter:

```yaml
---
name: cull-release-check
description: Check Cull release readiness without changing repositories or external systems. Use when the user asks whether Cull is ready to release, requests a release audit or dry run, asks what blocks a Cull version, or before any Cull prepare/publish operation.
---
```

The body must instruct the agent to:

1. Resolve the Cull checkout using the shared runtime order.
2. Require an explicit bump only when calculating a target; otherwise inspect the current candidate.
3. Run `npm run release:cull -- check --bump "$KIND" --json`.
4. Parse exactly one JSON envelope and fail closed on extra stdout.
5. Report current/target version, source SHA, branch/sync state, disk, toolchains,
   contract gates, CI, secrets by presence, and blockers.
6. Run no preparation, commit, tag, workflow dispatch, publication, Homebrew, or
   recovery apply action.
7. Never print environment values or secret contents.

- [ ] **Step 2: Add an explicit read-only invariant**

Include:

```text
Capture `git status --porcelain` and `git rev-parse HEAD` before and after the
command. If either changes, return `CHECK_MUTATED_REPOSITORY` and stop.
```

- [ ] **Step 3: Regenerate metadata and validate**

```bash
SC="$HOME/.codex/skills/.system/skill-creator/scripts"
python3 "$SC/generate_openai_yaml.py" /tmp/claude-skills-cull-release/cull-release-check \
  --interface display_name="Cull Release Check" \
  --interface short_description="Audit Cull release readiness safely" \
  --interface default_prompt='Use $cull-release-check to report every release blocker without changing the repository.'
```

After generation, restore this policy before validation:

```yaml
policy:
  allow_implicit_invocation: true
```

Then run:

```bash
python3 "$SC/quick_validate.py" /tmp/claude-skills-cull-release/cull-release-check
```

- [ ] **Step 4: Commit**

```bash
git -C /tmp/claude-skills-cull-release add cull-release-check
git -C /tmp/claude-skills-cull-release commit -m "feat: add Cull release readiness skill"
```

---

### Task 3: Guarded Release Preparation Skill

**Files:**
- Modify: `cull-release-prepare/SKILL.md`
- Regenerate: `cull-release-prepare/agents/openai.yaml`

**Interfaces:**
- Consumes: explicit bump, successful check envelope, curated notes file, structured compatibility review, and `release:cull prepare`.
- Produces: one focused release commit and state `prepared`; never tag or push.

- [ ] **Step 1: Write exact frontmatter**

```yaml
---
name: cull-release-prepare
description: Prepare Cull release metadata and its focused release commit for an explicitly requested patch, minor, or major version. Use when the user asks to prepare or bump a Cull release, curate its changelog and compatibility review, or create the release commit without tagging, pushing, or publishing.
---
```

- [ ] **Step 2: Encode the preparation sequence**

The body must require:

1. Explicit `patch|minor|major`; never infer it.
2. A dedicated clean worktree from current `origin/main`.
3. Successful `$cull-release-check` evidence bound to `expectedSource` and
   `expectedVersion`.
4. Curated release notes and a compatibility review JSON matching the repository
   CLI schema.
5. A dry run first, followed by the real prepare command only when dry-run output
   lists the expected files.
6. Verification that the result contains one `chore(release): vX.Y.Z` commit,
   state `prepared`, no tag, and no remote mutation.
7. Failure preservation: show changed files and diagnostics; never reset or discard.

Use shell argument arrays or individually quoted variables. Never concatenate a
user-provided path into an evaluated shell string.

- [ ] **Step 3: Keep automatic publication authority out of preparation**

State explicitly that preparation cannot tag, push, dispatch workflows, publish,
or update Homebrew even when invoked by the full orchestrator.

- [ ] **Step 4: Regenerate, validate, and commit**

Run:

```bash
SC="$HOME/.codex/skills/.system/skill-creator/scripts"
python3 "$SC/generate_openai_yaml.py" /tmp/claude-skills-cull-release/cull-release-prepare \
  --interface display_name="Cull Release Prepare" \
  --interface short_description="Prepare Cull release metadata and commit" \
  --interface default_prompt='Use $cull-release-prepare to prepare a Cull patch release without tagging or pushing.'
```

After generation, restore `policy.allow_implicit_invocation: false` before
validation, then run:

```bash
python3 "$SC/quick_validate.py" /tmp/claude-skills-cull-release/cull-release-prepare
```

Then commit:

```bash
git -C /tmp/claude-skills-cull-release add cull-release-prepare
git -C /tmp/claude-skills-cull-release commit -m "feat: add guarded Cull release preparation skill"
```

---

### Task 4: Automatic Verified Publication Skill

**Files:**
- Modify: `cull-release-publish/SKILL.md`
- Regenerate: `cull-release-publish/agents/openai.yaml`

**Interfaces:**
- Consumes: state `prepared`, explicit release authority inherited from the user’s full release request or direct publish request, pushed main release commit, immutable tag, release workflow, provenance.
- Produces: state `homebrew-promoted` or a non-destructive failure record.

- [ ] **Step 1: Write exact frontmatter**

```yaml
---
name: cull-release-publish
description: Tag, build, verify, automatically publish, and distribute an already prepared Cull release. Use only when the user explicitly asks to publish or complete a Cull release, or when the authorized cull-release orchestrator reaches publication after every repository and artifact gate passes.
---
```

- [ ] **Step 2: Encode the no-second-confirmation authority rule**

Include this exact policy:

```text
An explicit “Release Cull patch|minor|major” or direct “Publish prepared Cull
release X.Y.Z” request authorizes publication. Do not ask for another confirmation
after gates pass. That authority does not waive or bypass any gate.
```

- [ ] **Step 3: Encode the publication sequence**

Require, in order:

1. Recheck prepared state, release commit on pushed `main`, exact metadata/version,
   and absence of a conflicting tag.
2. Create and push one annotated tag.
3. Watch the tag-bound release workflow; never substitute a local build.
4. Require verified workflow artifact and provenance.
5. Allow the workflow to publish automatically.
6. Wait for checksum-pinned Homebrew promotion.
7. Write evidence transitions through `published` and `homebrew-promoted`.

On any failure, invoke recovery diagnosis. Never delete/rewrite the tag, replace
published assets, or force a workflow state.

- [ ] **Step 4: Regenerate, validate, and commit**

Run:

```bash
SC="$HOME/.codex/skills/.system/skill-creator/scripts"
python3 "$SC/generate_openai_yaml.py" /tmp/claude-skills-cull-release/cull-release-publish \
  --interface display_name="Cull Release Publish" \
  --interface short_description="Publish verified Cull release artifacts" \
  --interface default_prompt='Use $cull-release-publish to publish an already prepared Cull release through verified distribution.'
```

After generation, restore `policy.allow_implicit_invocation: false` before
validation, then run:

```bash
python3 "$SC/quick_validate.py" /tmp/claude-skills-cull-release/cull-release-publish
```

Confirm implicit invocation remains false, then commit:

```bash
git -C /tmp/claude-skills-cull-release add cull-release-publish
git -C /tmp/claude-skills-cull-release commit -m "feat: add verified Cull publication skill"
```

---

### Task 5: Verification and Recovery Skills

**Files:**
- Modify: `cull-release-verify/SKILL.md`
- Modify: `cull-release-recover/SKILL.md`
- Regenerate: both `agents/openai.yaml` files.

**Interfaces:**
- Verify consumes: version or latest published release; produces evidence and no external mutation.
- Recover consumes: version or failed state; produces classification, bd incident, and patch plan without publication.

- [ ] **Step 1: Write Verify frontmatter**

```yaml
---
name: cull-release-verify
description: Verify Cull GitHub release assets, signatures, notarization, updater metadata, Homebrew version and checksum, installed version, and launch health. Use when the user asks to verify a Cull release, audit the latest release, check a DMG, updater, Homebrew cask, or post-publish state.
---
```

- [ ] **Step 2: Encode read-only verification**

Require the repository CLI and exact artifact verifier; compare Git tag/SHA,
published release, provenance, updater files, cask version/SHA, installed version,
and launch evidence. Default to no installation. When an install smoke is explicitly
requested, use an isolated location and preserve the existing app. Never edit the
release, tap, or state except to attach read-only evidence through the orchestrator.

- [ ] **Step 3: Write Recover frontmatter**

```yaml
---
name: cull-release-recover
description: Diagnose and safely recover a failed or inconsistent Cull release without deleting or rewriting release history. Use when a Cull release is stuck, a workflow or artifact failed, Homebrew is behind, post-publish verification failed, or the user asks for a release recovery or patch plan.
---
```

- [ ] **Step 4: Encode recovery classification**

Reconstruct truth from commit, tag, workflow run, assets, release, updater metadata,
and tap. Map exact states to `rerun-check`, `prepare-new-version`, `watch-build`,
`verify-artifact`, `publish-verified-artifacts`, `promote-homebrew`, or
`prepare-patch-plan`. A post-publish failure creates/updates a P0 bd issue and blocks
future release checks. Never delete or move a tag/release and never publish the patch.

- [ ] **Step 5: Regenerate, validate, and commit**

Run:

```bash
SC="$HOME/.codex/skills/.system/skill-creator/scripts"
python3 "$SC/generate_openai_yaml.py" /tmp/claude-skills-cull-release/cull-release-verify \
  --interface display_name="Cull Release Verify" \
  --interface short_description="Verify Cull artifacts and distribution" \
  --interface default_prompt='Use $cull-release-verify to verify a Cull release and all distribution channels.'
python3 "$SC/generate_openai_yaml.py" /tmp/claude-skills-cull-release/cull-release-recover \
  --interface display_name="Cull Release Recover" \
  --interface short_description="Recover a failed Cull release safely" \
  --interface default_prompt='Use $cull-release-recover to diagnose a failed Cull release and prepare the safest resumable action.'
```

After generation, set Verify implicit true and Recover implicit false, validate
both with:

```bash
python3 "$SC/quick_validate.py" /tmp/claude-skills-cull-release/cull-release-verify
python3 "$SC/quick_validate.py" /tmp/claude-skills-cull-release/cull-release-recover
```

Then commit:

```bash
git -C /tmp/claude-skills-cull-release add cull-release-verify cull-release-recover
git -C /tmp/claude-skills-cull-release commit -m "feat: add Cull release verification and recovery skills"
```

---

### Task 6: Public Orchestrator and Phase Contract Reference

**Files:**
- Modify: `cull-release/SKILL.md`
- Create: `cull-release/references/phase-contracts.md`
- Regenerate: `cull-release/agents/openai.yaml`
- Modify: `skills/release/SKILL.md`

**Interfaces:**
- Consumes: all five phase contracts and explicit bump.
- Produces: complete state progression through `post-publish-verified`.

- [ ] **Step 1: Write orchestrator frontmatter**

```yaml
---
name: cull-release
description: Orchestrate Cull's complete guarded release cycle from an explicit patch, minor, or major request through readiness, preparation, signed build, exact-artifact verification, automatic GitHub publication, checksum-pinned Homebrew promotion, and post-publish verification. Also use to resume or report the current Cull release state.
---
```

- [ ] **Step 2: Write the phase contract reference**

Document, for every phase, accepted states, command, expected JSON result, legal next
state, exit codes, evidence, mutation boundary, and recovery route. Include the nine
states verbatim from the approved design and the complete-request publication
authority rule.

- [ ] **Step 3: Encode orchestration**

The SKILL body must:

1. Require explicit bump for new releases.
2. Read `references/phase-contracts.md`.
3. Run check and stop on blockers.
4. Run prepare in the isolated worktree.
5. Run publish without a second confirmation after gates pass.
6. Run verify after GitHub and Homebrew promotion.
7. Resume from evidence-derived state instead of repeating completed work.
8. Route every failure through recover.
9. Report version, commit, tag, workflow, asset hashes, release URL, tap commit, and
   final state.

- [ ] **Step 4: Prevent generic skill conflicts**

Add one sentence near the top of `skills/release/SKILL.md`:

```text
For the Cull repository, use `$cull-release`; Cull’s repository-enforced state,
artifact, updater, and Homebrew gates are stricter than this generic workflow.
```

- [ ] **Step 5: Regenerate, validate, and commit**

Run:

```bash
SC="$HOME/.codex/skills/.system/skill-creator/scripts"
python3 "$SC/generate_openai_yaml.py" /tmp/claude-skills-cull-release/cull-release \
  --interface display_name="Cull Release" \
  --interface short_description="Orchestrate Cull's guarded release cycle" \
  --interface default_prompt='Use $cull-release to release Cull safely from checks through verification.'
```

After generation, restore `policy.allow_implicit_invocation: false`, then run:

```bash
python3 "$SC/quick_validate.py" /tmp/claude-skills-cull-release/cull-release
```

Then commit:

```bash
git -C /tmp/claude-skills-cull-release add cull-release skills/release/SKILL.md
git -C /tmp/claude-skills-cull-release commit -m "feat: orchestrate the complete Cull release lifecycle"
```

---

### Task 7: Idempotent Codex Discovery Installer

**Files:**
- Create: `cull-release/scripts/install-suite.sh`
- Create: `cull-release/scripts/install-suite.test.sh`

**Interfaces:**
- Consumes: canonical suite checkout.
- Produces: six correct symlinks under `~/.agents/skills/`; never replaces content.

- [ ] **Step 1: Write an isolated installer test harness**

The script accepts `AGENT_SKILLS_DIR` for tests. Create a temp directory containing:

- no destination;
- the expected symlink;
- a wrong symlink;
- a real directory.

Expect create, idempotent success, refusal, and refusal respectively.

- [ ] **Step 2: Implement guarded installation**

Use this core behavior:

```bash
skills=(cull-release cull-release-check cull-release-prepare cull-release-publish cull-release-verify cull-release-recover)
source_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
target_root="${AGENT_SKILLS_DIR:-$HOME/.agents/skills}"
mkdir -p "$target_root"

for skill in "${skills[@]}"; do
  source_path="$source_root/$skill"
  target_path="$target_root/$skill"
  if [[ -L "$target_path" && "$(readlink "$target_path")" == "$source_path" ]]; then
    continue
  fi
  if [[ -e "$target_path" || -L "$target_path" ]]; then
    echo "refusing to replace existing skill path: $target_path" >&2
    exit 2
  fi
  ln -s "$source_path" "$target_path"
done
```

Do not use `ln -sfn`, `rm`, or `trash`; conflicting content must remain untouched.

- [ ] **Step 3: Run installer tests and real install**

```bash
bash cull-release/scripts/install-suite.test.sh
bash cull-release/scripts/install-suite.sh
```

Expected: all six symlinks resolve into the canonical worktree after merge into the
canonical source checkout.

- [ ] **Step 4: Commit**

```bash
git -C /tmp/claude-skills-cull-release add cull-release/scripts
git -C /tmp/claude-skills-cull-release commit -m "feat: install Cull release skills for Codex"
```

---

### Task 8: Validation and Forward Tests

**Files:**
- Modify only skills that fail validation or forward tests.

**Interfaces:**
- Consumes: complete suite and Cull dry-run fixtures.
- Produces: validated, discoverable skills with raw forward-test evidence.

- [ ] **Step 1: Run structural validation**

```bash
SC="$HOME/.codex/skills/.system/skill-creator/scripts"
for skill in cull-release cull-release-check cull-release-prepare cull-release-publish cull-release-verify cull-release-recover; do
  python3 "$SC/quick_validate.py" "/tmp/claude-skills-cull-release/$skill"
done
bash -n /tmp/claude-skills-cull-release/cull-release/scripts/install-suite.sh
```

Expected: PASS.

- [ ] **Step 2: Forward-test read-only skills**

Use fresh agents with raw fixture repositories:

```text
Use $cull-release-check to assess whether this Cull checkout is ready for a patch release.
Use $cull-release-verify to verify Cull v0.2.6 from the supplied fixture evidence.
```

Assert zero Git diff, unchanged HEAD, no workflow dispatch, and no secret output.

- [ ] **Step 3: Forward-test preparation**

Prompt a fresh agent to prepare a patch in a temp Cull fixture. Assert exactly the
declared version/changelog/compatibility files and one release commit change; no tag
or remote call occurs. Repeat in dry-run and assert zero writes.

- [ ] **Step 4: Forward-test publication against fakes**

Use a bare Git remote, fake `gh`, fixture workflow artifacts, and fake tap. Test
successful automatic publication, failed CI, mismatched tag/version, missing updater
signature, bad SHA, and partial Homebrew promotion. No test may address the real
`glebis/cull` release or `glebis/homebrew-tap` mutation APIs.

- [ ] **Step 5: Forward-test recovery prohibitions**

Supply tag-only, failed-action, partial-draft, published/tap-stale, and failed
post-publish states. Search raw actions and output for tag deletion, release deletion,
force-push, asset replacement, `git reset --hard`, and database cleanup; all must be
absent.

- [ ] **Step 6: Review discoverability**

Start a fresh Codex task after installation and confirm all six names appear in the
available skills catalog. Explicitly invoke `$cull-release-check` once.

- [ ] **Step 7: Commit fixes and push**

```bash
git -C /tmp/claude-skills-cull-release add cull-release*
git -C /tmp/claude-skills-cull-release commit -m "test: validate Cull release skill suite"
git -C /tmp/claude-skills-cull-release push -u origin codex/cull-release-skills
```

Expected: branch push succeeds; the original dirty skills checkout remains untouched.

---

### Task 9: Integrate After Cull Automation Lands

**Files:**
- No new files unless integration reveals a documented contract mismatch.

**Interfaces:**
- Consumes: landed Cull automation branch and validated skills branch.
- Produces: installed suite ready for a signed non-publishing canary and subsequent explicit release.

- [ ] **Step 1: Verify Cull CLI compatibility**

Run every command named in `phase-contracts.md` against Cull dry-run fixtures and
compare envelope schema, exit codes, states, and evidence fields exactly.

- [ ] **Step 2: Merge and install the skills branch**

Use the skills repository’s normal review/landing workflow. After merge, run the
installer from `$HOME/ai_projects/claude-skills/cull-release/scripts/install-suite.sh`
so symlinks target the canonical checkout rather than the temp worktree.

- [ ] **Step 3: Run a complete dry release**

Invoke `$cull-release` with `patch` against a fixture/fork where publication APIs are
fake. Confirm the state reaches `post-publish-verified` and resume at every boundary
does not repeat the signed build.

- [ ] **Step 4: Run the real signed canary**

Invoke the check skill on Cull `main`, then dispatch the repository’s non-publishing
canary. Verify signed artifact and provenance. Confirm there is no new tag, GitHub
release, or tap commit.

- [ ] **Step 5: Enable production use**

Only after the Cull foundation plan’s stable contracts, blocking tests, and canary
pass, use `$cull-release patch|minor|major` for a real release. Keep the generic
`$release` skill available for non-Cull repositories.
