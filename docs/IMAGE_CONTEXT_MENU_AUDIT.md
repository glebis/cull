# Image Context Menu Audit

Status: researched and implemented first macOS share slice on 2026-05-21.

## Sources Checked

- Apple Human Interface Guidelines, context menus: keep item menus relevant, short, consistent, available elsewhere, and use one submenu level when needed. Source: https://developer.apple.com/design/human-interface-guidelines/context-menus
- Apple macOS Share menu help: Finder and apps expose the system Share button/menu for one or more selected items, with services controlled by Sharing Extensions settings. Source: https://support.apple.com/en-ie/guide/mac-help/mh40614/mac
- Apple AppKit `NSSharingServicePicker`: native macOS share picker for one or more items; file URLs share file contents. Source: https://developer.apple.com/documentation/appkit/nssharingservicepicker
- Capture One external editor flow: `Image > Open With`, also available by right-clicking an image in the browser. Source: https://support.captureone.com/hc/en-us/articles/360002627737-Opening-images-in-an-external-editor
- Capture One deletion flow: right-clicking browser images exposes move-to-trash and delete-from-disk variants. Source: https://support.captureone.com/hc/en-us/articles/360002546757-Deleting-images-and-variants-in-general
- Capture One rating/tag flow: rating and color tag changes are available from thumbnail/viewer right-click menus. Source: https://support.captureone.com/hc/en-us/article_attachments/360007177777
- Adobe Lightroom Classic shortcuts: `Command+R` shows in Finder, `Command+Shift+E` exports, `Command+E` edits in Photoshop, and macOS delete flows map selected photos to Trash. Source: https://helpx.adobe.com/lightroom-classic/help/keyboard-shortcuts.html

## Current Cull Menu

Already present:

- Rating: 0-5 stars.
- Decision: select, reject, clear.
- Collection actions: add, create, remove from active collection.
- Visual search: find similar.
- File management: reveal in Finder, rename, move to folder, trash.
- Multi-selection handling for most curation and file operations.

This means Cull is not missing the core culling context menu. The gap is macOS integration around moving the chosen file out of Cull into the rest of the system.

## Definitely Lacking On macOS

1. Native Share sheet from an image context menu.
   - Users expect AirDrop, Messages, Mail, Notes, Shortcuts, and installed share extensions to appear through the system share picker.
   - This should share the selected file URLs, not generated thumbnails or copied paths.

2. Open in external/default app.
   - Capture One exposes Open With from browser right-click. Cull at least needs "Open in Default App" before building a full app picker.

3. Richer copy options.
   - Copy Path existed, but multi-select copied only the clicked image path.
   - Useful macOS-adjacent options are filenames and `file://` URLs for scripting/automation.

4. Menu bar parity for image-specific commands.
   - Apple guidance says context-menu commands should also exist in the main interface on macOS.
   - Cull has native app menus, but image-specific Share/Open/Reveal/Trash commands still live only in the context menu or keyboard paths.

5. Finder-level integration.
   - `CFBundleDocumentTypes` handles Finder "Open With" registration.
   - A Share Extension would make Cull appear inside other apps' Share sheets.
   - Services/Quick Actions are separate follow-up work, not required for the in-app share menu.

## Implemented Slice

- Added `Share...` to image context menus. It shares the clicked image, or all selected images when the clicked image is part of the selection.
- Added a Rust `share_images` command that resolves image IDs to source file paths and presents AppKit's native `NSSharingServicePicker` on macOS.
- Added `Open in Default App` for single-image context menus.
- Expanded Copy to support paths, filenames, and file URLs, with multi-select newline output.
- Fixed context-menu keyboard traversal to follow actual visible menu items instead of sparse hard-coded indexes.

## Next Decisions

- Add a main-menu `Image` or `Photo` menu with Share, Open in Default App, Reveal in Finder, Rename, Move to, and Trash for macOS parity.
- Decide whether "Open With..." should be a native app-picker flow or a small recent-app submenu. The default-app action is a useful minimum, but it is not full Capture One parity.
- Add color labels only if we commit to using them in filters/smart collections; otherwise they are UI clutter.
- Add Export Selected to the context menu only if it deep-links into the existing Export view without creating a second export path.
