# Image Context Menu Audit

Status: researched and implemented macOS share/menu follow-up slice on 2026-05-21.

## Sources Checked

- Apple Human Interface Guidelines, context menus: keep item menus relevant, short, consistent, available elsewhere, and use one submenu level when needed. Source: https://developer.apple.com/design/human-interface-guidelines/context-menus
- Apple macOS Share menu help: Finder and apps expose the system Share button/menu for one or more selected items, with services controlled by Sharing Extensions settings. Source: https://support.apple.com/en-ie/guide/mac-help/mh40614/mac
- Apple AppKit `NSSharingServicePicker`: native macOS share picker for one or more items; file URLs share file contents. Source: https://developer.apple.com/documentation/appkit/nssharingservicepicker
- Apple App Extension Programming Guide, Share: Share extensions are separate extension targets with their own `Info.plist`, `NSExtensionPointIdentifier`, and activation rules. Source: https://developer.apple.com/library/archive/documentation/General/Conceptual/ExtensibilityPG/Share.html
- Apple App Extension Programming Guide, creating extensions: macOS extension templates include sandbox and user-selected file entitlements by default, and Developer ID signing affects extension availability. Source: https://developer.apple.com/library/archive/documentation/General/Conceptual/ExtensibilityPG/ExtensionCreation.html
- Apple `NSServices` Info.plist key: Services are advertised through an app's `NSServices` property and appear in the macOS Services menu. Source: https://developer.apple.com/documentation/bundleresources/information-property-list/nsservices
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

This means Cull is not missing the core culling context menu. The gap was macOS integration around moving the chosen file out of Cull into the rest of the system.

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
   - Implemented with a native `Image` menu.

5. Finder-level integration.
   - `CFBundleDocumentTypes` handles Finder "Open With" registration.
   - A Share Extension would make Cull appear inside other apps' Share sheets.
   - Services/Quick Actions are separate follow-up work, not required for the in-app share menu.

## Implemented Slice

- Added `Share...` to image context menus. It shares the clicked image, or all selected images when the clicked image is part of the selection.
- Added a Rust `share_images` command that resolves image IDs to source file paths and presents AppKit's native `NSSharingServicePicker` on macOS.
- Added `Open in Default App` for single-image context menus.
- Added `Open With...` for single-image context menus. The first version uses a native folder picker pointed at `/Applications` and validates the selected `.app` bundle before launching the image with `open -a`.
- Added a native macOS `Image` menu with Share, Open in Default App, Open With, Reveal in Finder, Rename, Move to Folder, and Move to Trash.
- Expanded Copy to support paths, filenames, and file URLs, with multi-select newline output.
- Fixed context-menu keyboard traversal to follow actual visible menu items instead of sparse hard-coded indexes.

## Inbound macOS Integration Decision

Recommended path: do not build inbound sharing until the app has a clear import/review workflow for files sent from other apps. When we do build it, use a macOS Share Extension first; treat Services/Quick Actions as a secondary Finder automation surface.

Why:

- A Share Extension is the correct way for Cull to appear inside other apps' Share sheets for images and file URLs.
- The implementation is not a Tauri menu change. It requires a separate macOS extension target, its own `Info.plist`, `NSExtensionPointIdentifier = com.apple.share-services`, an `NSExtensionActivationRule` limited to image/file URL payloads, extension entitlements, signing, and a handoff mechanism to the containing app.
- The handoff should write received file references or staged imports into an App Group container, then open Cull through the existing deep-link/open-file path. The extension should not try to share the main app's Rust/Tauri runtime.
- Services via `NSServices` are useful for Finder-style commands such as "Open in Cull" or "Import to Cull", but they are less visible in modern Share-sheet workflows and should not be the first inbound integration unless Finder automation becomes the explicit goal.
- Quick Look extensions are not appropriate for this need; they preview custom file types and do not solve inbound image import/share.

Follow-up implementation task to keep: prototype a Share Extension only after we settle the exact inbound UX: import immediately, open as temporary review set, or add to current collection/session.

## Next Decisions

- Replace the folder-based `Open With...` picker with a recent-app submenu or a Launch Services app list if users need faster repeated handoff to editors.
- Add color labels only if we commit to using them in filters/smart collections; otherwise they are UI clutter.
- Add Export Selected to the context menu only if it deep-links into the existing Export view without creating a second export path.
