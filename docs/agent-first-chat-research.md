<!-- Copyright (c) 2026-present Gleb Kalinin. Architecture and design by author. -->
<!-- Implementation assisted by OpenAI GPT-5.5. See AUTHORSHIP.md. -->

# Agent-First Chat Research Brief

## Why this exists

Cull already has an agent-native foundation: headless CLI tools, an MCP server for
live-app control, scoped tokens, audit logs, and the agent-snapshot observe/act
loop. The next product question is whether Cull should grow a first-party chat
surface, expose agent interaction as a plugin, or integrate with an existing
agent runtime such as OpenCode or Claude Agent SDK.

This brief records the previous-session findings that are already in the repo and
lists the research needed before choosing an implementation.

## What previous sessions already decided

### Existing agent surface

`docs/agents.md` establishes the current contract: Cull's tool layer is the
product API, exposed through a headless CLI slice and a live-app MCP server. It
also documents the approval boundary: Cull can expose execution tools, but the
agent runtime or app UI must own confirmations for destructive decisions.

Relevant existing decisions:

- Keep CLI and MCP tool names/JSON fields aligned so agents reuse one mental
  model.
- Use MCP for live app control, token management, audit logs, and snapshots.
- Treat agent calls as execution surfaces, not as the confirmation mechanism.
- Keep destructive-operation confirmation in Cull UI, the MCP client, a shell
  wrapper, or a human operator workflow before tool execution.

### Plugin boundary

The plugin design intentionally deferred a Cull-mediated MCP client bridge. V1
plugins are trusted, user-installed webview/plugin actors that call narrow host
APIs guarded by Rust-side capability checks. Earlier plugin planning said a
destination plugin may run its own MCP client or bridge process only if the user
grants the relevant file, metadata, and network scopes, but Cull should not proxy
arbitrary MCP calls until there is a typed MCP client contract.

Relevant existing decisions:

- No direct database access for plugins.
- No arbitrary lifecycle hooks in v1.
- No Cull-mediated `mcp:call` until a typed MCP client bridge exists.
- Plugin actions exposed through UI and MCP must still obey plugin grants.
- Plugin code runs in the main webview in the current v1 implementation, so any
  agent/chat plugin must rely on Rust-enforced host APIs for privileged actions.

## Current external runtime signals to verify

These are not implementation decisions; they are research checkpoints from the
current public docs as of 2026-06-22.

- Claude Agent SDK is positioned as the programmable Python/TypeScript version of
  Claude Code's agent loop, tools, and context management. It is a candidate for
  a first-party agent runtime bridge when Cull wants programmatic sessions rather
  than only MCP exposure.
- Claude Agent SDK approval/user-input behavior needs extra review before Cull
  delegates critical confirmations to it. Cull's existing docs already cite the
  `AskUserQuestion` limitation for subagents, so this remains a boundary to
  re-check before design.
- OpenCode exposes a headless server via `opencode serve` with an HTTP/OpenAPI
  surface. That makes it a candidate for a local-agent adapter, but Cull needs to
  evaluate auth, session lifecycle, MCP support, and streaming behavior before
  embedding or driving it.

Primary docs to review during the decision spike:

- Claude Agent SDK overview: <https://code.claude.com/docs/en/agent-sdk/overview>
- Claude Agent SDK TypeScript reference: <https://code.claude.com/docs/en/agent-sdk/typescript>
- Claude Agent SDK user-input/approval limitations: <https://code.claude.com/docs/en/agent-sdk/user-input#limitations>
- OpenCode CLI: <https://opencode.ai/docs/cli/>
- OpenCode server: <https://opencode.ai/docs/server/>
- OpenCode agents: <https://opencode.ai/docs/agents/>

## Implementation options to compare

### Option A: First-party Cull chat surface over existing MCP tools

Build a chat panel in core Cull that talks to a selected agent runtime adapter.
Cull owns session persistence, selected-image context, confirmation UI, audit
visibility, and tool-call rendering. The runtime adapter owns model execution.

Research needed:

1. Which local runtime API is stable enough for v1: Claude Agent SDK, OpenCode
   server, both behind an adapter, or no bundled runtime?
2. How does the chat stream tool calls, file references, image references,
   progress, errors, and final answers into Svelte state?
3. How are confirmations routed so the human sees Cull-native prompts before
   destructive tools run?
4. Where are chat transcripts stored, and what is redacted from export/sync?
5. Can chat sessions attach to collection, folder, selection, and snapshot
   context without copying original image files into provider prompts by default?

Best if: chat becomes a core product surface and must feel native.

Main risk: Cull becomes responsible for fast-moving agent-runtime compatibility.

### Option B: Agent chat as an installable plugin

Ship a `cull-agent-chat` plugin that renders a chat UI and invokes a narrow host
API for context, snapshots, and approved tool calls. Runtime-specific adapters
can then live outside the core app.

Research needed:

1. Does the current plugin runtime allow long-lived streaming UI without blocking
   the host app?
2. What new host APIs are required: read selected context, request snapshot,
   request approved tool execution, persist transcript, manage local secrets?
3. How should plugin grants express model/network/secrets access in plain
   language?
4. Can plugin code safely connect to OpenCode/Claude Agent SDK without bypassing
   Cull audit and confirmation boundaries?
5. Should this plugin be first-party and preinstalled, first-party but optional,
   or external registry-only?

Best if: agent UX needs to evolve quickly or support multiple runtimes without
making core Cull depend on one vendor/runtime.

Main risk: current plugins run in the main webview, so any broad host capability
must be Rust-enforced and carefully scoped.

### Option C: No in-app chat; improve agent handoff to external clients

Keep Cull as an MCP server and make external agents the chat UI. Improve the
agent-snapshot flow, app-deep-links, context packs, and docs for OpenCode,
Claude Code/Agent SDK, and other MCP clients.

Research needed:

1. Which agent clients can consume Cull's MCP server and image snapshots most
   reliably?
2. What instructions, skills, or config snippets should Cull generate per
   client?
3. What is missing from the current snapshot/tool surface for useful curation
   conversations?
4. Can external clients present confirmations well enough for destructive or
   batch operations?

Best if: the fastest path is to be the best image-library MCP server rather than
building another chat UI.

Main risk: the user experience depends on external clients and may not feel
agent-first inside Cull itself.

## Recommended decision spike

Before implementing chat, run a short spike with a narrow acceptance target:

1. Create a runtime capability matrix for Claude Agent SDK and OpenCode covering
   install footprint, license, auth, local/server mode, streaming protocol,
   session persistence, MCP client/server support, tool approval hooks, image
   input support, cancellation, and offline behavior.
2. Prototype one vertical slice outside the core UI: selected images plus a text
   prompt produce a streamed answer and one safe Cull tool call.
3. Validate the confirmation boundary with one destructive mock operation: the
   runtime must request action, Cull must show the confirmation, and the tool must
   not execute if the user denies it.
4. Decide whether the prototype belongs in core, a first-party plugin, or an
   external-client handoff.

## Open product questions

- Is the primary user story "talk to Cull" inside the app, or "let my existing
  coding/agent client operate Cull"?
- Should Cull ship with a default runtime adapter, or should users configure one?
- Are chat transcripts first-class library artifacts, or temporary operator logs?
- Should an agent be able to create collections, rate images, and export files in
  one conversational flow by default, or only after explicit scoped grants?
- How much of the agent experience should be local-only to preserve Cull's
  privacy positioning?
