<!-- Copyright (c) 2026-present Gleb Kalinin. Architecture and design by author. -->
<!-- Implementation assisted by Claude (Anthropic). See AUTHORSHIP.md. -->
<script lang="ts">
    import '../app.css';
    import TabBar from '$lib/components/TabBar.svelte';
    import Sidebar from '$lib/components/Sidebar.svelte';
    import StatusBar from '$lib/components/StatusBar.svelte';
    import Grid from '$lib/components/Grid.svelte';
    import Compare from '$lib/components/Compare.svelte';
    import Loupe from '$lib/components/Loupe.svelte';
    import Canvas from '$lib/components/Canvas.svelte';
    import EmbeddingExplorer from '$lib/components/EmbeddingExplorer.svelte';
    import UpdateBanner from '$lib/components/UpdateBanner.svelte';
    import CommandBar from '$lib/components/CommandBar.svelte';
    import CommandPalette from '$lib/components/CommandPalette.svelte';
    import KeyboardShortcuts from '$lib/components/KeyboardShortcuts.svelte';
    import ExportFolderDialog from '$lib/components/ExportFolderDialog.svelte';
    import ContactSheetDialog from '$lib/components/ContactSheetDialog.svelte';
    import GroupRankingDialog from '$lib/components/GroupRankingDialog.svelte';
    import Export from '$lib/components/Export.svelte';
    import PluginViewHost from '$lib/components/PluginViewHost.svelte';
    import Toast from '$lib/components/Toast.svelte';
    import ImportBanner from '$lib/components/ImportBanner.svelte';
    import LineageView from '$lib/components/LineageView.svelte';
    import Tinder from '$lib/components/Tinder.svelte';
    import McpSettings from '$lib/components/McpSettings.svelte';
    import UndoHistoryPanel from '$lib/components/UndoHistoryPanel.svelte';
    import AboutDialog from '$lib/components/AboutDialog.svelte';
    import AgentSkillsDialog from '$lib/components/AgentSkillsDialog.svelte';
    import AgentProposalDock from '$lib/components/AgentProposalDock.svelte';
    import ActionProposalReviewDialog from '$lib/components/ActionProposalReviewDialog.svelte';
    import JobProgressPanel from '$lib/components/JobProgressPanel.svelte';
    import TrashConfirmDialog from '$lib/components/TrashConfirmDialog.svelte';
    import TextInputDialog from '$lib/components/TextInputDialog.svelte';
    import ConfirmDialog from '$lib/components/ConfirmDialog.svelte';
    import CollectionTargetDialog from '$lib/components/CollectionTargetDialog.svelte';
    import GenerationResultsStrip from '$lib/components/GenerationResultsStrip.svelte';
    import PreviewDisplay from '$lib/components/PreviewDisplay.svelte';
    import { handleKeydown } from '$lib/keys';
    import { images, focusedIndex, focusedImage, viewMode, sidebarVisible, zenMode, minSizeFilter, showToast, settingsOpen, aboutOpen, agentSkillsOpen, searchOpen, showMissing, smartCollections, activeSmartCollection, activeFolder, activeCollection, activeDetectedClass, staticPublishingEnabled, clientToolsEnabled, voiceDictationEnabled, pluginsEnabled, selectedIds, activeCanvas, activeSession, collections, windowLabel, agentPanelPinned, agentPanelVisible, agentVisualLevel, activeAgentProposalId, activeAgentSelectionPresetId, cycleAgentVisualLevel } from '$lib/stores';
    import { trashImages, trashImagesDetailed, deleteImagesPermanently, getAppSetting, setAppSetting, checkLibraryHealth, regenerateThumbnailsByIds, listCollections, listSmartCollections, updatePreviewState, captureAgentWindowSnapshot, completeAgentViewSnapshot, createActionProposal, listActionProposals, applyActionProposal, dismissActionProposal, listAgentSelectionPresets, upsertAgentSelectionPreset, runClaudeAgentChatTurn, type AgentActionProposal, type AgentChatImageContext, type AgentSelectionPreset, type AgentVisualLevel, type ClaudeAgentStreamEvent, type ImageWithFile, type PreviewState } from '$lib/api';
    import { initDeepLink } from '$lib/deeplink';
    import { initMenu } from '$lib/menu';
    import { loadInstalledPlugins, activateBundledPlugins } from '$lib/plugins/loader';
    import { registerCoreTabs, tabRegistry } from '$lib/plugins/tab-registry';
    import { BUNDLED_PLUGINS } from '$lib/plugins/bundled';
    import { isPreviewDisplayRoute, nextPreviewFocusPayload, previewSyncImageId } from '$lib/preview-display';
    import {
        PREVIEW_DISPLAY_ALWAYS_ON_TOP_SETTING,
        PREVIEW_DISPLAY_MODE_SETTING,
        PREVIEW_DISPLAY_OVERLAY_SETTING,
        parsePreviewDisplayMode,
        parsePreviewDisplayOverlay,
        previewDisplayAlwaysOnTop,
        previewDisplayBlanked,
        previewDisplayFrozen,
        previewDisplayMode,
        previewDisplayOverlay,
        setPreviewDisplayAlwaysOnTop,
        setPreviewDisplayMode,
        setPreviewDisplayOverlay,
    } from '$lib/preview-display-store';
    import { saveAppState, restoreAppStateBeforeImages, applyRestoredViewState, type PersistedState } from '$lib/persistence';
    import { invalidateImageCache, loadImagesForCurrentScope, refreshImageCount, type ImageLoadOptions } from '$lib/image-loading';
    import { clampFocusIndexToList, nextFocusIndexAfterFocusedRemoval } from '$lib/image-removal';
    import { buildAgentSnapshotManifest, collectVisibleImageTargets, drawAnnotatedSnapshot, type AgentSnapshotScope } from '$lib/agent-view-snapshot';
    import { estimateAgentBudget } from '$lib/agent-token-estimate';
    import { effectiveAgentVisualLevel } from '$lib/agent-visual-context';
    import { listen } from '@tauri-apps/api/event';
    import { onMount } from 'svelte';

    let dragOver = $state(false);
    let trashConfirmVisible = $state(false);
    let trashConfirmFileName = $state('');
    let skipTrashConfirmSession = $state(false);
    const previewDisplayWindow = isPreviewDisplayRoute();
    let previewSyncState = $state<PreviewState | null>(null);
    let lastPreviewSyncKey = $state('');
    let agentProposals = $state<AgentActionProposal[]>([]);
    let agentSelectionPresets = $state<AgentSelectionPreset[]>([]);
    let agentChatBusy = $state(false);
    let lastAgentMessage = $state<string | null>(null);
    let lastAgentInstruction = $state<string | null>(null);
    let activeAgentRequestId = $state<string | null>(null);
    let agentStreamEvents = $state<ClaudeAgentStreamEvent[]>([]);
    let reviewProposalId = $state<string | null>(null);

    let immersive = $derived($viewMode === 'loupe' || $viewMode === 'compare');
    let noSidebar = $derived(immersive || !$sidebarVisible);
    let reviewProposal = $derived(agentProposals.find(p => p.id === reviewProposalId) ?? null);
    let agentCandidateCount = $derived(proposalCandidateIds().length);

    async function loadImages(options: ImageLoadOptions = {}) {
        await loadImagesForCurrentScope(options);
    }

    async function restoreSmartCollectionScope(restored: PersistedState | null) {
        if (!restored?.activeSmartCollectionId) return;
        const restoredSmartCollections = await listSmartCollections();
        smartCollections.set(restoredSmartCollections);
        const active = restoredSmartCollections.find(sc => sc.id === restored.activeSmartCollectionId);
        if (!active) return;
        activeSmartCollection.set(active);
        activeFolder.set(null);
        activeCollection.set(null);
        activeDetectedClass.set(null);
    }

    function removeVisibleImageById(imageId: string, plannedFocusIndex: number) {
        let nextLength = 0;
        images.update(list => {
            const next = list.filter(item => item.image.id !== imageId);
            nextLength = next.length;
            return next;
        });
        focusedIndex.set(clampFocusIndexToList(plannedFocusIndex, nextLength));
    }

    async function refreshCollectionCountsAfterRemoval(context: string) {
        try {
            collections.set(await listCollections());
        } catch (e) {
            console.error(`Failed to refresh collection counts after ${context}:`, e);
        }
    }

    async function executeTrash() {
        const imgs = $images;
        const idx = $focusedIndex;
        const img = imgs[idx];
        if (!img) return;
        const nextFocusIndex = nextFocusIndexAfterFocusedRemoval(idx, imgs.length);
        const count = await trashImages([img.image.id]);
        if (count > 0) {
            const name = img.path.split('/').pop() ?? '';
            showToast(`Moved to Trash`, { detail: name, type: 'info', duration: 5000 });
            invalidateImageCache();
            removeVisibleImageById(img.image.id, nextFocusIndex);
            refreshImageCount().catch(e => console.error('Failed to refresh image count after trash:', e));
            refreshCollectionCountsAfterRemoval('trash');
        }
    }

    async function handleTrash() {
        const img = $images[$focusedIndex];
        if (!img) return;

        if (skipTrashConfirmSession) {
            await executeTrash();
            return;
        }

        const alwaysSkip = await getAppSetting('skip_trash_confirm');
        if (alwaysSkip === 'true') {
            await executeTrash();
            return;
        }

        trashConfirmFileName = img.path.split('/').pop() ?? '';
        trashConfirmVisible = true;
    }

    async function handleTrashConfirm(suppress: 'none' | 'session' | 'always') {
        trashConfirmVisible = false;
        if (suppress === 'session') skipTrashConfirmSession = true;
        if (suppress === 'always') await setAppSetting('skip_trash_confirm', 'true');
        await executeTrash();
    }

    async function handlePermanentDelete() {
        const imgs = $images;
        const idx = $focusedIndex;
        const img = imgs[idx];
        if (!img) return;
        const name = img.path.split('/').pop() ?? '';
        if (!confirm(`Permanently delete "${name}"? This cannot be undone.`)) return;
        const count = await deleteImagesPermanently([img.image.id]);
        if (count > 0) {
            showToast(`Deleted permanently`, { detail: name, type: 'warning', duration: 5000 });
            invalidateImageCache();
            removeVisibleImageById(img.image.id, nextFocusIndexAfterFocusedRemoval(idx, imgs.length));
            refreshImageCount().catch(e => console.error('Failed to refresh image count after delete:', e));
            refreshCollectionCountsAfterRemoval('delete');
        }
    }

    async function restorePreviewDisplaySettings() {
        const mode = parsePreviewDisplayMode(await getAppSetting(PREVIEW_DISPLAY_MODE_SETTING));
        setPreviewDisplayMode(mode);
        const overlay = parsePreviewDisplayOverlay(await getAppSetting(PREVIEW_DISPLAY_OVERLAY_SETTING));
        if (overlay) setPreviewDisplayOverlay(overlay);
        setPreviewDisplayAlwaysOnTop((await getAppSetting(PREVIEW_DISPLAY_ALWAYS_ON_TOP_SETTING)) === 'true');
    }

    async function syncFocusedImageToPreviewDisplay(image: ImageWithFile | null) {
        const payload = nextPreviewFocusPayload(image, previewSyncState);
        const imageId = previewSyncImageId(image, previewSyncState, $previewDisplayFrozen, $previewDisplayBlanked);
        const syncKey = JSON.stringify({
            imageId,
            displayMode: $previewDisplayMode,
            overlay: $previewDisplayOverlay,
            frozen: $previewDisplayFrozen,
            blanked: $previewDisplayBlanked,
            alwaysOnTop: $previewDisplayAlwaysOnTop,
        });
        if (syncKey === lastPreviewSyncKey) return;
        lastPreviewSyncKey = syncKey;
        previewSyncState = await updatePreviewState(
            imageId,
            $previewDisplayMode ?? payload.displayMode,
            $previewDisplayOverlay ?? payload.overlay,
            $previewDisplayFrozen,
            $previewDisplayBlanked
        );
    }

    function handleWindowKeydown(event: KeyboardEvent) {
        if (previewDisplayWindow) return;
        handleKeydown(event);
    }

    type AgentSnapshotCaptureOptions = {
        requestId?: string;
        snapshotId?: string;
        clipboard?: boolean;
        captureReason?: string;
    };

    type AgentSnapshotSelectionPayload = {
        image_ids?: string[];
        imageIds?: string[];
        mode?: 'replace' | 'add' | 'toggle';
        focus_first?: boolean;
        focusFirst?: boolean;
    };

    function createAgentSnapshotId(): string {
        const stamp = new Date().toISOString().replace(/[-:.TZ]/g, '').slice(0, 17);
        const random = typeof crypto !== 'undefined' && 'randomUUID' in crypto
            ? crypto.randomUUID().replace(/-/g, '').slice(0, 8)
            : Math.random().toString(36).slice(2, 10);
        return `snap_${stamp}_${random}`;
    }

    function currentAgentSnapshotScope(): AgentSnapshotScope {
        if ($activeCanvas) {
            return { kind: 'canvas', id: $activeCanvas.id, label: $activeCanvas.name, path: null };
        }
        if ($activeSession) {
            return { kind: 'session', id: $activeSession.id, label: $activeSession.name, path: $activeSession.folder_path };
        }
        if ($activeSmartCollection) {
            return { kind: 'smart_collection', id: $activeSmartCollection.id, label: $activeSmartCollection.name, path: null };
        }
        if ($activeCollection) {
            const collection = $collections.find(([id]) => id === $activeCollection);
            return { kind: 'collection', id: $activeCollection, label: collection?.[1] ?? $activeCollection, path: null };
        }
        if ($activeFolder) {
            return {
                kind: 'folder',
                id: null,
                label: $activeFolder.split('/').filter(Boolean).pop() ?? $activeFolder,
                path: $activeFolder,
            };
        }
        if ($activeDetectedClass) {
            return { kind: 'detected_class', id: null, label: $activeDetectedClass, path: null };
        }
        return { kind: 'all', id: null, label: 'All Images', path: null };
    }

    async function captureAgentViewSnapshot(options: AgentSnapshotCaptureOptions = {}) {
        const snapshotId = options.snapshotId ?? createAgentSnapshotId();
        const clipboard = options.clipboard ?? false;
        try {
            const rawPngBase64 = await captureAgentWindowSnapshot();
            const visibleImages = collectVisibleImageTargets({
                viewMode: $viewMode,
                selectedIds: $selectedIds,
                focusedImageId: $focusedImage?.image.id ?? null,
            });
            const annotatedPngBase64 = await drawAnnotatedSnapshot(rawPngBase64, visibleImages);
            const packageHint = `Agent Snapshots/${snapshotId}`;
            const manifest = buildAgentSnapshotManifest({
                snapshotId,
                createdAt: new Date().toISOString(),
                viewMode: $viewMode,
                captureReason: options.captureReason ?? 'shortcut',
                destination: { kind: clipboard ? 'clipboard' : 'local', detail: packageHint },
                files: {
                    raw_png: `${packageHint}/raw.png`,
                    annotated_png: `${packageHint}/annotated.png`,
                    manifest_json: `${packageHint}/manifest.json`,
                },
                window: {
                    label: $windowLabel,
                    title: document.title || 'Cull',
                    width_css: window.innerWidth,
                    height_css: window.innerHeight,
                    device_pixel_ratio: window.devicePixelRatio || 1,
                },
                scope: currentAgentSnapshotScope(),
                visibleImages,
            });
            const written = await completeAgentViewSnapshot({
                request_id: options.requestId,
                snapshot_id: snapshotId,
                manifest,
                raw_png_base64: rawPngBase64,
                annotated_png_base64: annotatedPngBase64,
                clipboard,
            });
            showToast(clipboard ? 'Agent snapshot saved and copied' : 'Agent snapshot saved', {
                detail: String(written.package_dir),
                type: 'success',
                duration: 6000,
            });
        } catch (e) {
            showToast('Agent snapshot failed', { detail: String(e), type: 'error', duration: 8000 });
        }
    }

    function applyAgentViewSnapshotSelection(payload: AgentSnapshotSelectionPayload) {
        const ids = payload.image_ids ?? payload.imageIds ?? [];
        const mode = payload.mode ?? 'replace';
        const next = new Set(mode === 'replace' ? [] : $selectedIds);
        for (const imageId of ids) {
            if (mode === 'toggle') {
                if (next.has(imageId)) next.delete(imageId);
                else next.add(imageId);
            } else {
                next.add(imageId);
            }
        }
        selectedIds.set(next);

        const focusFirst = payload.focus_first ?? payload.focusFirst ?? true;
        if (focusFirst && ids.length > 0) {
            const idx = $images.findIndex(item => item.image.id === ids[0]);
            if (idx >= 0) focusedIndex.set(idx);
        }
    }

    async function refreshAgentPanelData() {
        const [proposals, presets] = await Promise.all([
            listActionProposals('pending', 20),
            listAgentSelectionPresets(),
        ]);
        agentProposals = proposals;
        agentSelectionPresets = presets;
        if (!$activeAgentSelectionPresetId && presets.length > 0) {
            activeAgentSelectionPresetId.set(presets[0].id);
        }
    }

    function proposalCandidateIds(): string[] {
        const selected = Array.from($selectedIds);
        if (selected.length > 0) return selected.slice(0, 24);
        return $images.slice(0, 12).map(item => item.image.id);
    }

    function proposalCandidateImages(ids: string[]): ImageWithFile[] {
        const wanted = new Set(ids);
        return $images.filter(item => wanted.has(item.image.id));
    }

    function visualLevelForAgentRequest(candidateImages: ImageWithFile[]) {
        return effectiveAgentVisualLevel({
            requestedVisualLevel: $agentVisualLevel,
            candidateCount: candidateImages.length,
            thumbnailCount: candidateImages.filter(item => !!item.thumbnail_path).length,
        });
    }

    function proposalKindForPreset(preset: AgentSelectionPreset | null, instruction: string) {
        const text = `${preset?.purpose ?? ''} ${instruction}`.toLowerCase();
        if (/\b(trash|cleanup|reject|remove)\b/.test(text)) return 'trash_images';
        return 'select_images';
    }

    function imageContextForAgent(candidateImages: ImageWithFile[], visualLevel: AgentVisualLevel): AgentChatImageContext[] {
        return candidateImages
            .map(item => ({
                image_id: item.image.id,
                filename: item.path.split(/[\\/]/).pop() ?? null,
                width: item.image.width ?? null,
                height: item.image.height ?? null,
                format: item.image.format ?? null,
                star_rating: item.selection?.star_rating ?? null,
                color_label: item.selection?.color_label ?? null,
                decision: item.selection?.decision ?? null,
                source_label: item.source_label ?? null,
                thumbnail_path: visualLevel === 'text' ? null : item.thumbnail_path,
            }));
    }

    async function createManualAgentProposal(presetId: string | null, instruction: string) {
        const preset = agentSelectionPresets.find(item => item.id === presetId) ?? agentSelectionPresets[0] ?? null;
        const ids = proposalCandidateIds();
        if (ids.length === 0) {
            showToast('No images available for proposal', { type: 'warning', duration: 5000 });
            return;
        }
        const candidateImages = proposalCandidateImages(ids);
        const visualLevel = visualLevelForAgentRequest(candidateImages);
        if (visualLevel !== $agentVisualLevel) agentVisualLevel.set(visualLevel);
        const kind = proposalKindForPreset(preset, instruction);
        const estimatedBudget = estimateAgentBudget({
            candidateCount: ids.length,
            instruction,
            visualLevel,
        });
        const items = ids.map((image_id, index) => ({
            image_id,
            reason: `${preset?.name ?? 'Manual preset'} candidate ${index + 1}: ${instruction}`,
            confidence: 'manual',
        }));
        const proposal = await createActionProposal({
            kind,
            persona: 'copilot',
            lens: preset?.purpose ?? 'selection',
            criteria: `${preset?.prompt ?? 'Selection proposal'}\n\nUser: ${instruction}`,
            visual_level: visualLevel,
            selection_preset_id: preset?.id ?? null,
            estimated_input_tokens: estimatedBudget.inputTokens,
            estimated_output_tokens: estimatedBudget.outputTokens,
            estimated_cost_eur: estimatedBudget.costEur,
            source_context_json: JSON.stringify({
                source: 'agent_chat_manual_seed',
                selected_count: $selectedIds.size,
                visible_count: $images.length,
            }),
            items_json: JSON.stringify(items),
            guard_results_json: JSON.stringify({ blocked: [] }),
        });
        agentProposals = [proposal, ...agentProposals.filter(item => item.id !== proposal.id)];
        activeAgentProposalId.set(proposal.id);
        agentPanelVisible.set(true);
        agentPanelPinned.set(true);
        showToast('Agent proposal created', {
            detail: kind === 'trash_images' ? 'Review before moving files to Trash' : 'Review before changing selection',
            type: 'info',
            duration: 5000,
        });
    }

    async function handleCreateAgentProposal(presetId: string | null, instruction: string) {
        if (agentChatBusy) return;
        const preset = agentSelectionPresets.find(item => item.id === presetId) ?? agentSelectionPresets[0] ?? null;
        const ids = proposalCandidateIds();
        const rawCandidateImages = proposalCandidateImages(ids);
        const visualLevel = visualLevelForAgentRequest(rawCandidateImages);
        if (visualLevel !== $agentVisualLevel) agentVisualLevel.set(visualLevel);
        const candidateImages = imageContextForAgent(rawCandidateImages, visualLevel);
        if (candidateImages.length === 0) {
            showToast('No images available for Claude', { type: 'warning', duration: 5000 });
            return;
        }

        agentChatBusy = true;
        lastAgentMessage = null;
        lastAgentInstruction = instruction;
        const requestId = crypto.randomUUID?.() ?? `agent-${Date.now()}`;
        activeAgentRequestId = requestId;
        agentStreamEvents = [];
        try {
            const result = await runClaudeAgentChatTurn({
                request_id: requestId,
                instruction,
                visual_level: visualLevel,
                preset,
                candidate_images: candidateImages,
                selected_count: $selectedIds.size,
                visible_count: $images.length,
                model: null,
                max_budget_usd: null,
            });
            lastAgentMessage = result.message;

            if (result.proposal) {
                agentProposals = [result.proposal, ...agentProposals.filter(item => item.id !== result.proposal?.id)];
                activeAgentProposalId.set(result.proposal.id);
                agentPanelVisible.set(true);
                agentPanelPinned.set(true);
                showToast('Claude proposal created', {
                    detail: result.proposal.kind === 'trash_images' ? 'Review before moving files to Trash' : 'Review before changing selection',
                    type: 'info',
                    duration: 5000,
                });
            }

            if (result.updated_preset) {
                agentSelectionPresets = agentSelectionPresets.map(item => item.id === result.updated_preset?.id ? result.updated_preset : item);
                activeAgentSelectionPresetId.set(result.updated_preset.id);
                showToast('Claude updated preset', { detail: result.updated_preset.name, type: 'success', duration: 5000 });
            }

            if (!result.proposal && !result.updated_preset) {
                showToast('Claude replied', { detail: result.message, type: 'info', duration: 6000 });
            }
        } catch (e) {
            showToast('Claude agent failed', { detail: String(e), type: 'error', duration: 9000 });
        } finally {
            agentChatBusy = false;
            activeAgentRequestId = null;
        }
    }

    async function handleUpdateAgentPreset(presetId: string, prompt: string) {
        const preset = agentSelectionPresets.find(item => item.id === presetId);
        if (!preset) return;
        const updated = await upsertAgentSelectionPreset({
            id: preset.id,
            name: preset.name,
            purpose: preset.purpose,
            prompt,
            criteria_json: preset.criteria_json,
            sort_order: preset.sort_order,
        });
        agentSelectionPresets = agentSelectionPresets.map(item => item.id === updated.id ? updated : item);
        showToast('Selection preset updated', { detail: updated.name, type: 'success', duration: 4000 });
    }

    async function handleDismissAgentProposal(proposalId: string) {
        await dismissActionProposal(proposalId);
        agentProposals = agentProposals.filter(item => item.id !== proposalId);
        if ($activeAgentProposalId === proposalId) activeAgentProposalId.set(null);
    }

    function handleCloseAgentPanel() {
        agentPanelVisible.set(false);
        agentPanelPinned.set(false);
    }

    function handleReviewAgentProposal(proposalId: string) {
        reviewProposalId = proposalId;
        activeAgentProposalId.set(proposalId);
    }

    async function handleApplyAgentProposal(proposalId: string, approvedImageIds: string[]) {
        const proposal = agentProposals.find(item => item.id === proposalId);
        if (!proposal) return;
        if (proposal.kind === 'trash_images') {
            const trashResult = await trashImagesDetailed(approvedImageIds);
            await applyActionProposal(proposalId, approvedImageIds, JSON.stringify(trashResult));
            const trashed = new Set(trashResult.results.filter(item => item.status === 'trashed').map(item => item.image_id));
            images.update(list => list.filter(item => !trashed.has(item.image.id)));
            invalidateImageCache();
            refreshImageCount().catch(e => console.error('Failed to refresh image count after proposal trash:', e));
            refreshCollectionCountsAfterRemoval('proposal trash');
            showToast('Trash proposal applied', {
                detail: `${trashResult.succeeded} moved to Trash, ${trashResult.failed} failed`,
                type: trashResult.failed > 0 ? 'warning' : 'info',
                duration: 6000,
            });
        } else {
            const visibleIds = new Set($images.map(item => item.image.id));
            const visibleApprovedIds = approvedImageIds.filter(id => visibleIds.has(id));
            if (visibleApprovedIds.length === 0) {
                showToast('Selection proposal no longer matches this view', {
                    detail: 'None of the approved images are currently loaded',
                    type: 'warning',
                    duration: 6000,
                });
                return;
            }
            selectedIds.set(new Set(visibleApprovedIds));
            const firstIndex = $images.findIndex(item => item.image.id === visibleApprovedIds[0]);
            if (firstIndex >= 0) focusedIndex.set(firstIndex);
            await applyActionProposal(
                proposalId,
                visibleApprovedIds,
                JSON.stringify({
                    selected: visibleApprovedIds.length,
                    missing: approvedImageIds.length - visibleApprovedIds.length,
                }),
            );
            showToast('Selection proposal applied', {
                detail: approvedImageIds.length === visibleApprovedIds.length
                    ? `${visibleApprovedIds.length} images selected`
                    : `${visibleApprovedIds.length} selected, ${approvedImageIds.length - visibleApprovedIds.length} no longer visible`,
                type: visibleApprovedIds.length === approvedImageIds.length ? 'success' : 'warning',
                duration: 6000,
            });
        }
        reviewProposalId = null;
        activeAgentProposalId.set(null);
        await refreshAgentPanelData();
    }

    $effect(() => {
        const image = $focusedImage;
        const frozen = $previewDisplayFrozen;
        const blanked = $previewDisplayBlanked;
        const alwaysOnTop = $previewDisplayAlwaysOnTop;
        const mode = $previewDisplayMode;
        const overlay = $previewDisplayOverlay;
        if (previewDisplayWindow) return;
        void frozen;
        void blanked;
        void alwaysOnTop;
        void mode;
        void overlay;
        syncFocusedImageToPreviewDisplay(image).catch((e) => {
            console.debug('Failed to sync Preview Display focus:', e);
        });
    });

    onMount(() => {
        if (previewDisplayWindow) return;

        const init = async () => {
            // Register core tabs and activate first-party bundled plugins
            // (e.g. cull-publish) BEFORE any view renders, so their tabs are in
            // the registry-driven Ctrl+Tab cycle and command palette. Bundled
            // plugins activate regardless of the module_plugins flag.
            registerCoreTabs();
            await activateBundledPlugins(BUNDLED_PLUGINS, { pluginsFlagEnabled: false });
            await restorePreviewDisplaySettings();
            const restored = restoreAppStateBeforeImages();
            await restoreSmartCollectionScope(restored);
            const restoredLoadedCount = restored?.loadedImageCount ?? 0;
            const restoredFocusCount = (restored?.focusedIndex ?? 0) + 1;
            await loadImages({
                resetFocus: false,
                minItems: Math.max(restoredLoadedCount, restoredFocusCount),
            });
            applyRestoredViewState(restored);
            await initDeepLink();
            staticPublishingEnabled.set((await getAppSetting('module_static_publishing')) === 'true');
            clientToolsEnabled.set((await getAppSetting('module_client_tools')) === 'true');
            voiceDictationEnabled.set((await getAppSetting('module_voice_dictation')) === 'true');
            // Plugin runtime: default OFF. When off, no plugin code loads and
            // no plugin surface (palette commands, views) is reachable.
            const pluginsRuntimeEnabled = (await getAppSetting('module_plugins')) === 'true';
            pluginsEnabled.set(pluginsRuntimeEnabled);
            if (pluginsRuntimeEnabled) {
                loadInstalledPlugins().catch((e) => console.error('[plugins] load failed:', e));
            }
            await refreshAgentPanelData();

            try {
                const health = await checkLibraryHealth();
                if (health.purged > 0) {
                    showToast(`Cleaned up library`, {
                        detail: `Removed ${health.purged} image${health.purged === 1 ? '' : 's'} with missing source files`,
                        type: 'info',
                        duration: 7000,
                    });
                    await loadImages({ force: true, invalidateCache: true });
                }
                if (health.to_regenerate.length > 0) {
                    regenerateThumbnailsByIds(health.to_regenerate).then((count) => {
                        if (count > 0) {
                            loadImages({ force: true });
                        }
                    });
                }
            } catch (e) {
                console.error('Library health check failed:', e);
            }
        };
        init().catch(e => {
            console.error('Failed to initialize app:', e);
            showToast('App initialization failed', { detail: String(e), type: 'error', duration: 10000 });
        });
        initMenu().catch(e => console.error('Failed to init menu:', e));

        const dragUnlisten = listen<boolean>('drag-hover', (event) => {
            dragOver = event.payload;
        });

        window.addEventListener('trash-focused-image', handleTrash);
        window.addEventListener('delete-focused-image', handlePermanentDelete);
        const handleReloadImages = () => loadImages({ resetFocus: false, force: true, invalidateCache: true }).catch(e => console.error('Failed to reload:', e));
        window.addEventListener('reload-images', handleReloadImages);
        const handleCreateAgentTestProposal = () => {
            createManualAgentProposal($activeAgentSelectionPresetId, 'Create a test proposal from the current selection')
                .catch(e => console.error('Failed to create test proposal:', e));
        };
        window.addEventListener('create-agent-test-proposal', handleCreateAgentTestProposal);
        const handleOpenAgentPanel = () => {
            agentPanelVisible.set(true);
            agentPanelPinned.set(true);
        };
        window.addEventListener('open-agent-panel', handleOpenAgentPanel);
        const handleAgentSnapshotCommand = (event: Event) => {
            const detail = event instanceof CustomEvent ? event.detail : {};
            captureAgentViewSnapshot({ clipboard: Boolean(detail?.clipboard) })
                .catch(e => console.error('Failed to capture agent snapshot:', e));
        };
        window.addEventListener('capture-agent-view-snapshot', handleAgentSnapshotCommand);

        const watcherUnlisten = listen<void>('images:changed', () => {
            loadImages({ resetFocus: false, force: true, invalidateCache: true }).catch(e => console.error('Failed to reload after fs change:', e));
        });

        const agentSnapshotRequestUnlisten = listen<{
            request_id: string;
            snapshot_id: string;
            clipboard: boolean;
            capture_reason: string;
        }>('agent-view-snapshot:request', (event) => {
            captureAgentViewSnapshot({
                requestId: event.payload.request_id,
                snapshotId: event.payload.snapshot_id,
                clipboard: event.payload.clipboard,
                captureReason: event.payload.capture_reason,
            }).catch(e => console.error('Failed to complete requested agent snapshot:', e));
        });

        const agentSnapshotSelectionUnlisten = listen<AgentSnapshotSelectionPayload>('agent-view-snapshot:select-images', (event) => {
            applyAgentViewSnapshotSelection(event.payload);
        });

        const agentStreamUnlisten = listen<ClaudeAgentStreamEvent>('claude-agent:stream-event', (event) => {
            if (event.payload.request_id !== activeAgentRequestId) return;
            agentStreamEvents = [
                ...agentStreamEvents.filter(item => item.sequence !== event.payload.sequence),
                event.payload,
            ]
                .sort((a, b) => a.sequence - b.sequence)
                .slice(-24);
        });

        const panicUnlisten = listen<{thread: string, location: string | null, message: string}>('rust-panic', (event) => {
            console.error('[rust-panic]', event.payload);
            showToast('Background thread crashed', { detail: event.payload.message, type: 'error', duration: 10000 });
        });

        const taskFailUnlisten = listen<{task: string, message: string, recoverable: boolean}>('background-task-failed', (event) => {
            console.error('[task-failed]', event.payload);
            showToast(`${event.payload.task} failed`, { detail: event.payload.message, type: 'error', duration: 8000 });
        });

        let cloudWarningShown = false;
        const cloudUnlisten = listen<{path: string, provider: string}>('watcher:cloud-eviction', (event) => {
            if (!cloudWarningShown) {
                cloudWarningShown = true;
                showToast(`Cloud files detected`, {
                    detail: `Some images in your ${event.payload.provider} folder are stored in the cloud. Open them in Finder to download locally.`,
                    type: 'info',
                    duration: 10000,
                });
            }
        });

        let first = true;
        const unsub = minSizeFilter.subscribe(() => {
            if (first) { first = false; return; }
            loadImages({ force: true }).catch(e => console.error('Failed to reload images with filter:', e));
        });

        let firstMissing = true;
        const unsubMissing = showMissing.subscribe(() => {
            if (firstMissing) { firstMissing = false; return; }
            loadImages({ force: true }).catch(e => console.error('Failed to reload images with missing filter:', e));
        });

        const saveTimer = setInterval(saveAppState, 5000);
        const handleBeforeUnload = () => saveAppState();
        window.addEventListener('beforeunload', handleBeforeUnload);

        return () => {
            unsub();
            unsubMissing();
            dragUnlisten.then(fn => fn());
            watcherUnlisten.then(fn => fn());
            agentSnapshotRequestUnlisten.then(fn => fn());
            agentSnapshotSelectionUnlisten.then(fn => fn());
            agentStreamUnlisten.then(fn => fn());
            panicUnlisten.then(fn => fn());
            taskFailUnlisten.then(fn => fn());
            cloudUnlisten.then(fn => fn());
            window.removeEventListener('trash-focused-image', handleTrash);
            window.removeEventListener('delete-focused-image', handlePermanentDelete);
            window.removeEventListener('reload-images', handleReloadImages);
            window.removeEventListener('create-agent-test-proposal', handleCreateAgentTestProposal);
            window.removeEventListener('open-agent-panel', handleOpenAgentPanel);
            window.removeEventListener('capture-agent-view-snapshot', handleAgentSnapshotCommand);
            clearInterval(saveTimer);
            window.removeEventListener('beforeunload', handleBeforeUnload);
            saveAppState();
        };
    });
</script>

<svelte:window onkeydown={handleWindowKeydown} />

{#if previewDisplayWindow}
    <PreviewDisplay />
{:else}
    <UpdateBanner />
    <div class="app-shell" class:no-sidebar={noSidebar} class:zen={$zenMode} class:agent-pinned={$agentPanelPinned && !$zenMode}>
        {#if !$zenMode}
            <TabBar />
        {/if}
        {#if !noSidebar && !$zenMode}
            <Sidebar />
        {/if}
        <ImportBanner />
        {#if $viewMode === 'grid'}
            <div class="main-with-commandbar">
                <div class="command-bar-area">
                    <CommandBar />
                </div>
                <Grid />
            </div>
        {:else if $viewMode === 'compare'}
            <Compare />
        {:else if $viewMode === 'loupe'}
            <Loupe />
        {:else if $viewMode === 'embeddings'}
            <EmbeddingExplorer />
        {:else if $tabRegistry.find(t => t.id === $viewMode && t.source === 'plugin')}
            <div class="plugin-view">
                <PluginViewHost pluginId={$viewMode} />
            </div>
        {:else if $viewMode === 'export'}
            <Export />
        {:else if $viewMode === 'lineage'}
            <LineageView />
        {:else if $viewMode === 'canvas'}
            <Canvas />
        {:else if $viewMode === 'tinder'}
            <Tinder />
        {:else}
            <div class="placeholder">
                <span class="placeholder-label">{$viewMode}</span>
                <span class="placeholder-text">Coming soon</span>
            </div>
        {/if}
        <div class="agent-dock-area">
            <AgentProposalDock
                proposals={agentProposals}
                presets={agentSelectionPresets}
                selectedCount={$selectedIds.size}
                pinned={$agentPanelPinned}
                visible={$agentPanelVisible}
                busy={agentChatBusy}
                lastMessage={lastAgentMessage}
                lastInstruction={lastAgentInstruction}
                streamEvents={agentStreamEvents}
                visualLevel={$agentVisualLevel}
                activePresetId={$activeAgentSelectionPresetId}
                activeProposalId={$activeAgentProposalId}
                candidateCount={agentCandidateCount}
                visibleImages={$images}
                onreviewproposal={handleReviewAgentProposal}
                ondismissproposal={handleDismissAgentProposal}
                oncreateproposal={handleCreateAgentProposal}
                onupdatepreset={handleUpdateAgentPreset}
                onselectpreset={(presetId) => activeAgentSelectionPresetId.set(presetId)}
                onselectproposal={(proposalId) => activeAgentProposalId.set(proposalId)}
                onvisuallevelcycle={cycleAgentVisualLevel}
                onclose={handleCloseAgentPanel}
            />
        </div>
        {#if !$zenMode}
            <StatusBar />
        {/if}

        <Toast />

        {#if dragOver}
            <div class="drop-overlay">
                <div class="drop-label">Drop to import</div>
            </div>
        {/if}
    </div>

    <JobProgressPanel />
    <GenerationResultsStrip />
    <CommandPalette />
    <KeyboardShortcuts />
    <ExportFolderDialog />
    <ContactSheetDialog />
    <GroupRankingDialog />
    <UndoHistoryPanel />

    {#if $settingsOpen}
        <McpSettings onclose={() => settingsOpen.set(false)} />
    {/if}

    {#if $aboutOpen}
        <AboutDialog onclose={() => aboutOpen.set(false)} />
    {/if}

    {#if $agentSkillsOpen}
        <AgentSkillsDialog onclose={() => agentSkillsOpen.set(false)} />
    {/if}

    <TrashConfirmDialog
        visible={trashConfirmVisible}
        fileName={trashConfirmFileName}
        onconfirm={handleTrashConfirm}
        oncancel={() => trashConfirmVisible = false}
    />

    <ActionProposalReviewDialog
        proposal={reviewProposal}
        visible={reviewProposal !== null}
        visibleImages={$images}
        onapplyproposal={handleApplyAgentProposal}
        oncancelreview={() => reviewProposalId = null}
    />

    <TextInputDialog />
    <ConfirmDialog />
    <CollectionTargetDialog />
{/if}

<style>
    .app-shell {
        display: grid;
        grid-template-areas:
            "tabbar tabbar"
            "sidebar main"
            "statusbar statusbar";
        grid-template-rows: var(--macos-titlebar-safe-area) 1fr 32px;
        grid-template-columns: 220px 1fr;
        height: 100vh;
        width: 100vw;
        background: var(--bg);
    }
    .app-shell.no-sidebar {
        grid-template-areas:
            "tabbar"
            "main"
            "statusbar";
        grid-template-columns: 1fr;
    }
    .app-shell.agent-pinned {
        grid-template-areas:
            "tabbar tabbar tabbar"
            "sidebar main agent"
            "statusbar statusbar statusbar";
        grid-template-columns: 220px minmax(0, 1fr) 360px;
    }
    .app-shell.no-sidebar.agent-pinned {
        grid-template-areas:
            "tabbar tabbar"
            "main agent"
            "statusbar statusbar";
        grid-template-columns: minmax(0, 1fr) 360px;
    }
    .app-shell.zen {
        grid-template-areas: "main";
        grid-template-rows: 1fr;
        grid-template-columns: 1fr;
        padding-top: var(--macos-titlebar-safe-area);
    }
    .placeholder {
        grid-area: main;
        display: flex;
        flex-direction: column;
        align-items: center;
        justify-content: center;
        gap: 8px;
        color: var(--text-secondary);
    }
    .placeholder-label {
        text-transform: uppercase;
        font-size: 14px;
        color: var(--text-secondary);
        font-weight: 700;
    }
    .placeholder-text {
        font-size: 12px;
        opacity: 0.5;
    }
    .plugin-view {
        grid-area: main;
        overflow-y: auto;
        background: var(--bg);
    }
    .agent-dock-area {
        display: contents;
    }
    .app-shell.agent-pinned .agent-dock-area {
        display: block;
        grid-area: agent;
        min-width: 0;
        overflow: hidden;
    }
    .app-shell.agent-pinned .agent-dock-area :global(.agent-dock) {
        height: 100%;
        min-width: 0;
    }
    .drop-overlay {
        position: fixed;
        inset: 0;
        background: color-mix(in srgb, var(--bg) 72%, transparent);
        border: 3px solid var(--blue);
        display: flex;
        align-items: center;
        justify-content: center;
        z-index: var(--z-drag-overlay);
        pointer-events: none;
    }
    .drop-label {
        font-size: 18px;
        font-weight: 700;
        color: var(--blue);
        text-transform: uppercase;
        letter-spacing: 0;
    }
    .main-with-commandbar {
        grid-area: main;
        display: flex;
        flex-direction: column;
        overflow: hidden;
    }
    .main-with-commandbar :global(.grid-container) {
        grid-area: unset;
        flex: 1;
        min-height: 0;
    }
    .command-bar-area {
        padding: 8px 12px 0;
        flex-shrink: 0;
    }
</style>
