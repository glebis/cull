# Multi-image Preview Display Design

## Summary

Cull should add a live multi-image preview mode that lets a user select up to 30 images in Grid and see them immediately recomposed on the Preview Display as a masonry, magazine, hero, strip, or dense overview layout. The feature is for judging a body of work quickly, not for building a final saved board.

The first version should evolve Preview Display from a focused-image presentation surface into a presentation surface that can render either one focused image or a selected-image composition. Grid remains the control surface. Canvas remains the deliberate saved spatial workspace.

## Jobs To Be Done

### Primary job

When I am reviewing a body of visual work and I only understand images one at a time, I want to instantly see selected images composed together in changing editorial layouts, so I can judge whether the work has a coherent visual language, strong pairings, gaps, or standout directions without manually building a board.

### User and trigger

The user is a visual creator or reviewer working with sets of images: AI artist, photographer, art director, stylist, editor, or curator. The trigger is selecting promising images in Grid and realizing that the normal thumbnail grid is too neutral and repetitive to reveal how the images behave as a collection.

### Current workaround

Users manually drag images into Canvas, a Finder folder, a presentation, a contact sheet, or an external layout tool. That is too much commitment when the active question is still whether the images belong together.

### Functional jobs

- Assess a selected set as a composed body of work.
- Notice visual relationships when an image is added or removed.
- Cycle through layout recipes with one command when one arrangement hides the answer.
- Present the composed result on another display while keeping the main Cull window private and operational.
- Later, support live intake where one surface shows the newest image and another shows the emerging set.

### Emotional job

The user wants the moment of recognition: "now I can see the work together." The desired outcome is confidence in visual judgment, not just faster browsing.

### Switch forces

- Push: Grid and Loupe are good at individual inspection but weak at rhythm, relationships, repetition, pairings, gaps, and visual hierarchy across a set.
- Pull: A live magazine-style preview gives immediate compositional feedback without committing to a saved canvas or export.
- Habit: Users already understand Grid selection, Loupe, Compare, and Canvas. They will keep using those if this mode feels separate or fussy.
- Anxiety: Automated recomposition can feel chaotic or gimmicky. Multi-window preview state can become confusing if each display has unclear ownership.

## Product Scope

### Version 1

Version 1 adds one selected-set preview mode to the existing Preview Display:

- A preview feed can be `focused_image` or `selected_set`.
- `selected_set` uses deterministic display order, capped at 30 images.
- If no images are selected, the display falls back to the focused image or shows the existing empty preview state.
- The user can cycle layout recipes with one keyboard shortcut and one View menu command.
- Layout changes affect only presentation state.
- Existing freeze, blank, and overlay behavior remains available.

### Later versions

Later versions can add:

- Multiple named preview surfaces, each with its own feed and layout settings. This is the first follow-up after version 1, but it should stay bounded rather than unlimited.
- A `latest_import` or `active_session` feed for live shoot and generation workflows.
- "Save current composition as Canvas" for deliberate layout work.
- Web stream support for selected-set layouts.

The first version must not implement unlimited preview windows. That is a state-management trap. A future multi-screen version should start with a bounded model, for example up to four named surfaces.

## UX Model

Grid is the control surface. Preview Display is the presentation surface.

The user flow:

1. User opens Preview Display.
2. User switches Preview Display feed to `Selected Set`.
3. User selects images in Grid.
4. Preview Display recomposes the selected images immediately.
5. User presses the layout-cycle shortcut to inspect the same selection through another arrangement.
6. User can freeze or blank the display without changing selection or library state.

The feature should feel live and disposable. It should not ask the user to name, save, or configure anything before showing value.

Selected-set display order should be deterministic:

1. Preserve the current selection store iteration order when it is meaningful.
2. For range selections, sort the selected IDs by their current Grid order.
3. When an ID is not present in the current loaded Grid slice, keep its relative selection-store order after visible images.

This avoids random recomposition while respecting the way users select from the visible grid.

## Layout Recipes

Version 1 should include a small fixed set of recipes:

- Masonry: balanced columns with variable image heights, useful for browsing shape and colour rhythm.
- Magazine: one dominant image with supporting images around it, useful for editorial hierarchy.
- Hero strip: one large image plus a horizontal or vertical sequence, useful for shoot review and campaign direction.
- Dense overview: compact balanced layout, useful for spotting repetition and gaps.

Recipes should be deterministic for a given ordered image set and viewport size. Cycling layout recipes should not randomly reshuffle the set unless a later explicit "shuffle composition" command is added.

First-pass raster prototypes live in `docs/prototypes/multi-image-preview/`. Use `masonry-preview.png` and `hero-strip-preview.png` as the primary v1 visual references; keep `multi-screen-control-room.png` as v2 direction only.

## State Model

Add a presentation state concept for preview feed and layout recipe. The state should be separate from library data and should not mutate images, ratings, selections, collections, canvases, or files.

Suggested shape:

```ts
type PreviewFeed =
    | { type: 'focused_image' }
    | { type: 'selected_set'; limit: 30 };

type PreviewLayoutRecipe =
    | 'single'
    | 'masonry'
    | 'magazine'
    | 'hero_strip'
    | 'dense_overview';
```

Existing `PreviewState` currently centres on `image_id`, display mode, overlay, freeze, and blank. The design should extend that model carefully rather than bolting selected IDs into arbitrary component state. For version 1, the main window can compute the ordered selected ID list and sync it to Preview Display as part of presentation state.

## Architecture

### Frontend

- Add a pure layout utility that converts image dimensions, container dimensions, selected order, and recipe into positioned layout items.
- Add a Preview Display renderer branch for multi-image layouts.
- Keep single-image rendering as the default path.
- Extend preview display stores with feed and recipe state.
- Add commands in the View menu and keyboard handling for feed toggle and recipe cycling.

### Backend and IPC

The existing Preview Display state is retrieved with `getPreviewState`, updated with `updatePreviewState`, and pushed to the preview window via `preview:state-changed`. Version 1 can extend this mechanism to include selected image IDs and layout settings.

The preview window should load selected images through `getImagesByIds`. The cap of 30 protects decode cost and keeps the UX legible.

### Canvas boundary

Canvas should not be the implementation substrate for version 1. It already represents saved spatial documents. This feature is a live presentation mode. The bridge to Canvas should be a later explicit command that copies the current selected-set composition into a saved canvas document.

## Error Handling

- If an image is missing, omit it from the composition and show a small unavailable tile only if omission would make the count misleading.
- If selected image loading fails entirely, show the existing preview error style.
- If more than 30 images are selected, use the first 30 in selection order and surface the cap in the preview header or status text.
- If the Preview Display is frozen, selected-set changes should not alter the visible composition.
- If the Preview Display is blanked, no selected-set images should be visible.

## Performance

The selected-set cap is 30. Layout calculation must be pure and cheap enough to run on selection changes and resize. Image loading should prefer browser-displayable originals when appropriate, with the same RAW and fallback behavior already used by Preview Display.

The first version should not add background thumbnail generation or new cached image pipelines. If layout rendering exposes decode stalls, fix that with targeted preloading inside the preview display path.

## Accessibility and Controls

The preview window is mostly presentation-only, but controls must remain reachable through the existing View menu and keyboard shortcuts. The layout cycle command needs a discoverable menu label. The preview header should expose the current feed and layout recipe in text while visible.

## Testing

Add focused tests for:

- Layout utility determinism and bounds.
- Selected-set cap behavior.
- Feed fallback when no images are selected.
- Freeze and blank behavior with selected-set feed.
- Preview state serialization and parsing.
- Keyboard/menu command routing.

For UI/browser coverage, use the existing manual browser smoke policy when the changed files fall under covered UI areas.

## Guardrails

- Do not make this another regular grid.
- Do not make Canvas the default mental model.
- Do not allow unlimited preview windows in version 1.
- Do not mutate user data from presentation changes.
- Do not require saving before the user sees the selected images together.
- Do not add mock Tauri invoke fallbacks.

## Evidence Notes

This design is based on product direction from the project owner, not an external user interview. Evidence weaknesses:

- No observed customer interview quote yet.
- The selected-set limit of 30 is a product judgment, not measured data.
- The owner selected the multi-screen control-room direction in the visual companion. That supports prioritising bounded multi-surface follow-up work after the first live selected-set preview exists.
- Tethered-camera value is plausible from the stated workflow but should be validated after selected-set and bounded multi-surface preview basics exist.
