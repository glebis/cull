# Cull Tooling Research — 2026-06-03

Research on research tools, Claude Code automation, MCP servers, CLIs, and workflows that would
accelerate development of Cull (Tauri 2 + SvelteKit 5 + Rust; SQLite/rusqlite, CLIP/ONNX embeddings,
Svelte 5 runes, a Tauri MCP server, CDP-driven browser E2E).

Current state (for context, so we don't recommend what's already in place):

- CI (`.github/workflows/ci.yml`): frontend job on Ubuntu, Rust job on `macos-latest`, both via
  `scripts/check-ci.sh`. Rust step = `cargo fmt --check` + `cargo clippy --locked --all-targets` +
  `cargo test --locked --all-targets`. Frontend = `npm ci` + `lint:issues` + `svelte-check` +
  `vitest run` + `vite build`.
- Tiered preflight (`scripts/preflight.sh`): `hook` / `quick` / `full` / `release`, plus bd hooks.
- `vitest` is installed and used (`src/lib/view-utils.test.ts`); **no component test harness yet**,
  no `@testing-library/svelte`, no browser-mode Vitest.
- E2E = bespoke CDP scripts against Chrome Beta + `tauri-mock.ts` (`tests/e2e/run-e2e.sh`),
  classified as a manual pre-push gate, not CI.
- Dependabot covers npm + cargo + actions. **No CVE scan, no SBOM** (`audit:licenses` is
  license-only). No `cargo-deny`, no `cargo-audit`, no `cargo-nextest`, no `clippy.toml` /
  `[lints]` table, no `sccache`/`bacon`.
- Pinned Rust 1.89.0 via `rust-toolchain.toml`.

Audit gaps this report maps onto (titles from `docs/cull-audit-2026-06-03.md`):

- **Async/blocking** — "Move blocking import, decode, and database work off async command paths" (P1).
- **A11y harness** — "Make destructive dialogs and palette overlays fully modal" (P1) + "Expose
  thumbnail state through keyboard and screen-reader semantics" (P1). Both acceptance criteria
  demand *automated accessibility tests* that don't exist yet.
- **CVE/SBOM** — "Add CVE and SBOM audits to release gates" (P2).
- **A11y/keyboard E2E in CI** — "Promote keyboard and accessibility E2E coverage into automation" (P2).

---

## 1. Rust / Tauri tooling

### Faster build + test loop

| Tool | What it does | Why it fits Cull | Install / setup | Source |
|---|---|---|---|---|
| **cargo-nextest** | Process-per-test runner, up to ~3x faster, better isolation, flaky-test retries, test-filter DSL, `.config/nextest.toml` profiles. | Cull's tests touch SQLite (`Mutex<Connection>`), ONNX `ort`, filesystem import. Process isolation stops one panicking DB/IO test from poisoning a shared binary, and retries tame flaky filesystem/network tests. Swap into `check-ci.sh` `run_rust`. | `cargo install cargo-nextest --locked` then `cargo nextest run --locked --all-targets` | [nexte.st](https://nexte.st/), [crates.io](https://crates.io/crates/cargo-nextest) |
| **sccache** | Shared compiler cache (content-addressed); reuses artifacts across projects and ephemeral CI. | Cull's Rust dep tree is heavy (`tauri`, `ort` with downloaded binaries, `image`, `reqwest`, `objc2-*`, `rmcp`). sccache cuts cold-build time on the macOS CI runner and locally. | `cargo install sccache --locked`; set `RUSTC_WRAPPER=sccache`. In CI use `mozilla-actions/sccache-action`. | [Depot: sccache in GH Actions](https://depot.dev/blog/sccache-in-github-actions), [Earthly](https://earthly.dev/blog/rust-sccache/) |
| **bacon** | Background `cargo check`/clippy/test watcher with a terminal UI; lighter and more Tauri-friendly than `cargo watch`. | Tight inner loop while editing `db_core/` / `commands/` without fighting `tauri dev` for the target dir (see split-target tip below). | `cargo install bacon --locked`; run `bacon clippy` in `src-tauri/`. | [Tauri Develop docs](https://v2.tauri.app/develop/), [cargo-watch](https://crates.io/crates/cargo-watch) |
| **Split target dir + matched `MACOSX_DEPLOYMENT_TARGET`** | Stops rust-analyzer and `tauri dev` invalidating each other's `target/` cache. | Cull is macOS-first; this is the single biggest local-loop win (reported ~60s → ~10s). Add to `.vscode/settings.json` and keep `minimumSystemVersion` in sync with `tauri.conf.json`. | `.vscode/settings.json`: `"rust-analyzer.cargo.targetDir": "target/analyzer"`, `"rust-analyzer.cargo.extraEnv": {"MACOSX_DEPLOYMENT_TARGET": "10.13"}`; dev cargo profile `[profile.dev.package."*"] opt-level = 1`. | [yuexunj.com](https://yuexunj.com/how-to-make-your-tauri-dev-faster/) |

### Linting / supply-chain (the CVE/SBOM gap)

| Tool | What it does | Why it fits Cull | Install / setup | Source |
|---|---|---|---|---|
| **cargo-deny** | One binary, four checks: `advisories` (RustSec CVEs), `bans` (duplicate/forbidden crates), `licenses` (SPDX policy), `sources`. Config in `deny.toml`. | Directly closes the P2 CVE gap *and* complements the existing `audit:licenses` on the Rust side. License gating in `deny.toml` enforces the Apache-2.0 / model-license posture AGENTS.md cares about. Single gate to add to `release` preflight. | `cargo install cargo-deny --locked`; `cargo deny init`; `cargo deny check` (in `src-tauri/`). | [cargo-deny book](https://github.com/EmbarkStudios/cargo-deny) via [SBOM Rust guide](https://sbomgenerator.com/guides/rust/) |
| **cargo-audit** | Focused RustSec advisory scan of `Cargo.lock`. | Lighter alternative/companion to cargo-deny's advisories check; pairs with `cargo-auditable` so shipped binaries are scannable. | `cargo install cargo-audit --locked`; `cargo audit`. | [rustsec / cargo-audit](https://github.com/rust-secure-code/cargo-auditable) |
| **cargo-auditable** | Embeds the exact dependency list into the release binary so `cargo audit`/Trivy can scan the *shipped* artifact later. | Cull ships signed/updated desktop binaries (`tauri-plugin-updater`); auditable binaries let you re-scan a released build against new CVEs without rebuilding. | `cargo install cargo-auditable --locked`; build with `cargo auditable build --release`. | [cargo-auditable](https://github.com/rust-secure-code/cargo-auditable) |
| **cargo-cyclonedx** | Official CycloneDX SBOM generator from Cargo metadata (per-binary, feature-aware, can omit dev-deps, records licenses). | Closes the SBOM half of the P2 gap for the Rust side; attach `bom.xml`/`bom.json` to GitHub Releases alongside the bundle. | `cargo install cargo-cyclonedx --locked`; `cargo cyclonedx --format json`. | [cargo-cyclonedx](https://crates.io/crates/cargo-cyclonedx), [CycloneDX rust-cargo](https://github.com/CycloneDX/cyclonedx-rust-cargo) |
| **clippy `[lints]` table + `clippy.toml`** | Workspace-wide lint policy in `Cargo.toml` (`[lints.clippy] pedantic = {level="warn",priority=-1}`, `unwrap_used="deny"`), enforced with `cargo clippy -- -D warnings`. | AGENTS.md notes `self.conn.lock().unwrap()` everywhere; a calibrated `unwrap_used`/`expect_used` policy (deny in `commands/`, allow in tests via `clippy.toml`) catches panics in IPC paths without drowning in pedantic noise. CI already runs clippy `--all-targets`, so add `-D warnings`. | Add `[lints.clippy]` to `src-tauri/Cargo.toml`; optional `src-tauri/clippy.toml`. | [Clippy configuration](https://doc.rust-lang.org/clippy/configuration.html), [PocketCmds rules](https://pocketcmds.com/rules/rust/rust-clippy-compliance) |

### Tauri-specific

| Tool | What it does | Why it fits Cull | Install / setup | Source |
|---|---|---|---|---|
| **tauri-plugin-fs-watch / `notify`** | File-system change events surfaced to the app. | Cull already depends on `notify` directly — consider whether the official fs-watch plugin's permissioned API is a cleaner fit for the clipboard/source rescan features than raw `notify`. | Already have `notify = "6"`; evaluate `tauri-plugin-fs-watch` if you want the JS-side API. | [tauri-plugin-fs-watch](https://github.com/tauri-apps/tauri-plugin-fs-watch), [plugins-workspace](https://github.com/tauri-apps/plugins-workspace) |
| **tauri-plugin-webdriver** *(macOS E2E)* | Embeds a WebDriver/automation server **inside** the app (debug builds), giving W3C-style automation on macOS where Apple ships no WKWebView driver. | **Critical for Cull**: standard `tauri-driver`/WebdriverIO/Selenium *do not work on macOS* — which is exactly why Cull fell back to CDP + `tauri-mock.ts`. This (or `tauri-webdriver-automation` / `tauri-pilot`) is the realistic path to driving the *real* Rust backend in E2E instead of the mock. | See `danielraffel/tauri-webdriver` and `tauri-webdriver-automation` crate; add the plugin (debug-only) + CLI. | [Tauri WebDriver docs](https://v2.tauri.app/develop/tests/webdriver/) (notes macOS unsupported), [danielraffel/tauri-webdriver](https://github.com/danielraffel/tauri-webdriver), [tauri-webdriver-automation](https://crates.io/crates/tauri-webdriver-automation) |

> macOS E2E caveat, confirmed: "On desktop, only Windows and Linux are supported due to macOS not
> having a WKWebView driver tool available." Cull's CDP-against-Vite approach is the pragmatic
> workaround; the plugins above are the only way to drive the actual webview/backend on macOS.

### SQLite migration tooling

| Tool | What it does | Why it fits Cull | Install / setup | Source |
|---|---|---|---|---|
| **rusqlite_migration** | Schema migrations tracked via SQLite `user_version` (a single integer, no metadata table); migrations as Rust string slices or a directory of `.sql`; applied atomically. | Cull already has hand-rolled `run_migrations()` and cares about *preserving schema 20 compatibility* (recent commit `8250f7d82`). `user_version`-based tracking matches that "never touch user data" stance and avoids an extra table; lighter than refinery. | `rusqlite_migration = "..."` in `src-tauri/Cargo.toml`; define `Migrations::new(vec![M::up(...)])`. | [rusqlite_migration](https://github.com/cljoly/rusqlite_migration), [docs.rs](https://docs.rs/rusqlite_migration) |
| **refinery** | Fuller migration toolkit (multi-DB), embeds migrations via `embed_migrations!`, detects divergent/missing migrations, optional CLI. | Heavier than needed for a single-file SQLite app, but its divergence detection is useful if migration history gets complex. Lower priority than `rusqlite_migration` for Cull. | `refinery = { features = ["rusqlite"] }`; `embed_migrations!("migrations")`. | [rust-db/refinery](https://github.com/rust-db/refinery) |

---

## 2. Svelte 5 / frontend (the a11y test harness gap)

This is the highest-leverage net-new capability: there is **no component test harness today**, yet
the two P1 accessibility issues both demand automated a11y tests. Build the harness first, then TDD
the modal/dialog primitive and thumbnail grid semantics against it.

| Tool | What it does | Why it fits Cull | Install / setup | Source |
|---|---|---|---|---|
| **vitest-browser-svelte** *(preferred over @testing-library/svelte)* | Renders Svelte 5 components in a **real browser** (Playwright provider) under Vitest browser mode. | The community has moved off `@testing-library/svelte` + jsdom for Svelte 5 specifically because **jsdom mis-handles `$state`/`$derived` reactivity** — exactly the bug class in Cull's memory note ("open-effect reactivity trap"). Real-browser rendering reproduces palette/modal reactivity faithfully. Cull already has Vitest + Playwright-capable CDP infra. | `npm i -D vitest-browser-svelte @vitest/browser playwright`; enable `test.browser` in `vite.config.js`. | [Scott Spence migration guide](https://scottspence.com/posts/migrating-from-testing-library-svelte-to-vitest-browser-svelte), [Svelte testing docs](https://svelte.dev/docs/svelte/testing) |
| **@testing-library/svelte** *(fallback)* | Classic component testing API (`render`, `screen`, `fireEvent`). | Use only if browser-mode setup is too heavy for CI; acceptable for pure-logic components, but expect reactivity false-negatives on rune-heavy components (palette, loupe). | `npm i -D @testing-library/svelte @testing-library/jest-dom jsdom`. | [Svelte testing docs](https://svelte.dev/docs/svelte/testing), [davipon guide](https://davipon.hashnode.dev/test-svelte-component-using-vitest-playwright) |
| **@axe-core/playwright** | Runs the axe accessibility engine inside Playwright/browser-mode tests; returns violations to assert on. | Provides the *automated accessibility assertions* both P1 acceptance criteria require — assert zero axe violations on `TrashConfirmDialog`, `CommandPalette` (focus trap, `aria-modal`), and the `Thumbnail` grid (roles, `aria-selected`, labels). Plug straight into the vitest-browser-svelte / Playwright run. | `npm i -D @axe-core/playwright`; in a test: `new AxeBuilder({ page }).analyze()`. | [John Lewis: automating a11y beyond axe](https://medium.com/john-lewis-software-engineering/automating-a11y-testing-part-2-beyond-axe-af0f32f366e7) |
| **eslint-plugin-svelte (a11y rules)** | Lints a11y at the ESLint layer — broader and more configurable than the compiler warnings, and lintable in CI even where Svelte's compiler warnings are suppressed. | The audit notes `TrashConfirmDialog.svelte` **suppresses Svelte's compiler a11y warnings**. An ESLint a11y gate catches what suppression hides, and runs in `check-ci.sh` frontend. Cull has no ESLint config today — adding one is itself a gap-closer. | `npm i -D eslint eslint-plugin-svelte`; add flat config with the recommended + a11y rules. | [Geoff Rich: a11y limits](https://geoffrich.net/posts/svelte-a11y-limits/), [Svelte a11y warnings](https://svelte.dev/docs/accessibility-warnings) |
| **svelte-check in CI** | Type + a11y compiler diagnostics. | **Already wired** (`npm run check` runs in CI). Keep it; just stop suppressing a11y warnings where the audit flagged it, so this gate has teeth. | (already present) | [Svelte CLI](https://svelte.dev/docs/cli) |
| **Storybook 9/10 + Svelte CSF** *(over Histoire)* | Component workshop / visual catalog with interaction + a11y addons. | Optional but useful for visually iterating the shared modal primitive and Tokyo Night token compliance. **Pick Storybook, not Histoire** — Histoire's Svelte-5 maintenance is effectively stalled, whereas Storybook's Svelte CSF addon is actively maintained for Svelte 5. | `npx storybook@latest init` (Svelte-Vite framework). | [Storybook for Svelte-Vite](https://storybook.js.org/docs/get-started/frameworks/svelte-vite), [Storybook 10 vs Ladle/Histoire](https://dev.to/themachinepulse/storybook-10-why-i-chose-it-over-ladle-and-histoire-for-component-documentation-2omn) |

---

## 3. Claude Code automation for this repo

Cull already has a tiered hook chain (`preflight:hook|quick|full|release`) and bd hooks. The gaps are
(a) an *automated review gate* that doesn't depend on the unreliable `codex` CLI, and (b) project
subagents/skills tuned to Cull's known pitfalls.

| Mechanism | What to add | Why it fits Cull | Source |
|---|---|---|---|
| **PreToolUse review gate (subagent, replaces codex)** | A `PreToolUse` hook matching `git commit`/`git push` that blocks until an adversarial **review subagent** inspects the working-tree diff and emits a `Verdict: CLEAN` tied to that exact diff. Findings must be `file:line` specific (anti-"review theater"). | The prompt explicitly flags `codex` CLI as unreliable here. An in-harness Claude subagent gate is deterministic (the commit literally can't proceed without the verdict) and uses the model you already trust. Run it at the `full`/push tier, not `hook`, so commits stay fast. | [IMTI: pre-commit review gate](https://imti.co/pre-commit-review-gate/), [Claude Code hooks guide](https://code.claude.com/docs/en/hooks-guide) |
| **PostToolUse fmt/clippy guard** | `PostToolUse` matcher on `Edit`/`Write` to `*.rs` that runs `cargo fmt` *inside `src-tauri/`* (AGENTS.md's #1 gotcha: root `cargo fmt` is a silent no-op) and a fast `cargo clippy`/`bacon` check. | Encodes the hard-won "fmt at root is a no-op → push fails on fmt drift" lesson mechanically so it can't recur. | [Claude Code hooks guide](https://code.claude.com/docs/en/hooks-guide) |
| **Project subagents** | (1) **rust-async-reviewer** — flags blocking IO/decode/`conn.lock()` held across `.await` in `commands/` (directly serves the P1 async issue). (2) **svelte-a11y-reviewer** — checks new components for `role`/`aria-*`/focus-trap and suppressed a11y warnings. (3) **migration-guard** — verifies schema changes preserve back-compat (schema-20 lesson). | Each targets a documented Cull failure mode and an open audit issue. | [Claude Code hooks guide](https://code.claude.com/docs/en/hooks-guide) |
| **Slash commands / skills** | `/cull-preflight` (wrap `preflight:full`), `/cull-e2e` (Chrome Beta + Vite + `run-e2e.sh` orchestration), `/cull-a11y` (run vitest-browser + axe on changed components). The repo already exposes a `cull` MCP server + skill. | Turns the multi-step E2E bring-up and the new a11y harness into one-command flows. | [Claude Code hooks guide](https://code.claude.com/docs/en/hooks-guide) |
| **claude-pre-commit (config validation)** | Pre-commit hooks that validate `SKILL.md`, hook configs, and subagent definitions. | Keeps the growing `.claude/` config honest as you add the subagents/gates above. | [freddo1503/claude-pre-commit](https://github.com/freddo1503/claude-pre-commit) |
| **MCP servers** | The repo's own **`cull` MCP server** is the highest-value one — drive `import_files`, `generate_embeddings`, `analyze_image_quality`, `find_similar` directly in agent loops for backend integration testing without the UI. Pair with a filesystem MCP for fixture management. | Gives agents a real-backend test surface that the CDP+mock E2E can't reach on macOS. | (in-repo MCP server; `rmcp` dependency) |

Split-the-workload principle to keep: commit-time = fast lint + touched-module tests; push-time =
wider integration/E2E/review gate. This matches Cull's existing `hook` vs `full` tiers — extend
rather than replace.

---

## 4. Community practice (sources & takes)

- **macOS WebDriver is the defining Tauri-E2E pain point.** Tauri's own docs state desktop WebDriver
  is Windows/Linux only because Apple ships no WKWebView driver. Multiple devs independently built
  fixes in early 2026 (`danielraffel/tauri-webdriver`, `tauri-webdriver-automation`, `tauri-pilot`,
  plus commercial CrabNebula Cloud). Takeaway for Cull: the CDP + `tauri-mock.ts` approach is the
  sane local workaround; if you want real-backend E2E on macOS, adopt an embedded-WebDriver plugin.
  Sources: [Tauri WebDriver docs](https://v2.tauri.app/develop/tests/webdriver/),
  [danielraffel build post](https://danielraffel.me/2026/02/14/i-built-a-webdriver-for-wkwebview-tauri-apps-on-macos/),
  ["I built a CLI to test Tauri apps because nothing else worked"](https://dev.to/mpiton/i-built-a-cli-to-test-tauri-apps-because-nothing-else-worked-3915),
  [Tauri discussion #10123](https://github.com/tauri-apps/tauri/discussions/10123).
- **Playwright + WebKit engine mismatch.** Testing a Tauri app's frontend in Playwright's bundled
  Chromium does *not* match the real macOS WKWebView engine — a known footgun. Cull tests in Chrome
  Beta, so be aware results can diverge from production WKWebView.
  Source: [zudo-tauri Playwright engine pitfall](https://takazudomodular.com/pj/zudo-tauri/docs/frontend/playwright-engine-pitfall/).
- **Svelte 5 testing consensus moved to browser mode.** The recurring community finding is that
  jsdom + `@testing-library/svelte` mis-reports rune reactivity; `vitest-browser-svelte` in real
  browsers is now the recommended default. This squarely matches Cull's logged "open-effect
  reactivity trap." Sources:
  [Scott Spence migration](https://scottspence.com/posts/migrating-from-testing-library-svelte-to-vitest-browser-svelte),
  [Scott Spence browser-mode guide](https://scottspence.com/posts/testing-with-vitest-browser-svelte-guide).
- **Rust tooling consensus.** cargo-nextest is the near-universal recommendation for parallelism +
  flaky-retry; cargo-deny is the standard advisory/license/ban gate; sccache for cache reuse. These
  pay off most past mid-size codebases — Cull's `src-tauri/` qualifies.
  Sources: [JetBrains: faster Rust tests with nextest](https://blog.jetbrains.com/rust/2026/05/01/faster-rust-tests-with-cargo-nextest/),
  ["secret tools real Rust teams use"](https://medium.com/@theopinionatedev/inside-the-secret-tools-real-rust-teams-use-that-cargo-doesnt-want-you-to-know-about-ee22b21be193).
- **Claude Code review-gate pattern.** The community pattern is a mechanical `PreToolUse` gate on
  `git commit` that an adversarial subagent must clear with a diff-bound `Verdict: CLEAN`, with a
  quality bar against "review theater." Sources:
  [IMTI pre-commit review gate](https://imti.co/pre-commit-review-gate/),
  [Husky + Claude quality gates](https://dev.to/myougatheaxo/git-hooks-with-claude-code-build-quality-gates-with-husky-and-pre-commit-27l0).
- **Reddit note:** direct r/rust / r/tauri / r/sveltejs thread retrieval returned nothing usable
  through the available search (the `site:reddit.com` operator and combined-topic queries came back
  empty), so the community takes above are drawn from blogs, official docs, Tauri GitHub
  discussions, and Hacker News rather than Reddit threads specifically.

---

## Top 5 to adopt next (mapped to open audit issues)

1. **vitest-browser-svelte + @axe-core/playwright** — build the component+a11y test harness.
   *Unblocks the P1 a11y issues* ("Make destructive dialogs/palette overlays fully modal" and
   "Expose thumbnail state through keyboard/screen-reader semantics"), whose acceptance criteria
   require automated a11y tests that don't exist. Real-browser mode also guards the logged rune
   reactivity trap. This is the single highest-leverage addition.
2. **cargo-deny (+ cargo-cyclonedx) in the `release` preflight + a CI job** — *closes the P2 CVE/SBOM
   gap*: advisories/bans/license/sources policy in `deny.toml`, CycloneDX SBOM attached to releases.
   `cargo-auditable` builds let you re-scan shipped binaries later.
3. **eslint-plugin-svelte a11y gate in `check-ci.sh` frontend** — catches the a11y issues that
   `TrashConfirmDialog.svelte` currently *suppresses* at the compiler level, and supports the P2
   "promote keyboard/accessibility coverage into automation" issue at lint speed (cheap, fast).
4. **cargo-nextest as the test runner** (in `check-ci.sh` and `preflight`) — faster, isolated,
   retry-capable tests; directly de-risks the *P1 async/blocking* refactor by making the
   import/decode/DB test suite fast and stable enough to iterate on safely.
5. **PreToolUse Claude review-gate subagent (replacing the unreliable `codex` CLI) + a
   `rust-async-reviewer` subagent** — an in-harness, diff-bound `Verdict: CLEAN` gate at push time,
   with a subagent specialized to flag blocking IO / `conn.lock()` held across `.await`, *directly
   supporting the P1 async issue* and giving Cull a trustworthy automated review path.

Honorable mentions: **tauri-plugin-webdriver / tauri-webdriver-automation** (only realistic path to
real-backend E2E on macOS, supporting the P2 keyboard/a11y-in-automation issue);
**rusqlite_migration** (formalize migrations while preserving the schema-20 back-compat the team
already guards); **sccache + split target dir** (local/CI build-loop speed).
