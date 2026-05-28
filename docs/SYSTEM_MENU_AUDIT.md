# System Menu Audit

Status: audited against `src-tauri/src/menu.rs`, `src/lib/menu.ts`, `src/lib/keys.ts`, and `src/lib/components/TabBar.svelte` on 2026-05-22.

This document is the release checklist for native menu behavior. Menu labels, shortcuts, frontend handlers, and keyboard shortcuts must stay aligned.

## Summary

| Area | Status | Notes |
|---|---|---|
| App menu | Pass | About, Settings, Services, Hide, Hide Others, Show All, Quit are present. About and Settings open in-app panels. |
| File menu | Pass | Open File and Open Folder are wired to import flows. Close Window uses the native predefined item. |
| Edit menu | Pass | Undo and Redo are wired to Cull's undo stack. Cut, Copy, Paste, Select All use native predefined items. Deselect All clears image selection and is disabled when nothing is selected. |
| Image menu | Pass | Current image/selection actions mirror the context menu and are disabled when no image is focused. |
| View menu | Pass | View labels and shortcuts match the tab bar and keyboard handler. Current view and sidebar state are reflected with checkmarks. |
| Window menu | Pass | Minimize, Zoom, and Bring All to Front use native predefined items. |
| Help menu | Pass | Cull User Guide opens the bundled native Apple Help Book in Tips. |
| Tray menu | Partial | Show Window and Quit are wired. Stats and MCP status are display-only placeholders until dynamic tray status refresh is implemented. |

## App Menu

| Label | Shortcut | Native ID / Event | Handler | Status |
|---|---|---|---|---|
| About Cull | native | `about` | Opens custom About dialog with linked credits | Pass |
| Settings... | `Cmd+,` | `settings` | Opens settings panel | Pass |
| Services | native | predefined services | macOS native | Pass |
| Hide Cull | `Cmd+H` | predefined hide | macOS native | Pass |
| Hide Others | `Option+Cmd+H` | predefined hide others | macOS native | Pass |
| Show All | native | predefined show all | macOS native | Pass |
| Quit Cull | `Cmd+Q` | predefined quit | Tauri native | Pass |

## File Menu

| Label | Shortcut | Native ID / Event | Handler | Status |
|---|---|---|---|---|
| Open File... | `Cmd+O` | `open_file` | Opens file picker, imports selected images, focuses first import | Pass |
| Open Folder... | `Cmd+Shift+O` | `open_folder` | Opens folder picker, imports recursively, scopes grid to folder | Pass |
| Close Window | `Cmd+W` | predefined close window | Tauri native | Pass |

## Edit Menu

| Label | Shortcut | Native ID / Event | Handler | Status |
|---|---|---|---|---|
| Undo | `Cmd+Z` | `undo` | Calls app undo stack and reloads images | Pass |
| Redo | `Cmd+Shift+Z` | `redo` | Calls app redo stack and reloads images | Pass |
| Cut | `Cmd+X` | predefined cut | Native text editing | Pass |
| Copy | `Cmd+C` | predefined copy | Native text editing | Pass |
| Paste | `Cmd+V` | predefined paste | Native text editing | Pass |
| Select All | `Cmd+A` | predefined select all | Native text editing | Pass |
| Deselect All | `Cmd+Shift+A` | `deselect_all` | Clears image selection; disabled when selection is empty | Pass |

## Image Menu

| Label | Shortcut | Native ID / Event | Handler | Status |
|---|---|---|---|---|
| Share... | none | `image_share` | Opens the native macOS Share sheet for the focused image or active selection | Pass |
| Open in Default App | none | `image_open_default` | Opens the focused image through the system default handler | Pass |
| Open With... | none | `image_open_with` | Shows compatible apps from macOS Launch Services, with Choose Application fallback | Pass |
| Reveal in Finder | none | `image_reveal` | Reveals the focused image or selected images in Finder | Pass |
| Rename... | none | `image_rename` | Opens Cull's rename dialog for the focused image | Pass |
| Move to Folder... | none | `image_move_to` | Opens a folder picker and moves the focused image or active selection | Pass |
| Move to Trash | none | `image_trash` | Moves the focused image or active selection to Trash and reloads the current scope | Pass |

## View Menu

| Label | Shortcut | Native ID / Event | Handler | Status |
|---|---|---|---|---|
| Grid | `Cmd+1` | `view_grid` | Navigates to Grid; checked when active | Pass |
| Loupe | `Cmd+2` | `view_loupe` | Navigates to Loupe; checked when active | Pass |
| Compare | `Cmd+3` | `view_compare` | Navigates to Compare; checked when active | Pass |
| Canvas | `Cmd+4` | `view_canvas` | Navigates to Canvas; checked when active | Pass |
| Lineage | `Cmd+5` | `view_lineage` | Navigates to Lineage; checked when active | Pass |
| Embedding Explorer | `Cmd+6` | `view_embeddings` | Navigates to Embedding Explorer; checked when active | Pass |
| Export | `Cmd+0` | `view_export` | Navigates to Export; checked when active | Pass |
| Toggle Sidebar | `Cmd+B` | `toggle_sidebar` | Toggles sidebar visibility; checked when sidebar is visible | Pass |
| Zoom In | `Cmd++` | `zoom_in` | Increases grid thumbnail size and Loupe scale | Pass |
| Zoom Out | `Cmd+-` | `zoom_out` | Decreases grid thumbnail size and Loupe scale | Pass |
| Actual Size | none | `actual_size` | Resets Loupe scale to 1x | Pass |
| Enter Full Screen | native | predefined fullscreen | Tauri native | Pass |

## Window Menu

| Label | Shortcut | Native ID / Event | Handler | Status |
|---|---|---|---|---|
| Minimize | `Cmd+M` | predefined minimize | Tauri native | Pass |
| Zoom | native | predefined maximize | Tauri native | Pass |
| Bring All to Front | native | predefined bring all to front | macOS native | Pass |

## Help Menu

| Label | Shortcut | Native ID / Event | Handler | Status |
|---|---|---|---|---|
| Cull User Guide | none | `help` | Registers and opens the bundled `Cull.help` Apple Help Book in Tips | Pass |

## Tray Menu

| Label | Native ID / Event | Handler | Status |
|---|---|---|---|
| Show Window | `show_hide` | Toggles main window visibility | Pass |
| Loading... | `stats` | Display-only placeholder | Partial |
| MCP: starting... | `mcp_status` | Display-only placeholder | Partial |
| Quit Cull | `quit_app` | Exits app | Pass |

## Release Notes

- README, tab bar, keyboard handler, and native menu now agree that `Cmd+2` is Loupe and `Cmd+3` is Compare.
- Help opens the task-oriented Cull User Guide as a bundled Apple Help Book in Tips instead of the repository README.
- Undo and redo were previously keyboard-only and are now exposed through the native Edit menu.
- Image-specific context actions are now also exposed through the native Image menu.
- Native menu state is now synchronized from Svelte: image actions disable with no focused image, Deselect All disables with no selection, View items are checked, and Toggle Sidebar reflects the sidebar state.
- About Cull now opens Cull's custom in-app About dialog so the release can show the app image and linked credits instead of the default native panel.
- The tray status placeholders should become dynamic before a polished binary release, but they do not block a source release.
