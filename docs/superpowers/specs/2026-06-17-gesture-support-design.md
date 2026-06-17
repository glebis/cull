# Gesture Support Design

Date: 2026-06-17
Branch: codex/gesture-support

## Audit Note

Claude Opus audit was requested, but the local Claude Code call failed with `Invalid API key`. A read-only GPT substitute audit was run against this spec and the nearby Cull files. The findings are incorporated here: avoid overloading wheel as zoom, keep smart zoom distinct from ordinary double-click, define Loupe transform helpers before integration, debounce Canvas persistence, preserve Grid viewport focus when thumbnail size changes, define input/modal suppression, and defer PDF page gestures.

## Summary

Cull should support Apple-style gestures for inspection, curation, and canvas work without making gestures the only way to complete any task. Trackpad is the priority input device. Magic Mouse, mouse wheel, pointer drag, and touch-like input should get equivalent behaviour where the platform exposes enough signal.

The recommended architecture is hybrid:

- Handle common gestures in Svelte/browser events: scroll, wheel, modifier-wheel zoom, gesture events where available, pointer drag, and swipe thresholds.
- Keep all resulting actions semantic: zoom view, pan view, navigate item, edit crop rectangle, reset fit.
- Add a small macOS native gesture bridge only after the web-event implementation is tested on real trackpad and Magic Mouse hardware and a specific gap remains.

The first implementation avoids default three- and four-finger app gestures. Apple documents those gestures as heavily used by macOS for Mission Control, App Expose, desktop reveal, app switching, Launchpad, Look Up, and three-finger drag depending on user settings. Cull can revisit them later as opt-in, native-backed power gestures, but they are not part of the default map.

## Source Guidance

Apple's Human Interface Guidelines recommend standard, familiar gestures, discoverable alternatives, and consistent feedback. Apple's Mac support docs list the system-level gesture vocabulary Cull must not casually conflict with:

- Two-finger scroll, pinch zoom, rotate, smart zoom, and swipe between pages are common app gestures.
- Three-finger drag is an Accessibility setting.
- Three-finger tap can trigger Look Up and data detectors.
- Thumb-plus-three-finger spread/pinch and four-finger swipes are commonly system gestures.
- Magic Mouse supports scroll, smart zoom, Mission Control, and horizontal page/photo swipes with a smaller gesture vocabulary than trackpad.

## Goals

- Make trackpad zoom, pan, and navigation feel native in Grid, Loupe, Compare, and Canvas.
- Preserve explicit controls for important actions, especially crop application.
- Keep behaviour contextual so gestures do not surprise users in dense curation views.
- Share gesture interpretation where possible instead of duplicating fragile per-component logic.
- Support Magic Mouse and wheel/mouse users with equivalent semantics.
- Make the feature testable with pure utility tests for thresholds and routing, plus focused component tests for integration points.

## Non-Goals

- Do not replace keyboard shortcuts, buttons, menus, or context menus.
- Do not apply crops with a gesture alone.
- Do not make three- or four-finger gestures required.
- Do not make unmodified wheel events mean zoom.
- Do not add a broad native event layer before proving the web layer cannot satisfy a specific gesture.
- Do not change Cull's database, import, embedding, or model pipeline behaviour.

## Interaction Model

### Global Principles

- Trackpad is primary.
- Gestures are accelerators, not exclusive controls.
- Two-finger gestures should do most of the work.
- Three- and four-finger gestures are future opt-in work only.
- Unmodified wheel/scroll means scroll or pan, not zoom.
- Zoom requires pinch, a native magnify event, or an explicit modifier-wheel route.
- Gestures should have a movement threshold before changing image focus.
- Crop output stays explicit: button, menu command, or Enter while crop mode is active.
- Gesture behaviour must be disabled while text inputs, dialogs, command palette, context menus, or modal layers own focus.

### Grid

Grid is a dense browsing and selection surface, so gestures are conservative.

- Two-finger vertical scroll: native grid scroll.
- Pinch/spread, native magnify, or modifier-wheel zoom: change thumbnail size through a shared thumbnail zoom command.
- Thumbnail zoom command updates `thumbnailSize`, `gridPreset`, `gridGap`, and `gridScrollTop` together so preset state stays consistent and the item under the pointer remains visually stable where practical.
- Smart zoom gesture: reset to the normal grid preset.
- Horizontal swipe: no image navigation. Grid remains a scroll/browse surface, not a swipe-to-focus surface.
- Pointer drag: unchanged.

Magic Mouse:

- One-finger surface scroll maps to grid scroll.
- Horizontal Magic Mouse swipe does not change focused image in Grid.
- Smart zoom equivalent resets to the normal grid preset if exposed by the web/native layer.

### Loupe

Loupe is the primary inspection surface, so it gets the richest default gesture map.

- Pinch/spread or native magnify: zoom image around the pointer/focal point, clamped to the existing `0.1..20` loupe scale range.
- Modifier-wheel zoom: zoom image around the pointer/focal point.
- Unmodified two-finger vertical/horizontal scroll while zoomed in: pan the zoomed image.
- Unmodified horizontal swipe while image is at fit scale or near rest: navigate previous/next image.
- Swipe threshold: require sufficient horizontal distance and dominance over vertical movement before changing focus.
- Smart zoom gesture: toggle fit and actual-size/last-inspection zoom.
- Pointer drag while zoomed: keep existing pan behaviour.
- Ordinary mouse double-click: keep existing behaviour unless a separate product decision changes it; do not treat mouse double-click as Apple smart zoom.

Loupe transform helpers are required before component integration:

- `computeLoupeFocalZoom(transform, viewport, image, focalPoint, factor)` returns scale and pan values that preserve the focal image point.
- `computeLoupeActualSizeScale(viewport, image)` computes a scale where image pixels map as closely as practical to screen pixels.
- `clampLoupePan(transform, viewport, image)` prevents empty-space drift after zoom or pan.
- `computeLoupeSmartZoom(transform, viewport, image, lastInspectionScale)` toggles fit versus actual-size or last inspection zoom.

PDF images:

- PDF page swipe gestures are deferred from the first implementation.
- Pinch and pan still operate on the rendered page.
- Existing PDF page keyboard/menu events remain unchanged.

Magic Mouse:

- One-finger scroll pans when zoomed.
- Two-finger horizontal swipe navigates items when not actively panning a zoomed image.
- Smart zoom toggles fit/actual-size if exposed.

### Compare

Compare gets navigation gestures only in the first implementation.

- Horizontal swipe follows existing Compare keyboard/menu semantics.
- If two images are explicitly selected, swipe advances the active side replacement using the same logic as existing commands.
- If Compare is showing an adjacent pair from `focusedIndex`, swipe advances the focused adjacent pair by one item.
- Pinch/spread and smart zoom are deferred until Compare has explicit zoom/pan state.

### Canvas

Canvas is a spatial work surface, so gestures prioritize viewport movement and item manipulation rather than item navigation.

- Two-finger scroll: pan the canvas viewport.
- Pinch/spread, native magnify, or modifier-wheel zoom: zoom around pointer using the existing viewport transform semantics in `canvas-interactions.ts`.
- Smart zoom gesture on empty canvas: fit canvas contents or reset viewport.
- Smart zoom gesture on item: focus/fit that item in the viewport.
- Horizontal swipe: pan canvas, not navigate images.
- Rotate gesture: out of scope for the first implementation. Keep existing rotate buttons/keyboard.
- Crop gestures: editing-only. Gestures may move or adjust an existing crop rectangle when crop mode is active, but applying the crop remains explicit.
- Suppress viewport gestures while item drag, resize, or crop-draw state is active unless the gesture target is the active crop edit.
- Persist live viewport changes once on gesture end or after a short idle debounce, not on every high-frequency input event.

Magic Mouse:

- Surface scroll pans the canvas.
- Modifier plus scroll or native zoom signal zooms the viewport.
- Horizontal swipe pans the viewport, not item navigation.

### Crop

Crop support has two distinct contexts:

- Loupe crop creates a derivative image through the Rust `crop_image` command.
- Canvas crop is non-destructive layout metadata on canvas items.

Gesture policy:

- Gestures can edit crop only after crop mode is entered through an explicit existing command or button.
- Gestures can adjust, move, or resize a visible crop rectangle while crop mode is active.
- Applying Loupe crop remains explicit with Apply/Enter.
- Saving Canvas crop metadata can still follow the existing queued-save behaviour after an edit is completed.
- Escape/cancel paths must remain keyboard and button accessible.

## Architecture

### Shared Gesture Interpreter

Add a focused frontend utility module, `src/lib/gesture-interactions.ts`, that converts raw input into semantic gesture intents. It should be pure and testable.

Core types:

```ts
type GestureSurface = 'grid' | 'loupe' | 'compare' | 'canvas';

type GestureIntent =
    | { type: 'zoom'; factor: number; focalX: number; focalY: number; source: GestureSource }
    | { type: 'pan'; deltaX: number; deltaY: number; source: GestureSource }
    | { type: 'navigate'; direction: 'previous' | 'next'; source: GestureSource }
    | { type: 'smart_zoom'; focalX: number; focalY: number; source: GestureSource }
    | { type: 'crop_adjust'; deltaX: number; deltaY: number; source: GestureSource };

type GestureSource = 'trackpad' | 'magic_mouse' | 'wheel' | 'pointer' | 'touch' | 'native_macos';
```

Responsibilities:

- Normalise wheel deltas including `deltaMode`.
- Treat unmodified wheel/scroll as pan or native scroll, not zoom.
- Route zoom only from pinch/native magnify or modifier-wheel inputs.
- Apply thresholds for swipe versus pan.
- Route intents by surface and current state.
- Suppress gestures through `shouldIgnoreGestureTarget(target, state)`.
- Avoid direct store mutation; components or a thin command layer apply intents.

### Input Suppression Contract

Create a shared predicate:

```ts
interface GestureSuppressionState {
    modalOpen?: boolean;
    contextMenuOpen?: boolean;
    commandPaletteOpen?: boolean;
    textEntryOpen?: boolean;
    cropModeActive?: boolean;
}

function shouldIgnoreGestureTarget(target: EventTarget | null, state?: GestureSuppressionState): boolean;
```

It returns true for:

- `input`, `textarea`, `select`, and content-editable targets.
- Elements inside dialogs, command palette, text-entry dialogs, context menus, or modal layers.
- Explicit active state flags from component stores.

### Component Integration

`Grid.svelte`

- Add pinch/native magnify/modifier-wheel thumbnail zoom through the shared utility.
- Keep unmodified scroll native.
- Do not add horizontal navigation.
- Preserve visible focus by adjusting `gridScrollTop` after thumbnail size changes.

`Loupe.svelte`

- Replace direct wheel-only zoom with routed zoom, pan, and navigation intents.
- Preserve existing loupe stores: `loupeScale`, `loupePanX`, `loupePanY`, `focusedIndex`.
- Add pure Loupe transform helpers in a separate module or in `gesture-interactions.ts` before wiring the component.
- Keep crop application explicit.
- Keep ordinary double-click separate from smart zoom.

`Canvas.svelte`

- Route wheel/pinch/pan through shared utilities.
- Generalize `computeCanvasWheelZoom` to accept a zoom factor.
- Keep item drag/resize/crop modes authoritative.
- Debounce or gesture-end persistence for viewport pan/zoom.

`Compare.svelte`

- Add navigation gesture routing only.
- Reuse existing Compare keyboard/menu semantics.

### Optional macOS Bridge

If browser/Tauri events cannot reliably expose required gestures after the web-event implementation is tested on real hardware, add a minimal macOS-only bridge that emits semantic events to the webview:

- `gesture-magnify`
- `gesture-smart-zoom`
- `gesture-swipe`

The bridge should not expose raw low-level AppKit details to components. It should publish the same semantic shape as the Svelte interpreter so tests and consumers stay stable.

## Thresholds And State

Initial threshold constants should live in `gesture-interactions.ts`:

- Swipe minimum horizontal distance: 80 px.
- Swipe horizontal dominance: `abs(dx) >= abs(dy) * 1.5`.
- Navigation cooldown: 250 ms per surface to prevent repeated accidental flips.
- Zoom clamp: reuse surface-specific clamps.
- Pan deadzone: ignore tiny deltas below 1 px after delta normalization.
- Wheel `deltaMode`: pixel is unchanged, line is multiplied by 16, page is multiplied by viewport height.

These values should be unit-tested and easy to tune.

## Accessibility And Discoverability

- All gesture actions must remain available through keyboard/menu/button paths.
- Existing crop controls remain visible and labelled.
- Status hints may mention zoom percent or canvas zoom, but the UI should not rely on instructional text.
- Respect modal/input focus by ignoring global gesture shortcuts while those controls are active.
- Do not intercept macOS system gestures where possible.

## Testing

Unit tests:

- Gesture threshold routing: pan versus navigate.
- Wheel `deltaMode` normalization.
- Zoom source routing: unmodified wheel does not produce zoom.
- Loupe focal zoom, actual-size scale, pan clamp, and smart zoom helpers.
- Surface routing rules for Grid, Loupe, Compare, Canvas.
- Crop editing-only policy.
- Input/modal suppression.
- Grid thumbnail zoom preserves preset consistency.

Component/contract tests:

- Grid pinch/modifier-wheel changes thumbnail size without changing focus and keeps preset/gap state coherent.
- Loupe unmodified wheel pans when zoomed and does not zoom.
- Loupe swipe changes item only when not actively panning a zoomed image.
- Loupe crop cannot be applied by gesture-only intent.
- Canvas pinch updates viewport transform and persists through the existing save path after debounce/gesture end.

Manual verification:

- Trackpad: pinch, scroll, smart zoom, swipe in Grid/Loupe/Canvas.
- Magic Mouse: scroll, smart zoom, horizontal swipe where available.
- Mouse wheel: native scroll/pan plus explicit modifier-wheel zoom.
- macOS System Settings gesture conflicts: Mission Control/app switching remain system-owned.

## Risks

- Browser event support for `gesture*` is inconsistent inside Tauri webviews.
- Trackpad and Magic Mouse can both arrive as wheel-like events, so source detection may be approximate.
- Current Loupe double-click navigates back; keeping that separate from smart zoom may require a native smart-zoom signal for true Apple semantics.
- Over-eager horizontal swipe detection could cause accidental item navigation.
- Native bridge work can become too broad if it tries to mirror all AppKit events.

## Default Decisions

- Loupe ordinary mouse double-click remains separate from Apple smart zoom.
- Grid smart zoom resets to the normal preset instead of cycling through presets.
- Compare receives navigation gestures only in the first implementation.
- PDF page gestures are deferred.
- The native macOS bridge waits until web-event support is tested on real hardware.

## Recommended Implementation Sequence

1. Add pure `gesture-interactions.ts` utilities and tests.
2. Add Loupe transform helpers and tests.
3. Integrate Grid thumbnail zoom and conservative smart zoom.
4. Integrate Loupe focal zoom, pan, and thresholded navigation.
5. Integrate Canvas pinch/pan through shared utilities with debounced persistence.
6. Add Compare navigation routing only.
7. Test manually on trackpad and Magic Mouse.
8. Decide whether a macOS native bridge is needed for remaining gaps.
