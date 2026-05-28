# DMG Background

`cull-dmg-background.png` is the macOS installer window background for the Tauri DMG bundle.

The illustration layer was generated with GPT Image on 2026-05-28, then resized to 660x400 and annotated locally with the exact installer instruction text:

```text
Drag Cull into Applications
```

Keep this asset light and do not add label-background bands under the icons. Finder draws icon labels in dark text inside the DMG window, so the base artwork itself needs readable light space around the app and Applications positions.

The DMG layout in `src-tauri/tauri.conf.json` places the Cull app icon and Applications alias over the two empty rounded-square zones.
