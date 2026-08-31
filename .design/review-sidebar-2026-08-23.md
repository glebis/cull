# Sidebar JTBD + review — 2026-08-23

**Epic:** `imageview-1i2k` (11 tasks) · **Corpus:** `~/jtbd/cull/jtbd.json` v2, outcomes `touch: "Sidebar"` (o6–o10, user-rated) · **Verdict:** Needs changes (0 HIGH / 9 MED / 1 LOW)

## The job (from interview)

After an import or mid-session, jump to the recently-relevant scope — folder-first, also generated-or-not, full-text searchable — and hide whatever isn't used, so just-imported material is visible without scroll-hunting. Opportunities (Ulwick): post-import findability **11** · one search model 7 · scope switching 6 · hide-anything 6 · clipboard monitor 4 (guardrail, not gap).

## Findings → task mapping

| # | Sev | Finding | Task |
|---|---|---|---|
| 1 | MED | No persistent pointer to just-imported folder (toast 8s `Sidebar.svelte:949`; tree alphabetical `sidebar-utils.ts:107`) | `.1` p1 |
| 2 | MED | `--text-secondary` APCA Lc −34.9 at 9–10px caps (measured 12 pairs) | `.7` |
| 3 | MED | Three competing filter models; sidebar filter skips detected/canvases | `.2` |
| 4 | MED | Zero/null counts render "0" everywhere | `.3` |
| 5 | MED | Color semantics illegible ("even for me") | `.5` |
| 6 | MED | Five glyph dialects at 8px; no-emoji guardrail | `.4` |
| 7 | MED | Clipboard Monitor buried (6th section); guardrail says promote | `.6` |
| 8 | MED | Twisty ~14px / preset chips ~19px hit areas | `.8` |
| 9 | MED | Bare empty states; SMART vanishes when empty | `.9` |
| 10 | LOW | Clipboard Monitor naming drift; Smart/SMART drift | `.10` |
| — | — | Density modes + scope grid + folder previews (user ideas, from open questions) | `.11` |

## Considered but rejected

"4 (27)" folder counts — intentional + tooltipped · eyebrow caps headers — project's terminal system (DESIGN.md) wins over layout-rules 2 · hover-only pin/… — focus-within covers keyboard, matches user's quiet-chrome direction · import progress — `JobProgressPanel` listens already (visual quality unverified) · preview popover `aria-hidden` — additive only.

## Not verified

Runtime states (no browser-tool rung for the Tauri app this session): hover delays, popover clamping, toast timing, JobProgressPanel visuals, rendered contrast. Craft domains (typography/motion/a11y depth): Not reviewed — `interfaces` plugin not installed. Single-evaluator caveat applies.

## Guardrails (verbatim)

"Don't lose the clipboard monitor — make it even easier" · "+" hover-only, transparent, unboxed · no emoji glyphs like Recent Imports · keep counts, tree, keyboard nav, density.
