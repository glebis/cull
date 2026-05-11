# Vision Brainstorm — Processed
**Raw source:** [2026-05-10-vision-brainstorm-raw.md](2026-05-10-vision-brainstorm-raw.md)
**Date:** 2026-05-10

---

## Design Principles

- [ ] **AI-first, AI-independent** — deterministic flows first, AI enhancement on top; never stop functioning without internet/AI
- [ ] **Dual accessibility** — fully accessible to agents (MCP, CLI) and humans (good UX)
- [ ] **Privacy-first, local-first**
- [ ] **Data portability** — structured output (files + JSON metadata), export always available
- [ ] **Modular from day one** — plugin architecture with clear contracts, validation, sandboxing

---

## Features & Directions

### Viewing
- [ ] Grid view → Canvas → Loop/zoom mode transitions
- [ ] Loop mode: show stars, ratings, approval/rejection status, prompt indicator
- [ ] Larger fonts / more metadata visible for hands-off and remote viewing
- [ ] Agents can control UX state via MCP

### Editing
- [ ] Canvas mode with crop, rotate, resize
- [ ] Everything should be croppable
- [ ] Annotations and comments on images
- [ ] Annotations saved to sidecar JSONs

### Prompt & Generation
- [ ] View prompt from sidecar JSON in loop mode
- [ ] Re-submit prompt to generation API directly from viewer
- [ ] Compare 2/4/N prompt variations side by side
- [ ] Tree-style prompt exploration: alter style, color, elements
- [ ] Budget calculations for generation built in
- [ ] Default backend: official API; also support CLI tools and agents

### AI/ML
- [ ] Multi-modal embeddings (Google) → vector space visualization
- [ ] Object detection (local models, already started)
- [ ] People detection (open tools)
- [ ] DSPy for deterministic flows with self-improvement loops

### Collaboration
- [ ] Real-time shared state across multiple users
- [ ] Role-based permissions: editor, observer, student, assistant
- [ ] Dynamic role assignment during sessions
- [ ] Moderation layer for large groups (20-30 people)
- [ ] Remote editorial workflow (inspired by live photo editing at exhibitions/workshops)

### Architecture & Integration
- [ ] Plugin system — kernel-level modularity, can intercept core processes
- [ ] Capability-based security for plugins
- [ ] App as both server and receiver — network broadcasting
- [ ] Read-only local server for data exposure
- [ ] Easy deployment (Netlify, Vercel, etc.) with agent-friendly prescriptions
- [ ] MCP with 32 tools (already built and tested)

---

## Already Built (skip)

- [x] Grid → Loupe transitions
- [x] Star ratings + accept/reject decisions
- [x] MCP tools (32)
- [x] Embedding explorer (1400+ LOC)
- [x] Object detection (local models)
- [x] Sidecar JSON reading
- [x] Compare view

---

## Sprint 1 — Editorial Workflow

> Focus: single-user creative workflow, viewing + editing loop

- [ ] **Canvas mode** — freeform image layout, drag/resize/arrange
- [ ] **Crop & rotate** — basic in-app editing, everything croppable
- [ ] **Prompt display in Loupe** — show generation prompt from sidecar JSON in loop mode (icon indicator + expandable panel)

## Sprint 2 — Creative Loop + Polish

> Focus: the app becomes a creative tool, not just a viewer

- [ ] **Prompt re-submit to API** — re-generate from viewer, compare N variations, budget display. Default backend: official image generation API. This is the experience shift from editing to creating.
- [ ] **Embedding explorer polish** — make the vector space visualization visually sharp, appealing, and performant. Currently functional but needs design attention.
- [ ] **Liveblocks research** — evaluate Liveblocks API for real-time collaboration. Key constraints: open-source app, users bring their own Liveblocks account. Research scope: what's possible, pricing model, Svelte support, conflict resolution approach.

## Deferred (later sprints)

- Plugin architecture — build once surface area is clear
- Collaboration roles/permissions — after Liveblocks research
- Network broadcasting — after collaboration layer
- DSPy integration — needs research
- People detection — nice-to-have
- Annotations/comments — after canvas is solid

---

## Open Questions

- [ ] Liveblocks vs alternatives: does their pricing model work for open-source (users bring own keys)?
- [ ] Prompt editor: inline in sidecar or dedicated panel?
- [ ] Canvas tech: HTML5 Canvas, SVG, or DOM-based (like Excalidraw)?
- [ ] Plugin security model: how deep should plugin access go?
- [ ] DSPy integration: which flows benefit from learned optimization vs. staying deterministic?
