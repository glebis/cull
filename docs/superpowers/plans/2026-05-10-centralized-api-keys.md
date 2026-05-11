# Centralized API Key Settings Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add an API Keys section to Settings with secure key management (never expose stored keys to frontend), and remove the inline key input from EmbeddingExplorer.

**Architecture:** Two new Rust commands (`delete_api_key`, `has_api_key`) complete the backend API. The Settings panel gets an API Keys section using `hasApiKey` for status display and `setApiKey`/`deleteApiKey` for mutations. EmbeddingExplorer delegates key management to Settings via the `settingsOpen` store. Mocks track state in-memory.

**Tech Stack:** Rust (Tauri commands), Svelte 5 (runes), TypeScript API layer

---

### Task 1: Add `delete_api_key` and `has_api_key` Rust commands

**Files:**
- Modify: `src-tauri/src/commands/embeddings.rs:190`
- Modify: `src-tauri/src/lib.rs:273`

- [ ] **Step 1: Add both commands to `embeddings.rs`**

Add after the `validate_api_key` function (line 190) in `src-tauri/src/commands/embeddings.rs`:

```rust
#[tauri::command]
pub async fn delete_api_key(state: State<'_, AppState>, provider: String) -> Result<(), String> {
    let secret_key = format!("api_key_{}", provider);
    state.secrets.delete(&secret_key)
}

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

- [ ] **Step 2: Register commands in `lib.rs`**

In `src-tauri/src/lib.rs`, add these two lines after `commands::embeddings::validate_api_key,` (line 273):

```rust
            commands::embeddings::delete_api_key,
            commands::embeddings::has_api_key,
```

- [ ] **Step 3: Verify it compiles**

Run: `cd src-tauri && cargo check 2>&1 | tail -5`
Expected: no errors

- [ ] **Step 4: Commit**

```bash
git add src-tauri/src/commands/embeddings.rs src-tauri/src/lib.rs
git commit -m "feat: add delete_api_key and has_api_key Tauri commands

Co-Authored-By: Claude Opus 4.6 (1M context) <noreply@anthropic.com>"
```

---

### Task 2: Add TypeScript API functions and mocks

**Files:**
- Modify: `src/lib/api.ts:263`
- Modify: `src/lib/tauri-mock.ts`

- [ ] **Step 1: Add API functions to `api.ts`**

Add after the `validateApiKey` function (line 263) in `src/lib/api.ts`:

```typescript
export async function deleteApiKey(provider: string): Promise<void> {
    return invoke('delete_api_key', { provider });
}

export async function hasApiKey(provider: string): Promise<boolean> {
    return invoke<boolean>('has_api_key', { provider });
}
```

- [ ] **Step 2: Add mock handlers with state to `tauri-mock.ts`**

In `src/lib/tauri-mock.ts`, add a state object near the top of the file (after line 10, before `function makeMockImage`):

```typescript
const mockApiKeys: Record<string, string> = {};
```

Then add these handlers inside `MOCK_HANDLERS` (before the `backfill_image_metadata` entry):

```typescript
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

- [ ] **Step 3: Commit**

```bash
git add src/lib/api.ts src/lib/tauri-mock.ts
git commit -m "feat: add deleteApiKey/hasApiKey API functions and mocks

Co-Authored-By: Claude Opus 4.6 (1M context) <noreply@anthropic.com>"
```

---

### Task 3: Add API Keys section and auto-purge toggle to Settings panel

**Files:**
- Modify: `src/lib/components/McpSettings.svelte`

This is the largest task. The file is 522 lines. We add: renamed title, auto-purge toggle, and full API Keys section.

- [ ] **Step 1: Update imports**

In `McpSettings.svelte`, update the import from `$lib/api` (line 3) to include the new functions. Change:

```typescript
    import { listMcpTokens, createMcpToken, revokeMcpToken, rotateMcpToken, getAppSetting, setAppSetting } from '$lib/api';
```

To:

```typescript
    import { listMcpTokens, createMcpToken, revokeMcpToken, rotateMcpToken, getAppSetting, setAppSetting, hasApiKey, setApiKey, deleteApiKey, validateApiKey } from '$lib/api';
```

- [ ] **Step 2: Add state variables for API keys and auto-purge**

After the existing state variables (around line 21, after `let copied = $state(false);`), add:

```typescript
    let autoPurge = $state(true);

    interface ApiKeyState {
        exists: boolean;
        inputValue: string;
        status: 'none' | 'connected' | 'invalid' | 'validating' | 'error';
    }
    const PROVIDERS = ['openai', 'google', 'openrouter'] as const;
    const PROVIDER_LABELS: Record<string, string> = { openai: 'OpenAI', google: 'Google', openrouter: 'OpenRouter' };
    const PROVIDER_PLACEHOLDERS: Record<string, string> = { openai: 'sk-...', google: 'AIza...', openrouter: 'sk-or-...' };
    let apiKeys = $state<Record<string, ApiKeyState>>({
        openai: { exists: false, inputValue: '', status: 'none' },
        google: { exists: false, inputValue: '', status: 'none' },
        openrouter: { exists: false, inputValue: '', status: 'none' },
    });
```

- [ ] **Step 3: Update onMount to load API key status and auto-purge setting**

In the `onMount` callback, update the `Promise.all` (line 32) to also load `auto_purge_missing` and API key status. Replace the entire `onMount` body:

```typescript
    onMount(async () => {
        try {
            const [toks, ctSetting, trashSetting, httpSetting, portSetting, purgeSetting] = await Promise.all([
                listMcpTokens(),
                getAppSetting('close_to_tray'),
                getAppSetting('skip_trash_confirm'),
                getAppSetting('mcp_http_enabled'),
                getAppSetting('mcp_http_port'),
                getAppSetting('auto_purge_missing'),
            ]);
            tokens = toks;
            closeToTray = ctSetting !== 'false';
            confirmTrash = trashSetting !== 'true';
            httpEnabled = httpSetting === 'true';
            if (portSetting) httpPort = portSetting;
            autoPurge = purgeSetting !== 'false';

            const [hasOpenai, hasGoogle, hasOpenrouter] = await Promise.all([
                hasApiKey('openai'),
                hasApiKey('google'),
                hasApiKey('openrouter'),
            ]);
            apiKeys.openai.exists = hasOpenai;
            apiKeys.openai.status = hasOpenai ? 'connected' : 'none';
            apiKeys.google.exists = hasGoogle;
            apiKeys.google.status = hasGoogle ? 'connected' : 'none';
            apiKeys.openrouter.exists = hasOpenrouter;
            apiKeys.openrouter.status = hasOpenrouter ? 'connected' : 'none';
        } catch (e) {
            console.error('Failed to load settings:', e);
        }
        loading = false;
    });
```

- [ ] **Step 4: Add handler functions for auto-purge and API keys**

Add after the existing `savePort` function (around line 70):

```typescript
    async function toggleAutoPurge() {
        autoPurge = !autoPurge;
        await setAppSetting('auto_purge_missing', autoPurge ? 'true' : 'false');
    }

    async function handleApiKeyBlur(provider: string) {
        const key = apiKeys[provider].inputValue.trim();
        if (!key) return;
        apiKeys[provider].status = 'validating';
        try {
            const valid = await validateApiKey(provider, key);
            if (valid) {
                await setApiKey(provider, key);
                apiKeys[provider].exists = true;
                apiKeys[provider].status = 'connected';
                apiKeys[provider].inputValue = '';
            } else {
                apiKeys[provider].status = 'invalid';
            }
        } catch {
            apiKeys[provider].status = 'error';
        }
    }

    async function handleRemoveApiKey(provider: string) {
        await deleteApiKey(provider);
        apiKeys[provider].exists = false;
        apiKeys[provider].status = 'none';
        apiKeys[provider].inputValue = '';
    }
```

- [ ] **Step 5: Update panel title from "MCP Server" to "Settings"**

Find in the template (line 148):
```svelte
            <h2>MCP Server</h2>
```
Replace with:
```svelte
            <h2>Settings</h2>
```

- [ ] **Step 6: Add auto-purge toggle to General section**

After the HTTP server setting row (after line 185, after the closing `</div>` of the HTTP server row), add:

```svelte
                <div class="setting-row">
                    <span>Auto-purge missing files</span>
                    <button class="toggle" class:on={autoPurge} onclick={toggleAutoPurge}>
                        {autoPurge ? 'ON' : 'OFF'}
                    </button>
                </div>
```

- [ ] **Step 7: Add API Keys section**

After the General section's closing `</div>` (the one that closes the `<div class="section">` containing General settings), add this new section before the `{#if revealedSecret}` block:

```svelte
            <div class="section">
                <div class="section-header">API Keys</div>
                {#each PROVIDERS as provider}
                    <div class="setting-row api-key-row">
                        <span class="provider-label">{PROVIDER_LABELS[provider]}</span>
                        <div class="api-key-controls">
                            {#if apiKeys[provider].exists && !apiKeys[provider].inputValue}
                                <span class="key-status connected">&#9679; Connected</span>
                                <button class="action-btn danger" onclick={() => handleRemoveApiKey(provider)}>Remove</button>
                            {:else}
                                <input
                                    type="password"
                                    placeholder={PROVIDER_PLACEHOLDERS[provider]}
                                    bind:value={apiKeys[provider].inputValue}
                                    class="api-input"
                                    onblur={() => handleApiKeyBlur(provider)}
                                />
                                {#if apiKeys[provider].status === 'validating'}
                                    <span class="key-status validating">Validating...</span>
                                {:else if apiKeys[provider].status === 'invalid'}
                                    <span class="key-status invalid">Invalid key</span>
                                {:else if apiKeys[provider].status === 'error'}
                                    <span class="key-status invalid">Could not validate</span>
                                {/if}
                            {/if}
                        </div>
                    </div>
                {/each}
                <div class="keychain-hint">&#128274; Stored securely in system keychain</div>
            </div>
```

- [ ] **Step 8: Add CSS styles for API Keys section**

Add to the `<style>` block:

```css
    .api-key-row {
        flex-wrap: wrap;
    }
    .provider-label {
        min-width: 90px;
    }
    .api-key-controls {
        display: flex;
        align-items: center;
        gap: 8px;
        flex: 1;
        justify-content: flex-end;
    }
    .api-input {
        background: var(--bg);
        border: 1px solid var(--border);
        border-radius: var(--radius);
        padding: 4px 8px;
        width: 180px;
        font-size: 12px;
        font-family: inherit;
        color: var(--text);
    }
    .api-input:focus {
        border-color: var(--blue);
        outline: none;
    }
    .key-status {
        font-size: 11px;
        white-space: nowrap;
    }
    .key-status.connected {
        color: var(--green);
    }
    .key-status.invalid {
        color: var(--red);
    }
    .key-status.validating {
        color: var(--orange);
    }
    .keychain-hint {
        font-size: 11px;
        color: var(--text-secondary);
        margin-top: 8px;
    }
```

- [ ] **Step 9: Verify it compiles**

Run: `npx svelte-check --threshold error 2>&1 | tail -10`
Expected: no errors

- [ ] **Step 10: Commit**

```bash
git add src/lib/components/McpSettings.svelte
git commit -m "feat: add API Keys section and auto-purge toggle to Settings

Co-Authored-By: Claude Opus 4.6 (1M context) <noreply@anthropic.com>"
```

---

### Task 4: Remove inline key management from EmbeddingExplorer

**Files:**
- Modify: `src/lib/components/EmbeddingExplorer.svelte`

- [ ] **Step 1: Update imports**

In `EmbeddingExplorer.svelte`, update the imports from `$lib/stores` (line 6) to include `settingsOpen`:

```typescript
    import { images, focusedIndex, focusedImageOverride, viewMode, zenMode, navigateTo, embeddingViewState, settingsOpen } from '$lib/stores';
```

Update the imports from `$lib/api` (lines 10-23). Remove `getApiKey`, `setApiKey`, `validateApiKey` and add `hasApiKey`:

```typescript
    import {
        isModelAvailable,
        downloadClipModel,
        generateEmbeddings,
        getAllEmbeddings,
        getEmbeddingCount,
        listImages,
        hasApiKey,
        generateGeminiEmbeddings,
        getImagesByIds,
        regenerateThumbnails,
    } from '$lib/api';
```

Remove the `openUrl` import (line 9) since we no longer link to the API key page:

```typescript
    // DELETE: import { openUrl } from '@tauri-apps/plugin-opener';
```

(Only delete if `openUrl` is not used elsewhere in the file. Check first — if it is, keep it.)

- [ ] **Step 2: Replace API key state with hasGoogleKey**

Replace the API key state variables (lines 40-42):

```typescript
    let apiKey = $state('');
    let keyValid = $state<boolean | null>(null);
    let validating = $state(false);
```

With:

```typescript
    let hasGoogleKey = $state(false);
```

- [ ] **Step 3: Replace `loadApiKeyState` function**

Replace the `loadApiKeyState` function (lines 117-128):

```typescript
    async function loadApiKeyState() {
        try {
            const key = await getApiKey('google');
            if (key) {
                apiKey = key;
                keyValid = true; // assume valid if stored
                geminiEmbeddingCount = await getEmbeddingCount('gemini-embedding-2');
            }
        } catch (e) {
            console.error('Failed to load API key state:', e);
        }
    }
```

With:

```typescript
    async function loadApiKeyState() {
        try {
            hasGoogleKey = await hasApiKey('google');
            if (hasGoogleKey) {
                geminiEmbeddingCount = await getEmbeddingCount('gemini-embedding-2');
            }
        } catch (e) {
            console.error('Failed to load API key state:', e);
        }
    }
```

- [ ] **Step 4: Remove `handleSaveApiKey` function**

Delete the entire `handleSaveApiKey` function (lines 130-148).

- [ ] **Step 5: Remove `openApiKeyPage` function**

Delete the `openApiKeyPage` function (lines 176-178).

- [ ] **Step 6: Add settings close watcher**

After the `loadApiKeyState` function, add a reactive subscription that refreshes key status when Settings closes. Add this inside the `<script>` block:

```typescript
    let prevSettingsOpen = false;
    $effect(() => {
        const isOpen = $settingsOpen;
        if (prevSettingsOpen && !isOpen) {
            loadApiKeyState();
        }
        prevSettingsOpen = isOpen;
    });
```

- [ ] **Step 7: Replace the config section template**

Replace the entire `{#if configOpen}` block (lines 831-858) which contains the "GEMINI API KEY" section:

```svelte
        {#if configOpen}
            <div class="panel-section config-section">
                {#if selectedProvider === 'gemini' && !hasGoogleKey}
                    <div class="section-header">GEMINI API KEY REQUIRED</div>
                    <p class="key-missing-text">Set your Google API key in Settings to use Gemini embeddings.</p>
                    <button class="settings-link-btn" onclick={() => settingsOpen.set(true)}>
                        Open Settings
                    </button>
                {/if}
            </div>
        {/if}
```

- [ ] **Step 8: Disable generate button when no key**

Find the Gemini generate button in the template. It likely calls `handleGenerateGemini`. Add a `disabled` condition. Find the button and update it to include:

```svelte
disabled={!hasGoogleKey}
```

If there's a wrapping condition like `{#if selectedProvider === 'gemini'}`, the button should look like:

```svelte
<button ... onclick={handleGenerateGemini} disabled={!hasGoogleKey} title={hasGoogleKey ? '' : 'Set Google API key in Settings'}>
```

- [ ] **Step 9: Add CSS for the new config section elements**

Add to the `<style>` block:

```css
    .key-missing-text {
        font-size: 12px;
        color: var(--text-secondary);
        margin: 0 0 8px 0;
    }
    .settings-link-btn {
        background: none;
        border: 1px solid var(--blue);
        border-radius: var(--radius, 4px);
        padding: 4px 12px;
        font-size: 12px;
        font-family: inherit;
        color: var(--blue);
        cursor: pointer;
    }
    .settings-link-btn:hover {
        background: rgba(122, 162, 247, 0.1);
    }
```

- [ ] **Step 10: Remove unused CSS**

Delete the `.api-key-row`, `.api-input`, `.key-status`, `.key-status.valid`, `.key-status.invalid` styles from EmbeddingExplorer if they exist (they were only used by the removed API key input).

- [ ] **Step 11: Verify it compiles**

Run: `npx svelte-check --threshold error 2>&1 | tail -10`
Expected: no errors

- [ ] **Step 12: Commit**

```bash
git add src/lib/components/EmbeddingExplorer.svelte
git commit -m "refactor: move API key management from EmbeddingExplorer to Settings

Co-Authored-By: Claude Opus 4.6 (1M context) <noreply@anthropic.com>"
```

---

### Task 5: Cleanup mockup files and build verification

**Files:**
- Delete: `docs/settings-mockup-draft.png`
- Delete: `docs/settings-mockup-v2.png`

- [ ] **Step 1: Delete mockup files**

```bash
trash docs/settings-mockup-draft.png docs/settings-mockup-v2.png
```

- [ ] **Step 2: Full Rust build check**

Run: `cd src-tauri && cargo check 2>&1 | tail -5`
Expected: no errors

- [ ] **Step 3: Full frontend check**

Run: `npx svelte-check --threshold error 2>&1 | tail -10`
Expected: no errors

- [ ] **Step 4: Run all tests**

Run: `cd src-tauri && cargo test 2>&1 | tail -10`
Expected: all tests pass

Run: `npx vitest run 2>&1 | tail -10`
Expected: all tests pass

- [ ] **Step 5: Commit cleanup**

```bash
git add -A
git commit -m "chore: remove design mockup files

Co-Authored-By: Claude Opus 4.6 (1M context) <noreply@anthropic.com>"
```
