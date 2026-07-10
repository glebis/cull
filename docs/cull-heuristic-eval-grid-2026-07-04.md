# Heuristic Evaluation — Cull image library (Grid view)

**Mode:** Evaluation · **Input type:** screenshot · **Date:** 2026-07-04
**Artifact:** `Screenshot 2026-07-03 at 15.59.36.png`
**Caveat:** one static state inspected; interaction-dependent heuristics (H3, H5, H9) are largely N/A.

**Inventory examined:** top window toolbar (traffic-light controls, display icon, center tab/icon cluster with active "Grid" tab, thumbnail-size slider); search bar (`/` prefix, placeholder, EN/mic/esc/× controls, focus ring); SUGGESTED filter chips row; left sidebar ("All Images" dropdown, LIBRARY tree with folder counts, Import Folder / Rebuild thumbnails / Rescan sources buttons); 2×3 thumbnail grid (one selected, GPT badges); bottom status bar (mode, count, selection, flags, shortcut hints).

## Findings by heuristic

### H1 — Visibility of system status · rollup: Sev 1
- **[Sev 1]** Status bar reads "9 selected" but only one thumbnail shows the blue selection border in the viewport; the other 8 selections are off-screen with no indicator to locate them. *Evidence:* bottom status bar "9 selected" vs. single blue-bordered thumbnail (top-center). *Fix:* show selection markers on the scrollbar or a "jump to selection" affordance.
- *Positives checked:* active "Grid" tab highlighted green; "400 / 2522 images" load state; focused search field ring — no issues.

### H2 — Match between system and the real world · rollup: Sev 2
- **[Sev 2]** The status-bar token "B:nsfw:hide" is engineer syntax, not user language — the meaning (a blur/hide filter for NSFW) isn't decodable. *Evidence:* bottom status bar, "B:nsfw:hide". *Fix:* render as a labeled control, e.g. "NSFW: hidden ▾".
- *Note (data, not app):* folder rows show raw UUID names (`019dc14f-ded2-7…`) — that's the user's real folder naming, so it's borderline; the app faithfully reflects the filesystem.

### H3 — User control and freedom · rollup: N/A
- **N/A** — undo/cancel of rating/reject/import actions can't be observed in a static image. *Positive affordance present:* search offers visible `esc` and `×` exits.

### H4 — Consistency and standards · rollup: None
- Checked: `/`-to-search convention, consistent "Label N" chip format, standard macOS window chrome — no inconsistencies found in this state.

### H5 — Error prevention · rollup: N/A
- **N/A** — whether "Rebuild thumbnails" / "Rescan sources" (potentially expensive) confirm before running isn't observable. *Flag to verify in a live pass.*

### H6 — Recognition rather than recall · rollup: Sev 2
- **[Sev 2]** The center toolbar is ~7 unlabeled icons (split-view, list, share, scatter, upload, download…) with no text or visible tooltips, forcing users to recall each by memory. *Evidence:* top-center icon cluster right of the "Grid" tab. *Fix:* add tooltips on hover and/or labels; several glyphs (share vs. scatter) are near-identical.
- **[Sev 1]** The thumbnail-size slider (top-right) has no label or value. *Evidence:* top-right slider with blue knob. *Fix:* add an icon/label or size readout.
- *Positive checked:* SUGGESTED chips and shortcut hints surface options instead of requiring recall.

### H7 — Flexibility and efficiency of use · rollup: None (strength)
- Checked — strong accelerator support: `⌘1`, `/`, `esc`, `Cmd+Shift+H` history, `? Shortcuts`, `Cmd+P` commands, suggested filters, size slider. Serves power users well; no finding.

### H8 — Aesthetic and minimalist design · rollup: Sev 1
- **[Sev 1]** The unlabeled center-icon cluster reads as mild clutter/noise because nothing disambiguates the glyphs. *Evidence:* top-center toolbar. *Fix:* overlaps with H6 — labels/tooltips resolve both. Otherwise hierarchy is clean (grid is the clear hero).

### H9 — Help users recognize, diagnose, recover from errors · rollup: N/A
- **N/A** — no error state captured.

### H10 — Help and documentation · rollup: None
- Checked — `? Shortcuts` and `Cmd+P Commands` give discoverable, in-context help; no issue in this state.

## Top 3 prioritized fixes
1. **H6** — Add tooltips/labels to the unlabeled center toolbar icons (Sev 2) — highest leverage: densest recognition burden, also fixes the H8 clutter.
2. **H2** — Replace the cryptic `B:nsfw:hide` status token with a labeled control (Sev 2).
3. **H1** — Surface off-screen selections (scrollbar markers / "jump to selection") when the count exceeds what's visible (Sev 1).

## Verdict
**Minor issues only** — nothing above Sev 2. A polished, keyboard-efficient pro UI; the gaps are labeling/recognition refinements, not blockers.

_Caveat: a single evaluator finds only ~1/3 of usability problems; 3–5 evaluators are recommended for confidence. This is not a "ready to ship" sign-off — and H3/H5/H9 need a live pass to assess._
