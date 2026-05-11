# Centralized API Key Settings

## Problem

API keys are only configurable via inline inputs buried in individual features (EmbeddingExplorer's gear menu for Google). OpenAI and OpenRouter keys have no UI at all — users must set them via MCP or CLI. The Settings panel ("MCP Server") doesn't mention API keys.

## Solution

Add an "API Keys" section to the existing Settings panel (`McpSettings.svelte`). Remove the inline key input from `EmbeddingExplorer.svelte` and replace it with a "go to Settings" prompt.

## Rust Changes

Two small backend additions needed (found via Codex review):

### 1. `delete_api_key` command

`set_api_key(provider, '')` does NOT delete from keychain — it stores an empty string, which downstream code like `resubmit_prompt` treats as "configured" and then fails at the provider API. The `KeychainStore` has a `delete()` method (`secrets.rs:39`) but no Tauri command exposes it.

Add `delete_api_key(provider)` command in `embeddings.rs`:
```rust
#[tauri::command]
pub async fn delete_api_key(state: State<'_, AppState>, provider: String) -> Result<(), String> {
    let secret_key = format!("api_key_{}", provider);
    state.secrets.delete(&secret_key)
}
```

Register in `lib.rs` invoke handler.

### 2. `has_api_key` command

Returning actual key values to the frontend is unnecessary for status display. Add a boolean check that never exposes the secret:

```rust
#[tauri::command]
pub async fn has_api_key(state: State<'_, AppState>, provider: String) -> Result<bool, String> {
    let secret_key = format!("api_key_{}", provider);
    match state.secrets.get(&secret_key) {
        Ok(Some(k)) => Ok(!k.is_empty()),
        Ok(None) => Ok(false),
        Err(e) => Err(e),
    }
}
```

The frontend uses `hasApiKey` for status display. `getApiKey` is only used when the user actively types a new key and we need to pre-fill (we won't pre-fill — the input starts empty, user types a new key to overwrite).

### TypeScript API additions (`api.ts`)

```typescript
export async function deleteApiKey(provider: string): Promise<void> {
    return invoke('delete_api_key', { provider });
}

export async function hasApiKey(provider: string): Promise<boolean> {
    return invoke<boolean>('has_api_key', { provider });
}
```

## Frontend Changes

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
- Password input field, always empty on mount — user types new key to set/overwrite (center)
- Status indicator (right): green dot + "Connected" when `hasApiKey` returns true, nothing when false
- "Remove" text button appears when key exists (`hasApiKey` is true)

**Behavior:**
- On mount: call `hasApiKey(provider)` for each provider to set status (never loads actual key)
- On blur of input (non-empty): validate via `validateApiKey(provider, key)`, if valid → `setApiKey(provider, key)` + clear input + show "Connected"
- On invalid: show red status "Invalid key", don't store
- On remove: call `deleteApiKey(provider)` + clear status
- Help text below rows: lock icon + "Stored securely in system keychain"
- Validation is a basic connectivity check (verifies `/models` endpoint access, not billing or model-specific permissions)

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
- On mount, check if Google key exists via `hasApiKey('google')`
- If missing, show a message in the config section: "Google API key required" with a button "Open Settings" that imports and sets `settingsOpen` from `$lib/stores` to `true`
- The generate button should be disabled when no key is set, with tooltip explaining why
- Listen for `settingsOpen` changes: when it transitions from `true` to `false`, re-check `hasApiKey('google')` to refresh status without requiring remount

### 4. Tauri mocks (`tauri-mock.ts`)

Add mock handlers with in-memory state for all API key commands:
```typescript
const mockApiKeys: Record<string, string> = {};

// In MOCK_HANDLERS:
set_api_key: (_: any, args: { provider: string; key: string }) => {
    mockApiKeys[`api_key_${args.provider}`] = args.key;
},
get_api_key: (_: any, args: { provider: string }) => {
    return mockApiKeys[`api_key_${args.provider}`] ?? null;
},
has_api_key: (_: any, args: { provider: string }) => {
    const k = mockApiKeys[`api_key_${args.provider}`];
    return k !== undefined && k !== '';
},
delete_api_key: (_: any, args: { provider: string }) => {
    delete mockApiKeys[`api_key_${args.provider}`];
},
validate_api_key: () => true,
```

### 5. Delete mockup files

Remove `docs/settings-mockup-draft.png` and `docs/settings-mockup-v2.png` — design artifacts, not needed in repo.

## Files Affected

- `src-tauri/src/commands/embeddings.rs` — add `delete_api_key` and `has_api_key` commands
- `src-tauri/src/lib.rs` — register new commands in invoke handler
- `src/lib/api.ts` — add `deleteApiKey` and `hasApiKey` functions
- `src/lib/components/McpSettings.svelte` — add API Keys section, add auto-purge toggle, rename title
- `src/lib/components/EmbeddingExplorer.svelte` — remove inline key input, add "Open Settings" redirect
- `src/lib/tauri-mock.ts` — add mock handlers for all API key commands

## Edge Cases

- **Empty key on blur**: skip validation, don't store
- **Key already stored, user types new key**: validate + overwrite on blur
- **Validation fails (network error)**: show "Could not validate" in orange, don't store
- **Multiple keys changed quickly**: each blur triggers independent validate+store
- **Settings panel closed → EmbeddingExplorer refreshes**: re-check `hasApiKey` when `settingsOpen` goes false
- **Key value never exposed**: `hasApiKey` returns bool only; `getApiKey` exists but is not called from the Settings UI
