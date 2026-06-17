# Multi-image Preview Prototypes

Generated on 2026-06-15 as first-pass raster prototypes for the live selected-set Preview Display idea.

## Files

- `layout-experiments.html` — interactive playground for tuning recipe, density, gaps, outer spacing, hero weight, rhythm, crop bias, radius, optional borders, border width, grid-preserving drag order, next-set previewing, save shortcut behavior, and metadata display.
- `minimal-masonry-preview.png` — preferred v1 direction. Pinterest-style selected-set layout with minimalist art, logo studies, typography fragments, and clean reference images.
- `minimal-magazine-preview.png` — preferred editorial direction. Best for judging hierarchy across art/logo/reference work.
- `minimal-hero-strip-preview.png` — preferred live-review direction. Keeps one image dominant while the strip shows related marks, posters, packaging, and type studies.
- `minimal-multi-screen-control-room.png` — preferred v2 direction for bounded multi-surface preview.
- `masonry-preview.png` — first-pass layout study. Useful for spatial rhythm, but the content skews too sci-fi/fantasy for the intended product feel.
- `magazine-preview.png` — first-pass editorial layout study.
- `hero-strip-preview.png` — first-pass live-review layout study.
- `multi-screen-control-room.png` — first-pass bounded multi-surface layout study.

## Product Notes

Use the `minimal-*` files as the main visual references. The product should feel closer to Pinterest-style art direction, logo review, and minimalist visual research than sci-fi concept-art browsing. Keep `minimal-multi-screen-control-room.png` as v2 direction only.

Dragging a tile should update the reusable template order, then immediately redraw the layout into the active grid recipe so spacing and alignment stay coherent. The next selected set should inherit that grid-preserving order so people can quickly test whether an arrangement works across different image groups. Saving the displayed preview should have a direct shortcut and should open the containing folder by default after the image is written.

The editorial recipe should avoid brutal cover crops and skinny side strips. It should preserve artwork/logo proportions by default, keep the side grid to realistic column counts, and use density as a spacing/hierarchy control rather than a way to force many images into unusable slivers.
