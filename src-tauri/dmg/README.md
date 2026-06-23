# DMG Background

`cull-dmg-background.png` is the macOS installer window background for the Tauri DMG bundle.

The illustration layer was generated with GPT Image on 2026-05-28, then resized to 660x400 and annotated locally with the exact installer instruction text:

```text
Drag Cull into Applications
```

Keep this asset light and do not draw placeholder cards, rounded-square zones, or label-background bands under the icons. Finder draws the real app and Applications icons plus dark icon labels inside the DMG window, so the base artwork itself needs readable light space around those positions.

The DMG layout in `src-tauri/tauri.conf.json` places the Cull app icon and Applications alias over the two empty rounded-square zones. The configured Finder window is taller than the background image so macOS Finder has room for its title bar, tab bar, and bottom path/status chrome without forcing the icon-view area to scroll.
