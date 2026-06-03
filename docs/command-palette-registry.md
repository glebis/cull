# Command Palette Registry Contract

Status: design note for `imageview-zu0.1`.

Cull has several command surfaces today: global shortcuts in `src/lib/keys.ts`, native menu events in `src/lib/menu.ts` and `src-tauri/src/menu.rs`, thumbnail context menu actions in `src/lib/components/ContextMenu.svelte`, search actions in `src/lib/components/CommandBar.svelte`, sidebar navigation/actions in `src/lib/components/Sidebar.svelte`, and direct Tauri calls in view components. The palette registry should become the shared contract for those surfaces without changing command behavior in the first migration slice.

## Registry Item

The frontend helper contract is implemented in `src/lib/command-palette.ts`.

```ts
type CommandPaletteItemKind = 'command' | 'destination';

interface CommandPaletteItem {
  id: string;
  title: string;
  subtitle?: string;
  category: string;
  kind: CommandPaletteItemKind;
  keywords?: string[];
  defaultShortcut?: string;
  disabled?: boolean;
  when?: () => boolean;
  run: () => void | Promise<void>;
}
```

Rules:

- `id` is stable and dot-namespaced, for example `app.search`, `view.grid`, `image.rating.5`, `scope.collection.<id>`.
- `title`, `subtitle`, `category`, and `keywords` are searchable.
- `kind: 'destination'` is used for navigation targets such as folders, collections, smart collections, and all images.
- `when` hides a command entirely when the current context does not apply.
- `disabled` keeps unavailable commands visible when useful, but prevents execution.
- `run` is the only execution entry point; native menu handlers, keyboard handlers, context menus, and future UI rows should delegate to the same command implementation once migrated.

## Categories

Initial registry categories:

- `App`: search, settings, reload, sidebar, zen mode.
- `View`: grid, loupe, compare, canvas, lineage, embeddings, export, Tinder, view history.
- `Edit`: undo, redo.
- `Selection`: select all, clear selection, add/remove selected images.
- `Image`: trash, permanent delete, rating, decision, rotate, reveal, copy path, rename, move.
- `Destination`: all images, folder, collection, smart collection, detected-class scope, session, canvas.
- `Collections`: create collection, pin/unpin collection, add to collection, remove from collection, delete collection.
- `Search`: open image search, apply query, save smart collection.
- `AI`: source rescan, object detection, NSFW detection, vision analysis, embedding generation, model download.
- `Batch`: regenerate thumbnails, batch export, batch move, batch analyze.
- `MCP/Settings`: MCP settings, privacy settings, publishing settings.

## Context Inputs

The registry builder can read Svelte stores and must treat them as the command context:

- active view: `viewMode`, `zenMode`, compare/loupe/canvas state.
- focused image: `images`, `focusedIndex`, `focusedImage`, `activeSession`.
- selection: `selectedIds`, selected count, collect mode state.
- active scope: `activeFolder`, `activeCollection`, `activeSmartCollection`, `activeDetectedClass`, `activeSession`, `activeCanvas`.
- navigation inventory: `folders`, `collections`, `smartCollections`, `sessionCanvases`.
- feature state: `showDetectionBoxes`, `showDetectionInspector`, AI model availability, privacy/cloud settings.
- UI modals: `searchOpen`, `settingsOpen`, future `commandPaletteOpen` and `commandPaletteMode`.

Commands that need additional state should take it through an explicit context builder, not by querying DOM nodes from command rows.

## Shortcut Policy

Default macOS shortcuts:

- `Cmd+K`: universal command palette with commands and destinations.
- `Cmd+P`: command-only palette and the native **View > Command Palette...** accelerator. Cull does not currently expose Print, so `Cmd+P` follows Obsidian-style command palette expectations.
- `Cmd+Shift+P`: alternate command-only shortcut for VS Code-style muscle memory.
- `/` and `Cmd+F`: image search. These remain search-specific because search has a different grammar and result model.
- Existing view shortcuts stay as `Cmd+1` through `Cmd+8`.
- Existing edit shortcuts stay as `Cmd+Z` and `Cmd+Shift+Z`.
- Existing destructive shortcuts stay as `Backspace` for Trash and `Cmd+Backspace` for permanent delete until a safer confirmation model replaces them.

Shortcut handling rules:

- Do not handle app shortcuts when focus is inside an input, textarea, select, or editable control.
- User-defined hotkeys must be stored separately from defaults and must not mutate the registry item.
- Conflict detection checks built-in app shortcuts, item defaults, and custom assignments.
- The UI should show the conflict owner before saving a custom shortcut.
- Native menu accelerators and keyboard handlers should eventually use the same registry data so shortcut labels cannot drift.

## Migration List

1. `src/lib/keys.ts`
   - Add `Cmd+K`, `Cmd+P`, and `Cmd+Shift+P` to open the palette.
   - Let custom hotkeys run through `runCommandForKeyboardEvent` without adding those direct hotkey executions to palette recents.
   - Leave navigation, rating, deletion, compare, loupe, and grid-specific movement in place until each command has a registry equivalent and tests.

2. `src/lib/menu.ts` and `src-tauri/src/menu.rs`
   - Expose **View > Command Palette...** as native ID `command_palette` with `Cmd+P`, opening command-only mode.
   - Map native menu IDs to registry IDs.
   - Replace duplicated view/settings/open behavior with registry execution where possible.
   - Keep file import dialogs as command implementations, not menu-only helpers.

3. `src/lib/components/ContextMenu.svelte`
   - Move image actions into registry commands with context for focused image plus selected IDs.
   - Cover rating, decision, find similar, copy path, reveal, rename, move, add/remove collection, trash, and permanent delete.

4. `src/lib/components/CommandBar.svelte`
   - Keep `/` and `Cmd+F` as image-search shortcuts.
   - Add palette command entries for open search, apply current query, and save smart collection.
   - Do not merge natural-language search results into command search until the two result models are intentionally combined.

5. `src/lib/components/Sidebar.svelte`
   - Convert all images, folders, collections, smart collections, sessions, canvases, import folder, rescan, regenerate thumbnails, and AI actions into registry destinations/commands.
   - Keep long-running actions behind job/progress-aware commands when available.

6. AI and batch actions
   - Register detection, vision, source rescan, thumbnail regeneration, embeddings, export, and publishing commands with explicit disabled states.
   - Commands that can run for more than a moment should route through the job system before being exposed prominently in the palette.

## First Implementation Boundary

`imageview-zu0.2` should build the shared registry UI and wire palette open/close behavior. It should not rewrite every command surface in one pass. The safer order is: open palette shortcuts, registry search/pins/recents/hotkey capture, then gradual migration of menu, context menu, sidebar, and AI/batch actions.

## Current Coverage For `imageview-zu0.2`

Palette behavior:

- `Cmd+P` and **View > Command Palette...** open command-only mode.
- `Cmd+K` opens commands and destinations.
- `Cmd+Shift+P` remains an alternate command-only shortcut.
- Palette rows are fuzzy-searchable by title, subtitle, category, keywords, acronym, and ID.
- Non-matching rows are hidden while searching.
- Rows show command categories and shortcut badges.
- `Enter` runs the selected row.
- The last five commands executed through the palette are stored as recents and ranked at the top of an empty query.
- Direct custom hotkey executions do not update palette recents.

Represented in the registry:

- view switching: `view.*`
- sidebar and zen toggles: `app.toggle-sidebar`, `app.toggle-zen`
- undo/redo: `edit.undo`, `edit.redo`
- trash/delete: `image.trash`, `image.delete-permanently`
- rating/decision: `image.rating.*`, `image.decision.*`
- settings/search: `app.settings`, `app.search`
- collection workflows: `collection.create-from-selection`, `collection.create-from-unselected`, `collection.toggle-collect-mode`, `collection.add-focused-to-collect-target`

Deferred with reasons:

- import files/folder: still owned by `src/lib/menu.ts` and `Sidebar.svelte`; moving it into the registry should first extract one shared import-dialog service so native menu, sidebar, and palette do not fork file-dialog/import/focus behavior.
- full context menu migration: should happen after command contexts can carry explicit image/selection targets instead of relying only on the globally focused image.
- AI/batch commands: should be exposed through job-aware command wrappers so long-running work has progress, cancellation, and disabled states.
