---
name: Cull
description: Local-first image curation for people and agents.
colors:
  archive-black: "#08080c"
  panel-black: "#0c0c12"
  rule-blueblack: "#1a1a2e"
  rule-strong: "#2a2a42"
  ink: "#e0e0e0"
  quiet-ink: "#7a7fa0"
  command-blue: "#7aa2f7"
  accept-green: "#9ece6a"
  warning-amber: "#e0af68"
  metadata-purple: "#bb9af7"
  reject-red: "#f7768e"
  field-border: "#30324a"
  placeholder: "#8a8da3"
typography:
  display:
    fontFamily: "Geist, ui-sans-serif, -apple-system, BlinkMacSystemFont, Helvetica Neue, Arial, sans-serif"
    fontSize: "clamp(42px, 7vw, 86px)"
    fontWeight: 700
    lineHeight: 0.92
    letterSpacing: "0"
  headline:
    fontFamily: "Geist, ui-sans-serif, -apple-system, BlinkMacSystemFont, Helvetica Neue, Arial, sans-serif"
    fontSize: "clamp(30px, 4vw, 52px)"
    fontWeight: 700
    lineHeight: 0.96
    letterSpacing: "0"
  title:
    fontFamily: "Geist, ui-sans-serif, -apple-system, BlinkMacSystemFont, Helvetica Neue, Arial, sans-serif"
    fontSize: "18px"
    fontWeight: 700
    lineHeight: 1.25
    letterSpacing: "0"
  body:
    fontFamily: "JetBrains Mono, ui-monospace, SFMono-Regular, Menlo, Monaco, Consolas, monospace"
    fontSize: "14px"
    fontWeight: 400
    lineHeight: 1.65
    letterSpacing: "0"
  label:
    fontFamily: "JetBrains Mono, ui-monospace, SFMono-Regular, Menlo, Monaco, Consolas, monospace"
    fontSize: "13px"
    fontWeight: 400
    lineHeight: 1.5
    letterSpacing: "0"
rounded:
  sm: "4px"
  logo: "5px"
spacing:
  unit: "8px"
  xs: "8px"
  sm: "16px"
  md: "24px"
  lg: "40px"
  xl: "64px"
components:
  button-primary:
    backgroundColor: "{colors.command-blue}"
    textColor: "{colors.archive-black}"
    typography: "{typography.title}"
    rounded: "{rounded.sm}"
    padding: "0 20px"
    height: "50px"
  input-field:
    backgroundColor: "{colors.panel-black}"
    textColor: "{colors.ink}"
    typography: "{typography.title}"
    rounded: "{rounded.sm}"
    padding: "0 16px"
    height: "50px"
  product-frame:
    backgroundColor: "{colors.panel-black}"
    textColor: "{colors.ink}"
    rounded: "{rounded.sm}"
    padding: "0"
  decision-chip-accept:
    backgroundColor: "{colors.accept-green}"
    textColor: "{colors.archive-black}"
    typography: "{typography.label}"
    rounded: "{rounded.sm}"
    padding: "2px 6px"
  decision-chip-reject:
    backgroundColor: "{colors.reject-red}"
    textColor: "{colors.archive-black}"
    typography: "{typography.label}"
    rounded: "{rounded.sm}"
    padding: "2px 6px"
---

# Design System: Cull

## 1. Overview

**Creative North Star: "The Local Contact Sheet"**

Cull should feel like a serious desktop curation room: dark, quiet, precise, and built around the image archive rather than the brand chrome. The system borrows the discipline of a terminal, the patience of a photo contact sheet, and the command density of a macOS power tool. It is technical without becoming novelty developer cosplay.

The dominant atmosphere is a dim review session on a Mac: hundreds of generated images, prompt metadata, ratings, decisions, collections, and an agent that can use the same command surface as the human. The interface should feel local-first and privacy-aware through restraint, visible file/state language, and predictable controls, not fear marketing.

The system explicitly rejects generic SaaS, corporate AI boosterism, privacy fear marketing, cute onboarding, vague productivity software, decorative gradient blobs, glassmorphism, rounded marketing cards everywhere, clinical feature grids, noisy animations, and copy that explains obvious UI mechanics.

**Key Characteristics:**
- Dark Tokyo Night base, one-pixel rules, and minimal tonal layering.
- Monospace-first app UI, with Geist reserved for landing-page hierarchy and signup readability.
- 8px spacing grid, 4px radius, and predictable keyboard-oriented surfaces.
- Accent colors carry state: blue for command/focus, green for accept, amber for warning/rating, red for reject/error.
- Image content and decision state are the main signal. Chrome stays quiet.

## 2. Colors

Cull uses a restrained Tokyo Night palette: tinted blacks, cool blue-gray rules, and sparse semantic accents.

### Primary
- **Command Blue**: Primary actions, focus affordances, links, command surfaces, active control emphasis.
- **Accept Green**: Accepted decisions, successful status, selected keepers, and positive completion.

### Secondary
- **Warning Amber**: Star ratings, warnings, attention states, and focus outlines on the landing page signup form.
- **Metadata Purple**: Technical metadata, model/provider tags, and special informational lines.

### Tertiary
- **Reject Red**: Rejected decisions, destructive or failed states, and explicit error messaging.

### Neutral
- **Archive Black**: App and landing page background. This is the default environment for image review.
- **Panel Black**: Sidebars, panels, framed product shots, inputs, and raised control surfaces.
- **Rule Blueblack**: One-pixel dividers, borders, and quiet structural lines.
- **Rule Strong**: Stronger panel boundaries and form field borders when the default divider is too faint.
- **Ink**: Primary text. Use sparingly on dense screens so labels and images can breathe.
- **Quiet Ink**: Secondary text, explanatory copy, status text, and low-priority metadata.
- **Placeholder**: Input placeholder text. It must remain legible without competing with real values.

### Named Rules

**The Semantic Accent Rule.** Blue, green, amber, purple, and red are state and command colors. They are not decoration.

**The One-Pixel Rule.** Dividers and frame borders are 1px. Do not thicken borders to create emphasis unless the element is a real focus or selection state.

**The No-Blob Rule.** Decorative gradient blobs, bokeh, and ambient color clouds are forbidden. Cull's atmosphere comes from dark surfaces, real images, and precise state colors.

## 3. Typography

**Display Font:** Geist with system sans fallbacks.
**Body Font:** JetBrains Mono with system monospace fallbacks.
**Label/Mono Font:** JetBrains Mono with system monospace fallbacks.

**Character:** The app is mono-forward because Cull is a keyboard and agent tool. The landing page may use Geist for H1, H2, H3, signup labels, inputs, and buttons so marketing hierarchy stays readable and does not feel like a terminal screenshot.

### Hierarchy
- **Display** (700, `clamp(42px, 7vw, 86px)`, 0.92): Landing page hero only. It should be large, plain, and direct.
- **Headline** (700, `clamp(30px, 4vw, 52px)`, 0.96): Major landing sections and long-form feature statements.
- **Title** (700, 18px, 1.25): Feature claim titles, workflow item headings, compact panel titles, primary controls.
- **Body** (400, 14px, 1.65): Landing page body copy and explanatory text. Keep prose capped near 65-75ch.
- **Label** (400, 13px, 1.5): Eyebrows, metadata, status lines, technical strings, command hints, and dense UI labels.

### Named Rules

**The Mono-With-Purpose Rule.** Use JetBrains Mono for app chrome, commands, metadata, status, and technical content. Do not use monospace as a lazy shorthand for every marketing heading or form control.

**The Sans-For-Reading Rule.** Use Geist when the user needs to quickly parse a large heading, form label, or action. The header and signup block must never regress to oversized mono text.

**The No-Explanation Copy Rule.** Do not write UI copy that explains obvious mechanics. Labels should tell users what changes, what is safe, or what action is available next.

## 4. Elevation

Cull is flat by default. Depth is conveyed through tonal layers, one-pixel borders, and selection/focus treatments rather than decorative shadows. The only substantial shadow vocabulary appears in overlays such as the Command Palette, where it separates a temporary command surface from the image workspace.

### Shadow Vocabulary
- **Overlay Shadow** (`0 24px 88px color-mix(in srgb, var(--bg) 82%, transparent)`): Command Palette and modal-like command surfaces only.
- **Compact Overlay Shadow** (`0 16px 44px color-mix(in srgb, var(--bg) 78%, transparent)`): Smaller popovers or menus where a full overlay shadow would feel heavy.

### Named Rules

**The Flat-Until-Transient Rule.** Resting panels, cards, images, sidebars, and controls do not use shadows. Shadows are reserved for temporary overlays that must float above the current task.

**The Surface-Not-Card Rule.** Avoid marketing-style cards. Use full-width bands, rows, panels, or framed tools. Cards are allowed only for repeated items or modal surfaces where framing is functional.

## 5. Components

### Buttons
- **Shape:** Compact rectangular controls with gently curved edges (4px radius).
- **Primary:** Command Blue background with Archive Black text, 50px height on the landing page, 44px or larger touch target minimum everywhere.
- **Hover / Focus:** Focus is explicit: 2px outline in the relevant focus color with a 2-3px offset. Hover may tint the background, but must not introduce decorative motion.
- **Secondary / Ghost:** App UI uses bordered or transparent buttons on Panel Black. Use Blue text for command actions, Green for accepted/success states, and Red only for destructive/error states.

### Chips
- **Style:** Small, dense, state-colored labels with 4px radius. Use Green for accepted, Red for rejected, Amber for rating/warning, Purple for metadata tags, and Blue for model/command tags.
- **State:** Chips communicate state or metadata. Do not use them as decorative badges.

### Cards / Containers
- **Corner Style:** Standard 4px radius. The logo frame may use 5px if matching the real app asset.
- **Background:** Panel Black on Archive Black.
- **Shadow Strategy:** No shadows at rest. Use 1px borders and tonal contrast.
- **Border:** Rule Blueblack or Rule Strong, always 1px unless indicating active focus/selection.
- **Internal Padding:** Use the 8px grid. Common steps are 8px, 16px, 24px, 40px, and 64px.

### Inputs / Fields
- **Style:** Panel Black fill, Rule Strong border, 4px radius, 50px height on the landing page, 16px horizontal padding.
- **Focus:** Use a visible 2px focus outline. The landing page uses Warning Amber; the app uses Command Blue.
- **Error / Disabled:** Error text and borders use Reject Red. Disabled controls keep their structure visible and reduce opacity, never disappear.

### Navigation
- **Style:** The app uses top tabs, sidebars, command palettes, and keyboard shortcuts. Navigation copy is short, technical, and stable.
- **Typography:** JetBrains Mono for app navigation and command surfaces. Geist is acceptable for the public site header and marketing hierarchy.
- **Default / Hover / Active:** Active state should be a precise underline, border, tint, or text color shift. Avoid large filled pills unless the control is a real selected mode.
- **Mobile Treatment:** Collapse nonessential chrome, keep signup controls full width, and hide decorative product screenshots when they compete with the message.

### Product Shot
- **Style:** A framed app state, not an illustration. It should show image decisions, batch counts, prompt metadata, and agent queue where possible.
- **Role:** The product shot proves Cull is a real curation tool. Do not replace it with abstract gradients, icons, or generic AI art.

### Command Palette
- **Style:** Dense overlay with Panel Black background, Rule Blueblack borders, compact rows, shortcut badges, and an overlay shadow.
- **Role:** The Command Palette is a core product metaphor. It connects keyboard flow, fuzzy search, recent commands, and agent-readable command surfaces.

## 6. Do's and Don'ts

### Do:
- **Do** keep the image archive as the main content. Chrome should help users inspect, compare, and decide without competing for attention.
- **Do** make local-first trust visible through clear copy, file/state language, and confirmation flows.
- **Do** use Command Blue for focus, links, and primary commands; Accept Green for accepted/success states; Reject Red for failure/rejection.
- **Do** preserve keyboard flow. Major actions should be reachable without a mouse and legible in the Command Palette.
- **Do** use the 8px grid and 4px radius. Small deviations need a local reason, such as the logo asset.
- **Do** keep signup and confirmation copy concrete: what happens next, what is safe, and what the user controls.
- **Do** keep all interactive targets at least 44px in both dimensions when they are clickable or touchable.
- **Do** use real Cull product imagery or app-state renders on the public site.

### Don't:
- **Don't** make Cull look or sound like generic SaaS.
- **Don't** use corporate AI boosterism, privacy fear marketing, cute onboarding, or vague productivity software language.
- **Don't** add decorative gradient blobs, glassmorphism, rounded marketing cards everywhere, clinical feature grids, or noisy animations.
- **Don't** write copy that explains obvious UI mechanics.
- **Don't** dilute the terminal-inspired, monospace-first app UI with unrelated type, color, or component patterns.
- **Don't** use gradient text, side-stripe card accents, or hero metric templates.
- **Don't** use color as decoration. Accent colors must map to command, state, metadata, warning, success, or rejection.
- **Don't** reintroduce old ImageView naming, stale 0px-radius guidance, or source-available positioning. Cull is Apache-2.0 open source.
