# Centralized API Key Settings

## Problem

API keys are only configurable via inline inputs buried in individual features (EmbeddingExplorer's gear menu for Google). OpenAI and OpenRouter keys have no UI at all — users must set them via MCP or CLI. The Settings panel ("MCP Server") doesn't mention API keys.

## Solution

Add an "API Keys" section to the existing Settings panel (`McpSettings.svelte`). Remove the inline key input from `EmbeddingExplorer.svelte` and replace it with a "go to Settings" prompt.

## No Rust Changes Needed

The backend already has everything:
- `set_api_key(provider, key)` — stores in OS keychain via `KeychainStore`
- `get_api_key(provider)` — retrieves from keychain
- `validate_api_key(provider, key)` — validates against provider API (Google, OpenAI, OpenRouter)
- Keys stored as `api_key_{provider}` in the OS keychain (macOS Keychain, Linux Secret Service)

MCP tools already use keys internally — `resubmit_prompt` reads from keychain via `provider_config().key_name`. No exposure of key values to MCP callers.

## Changes

### 1. McpSettings.svelte — Add "API Keys" section

Add between the "General" and "Access Tokens" sections.

**Provider list:**

| Provider | Placeholder | Validate endpoint |
|----------|------------|-------------------|
| OpenAI | `sk-...` | `api.openai.com/v1/models` |
| Google | `AIza...` | `generativelanguage.googleapis.com/v1beta/models` |
| OpenRouter | `sk-or-...` | `openrouter.ai/api/v1/models` |

**Per-provider row:**
- Provider name label (left)
- Password input field, masked (center)
- Status indicator (right): green dot + "Connected" when valid key stored, nothing when empty
- "Remove" text button appears when a key is stored

**Behavior:**
- On mount: load all three keys via `getApiKey(provider)`, set status for each
- On blur of input: validate via `validateApiKey(provider, key)`, if valid → `setApiKey(provider, key)` + show "Connected"
- On invalid: show red status "Invalid key"
- On remove: call `setApiKey(provider, '')` (empty string deletes from keychain) + clear input + clear status
- Help text below rows: lock icon + "Stored securely in system keychain"

**Also rename the panel** from "MCP Server" to "Settings" — it's the app's general settings now.

### 2. McpSettings.svelte — Add "Auto-purge missing files" toggle

Add to the General section (alongside existing Close to Tray, Confirm Trash, HTTP server toggles):
- "Auto-purge missing files" — reads/writes `auto_purge_missing` setting, defaults to ON
- This was added by the library health check feature but had no UI

### 3. EmbeddingExplorer.svelte — Remove inline key management

Remove:
- The `apiKey` state variable
- The `handleSaveApiKey()` function
- The `loadApiKeyState()` function
- The API key input UI in the config section (the "GEMINI API KEY" section with input + "Get Key →" button + validation status)

Replace with:
- On mount, check if Google key exists via `getApiKey('google')`
- If missing, show a message in the config section: "Google API key required" with a button "Open Settings" that sets `settingsOpen` store to `true`
- The generate button should be disabled when no key is set, with tooltip explaining why

### 4. Delete mockup files

Remove `docs/settings-mockup-draft.png` and `docs/settings-mockup-v2.png` — design artifacts, not needed in repo.

## Files Affected

- `src/lib/components/McpSettings.svelte` — add API Keys section, add auto-purge toggle, rename title
- `src/lib/components/EmbeddingExplorer.svelte` — remove inline key input, add "Open Settings" redirect
- `src/lib/tauri-mock.ts` — add mock for `validate_api_key` if missing

## Edge Cases

- **Empty key on blur**: skip validation, don't store
- **Key already stored, user clears input**: treat as remove
- **Validation fails (network error)**: show "Could not validate" in orange, don't store
- **Multiple keys changed quickly**: each blur triggers independent validate+store, no debounce needed
