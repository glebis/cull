<script lang="ts">
    import { onMount } from 'svelte';
    import { openUrl } from '@tauri-apps/plugin-opener';
    import {
        checkOllama, deleteApiKey, getAppSetting, getOllamaConfig, hasApiKey,
        isNudenetAvailable, isYoloAvailable, setApiKey, setAppSetting,
        setOllamaConfig, validateApiKey,
    } from '$lib/api';
    import { MODEL_SETUP_GUIDE_URL } from '$lib/onboarding';

    interface ApiKeyState { exists: boolean; inputValue: string; status: 'none' | 'connected' | 'invalid' | 'validating' | 'error' }
    const PROVIDERS = ['openai', 'google', 'cohere', 'openrouter'] as const;
    const LABELS = { openai: 'OpenAI', google: 'Google', cohere: 'Cohere', openrouter: 'OpenRouter' };
    const PLACEHOLDERS = { openai: 'sk-...', google: 'AIza...', cohere: 'co-...', openrouter: 'sk-or-...' };
    let apiKeys = $state<Record<(typeof PROVIDERS)[number], ApiKeyState>>({
        openai: { exists: false, inputValue: '', status: 'none' },
        google: { exists: false, inputValue: '', status: 'none' },
        cohere: { exists: false, inputValue: '', status: 'none' },
        openrouter: { exists: false, inputValue: '', status: 'none' },
    });
    let yoloReady = $state(false);
    let nudenetReady = $state(false);
    let ollamaModels = $state<string[]>([]);
    let ollamaServiceAvailable = $state<boolean | null>(null);
    let yoloVariant = $state('medium');
    let savedYoloVariant = $state('medium');
    let yoloVariantError = $state<string | null>(null);
    let ollamaUrl = $state('http://localhost:11434');
    let ollamaModel = $state('llava');
    let cohereEmbeddingModel = $state('embed-v4.0');
    let openaiEmbeddingModel = $state('text-embedding-3-large');
    let ollamaEmbeddingUrl = $state('http://localhost:11434/api/embed');
    let ollamaEmbeddingModel = $state('embeddinggemma');
    let initializationState = $state<'loading' | 'ready' | 'error'>('loading');

    onMount(() => {
        void initialize();
    });

    async function initialize() {
        initializationState = 'loading';
        try {
            const [variant, visionConfig, cohereModel, openaiModel, embedUrl, embedModel] = await Promise.all([
                getAppSetting('yolo_variant'), getOllamaConfig(), getAppSetting('cohere_embedding_model'),
                getAppSetting('openai_embedding_model'), getAppSetting('ollama_embedding_url'),
                getAppSetting('ollama_embedding_model'),
            ]);
            yoloVariant = variant || 'medium';
            savedYoloVariant = yoloVariant;
            [ollamaUrl, ollamaModel] = visionConfig;
            cohereEmbeddingModel = cohereModel || cohereEmbeddingModel;
            openaiEmbeddingModel = openaiModel || openaiEmbeddingModel;
            ollamaEmbeddingUrl = embedUrl || ollamaEmbeddingUrl;
            ollamaEmbeddingModel = embedModel || ollamaEmbeddingModel;
            const [yolo, nude, ollama, ...keys] = await Promise.all([
                isYoloAvailable(yoloVariant), isNudenetAvailable(),
                checkOllama()
                    .then(models => ({ available: true, models }))
                    .catch(() => ({ available: false, models: [] as string[] })),
                ...PROVIDERS.map(provider => hasApiKey(provider)),
            ]);
            yoloReady = yolo;
            nudenetReady = nude;
            ollamaModels = ollama.models;
            ollamaServiceAvailable = ollama.available;
            PROVIDERS.forEach((provider, index) => {
                apiKeys[provider].exists = keys[index];
                apiKeys[provider].status = keys[index] ? 'connected' : 'none';
            });
            initializationState = 'ready';
        } catch {
            initializationState = 'error';
        }
    }

    async function saveKey(provider: (typeof PROVIDERS)[number]) {
        const key = apiKeys[provider].inputValue.trim();
        if (!key) return;
        apiKeys[provider].status = 'validating';
        try {
            if (!(await validateApiKey(provider, key))) { apiKeys[provider].status = 'invalid'; return; }
            await setApiKey(provider, key);
            apiKeys[provider] = { exists: true, inputValue: '', status: 'connected' };
        } catch { apiKeys[provider].status = 'error'; }
    }

    async function removeKey(provider: (typeof PROVIDERS)[number]) {
        await deleteApiKey(provider);
        apiKeys[provider] = { exists: false, inputValue: '', status: 'none' };
    }

    async function saveVision() {
        await setOllamaConfig(ollamaUrl.trim() || undefined, ollamaModel.trim() || undefined);
    }

    async function changeYoloVariant() {
        const nextVariant = yoloVariant;
        yoloVariantError = null;
        try {
            await setAppSetting('yolo_variant', nextVariant);
        } catch {
            yoloVariant = savedYoloVariant;
            yoloVariantError = 'Could not save YOLO variant. The previous selection was kept.';
            return;
        }
        savedYoloVariant = nextVariant;
        yoloReady = await isYoloAvailable(nextVariant);
    }

    async function saveEmbeddings() {
        await Promise.all([
            setAppSetting('cohere_embedding_model', cohereEmbeddingModel.trim() || 'embed-v4.0'),
            setAppSetting('openai_embedding_model', openaiEmbeddingModel.trim() || 'text-embedding-3-large'),
            setAppSetting('ollama_embedding_url', ollamaEmbeddingUrl.trim() || 'http://localhost:11434/api/embed'),
            setAppSetting('ollama_embedding_model', ollamaEmbeddingModel.trim() || 'embeddinggemma'),
        ]);
    }
</script>

{#if initializationState === 'loading'}
    <section class="initialization-state"><p role="status">Loading AI settings…</p></section>
{:else if initializationState === 'error'}
    <section class="initialization-state"><div role="alert"><p>Could not load AI settings.</p><button onclick={initialize}>Retry</button></div></section>
{:else}
<section class="settings-section">
    <h3>Provider Credentials</h3>
    {#each PROVIDERS as provider}
        <div class="setting-row">
            <span>{LABELS[provider]}</span>
            <div class="controls">
                {#if apiKeys[provider].exists && !apiKeys[provider].inputValue}
                    <span class="status ready">● Connected</span>
                    <button class="danger" onclick={() => removeKey(provider)}>Remove</button>
                {:else}
                    <input type="password" aria-label={`${LABELS[provider]} API key`} placeholder={PLACEHOLDERS[provider]} bind:value={apiKeys[provider].inputValue} onblur={() => saveKey(provider)} />
                    {#if apiKeys[provider].status === 'validating'}<span class="status">Validating…</span>{/if}
                    {#if apiKeys[provider].status === 'invalid'}<span class="status error">Invalid key</span>{/if}
                    {#if apiKeys[provider].status === 'error'}<span class="status error">Could not validate</span>{/if}
                {/if}
            </div>
        </div>
    {/each}
    <p class="note">Stored securely in the system keychain.</p>
</section>

<section class="settings-section">
    <h3>Local Models</h3>
    <div class="setting-row"><span>Object detection · YOLO</span><span class:ready={yoloReady} class="status">{yoloReady ? 'Ready' : 'Not installed'}</span></div>
    <label class="setting-row"><span>YOLO variant</span><select bind:value={yoloVariant} onchange={changeYoloVariant} aria-describedby={yoloVariantError ? 'yolo-variant-error' : undefined}><option value="nano">Nano · 6 MB</option><option value="small">Small · 22 MB</option><option value="medium">Medium · 50 MB</option></select></label>
    {#if yoloVariantError}<p id="yolo-variant-error" class="status error" role="alert">{yoloVariantError}</p>{/if}
    <div class="setting-row"><span>Content filter · NudeNet</span><span class:ready={nudenetReady} class="status">{nudenetReady ? 'Ready' : 'Not installed'}</span></div>
    <div class="setting-row"><span>Image descriptions · Ollama</span><span class:ready={ollamaServiceAvailable === true && ollamaModels.length > 0} class="status">{ollamaServiceAvailable === null ? 'Checking…' : !ollamaServiceAvailable ? 'Service unavailable' : ollamaModels.length === 0 ? 'No models installed' : `Ready · ${ollamaModels.length} ${ollamaModels.length === 1 ? 'model' : 'models'} installed`}</span></div>
    <label class="setting-row"><span>Ollama URL</span><input bind:value={ollamaUrl} onblur={saveVision} /></label>
    <label class="setting-row"><span>Vision model</span><input bind:value={ollamaModel} onblur={saveVision} /></label>
    <button class="link-button" onclick={() => openUrl(MODEL_SETUP_GUIDE_URL)}>Local model setup guide ↗</button>
    <p class="note">YOLO and NudeNet weights are user-supplied and separately licensed.</p>
    <p class="note">Run library analysis from the command palette.</p>
</section>

<section class="settings-section">
    <h3>Embedding Models</h3>
    <label class="setting-row"><span>Cohere</span><input bind:value={cohereEmbeddingModel} onblur={saveEmbeddings} /></label>
    <label class="setting-row"><span>OpenAI</span><input bind:value={openaiEmbeddingModel} onblur={saveEmbeddings} /></label>
    <label class="setting-row"><span>Ollama URL</span><input bind:value={ollamaEmbeddingUrl} onblur={saveEmbeddings} /></label>
    <label class="setting-row"><span>Ollama model</span><input bind:value={ollamaEmbeddingModel} onblur={saveEmbeddings} /></label>
</section>
{/if}

<style>
    .initialization-state { padding: 24px 20px; color: var(--text-secondary); font-size: 11px; }
    .initialization-state p { margin: 0 0 10px; }
    .settings-section { padding: 16px 20px; border-bottom: 1px solid var(--border); }
    h3 { margin: 0 0 12px; color: var(--text-secondary); font-size: 11px; letter-spacing: .08em; text-transform: uppercase; }
    .setting-row { min-height: 34px; display: flex; align-items: center; justify-content: space-between; gap: 16px; color: var(--text); font-size: 12px; }
    .controls { display: flex; align-items: center; gap: 8px; }
    input, select { min-width: 190px; box-sizing: border-box; background: var(--bg); color: var(--text); border: 1px solid var(--border); border-radius: var(--radius); padding: 6px 8px; font: 11px var(--font); }
    button { font: 11px var(--font); cursor: pointer; }
    .danger, .link-button { background: none; border: 0; color: var(--red); }
    .link-button { color: var(--blue); padding: 8px 0; }
    .status { color: var(--text-secondary); font-size: 11px; }
    .status.ready { color: var(--green); }
    .status.error { color: var(--red); }
    .note { margin: 8px 0 0; color: var(--text-secondary); font-size: 10px; line-height: 1.5; }
</style>
