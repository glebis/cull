# ImageView Design System

## Visual Identity

**Aesthetic:** Terminal-inspired, monospace-first, ultra-dark. "Neovim meets Lightroom."
**Tagline:** Agent-friendly AI image viewer for macOS.

## Color Palette

| Token | Hex | Usage |
|-------|-----|-------|
| `--bg` | `#08080c` | App background |
| `--surface` | `#0c0c12` | Panels, sidebars, tab bar |
| `--border` | `#1a1a2e` | 1px borders, dividers |
| `--text` | `#e0e0e0` | Primary text |
| `--text-secondary` | `#7a7fa0` | Secondary text (WCAG AA compliant) |
| `--green` | `#9ece6a` | Selection, active state, accept |
| `--blue` | `#7aa2f7` | Focus, links, accents |
| `--orange` | `#e0af68` | Star ratings, warnings |
| `--purple` | `#bb9af7` | Tags, special elements |
| `--red` | `#f7768e` | Reject, errors |

## Typography

- **Font:** JetBrains Mono, SF Mono, Fira Code, monospace
- **Base size:** 13px
- **Line height:** 1.5
- **Section headers:** 10px uppercase, letter-spacing 1px, `--text-secondary`
- **Status bar:** 11px monospace

## Layout Principles

- 8px spacing grid
- 1px borders only — no shadows, no gradients
- 0px border-radius (no rounded corners)
- Sidebar: 220px width, collapsible
- Tab bar: 40px height, seamless with title bar
- Status bar: 32px height

## GPT Image Prompt Template

### Full App Window (any view)
```
A pixel-perfect macOS application window mockup for [VIEW_NAME] view of an AI image viewer called 'ImageView'. Complete window with macOS traffic lights (red/yellow/green). Background #08080c. All monospace text (JetBrains Mono style).

TOP BAR: traffic lights top-left, centered monospace tabs 'Grid  Compare  Loupe  Canvas  Lineage  Embeddings  Export' with '[ACTIVE_TAB]' having a green #9ece6a underline.

[SIDEBAR if applicable]: 220px wide, #0c0c12 background, 1px #1a1a2e right border. Sections with tiny uppercase gray headers.

MAIN AREA: [DESCRIBE VIEW CONTENT]

BOTTOM STATUS BAR: 32px, #0c0c12, 1px top border. Left: '[MODE]' in green, stats in gray. Right: keyboard hints in dim gray.

Premium developer tool aesthetic. No rounded corners. No decorative elements. Terminal aesthetic, monospace everything.
```

### Grid View
```
MAIN AREA: [N]x[N] grid of AI-generated image thumbnails. Each thumbnail [SIZE]px with [GAP]px gaps, perfectly aligned. One thumbnail has a 2px #9ece6a green border (selected). Another shows orange star rating ★★★★ overlay bottom-left. Images are diverse [CONTENT DESCRIPTION].

LEFT SIDEBAR: 'LIBRARY' section with 'All Images ([COUNT])'. 'FOLDERS' tree. 'COLLECTIONS' list. 'FILTERS' section with size preset buttons.
```

### Compare View
```
MAIN AREA: Two large images side by side with 1px #1a1a2e vertical divider. Left image has blue #7aa2f7 border (focused). Labels above each: filename in monospace. Below: star rating in orange, decision badge ('ACCEPTED' green pill or 'reviewing' gray). Both images show [CONTENT].

No sidebar (immersive mode).
```

### Loupe View
```
MAIN AREA: Single large image centered on pure black #08080c, taking ~70-80% of viewport. No sidebar.

BOTTOM OVERLAY: Semi-transparent bar (rgba(8,8,12,0.85)) showing: 'filename.png | WxH | format | ★★★★ | ACCEPTED | 150%' with stars orange, status green/red, zoom blue.
```

### Embedding Explorer
```
LEFT PANEL (25%, #0c0c12): 'MODEL' section showing 'CLIP ViT-B/32' or 'Gemini Embedding 2'. Stats: 'N/M images'. Gear icon for settings. 'CLUSTERS' section with named clusters showing count and 4 tiny preview thumbnails per cluster.

RIGHT PANEL (75%): UMAP 2D scatter plot on pure black. Clusters of tiny image thumbnails (8-20px at overview, larger when zoomed). Cluster name labels as floating pills. Clusters clearly separated with distinct colors. Lasso selection with dashed green line.
```

### Canvas View
```
MAIN AREA: Infinite spatial canvas on pure black. Several images freely placed at different positions and sizes. A group outlined with dashed green rectangle labeled 'group name'. Thin gray arrows connecting related images. One image has blue border (focused). Zoom indicator bottom-right.
```

### Lineage View
```
MAIN AREA: Iteration tree flowing left-to-right. Root image on left, arrows branching to variant thumbnails. Each node ~120px with filename below. Prompt diffs shown as tiny monospace labels: '+add flying cars'. Blue border on selected node. Tree structure like a git commit graph.
```

### Collections View
```
MAIN AREA: Reorderable grid with index numbers (01, 02, 03...) in green monospace top-left of each thumbnail. One image being dragged with subtle elevation. Green insertion indicator line.

LEFT SIDEBAR: 'COLLECTIONS' list with active collection highlighted. 'EXPORT' section with preset dropdown and green export button.
```

## Icon / Favicon

**Concept:** Embedding scatter plot — dots clustering in space.
**Style:** Ultra-minimalistic, green (#9ece6a) and blue (#7aa2f7) dots on black (#08080c).
**Variants generated:** favicon-embed-v1 through v4.

```
App icon. Pure black #08080c background. [DESCRIBE DOT ARRANGEMENT]. Ultra minimal, no text, no border. Must be recognizable at 16x16px.
```

## Generated Mockups

| File | View | Description |
|------|------|-------------|
| `imageview-final-grid.png` | Grid | Full window with sidebar, thumbnails, ratings |
| `imageview-final-compare.png` | Compare | Side-by-side cityscapes with accept/reject |
| `imageview-final-loupe.png` | Loupe | Immersive single portrait with overlay bar |
| `imageview-final-embeddings.png` | Embeddings | UMAP scatter with cluster tree |
| `imageview-final-collections.png` | Collections | Numbered reorderable grid with export |
| `imageview-final-canvas.png` | Canvas | Freeform spatial layout with groups |
| `imageview-final-lineage.png` | Lineage | Iteration tree with prompt diffs |
| `imageview-mockup-embeddings-settings.png` | Embeddings | Settings panel with model dropdown, download progress, API keys |
| `imageview-mockup-api-key.png` | Settings | API key input with Get Key link |
| `imageview-target-loupe.png` | Loupe | Target rendering reference |

## Key Design Decisions

1. **No rounded corners** — 0px border-radius everywhere
2. **No borders on thumbnails** in Grid (except selection indicators)
3. **Images never cropped** — `object-fit: contain`, centered in box
4. **Sidebar hidden** in Loupe and Compare (immersive modes)
5. **Slider only in Grid** — hidden in other views
6. **Tab underline** not background highlight for active state
7. **Seamless title bar** — `titleBarStyle: Overlay` with `-webkit-app-region: drag`
8. **Zen mode** (Shift+.) hides ALL chrome including sidebar panels
9. **Status bar** shows context-relevant keyboard hints per view mode
10. **Monospace everywhere** — no sans-serif anywhere in the UI

## Cluster Colors (18 total)
```
#7aa2f7 #9ece6a #e0af68 #bb9af7 #f7768e
#73daca #ff9e64 #2ac3de #c0caf5 #a9b1d6
#41a6b5 #c3e88d #fc5d7c #89b4fa #f5c2e7
#fab387 #94e2d5 #cba6f7
```
