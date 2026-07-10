# AI Settings and Agent Access Design

**Date:** 2026-07-10  
**Status:** Approved for implementation planning

## Goal

Remove the AI Models block from the library sidebar, reorganize Settings around
clear ownership boundaries, and move library-wide AI processing into the command
palette.

The result should make Settings the place to configure AI and agent access while
keeping library navigation and operations in their natural surfaces.

## Current Problems

The sidebar AI Models block currently mixes four responsibilities:

1. Local model and service readiness.
2. Library-wide processing actions.
3. Processing coverage counts.
4. Detected-object navigation filters.

Settings has the inverse problem. `McpSettings.svelte` has grown into a large
modal that combines app behavior, feature modules, provider credentials,
embedding configuration, MCP networking, access tokens, and client setup.

Two existing behaviors are also misleading:

- “Detect objects” runs both YOLO and NudeNet even though the label names only
  object detection.
- The sidebar reports a number of images “remaining,” but the action sends all
  library image IDs through the model again.

This design fixes those boundaries and semantics rather than moving the existing
block wholesale.

## Settings Information Architecture

Settings has six top-level tabs in this order:

1. **General**
2. **Appearance**
3. **AI**
4. **Agent Access**
5. **Privacy**
6. **Plugins**

The tab labels remain on one line. The strip scrolls horizontally at narrow
widths instead of wrapping. It uses `tablist` and `tab` semantics with arrow-key
navigation, visible focus, and an exposed selected state.

### General

General owns ordinary app behavior and built-in feature modules:

- Close to tray
- Confirm before Trash
- Auto update
- Auto-purge missing files
- Paste filename date format
- RAW File Support
- Static Publishing
- Client Tools
- Voice Dictation

Remote access, provider credentials, embedding configuration, access tokens,
and Claude Code configuration leave General.

### Appearance

Appearance remains a dedicated tab. Its current app-icon controls stay in place,
and the tab remains available for future appearance features.

### AI

The AI tab contains three blocks in this order.

#### 1. Provider Credentials

Provider credentials move from General to the top of AI:

- OpenAI
- Google
- Cohere
- OpenRouter

The existing security behavior is preserved. Settings checks whether a key
exists, validates new values, stores them in the system keychain, and never reads
stored secret values back into the UI.

#### 2. Local Models

This block shows configuration and readiness only; it contains no library-wide
processing buttons.

- **Object detection — YOLO:** selected variant and Ready / Not installed state.
- **Content safety — NudeNet:** Ready / Not installed state.
- **Image descriptions — Ollama:** Ready with installed-model count, or Service
  unavailable.

The selected YOLO variant is persisted and is the variant used by the command
palette action. Missing YOLO or NudeNet weights link to the existing setup guide.
The UI states explicitly that model weights are user-supplied and separately
licensed. Cull does not restore built-in downloads for these weights.

Ollama service settings used for image descriptions remain distinct from the
Ollama embedding endpoint and model below.

#### 3. Embedding Models

The existing embedding configuration moves from General without changing its
defaults or persistence:

- Cohere embedding model
- OpenAI embedding model
- Ollama embedding URL
- Ollama embedding model

The bottom of the tab may include a compact informational hint that library jobs
are available through Command-K. It must not duplicate the job buttons.

### Agent Access

Agent Access contains four blocks in setup order.

#### 1. Cull Agent Skill

This section explains that the Cull skill teaches compatible agents how to use
Cull through its CLI, URL scheme, GUI, and optional MCP surface. It links to the
published [Cull `SKILL.md`](https://github.com/glebis/claude-skills/blob/main/cull/SKILL.md).

Installation choices are presented as copyable alternatives:

**npx (recommended, cross-agent)**

```sh
npx skills add glebis/claude-skills --skill cull
```

**Claude Code marketplace**

```sh
claude plugin marketplace add glebis/claude-skills
claude plugin install cull@glebis-skills
```

**Codex prompt**

```text
Use $skill-installer to install the Cull skill from https://github.com/glebis/claude-skills/tree/main/cull
```

**Generic agent prompt**

```text
Install the Cull skill from https://github.com/glebis/claude-skills/blob/main/cull/SKILL.md. Use your native skill installer if available; otherwise run `npx skills add glebis/claude-skills --skill cull`. Verify that the skill is available in a new agent turn.
```

Cull only copies these commands or prompts. It does not execute package managers,
agent CLIs, or installers. The UI notes that a new agent turn or session may be
needed for discovery. OpenAI’s current Codex guidance confirms that global skills
live under `~/.codex/skills` and are available across repositories:
[Save workflows as skills](https://learn.chatgpt.com/codex/use-cases/reusable-codex-skills).

#### 2. MCP Connection (Optional)

The existing HTTP enablement and port controls move here. Copy explains that the
Cull skill is CLI-first and does not require MCP, while MCP remains useful for
richer interactive control. Existing loopback and explicit remote-opt-in safety
language is preserved.

#### 3. Access Tokens

Token creation, one-time secret reveal, expiry status, rotation, and revocation
move here unchanged. Token failures remain visible and actionable.

#### 4. Claude Code MCP Config

The existing copyable MCP configuration remains last because it depends on the
connection and token choices above. Audit history remains in Privacy.

### Privacy

The existing Privacy & Data dashboard remains functionally unchanged. The tab
label shortens to “Privacy” so the six-tab strip remains compact; the panel may
retain “Privacy & Data” as its content heading.

### Plugins

Plugin management remains unchanged in its dedicated tab.

## Sidebar

The AI Models section and its expansion state are removed from the sidebar.
Model availability, processing coverage, setup links, and job buttons no longer
render there.

Detected object classes remain in the sidebar because they are library filters,
not AI configuration. They move under the existing Filters section in a
conditional **Detected objects** subsection. The subsection appears only when at
least one detected class has a nonzero count. Selecting a class preserves the
existing detected-class filtering behavior.

The detected-class loader becomes independent of the removed model-panel state.

## Command Palette Actions

Three explicit commands appear in the existing AI category:

1. **Detect Objects in Library** — YOLO only.
2. **Scan Library for Sensitive Content** — NudeNet only.
3. **Describe Images in Library** — configured Ollama vision model only.

All three are global library operations, regardless of the currently visible
folder or collection. The title therefore says “in Library” rather than implying
the current scope.

### Pending-Only Semantics

Before starting, each command queries image IDs that do not yet have a stored
result for the exact active model:

- YOLO uses the persisted variant’s model name.
- NudeNet uses `nudenet`.
- Vision uses the configured Ollama vision model.

The pending-ID query must be performed in SQLite, not by issuing one frontend IPC
request per image. Existing results from a different YOLO variant or vision model
do not count as results for the active model.

If no IDs remain, the command exits without starting a job and shows an
informational “Library already processed” message.

### Prerequisites

Each command checks its prerequisite before invoking processing:

- YOLO variant file is available.
- NudeNet file is available.
- Ollama service is reachable and the configured vision model is available.

If a prerequisite is missing, Cull shows an actionable error with an **Open AI
Settings** action. That action opens Settings directly on the AI tab.

### Progress and Completion

The existing Jobs panel remains the single progress surface. It receives:

- `detection-progress` for YOLO.
- `nsfw-progress` for NudeNet.
- `vision-progress` for Ollama vision.

The Jobs panel gains the missing `nsfw-progress` listener. Each action prevents a
second concurrent run of the same job kind. Different job kinds may run
independently unless the backend’s model-engine constraints require serialization.

On completion:

- YOLO refreshes detected-class counts and the current image scope if needed.
- NudeNet refreshes content-safety state used by the active view.
- Vision refreshes image metadata used by the active view.

Per-image failures are counted. Completion reports processed, failed, and skipped
counts instead of swallowing failures. A whole-job prerequisite or database error
is reported as an error and does not claim success.

## Component Boundaries

`McpSettings.svelte` becomes the modal shell and tab router rather than the owner
of every setting. Extract at least these focused components:

- `GeneralSettings.svelte`
- `AiSettings.svelte`
- `AgentAccessSettings.svelte`

Existing `PrivacyDashboard.svelte` and `PluginsSettings.svelte` remain isolated.
Appearance may stay in the shell initially if it remains small, but it must keep a
clear tab-owned block.

Each tab loads only the data it owns. A loading or failure state in one tab does
not block the Settings shell or unrelated tabs.

A shared settings-navigation store or helper owns the selected tab. Callers can
open Settings at a specific destination, including AI and Agent Access. Existing
generic callers continue to open General by default.

Library AI command orchestration lives outside sidebar and settings components.
The command palette calls a focused helper or API boundary that performs pending
selection, prerequisite checks, job invocation, and completion refreshes.

## Data and Persistence

Existing setting keys remain unchanged except for the new persisted YOLO variant.
The implementation defines one canonical key and default (`medium`) shared by
Settings and command execution.

No database migration may delete or reset user data. Pending-result queries read
the existing detections and vision metadata tables and preserve all accumulated
ratings, selections, collections, and model results.

## Error and Empty States

- Provider credentials: preserve Connected, Validating, Invalid, and validation
  error states.
- YOLO/NudeNet: distinguish Ready from Not installed.
- Ollama: distinguish Ready from Service unavailable; do not describe a network
  error as a missing model file.
- Settings load failure: show a tab-local retryable error.
- No pending images: informational completion, not an error.
- Duplicate run: do not enqueue; report that the job is already running.
- Partial processing failure: preserve successful results and report the failure
  count.

## Accessibility

- Tabs expose `role="tablist"`, `role="tab"`, selected state, and associated
  tab panels.
- Left/Right arrows move between tabs; Home/End select the first/last tab.
- Copy controls have explicit accessible names that include the installation
  method or configuration being copied.
- Readiness is conveyed by text as well as color.
- Progress remains available through the Jobs panel’s status semantics.
- Detected object filters retain keyboard-operable buttons and an exposed active
  state.

## Verification

### Rust

- Database tests return only images missing detections for a requested model.
- Database tests return only images missing vision metadata for a requested
  source/model.
- Results from another model do not suppress pending work for the active model.
- Empty libraries and fully processed libraries return no pending IDs.
- Existing results and user data remain untouched.

### Frontend unit and contract tests

- Settings renders the six tabs in the approved order.
- General, AI, and Agent Access own only their approved sections.
- Deep links open the requested Settings tab.
- API keys never read stored secret values.
- The AI tab persists and displays the active YOLO variant.
- Agent installation commands and source link match this specification.
- The sidebar no longer contains AI Models.
- Detected objects remain under Filters and appear conditionally.
- All three command-palette actions appear in the AI category.
- Commands handle missing prerequisites, no pending work, partial failures, and
  duplicate runs.
- `nsfw-progress` appears in the Jobs panel.

### Browser smoke coverage

- Open Settings and navigate to AI and Agent Access.
- Confirm provider, model, skill, and MCP sections are discoverable.
- Find all three library AI commands through Command-K.
- Confirm detected-class filters remain operable after the sidebar move.

### Documentation

Update user-facing documentation that describes:

- The Settings tabs.
- AI model setup.
- Cull skill installation.
- Optional MCP configuration.
- Library-wide AI commands.

Update the command palette’s Settings subtitle so it no longer describes the
panel primarily as “MCP, publishing, and app settings.”

## Non-Goals

- Running installers from Cull.
- Re-enabling built-in YOLO or NudeNet weight downloads.
- Changing provider-key storage or validation behavior.
- Redesigning the Privacy or Plugins content.
- Adding automatic or scheduled AI processing.
- Moving detected-object navigation into Settings.
- Changing individual-image AI actions outside the library-wide commands.

## Implementation Sequence

1. Add pending-result queries and tests.
2. Introduce selected-tab navigation and extract Settings components.
3. Build AI and Agent Access tabs, including skill installation copy.
4. Add the three command-palette jobs and progress integration.
5. Remove the sidebar model block and relocate detected-object filters.
6. Update contracts, browser smoke coverage, and user documentation.

