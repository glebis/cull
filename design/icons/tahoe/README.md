# Cull Tahoe Icon Set

This folder is the source of truth for the current Cull app icon direction.

## Folders

- `masters-unmasked/` contains 1024px square PNG masters. These are source images only. Do not bake a rounded-square mask into these files.
- `icon-composer-layers/` contains a two-layer setup per variant: `background.png` and transparent `foreground.png`. Use these in Apple's Icon Composer to create a layered `.icon` document.
- `icon-composer-documents/` contains generated `.icon` document packages that can be opened in Apple's Icon Composer.
- `icon-composer-renders/` contains flattened 1024px exports produced by Apple's `ictool` from the `.icon` documents. These are reference-only because `ictool` adds Liquid Glass-style shading.
- `previews-masked/` contains flat Tahoe-mask previews for quick visual review.
- `icns/` contains flat `.icns` exports created with Apple's `iconutil` for the Tauri/macOS bundle path.

The runtime alternate icons in `src-tauri/icons/variants/`, the web previews in `static/icon-variants/`, and the bundled app icons in `src-tauri/icons/` use the flat masked renderer, not the Icon Composer renders.

Apple's current app icon workflow is Icon Composer-first:
https://developer.apple.com/documentation/Xcode/creating-your-app-icon-using-icon-composer
