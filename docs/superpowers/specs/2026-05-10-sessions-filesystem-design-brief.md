# Sessions & File-System Architecture — Design Brief

**Date:** 2026-05-10
**Status:** Pre-brainstorm — ready for next session
**Inspiration:** Capture One Sessions model

---

## Core Concept

Shift ImageView from a purely DB-centric model to a **file-system-based session model** where each session is a real folder on disk — accessible to Claude Code, MCP tools, and external scripts.

## Current State

- Import batches exist in DB (`import_batch_id` on images table) but are transient — used only for the "just imported" filter, then forgotten
- All image metadata lives in SQLite with opaque UUIDs
- No concept of "project" or "session" — everything is one flat library
- Canvas exists but has no connection to import history
- Lineage view tracks image derivation chains but not session context

## Proposed Structure

```
~/ImageView/Sessions/
  2026-05-10-portrait-shoot/
    Imports/          -- originals land here
    Selects/          -- accepted images (symlinks or copies)
    Exports/          -- rendered outputs
    session.json      -- metadata, ratings, notes, canvas layouts
  2026-05-08-logo-exploration/
    ...
```

## Key Design Questions (for brainstorm)

1. **Session vs Library:** Are sessions the only way to work, or can users still have a flat "All Images" library? (Capture One offers both Sessions and Catalogs)
2. **File ownership:** Do originals get copied/moved into the session folder, or does the session just reference external paths? (Copy = portable but doubles storage; reference = lightweight but fragile)
3. **Session metadata:** What lives in `session.json` vs the SQLite DB? Ratings, selections, canvas layouts, embeddings?
4. **Import flow:** Does every import create a session automatically, or is session creation explicit?
5. **Canvas integration:** Each session gets its own canvas? Or canvas is a cross-session workspace?
6. **Sequence navigation:** How do you browse session history? Sidebar section? Dedicated view? Timeline?
7. **Claude Code / MCP access:** How do external tools discover and interact with sessions? Just read the folder? MCP tools for session CRUD?
8. **Migration:** How to handle existing 553 images already in the DB without sessions?
9. **Embeddings per session:** Are CLIP embeddings session-scoped or global?

## What's Already Built (context for implementation)

- **Import pipeline:** `importFolder()`, `importFiles()` with batch IDs
- **Canvas view:** drag-to-arrange, zoom/pan, multi-select
- **Lineage view:** tracks derivation chains across images
- **Persistence:** localStorage autosave (just added — Fix 4)
- **Embeddings:** UMAP projection with cluster view (just fixed — Fixes 1-4)
- **Collections:** manual grouping exists but is DB-only
- **Smart Collections:** filter-based virtual collections

## Capture One Session Features (reference)

- Session = folder on disk with predictable subfolders
- Each session is self-contained and portable (copy folder = copy everything)
- Trash is session-local (not system trash)
- Output recipes generate into session's Output folder
- Multiple sessions can be open (tabs)
- Session favorites/selects are physically separated into Selects folder
- File system IS the source of truth; catalog indexes it

## Relationship to Existing Fixes

The embeddings fixes from this session (Fixes 1-4) remain valid regardless of the sessions architecture. The persistence layer (Fix 4) will need to be extended to save/restore active session. The `focusedImageOverride` pattern (Fix 1) naturally supports session-scoped image navigation.
