# System Menu Audit

Status: audited against `src-tauri/src/menu.rs`, `src/lib/menu.ts`, `src/lib/keys.ts`, and `src/lib/components/TabBar.svelte` on 2026-05-21.

This document is the release checklist for native menu behavior. Menu labels, shortcuts, frontend handlers, and keyboard shortcuts must stay aligned.

## Summary

| Area | Status | Notes |
|---|---|---|
| App menu | Pass | About, Settings, Services, Hide, Hide Others, Show All, Quit are present. Settings opens the in-app settings panel. |
| File menu | Pass | Open File and Open Folder are wired to import flows. Close Window uses the native predefined item. |
| Edit menu | Pass | Undo and Redo are wired to Cull's undo stack. Cut, Copy, Paste, Select All use native predefined items. Deselect All clears image selection. |
| View menu | Pass | View labels and `Cmd+1` through `Cmd+7` match the tab bar and keyboard handler. |
| Window menu | Pass | Minimize, Zoom, and Bring All to Front use native predefined items. |
| Help menu | Pass | Cull Help opens the repository README. |
| Tray menu | Partial | Show Window and Quit are wired. Stats and MCP status are display-only placeholders until dynamic tray status refresh is implemented. |

## App Menu

| Label | Shortcut | Native ID / Event | Handler | Status |
|---|---|---|---|---|
| About Cull | native | predefined about | Tauri native | Pass |
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
| Deselect All | `Cmd+Shift+A` | `deselect_all` | Clears image selection | Pass |

## View Menu

| Label | Shortcut | Native ID / Event | Handler | Status |
|---|---|---|---|---|
| Grid | `Cmd+1` | `view_grid` | Navigates to Grid | Pass |
| Loupe | `Cmd+2` | `view_loupe` | Navigates to Loupe | Pass |
| Compare | `Cmd+3` | `view_compare` | Navigates to Compare | Pass |
| Canvas | `Cmd+4` | `view_canvas` | Navigates to Canvas | Pass |
| Lineage | `Cmd+5` | `view_lineage` | Navigates to Lineage | Pass |
| Embedding Explorer | `Cmd+6` | `view_embeddings` | Navigates to Embedding Explorer | Pass |
| Export | `Cmd+7` | `view_export` | Navigates to Export | Pass |
| Toggle Sidebar | `Cmd+B` | `toggle_sidebar` | Toggles sidebar visibility | Pass |
| Zoom In | `Cmd++` | `zoom_in` | Increases grid thumbnail size and Loupe scale | Pass |
| Zoom Out | `Cmd+-` | `zoom_out` | Decreases grid thumbnail size and Loupe scale | Pass |
| Actual Size | `Cmd+0` | `actual_size` | Resets Loupe scale to 1x | Pass |
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
| Cull Help | none | `help` | Opens `https://github.com/glebis/imageview#readme` | Pass |

## Tray Menu

| Label | Native ID / Event | Handler | Status |
|---|---|---|---|
| Show Window | `show_hide` | Toggles main window visibility | Pass |
| Loading... | `stats` | Display-only placeholder | Partial |
| MCP: starting... | `mcp_status` | Display-only placeholder | Partial |
| Quit Cull | `quit_app` | Exits app | Pass |

## Release Notes

- README, tab bar, keyboard handler, and native menu now agree that `Cmd+2` is Loupe and `Cmd+3` is Compare.
- Help was previously a no-op and is now wired.
- Undo and redo were previously keyboard-only and are now exposed through the native Edit menu.
- The tray status placeholders should become dynamic before a polished binary release, but they do not block a source release.
