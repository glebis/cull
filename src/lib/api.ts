// Copyright (c) 2026-present Gleb Kalinin. Architecture and design by author.
// Implementation assisted by Claude (Anthropic). See AUTHORSHIP.md.

import { invoke } from '@tauri-apps/api/core';

function emitSessionEventsRefresh() {
    if (typeof window !== 'undefined') {
        window.dispatchEvent(new CustomEvent('session-events-refresh'));
    }
}

export interface Image {
    id: string;
    sha256_hash: string;
    width: number;
    height: number;
    format: string;
    file_size: number;
    created_at: string;
    imported_at: string;
    ai_prompt: string | null;
    raw_metadata: string | null;
}

const RAW_EXTENSIONS = ['cr2', 'cr3', 'nef', 'arw', 'dng', 'orf', 'raf', 'rw2', 'pdf'];

export function isRawFormat(format: string): boolean {
    return RAW_EXTENSIONS.includes(format.toLowerCase());
}

export interface Selection {
    image_id: string;
    project_id: string | null;
    star_rating: number | null;
    color_label: string | null;
    decision: string;
}

export interface ImageWithFile {
    image: Image;
    path: string;
    thumbnail_path: string | null;
    selection: Selection | null;
    source_label: string | null;
    missing_at: string | null;
}

export interface MediaAsset {
    id: string;
    media_type: string;
    primary_image_id: string;
    sha256_hash: string;
    format: string;
    file_size: number;
    page_count: number | null;
    title: string | null;
    created_at: string;
    imported_at: string;
}

export interface MediaFile {
    id: string;
    media_asset_id: string;
    path: string;
    last_seen_at: string;
    missing_at: string | null;
    last_seen_size: number | null;
    last_seen_mtime: string | null;
}

export interface PdfPage {
    id: string;
    media_asset_id: string;
    page_index: number;
    width_points: number | null;
    height_points: number | null;
    thumbnail_path: string | null;
    preview_path: string | null;
    extracted_text: string | null;
    text_extracted_at: string | null;
}

export interface ImageQualityMetrics {
    image_id: string;
    analyzer_version: string;
    focus_score: number;
    blur_score: number;
    exposure_score: number;
    clipped_shadow_pct: number;
    clipped_highlight_pct: number;
    mean_luma: number;
    contrast: number;
    analyzed_at: string;
}

export interface ImagePaletteColor {
    hex: string;
    red: number;
    green: number;
    blue: number;
    percentage: number;
}

export interface ImageColorMetrics {
    image_id: string;
    analyzer_version: string;
    dominant_hex: string;
    palette: ImagePaletteColor[];
    dominant_hue_bucket: string;
    mean_luma: number;
    mean_saturation: number;
    colorfulness: number;
    contrast: number;
    analyzed_at: string;
}

export interface SimilarityGroupSummary {
    id: string;
    model_name: string;
    threshold: number;
    method: string;
    representative_image_id: string | null;
    image_count: number;
    created_at: string;
    updated_at: string;
}

export interface SimilarityGroupingResult {
    model_name: string;
    threshold: number;
    method: string;
    groups_created: number;
    images_grouped: number;
    singleton_images: number;
}

export interface ImageTag {
    id: string;
    image_id: string;
    name: string;
    normalized_name: string;
    tag_type: string;
    source: string;
    confidence: number | null;
    created_at: string;
}

export interface TagSummary {
    id: string;
    name: string;
    normalized_name: string;
    tag_type: string;
    image_count: number;
}

export interface TagBackfillResult {
    images_processed: number;
    tags_created: number;
    image_tags_created: number;
}

export interface ImagePerceptualHash {
    image_id: string;
    algorithm: string;
    hash_hi: number;
    hash_lo: number;
    band0: number;
    band1: number;
    band2: number;
    band3: number;
    analyzed_at: string;
}

export interface NearDuplicateImage {
    image: ImageWithFile;
    algorithm: string;
    distance: number;
}

export interface ImportResponse {
    imported: number;
    skipped: number;
    errors: string[];
    batch_id: string | null;
    image_ids: string[];
}

export interface GenerationRun {
    id: string;
    prompt: string | null;
    negative_prompt: string | null;
    provider: string | null;
    model: string | null;
    settings_json: string;
    seed: string | null;
    parent_run_id: string | null;
    source_type: string;
    source_path: string | null;
    raw_metadata_json: string | null;
    created_at: string | null;
    imported_at: string;
}

export interface OpenWithApplication {
    name: string;
    path: string;
    is_default: boolean;
}

export interface PastedImageResult {
    path: string;
    image_id: string | null;
}

export interface ImageFileBytes {
    bytes: number[];
    mime_type: string;
}

export interface AgentSnapshotPackage {
    snapshot_id: string;
    package_dir: string;
    raw_png_path: string;
    annotated_png_path: string;
    manifest_json_path: string;
    manifest: unknown;
}

export interface CompleteAgentViewSnapshotRequest {
    request_id?: string;
    snapshot_id: string;
    manifest: unknown;
    raw_png_base64: string;
    annotated_png_base64: string;
    clipboard: boolean;
}

export interface MenuStatePayload {
    viewMode: string;
    sidebarVisible: boolean;
    hasFocusedImage: boolean;
    selectedCount: number;
    staticPublishingEnabled: boolean;
    showLoupeHistogram: boolean;
    previewDisplayFrozen: boolean;
    previewDisplayBlanked: boolean;
    previewDisplayAlwaysOnTop: boolean;
    previewDisplayMode: PreviewDisplayMode;
    previewDisplayOverlay: PreviewOverlayConfig;
    previewDisplayWebStreamActive: boolean;
}

export type PreviewDisplayMode = 'image_only' | 'client_review' | 'metadata_review';
export type PreviewRailSide = 'left' | 'right';
export type PreviewRailWidth = 'narrow' | 'medium' | 'wide';
export type PreviewRailTextSize = 'small' | 'medium' | 'large';

export interface PreviewOverlayConfig {
    showFilename: boolean;
    showRating: boolean;
    showDecision: boolean;
    showMetadataRail: boolean;
    showDimensions: boolean;
    showFormat: boolean;
    showSource: boolean;
    showPrompt: boolean;
    showTags: boolean;
    showHistogram: boolean;
    railSide: PreviewRailSide;
    railWidth: PreviewRailWidth;
    railTextSize: PreviewRailTextSize;
}

export interface PreviewState {
    image_id: string | null;
    display_mode: PreviewDisplayMode;
    overlay: PreviewOverlayConfig;
    frozen: boolean;
    blanked: boolean;
    version: number;
    updated_at_ms: number;
}

export interface PreviewDisplayMonitor {
    id: string;
    name: string | null;
    x: number;
    y: number;
    width: number;
    height: number;
    scale_factor: number;
    primary: boolean;
}

export interface PreviewWebStreamStatus {
    active: boolean;
    url: string | null;
    host: string | null;
    bound_host: string | null;
    port: number | null;
    remote_access: boolean;
}

export interface ImageHistogram {
    image_id: string;
    source: 'original' | 'thumbnail';
    pixel_count: number;
    red: number[];
    green: number[];
    blue: number[];
    luma: number[];
}

export interface WindowInfo {
    label: string;
    title: string;
}

export async function openPreviewDisplay(): Promise<string> {
    return invoke<string>('open_preview_display');
}

export async function setPreviewDisplayAlwaysOnTop(alwaysOnTop: boolean): Promise<boolean> {
    return invoke<boolean>('set_preview_display_always_on_top', { alwaysOnTop });
}

export async function listPreviewDisplayMonitors(): Promise<PreviewDisplayMonitor[]> {
    return invoke<PreviewDisplayMonitor[]>('list_preview_display_monitors');
}

export async function placePreviewDisplay(monitorId: string | null, fullscreen: boolean): Promise<string> {
    return invoke<string>('place_preview_display', { monitorId, fullscreen });
}

export async function startPreviewDisplayWebStream(host?: string | null, port?: number | null): Promise<PreviewWebStreamStatus> {
    return invoke<PreviewWebStreamStatus>('start_preview_display_web_stream', { host: host ?? null, port: port ?? null });
}

export async function stopPreviewDisplayWebStream(): Promise<PreviewWebStreamStatus> {
    return invoke<PreviewWebStreamStatus>('stop_preview_display_web_stream');
}

export async function getPreviewDisplayWebStreamStatus(): Promise<PreviewWebStreamStatus> {
    return invoke<PreviewWebStreamStatus>('get_preview_display_web_stream_status');
}

export async function getImageHistogram(imageId: string): Promise<ImageHistogram | null> {
    return invoke<ImageHistogram | null>('get_image_histogram', { imageId });
}

export async function getPreviewState(): Promise<PreviewState> {
    return invoke<PreviewState>('get_preview_state');
}

export async function updatePreviewState(
    imageId: string | null,
    displayMode: PreviewDisplayMode,
    overlay: PreviewOverlayConfig,
    frozen?: boolean,
    blanked?: boolean
): Promise<PreviewState> {
    return invoke<PreviewState>('update_preview_state', {
        imageId,
        displayMode,
        overlay,
        frozen,
        blanked,
    });
}

export async function createWindow(name?: string | null): Promise<string> {
    return invoke<string>('create_window', { name: name ?? null });
}

export async function listWindows(): Promise<WindowInfo[]> {
    return invoke<WindowInfo[]>('list_windows');
}

export async function renameWindow(label: string, newName: string): Promise<void> {
    return invoke<void>('rename_window', { label, newName });
}

export async function sendToWindow(windowName: string, event: string, payload: unknown): Promise<string> {
    return invoke<string>('send_to_window', { windowName, event, payload });
}

export async function startDictation(locale?: string | null): Promise<void> {
    return invoke<void>('start_dictation', { locale: locale ?? null });
}

export async function stopDictation(): Promise<void> {
    return invoke<void>('stop_dictation');
}

export interface CullExchangeExportOptions {
    target_dir: string;
    image_ids?: string[] | null;
    copy_originals: boolean;
    include_xmp: boolean;
}

export interface CullExchangeImportPreview {
    valid: boolean;
    format: string;
    version: number;
    image_count: number;
    collection_count: number;
    smart_collection_count: number;
    generation_run_count: number;
    missing_originals: string[];
    errors: string[];
}

export interface CullExchangeImportResult {
    imported_images: number;
    imported_collections: number;
    imported_smart_collections: number;
    imported_generation_runs: number;
}

// Smart Collections types

export interface FilterGroup {
    type: 'group';
    op: 'and' | 'or';
    children: FilterNode[];
}

export interface FilterNot {
    type: 'not';
    child: FilterNode;
}

export interface FilterRule {
    type: 'rule';
    field: string;
    op: string;
    value: any;
}

export type FilterNode = FilterGroup | FilterNot | FilterRule;

export interface SmartCollection {
    id: string;
    name: string;
    description: string | null;
    collection_type: string;
    filter_json: string | null;
    nl_query: string | null;
    is_preset: boolean;
    sort_order: number;
    created_at: string;
    image_count: number | null;
}

export type AgentPersona = 'curator' | 'copilot' | 'operator';
export type AgentVisualLevel = 'text' | 'tiny' | 'preview' | 'full';
export type AgentProposalStatus = 'pending' | 'applied' | 'dismissed';

export interface AgentActionProposal {
    id: string;
    kind: string;
    status: AgentProposalStatus;
    persona: AgentPersona;
    lens: string | null;
    criteria: string;
    visual_level: AgentVisualLevel;
    selection_preset_id: string | null;
    estimated_input_tokens: number | null;
    estimated_output_tokens: number | null;
    estimated_cost_eur: number | null;
    source_context_json: string;
    items_json: string;
    guard_results_json: string;
    apply_result_json: string | null;
    undo_journal_json: string | null;
    created_at: string;
    updated_at: string;
    applied_at: string | null;
}

export interface CreateActionProposalRequest {
    kind: string;
    persona: AgentPersona;
    lens: string | null;
    criteria: string;
    visual_level: AgentVisualLevel;
    selection_preset_id: string | null;
    estimated_input_tokens: number | null;
    estimated_output_tokens: number | null;
    estimated_cost_eur: number | null;
    source_context_json: string;
    items_json: string;
    guard_results_json: string;
}

export interface ApplyActionProposalResult {
    proposal_id: string;
    status: string;
    applied_count: number;
    failed_count: number;
    result_json: string;
}

export interface AgentSelectionPreset {
    id: string;
    name: string;
    purpose: string;
    prompt: string;
    criteria_json: string;
    sort_order: number;
    created_at: string;
    updated_at: string;
}

export interface UpsertAgentSelectionPresetRequest {
    id: string | null;
    name: string;
    purpose: string;
    prompt: string;
    criteria_json: string;
    sort_order: number | null;
}

export interface AgentChatImageContext {
    image_id: string;
    filename: string | null;
    width: number | null;
    height: number | null;
    format: string | null;
    star_rating: number | null;
    color_label: string | null;
    decision: string | null;
    source_label: string | null;
    thumbnail_path: string | null;
}

export interface ClaudeAgentChatTurnRequest {
    request_id?: string | null;
    instruction: string;
    visual_level: AgentVisualLevel;
    preset: AgentSelectionPreset | null;
    candidate_images: AgentChatImageContext[];
    selected_count: number;
    visible_count: number;
    model: string | null;
    max_budget_usd: number | null;
}

export interface ClaudeAgentChatTurnResult {
    operation: 'answer' | 'create_proposal' | 'update_preset';
    message: string;
    proposal: AgentActionProposal | null;
    updated_preset: AgentSelectionPreset | null;
    usage_json: string;
    raw_result_json: string;
}

export interface ClaudeAgentStreamEvent {
    request_id: string;
    sequence: number;
    phase: string;
    message: string;
    details: Record<string, unknown> | null;
    is_final: boolean;
    is_error: boolean;
}

export async function listImages(limit: number, offset: number): Promise<ImageWithFile[]> {
    return invoke<ImageWithFile[]>('list_images', { limit, offset });
}

export async function listMediaAssets(
    mediaType: string | null,
    limit: number,
    offset: number,
): Promise<MediaAsset[]> {
    return invoke<MediaAsset[]>('list_media_assets', {
        mediaType,
        limit,
        offset,
    });
}

export async function getMediaAsset(mediaAssetId: string): Promise<MediaAsset | null> {
    return invoke<MediaAsset | null>('get_media_asset', { mediaAssetId });
}

export async function getMediaAssetForImage(imageId: string): Promise<MediaAsset | null> {
    return invoke<MediaAsset | null>('get_media_asset_for_image', { imageId });
}

export async function listMediaFiles(mediaAssetId: string): Promise<MediaFile[]> {
    return invoke<MediaFile[]>('list_media_files', { mediaAssetId });
}

export async function listPdfPages(mediaAssetId: string): Promise<PdfPage[]> {
    return invoke<PdfPage[]>('list_pdf_pages', { mediaAssetId });
}

export async function getImageCount(): Promise<number> {
    return invoke<number>('get_image_count');
}

export async function listImageIds(): Promise<string[]> {
    return invoke<string[]>('list_image_ids');
}

export async function importFolder(folderPath: string, sessionId?: string | null): Promise<ImportResponse> {
    const result = await invoke<ImportResponse>('import_folder', { folderPath, sessionId: sessionId ?? null });
    emitSessionEventsRefresh();
    return result;
}

export async function importFiles(filePaths: string[], sessionId?: string | null): Promise<ImportResponse> {
    const result = await invoke<ImportResponse>('import_files', { filePaths, sessionId: sessionId ?? null });
    emitSessionEventsRefresh();
    return result;
}

export async function exportCullExchange(options: CullExchangeExportOptions): Promise<string> {
    return invoke<string>('export_cull_exchange', { options });
}

export async function previewCullExchangeImport(bundleDir: string): Promise<CullExchangeImportPreview> {
    return invoke<CullExchangeImportPreview>('preview_cull_exchange_import', { bundleDir });
}

export async function importCullExchange(bundleDir: string): Promise<CullExchangeImportResult> {
    return invoke<CullExchangeImportResult>('import_cull_exchange', { bundleDir });
}

export async function regenerateThumbnails(): Promise<number> {
    return invoke<number>('regenerate_thumbnails');
}

export interface LibraryHealthResult {
    purged: number;
    missing_sources: number;
    to_regenerate: string[];
}

export async function checkLibraryHealth(): Promise<LibraryHealthResult> {
    return invoke<LibraryHealthResult>('check_library_health');
}

export async function regenerateThumbnailsByIds(imageIds: string[]): Promise<number> {
    return invoke<number>('regenerate_thumbnails_by_ids', { imageIds });
}

export async function regenerateSingleThumbnail(imageId: string): Promise<string> {
    return invoke<string>('regenerate_single_thumbnail', { imageId });
}

export interface JobSnapshot {
    job_id: string;
    kind: string;
    status: string;
    current: number;
    total: number;
    message: string | null;
    error: string | null;
    created_at: string;
    updated_at: string;
}

export async function getJob(jobId: string): Promise<JobSnapshot | null> {
    return invoke<JobSnapshot | null>('get_job', { jobId });
}

export async function listJobs(): Promise<JobSnapshot[]> {
    return invoke<JobSnapshot[]>('list_jobs');
}

export async function cancelJob(jobId: string): Promise<void> {
    return invoke<void>('cancel_job', { jobId });
}

export async function pauseJob(jobId: string): Promise<void> {
    return invoke<void>('pause_job', { jobId });
}

export async function resumeJob(jobId: string): Promise<void> {
    return invoke<void>('resume_job', { jobId });
}

export interface OcrBatchRequest {
    image_ids: string[];
    skip_existing: boolean;
    overwrite: boolean;
}

export interface OcrBatchStartResponse {
    job_id: string;
}

export async function startOcrBatch(request: OcrBatchRequest): Promise<OcrBatchStartResponse> {
    return invoke<OcrBatchStartResponse>('start_ocr_batch', { request });
}

export async function rescanSources(): Promise<number> {
    return invoke<number>('rescan_sources');
}

export async function setRating(imageId: string, rating: number, sessionId?: string | null): Promise<void> {
    await invoke<void>('set_rating', { imageId, rating, sessionId: sessionId ?? null });
    emitSessionEventsRefresh();
}

export async function setDecision(imageId: string, decision: string, sessionId?: string | null): Promise<void> {
    await invoke<void>('set_decision', { imageId, decision, sessionId: sessionId ?? null });
    emitSessionEventsRefresh();
}

export async function getImagesByIds(imageIds: string[]): Promise<ImageWithFile[]> {
    return invoke<ImageWithFile[]>('get_images_by_ids', { imageIds });
}

export async function getImageByPath(path: string): Promise<ImageWithFile | null> {
    return invoke<ImageWithFile | null>('get_image_by_path', { path });
}

export async function getImageFileBytes(imageId: string): Promise<ImageFileBytes> {
    return invoke<ImageFileBytes>('get_image_file_bytes', { imageId });
}

export async function getIterationSiblings(parentId: string): Promise<ImageWithFile[]> {
    return invoke<ImageWithFile[]>('get_iteration_siblings', { parentId });
}

export async function listFolders(): Promise<[string, number][]> {
    return invoke('list_folders');
}

export async function listImagesByFolder(folder: string, limit: number, offset: number): Promise<ImageWithFile[]> {
    return invoke('list_images_by_folder', { folder, limit, offset });
}

export async function deleteFolder(folder: string): Promise<number> {
    const result = await invoke<number>('delete_folder', { folder });
    emitSessionEventsRefresh();
    return result;
}

export async function listImagesFiltered(minWidth: number | null, minHeight: number | null, limit: number, offset: number): Promise<ImageWithFile[]> {
    return invoke('list_images_filtered', { minWidth, minHeight, limit, offset });
}

export async function createCollection(name: string): Promise<string> {
    const result = await invoke<string>('create_collection', { name });
    emitSessionEventsRefresh();
    return result;
}

export async function listCollections(): Promise<[string, string, number][]> {
    return invoke('list_collections');
}

export async function renameCollectionApi(collectionId: string, name: string): Promise<void> {
    await invoke('rename_collection', { collectionId, name });
    emitSessionEventsRefresh();
}

export async function addToCollection(collectionId: string, imageIds: string[]): Promise<void> {
    await invoke('add_to_collection', { collectionId, imageIds });
    emitSessionEventsRefresh();
}

export async function listCollectionImages(collectionId: string, limit?: number, offset?: number): Promise<ImageWithFile[]> {
    return invoke('list_collection_images', { collectionId, limit: limit ?? null, offset: offset ?? null });
}

export async function removeFromCollection(collectionId: string, imageIds: string[]): Promise<void> {
    await invoke('remove_from_collection', { collectionId, imageIds });
    emitSessionEventsRefresh();
}

export async function deleteCollectionApi(collectionId: string): Promise<void> {
    await invoke('delete_collection', { collectionId });
    emitSessionEventsRefresh();
}

export interface ExportImagesParams {
    image_ids?: string[] | null;
    collection_id?: string | null;
    folder_path?: string | null;
    output_dir: string;
    format?: string | null;
    flatten?: boolean | null;
    naming?: string | null;
}

export interface ExportedImage {
    image_id: string;
    source_path: string;
    output_path: string;
    format: string;
    bytes_written: number;
}

export interface ExportImagesResult {
    exported: number;
    skipped: number;
    errors: string[];
    output_dir: string;
    files: ExportedImage[];
}

// Export images to a destination folder with optional format conversion and a
// filename naming template. Provide exactly one selector.
export async function exportImagesToFolder(params: ExportImagesParams): Promise<ExportImagesResult> {
    return invoke('export_images_to_folder', { params });
}

// Write a base64-encoded PNG (e.g. a canvas-rendered contact sheet) to an
// absolute path chosen via the native save dialog.
export async function savePngToPath(outputPath: string, base64Data: string): Promise<string> {
    return invoke('save_png_to_path', { outputPath, base64Data });
}

// Write UTF-8 text (e.g. a delivery CSV) to an absolute path.
export async function saveTextToPath(outputPath: string, contents: string): Promise<string> {
    return invoke('save_text_to_path', { outputPath, contents });
}

export interface ClientFeedback {
    image_id: string;
    favorite: boolean;
    comment: string | null;
    updated_at: string;
}

// Client feedback is stored separately from curator selections.
export async function setClientFeedback(imageId: string, favorite: boolean, comment: string | null): Promise<void> {
    await invoke('set_client_feedback', { imageId, favorite, comment: comment ?? null });
    emitSessionEventsRefresh();
}

export async function getClientFeedback(imageId: string): Promise<ClientFeedback | null> {
    return invoke('get_client_feedback', { imageId });
}

export async function listClientFeedback(): Promise<ClientFeedback[]> {
    return invoke('list_client_feedback');
}

export interface ClipboardMonitorStatus {
    running: boolean;
    supported: boolean;
    access_status: string;
    collection_id: string | null;
    collection_name: string | null;
    capture_dir: string;
    captured_count: number;
    capture_existing_on_start: boolean;
    last_error: string | null;
}

export interface ClipboardPublishResult {
    collection_id: string;
    image_count: number;
    site_dir: string;
    url: string;
    manifest_path: string;
    instructions_path: string;
}

export async function getClipboardMonitorStatus(): Promise<ClipboardMonitorStatus> {
    return invoke('get_clipboard_monitor_status');
}

export async function startClipboardMonitor(captureDir?: string | null): Promise<ClipboardMonitorStatus> {
    return invoke('start_clipboard_monitor', { captureDir: captureDir ?? null });
}

export async function stopClipboardMonitor(): Promise<ClipboardMonitorStatus> {
    return invoke('stop_clipboard_monitor');
}

export async function setClipboardMonitorCaptureDir(path: string): Promise<ClipboardMonitorStatus> {
    return invoke('set_clipboard_monitor_capture_dir', { path });
}

export async function setClipboardMonitorCaptureExistingOnStart(enabled: boolean): Promise<ClipboardMonitorStatus> {
    return invoke('set_clipboard_monitor_capture_existing_on_start', { enabled });
}

export async function moveClipboardCaptureFolder(newPath: string): Promise<ClipboardMonitorStatus> {
    return invoke('move_clipboard_capture_folder', { newPath });
}

export async function publishClipboardCollection(collectionId?: string | null): Promise<ClipboardPublishResult> {
    return invoke('publish_clipboard_collection', { collectionId: collectionId ?? null });
}

// Smart Collection commands

export async function listSmartCollections(): Promise<SmartCollection[]> {
    return invoke('list_smart_collections');
}

export async function createSmartCollection(
    name: string,
    filterJson: string,
    nlQuery?: string,
): Promise<string> {
    const result = await invoke<string>('create_smart_collection', { name, filterJson, nlQuery });
    emitSessionEventsRefresh();
    return result;
}

export async function evaluateSmartCollection(filterJson: string, limit?: number, offset?: number): Promise<ImageWithFile[]> {
    return invoke('evaluate_smart_collection', { filterJson, limit: limit ?? null, offset: offset ?? null });
}

export async function countSmartCollection(filterJson: string): Promise<number> {
    return invoke<number>('count_smart_collection', { filterJson });
}

export async function deleteSmartCollectionApi(id: string): Promise<void> {
    await invoke('delete_smart_collection', { id });
    emitSessionEventsRefresh();
}

export async function updateSmartCollectionApi(
    id: string,
    name: string,
    filterJson: string,
    nlQuery?: string,
): Promise<void> {
    await invoke('update_smart_collection', { id, name, filterJson, nlQuery });
    emitSessionEventsRefresh();
}

export async function parseNlQuery(query: string): Promise<string> {
    return invoke('parse_nl_query', { query });
}

export async function backfillImageMetadata(): Promise<number> {
    return invoke('backfill_image_metadata');
}

export async function backfillImageTags(): Promise<TagBackfillResult> {
    return invoke('backfill_image_tags');
}

export async function listImageTags(imageId: string): Promise<ImageTag[]> {
    return invoke('list_image_tags', { imageId });
}

export async function listTags(limit = 100, offset = 0): Promise<TagSummary[]> {
    return invoke('list_tags', { limit, offset });
}

// Embedding commands
export interface ClipModelDownloadInfo {
    model_id: string;
    url: string;
    expected_sha256: string;
    expected_size_bytes: number;
    spdx_license: string;
    source_repo: string;
    model_card_url: string;
    model_path: string;
    part_path: string;
    curl_command: string;
}

export type EmbeddingModelDownloadInfo = ClipModelDownloadInfo;

export interface EmbeddingProviderInfo {
    id: string;
    label: string;
    shortLabel: string;
    modelName: string;
    dimensions: number;
    dimensionsLabel: string;
    scope: string;
    runtime: string;
    status: string;
    available: boolean;
    downloadable: boolean;
    downloadLabel: string | null;
    expectedSha256: string | null;
    expectedSizeBytes: number | null;
    spdxLicense: string | null;
    sourceRepo: string | null;
    modelCardUrl: string | null;
    apiKeyProvider: string | null;
}

export async function getClipModelDownloadInfo(): Promise<ClipModelDownloadInfo> {
    return invoke('get_clip_model_download_info');
}

export async function getEmbeddingModelDownloadInfo(model = 'clip-vit-b32'): Promise<EmbeddingModelDownloadInfo> {
    return invoke('get_embedding_model_download_info', { model });
}

export async function listEmbeddingProviders(): Promise<EmbeddingProviderInfo[]> {
    return invoke('list_embedding_providers');
}

export async function downloadClipModel(): Promise<string> {
    return invoke('download_clip_model');
}

export async function downloadEmbeddingModel(model = 'clip-vit-b32'): Promise<string> {
    return invoke('download_embedding_model', { model });
}

export async function isModelAvailable(): Promise<boolean> {
    return invoke('is_model_available');
}

export async function isEmbeddingModelAvailable(model = 'clip-vit-b32'): Promise<boolean> {
    return invoke('is_embedding_model_available', { model });
}

export async function generateEmbeddings(imageIds: string[]): Promise<number> {
    return invoke('generate_embeddings', { imageIds });
}

export async function generateModelEmbeddings(model: string, imageIds: string[]): Promise<number> {
    return invoke('generate_model_embeddings', { model, imageIds });
}

export interface EmbeddingPage {
    ids: string[];
    vectors: number[];
    dims: number;
    total: number;
    offset: number;
    limit: number;
    has_more: boolean;
}

export async function getEmbeddingPage(model?: string, limit = 5000, offset = 0): Promise<EmbeddingPage> {
    return invoke('get_embedding_page', { model: model ?? null, limit, offset });
}

export async function findSimilarImages(imageId: string, topK: number, model?: string): Promise<[string, number][]> {
    return invoke('find_similar_images', { imageId, topK, model: model ?? null });
}

export async function generateSimilarityGroups(
    model?: string,
    threshold = 0.88,
    minGroupSize = 2,
): Promise<SimilarityGroupingResult> {
    return invoke('generate_similarity_groups', {
        model: model ?? null,
        threshold,
        minGroupSize,
    });
}

export async function listSimilarityGroups(limit = 100, offset = 0): Promise<SimilarityGroupSummary[]> {
    return invoke('list_similarity_groups', { limit, offset });
}

export async function listSimilarityGroupImages(groupId: string): Promise<ImageWithFile[]> {
    return invoke('list_similarity_group_images', { groupId });
}

export async function getEmbeddingCount(model?: string): Promise<number> {
    return invoke('get_embedding_count', { model: model ?? null });
}

export async function setApiKey(provider: string, key: string): Promise<void> {
    return invoke('set_api_key', { provider, key });
}

export async function validateApiKey(provider: string, key: string): Promise<boolean> {
    return invoke('validate_api_key', { provider, key });
}

export async function deleteApiKey(provider: string): Promise<void> {
    return invoke('delete_api_key', { provider });
}

export async function hasApiKey(provider: string): Promise<boolean> {
    return invoke<boolean>('has_api_key', { provider });
}

export async function generateGeminiEmbeddings(imageIds: string[]): Promise<number> {
    return invoke('generate_gemini_embeddings', { imageIds });
}

export async function checkOllamaEmbedding(): Promise<string[]> {
    return invoke('check_ollama_embedding');
}

export async function getOllamaEmbeddingConfig(): Promise<[string, string]> {
    return invoke('get_ollama_embedding_config');
}

export async function setOllamaEmbeddingConfig(url?: string, model?: string): Promise<void> {
    return invoke('set_ollama_embedding_config', { url: url ?? null, model: model ?? null });
}

// Detection commands (YOLO + NudeNet)
export interface Detection {
    class_name: string;
    confidence: number;
    x: number;
    y: number;
    width: number;
    height: number;
}

export async function downloadYoloModel(variant: string): Promise<string> {
    return invoke('download_yolo_model', { variant });
}

export async function downloadNudenetModel(): Promise<string> {
    return invoke('download_nudenet_model');
}

export async function detectObjects(imageIds: string[], variant?: string): Promise<number> {
    return invoke('detect_objects', { imageIds, variant: variant ?? null });
}

export async function detectNsfw(imageIds: string[]): Promise<number> {
    return invoke('detect_nsfw', { imageIds });
}

export async function getDetections(imageId: string, model?: string): Promise<Detection[]> {
    return invoke('get_detections', { imageId, model: model ?? null });
}

export async function searchByDetectedClass(className: string, limit?: number): Promise<[string, number][]> {
    return invoke('search_by_detected_class', { className, limit: limit ?? 100 });
}

export async function countByDetectedClass(className: string): Promise<number> {
    return invoke('count_by_detected_class', { className });
}

export async function listImagesByDetectedClass(className: string, limit: number, offset: number): Promise<ImageWithFile[]> {
    return invoke('list_images_by_detected_class', { className, limit, offset });
}

export async function isYoloAvailable(variant?: string): Promise<boolean> {
    return invoke('is_yolo_available', { variant: variant ?? null });
}

export async function isNudenetAvailable(): Promise<boolean> {
    return invoke('is_nudenet_available');
}

export async function getDetectionCount(model: string): Promise<number> {
    return invoke('get_detection_count', { model });
}

export async function openWithParams(params: {
    path?: string;
    paths?: string[];
    folder?: string;
    view?: string;
    size?: number;
    zoom?: number;
    fullscreen?: boolean;
    focus?: number;
    imageId?: string;
    gap?: number;
}): Promise<void> {
    return invoke('open_with_params', params);
}

export async function drainPendingOpenParams<T>(): Promise<T[]> {
    return invoke<T[]>('drain_pending_open_params');
}

export async function openDeepLinkUrls(urls: string[]): Promise<void> {
    return invoke('open_deep_link_urls', { urls });
}

// Vision / Ollama commands
export async function checkOllama(): Promise<string[]> {
    return invoke('check_ollama');
}

export async function setOllamaConfig(url?: string, model?: string): Promise<void> {
    return invoke('set_ollama_config', { url: url ?? null, model: model ?? null });
}

export async function getOllamaConfig(): Promise<[string, string]> {
    return invoke('get_ollama_config');
}

export async function analyzeImages(imageIds: string[]): Promise<number> {
    return invoke('analyze_images', { imageIds });
}

export async function getVisionMetadata(imageId: string): Promise<[string, string, string][]> {
    return invoke('get_vision_metadata', { imageId });
}

export async function getVisionCount(source?: string): Promise<number> {
    return invoke('get_vision_count', { source: source ?? null });
}

export async function analyzeImageQuality(imageIds: string[]): Promise<number> {
    return invoke('analyze_image_quality', { imageIds });
}

export async function getImageQuality(imageId: string): Promise<ImageQualityMetrics | null> {
    return invoke('get_image_quality', { imageId });
}

export async function getQualityCount(): Promise<number> {
    return invoke('get_quality_count');
}

export async function analyzeImageColors(imageIds: string[]): Promise<number> {
    return invoke('analyze_image_colors', { imageIds });
}

export async function getImageColorMetrics(imageId: string): Promise<ImageColorMetrics | null> {
    return invoke('get_image_color_metrics', { imageId });
}

export async function getColorMetricsCount(): Promise<number> {
    return invoke('get_color_metrics_count');
}

export async function listImagesByColorBucket(
    bucket: string,
    limit = 100,
    offset = 0,
): Promise<ImageWithFile[]> {
    return invoke('list_images_by_color_bucket', { bucket, limit, offset });
}

export async function analyzePerceptualHashes(imageIds: string[]): Promise<number> {
    return invoke('analyze_perceptual_hashes', { imageIds });
}

export async function getImagePerceptualHash(
    imageId: string,
    algorithm?: string,
): Promise<ImagePerceptualHash | null> {
    return invoke('get_image_perceptual_hash', { imageId, algorithm: algorithm ?? null });
}

export async function getPerceptualHashCount(algorithm?: string): Promise<number> {
    return invoke('get_perceptual_hash_count', { algorithm: algorithm ?? null });
}

export async function findNearDuplicatesByPhash(
    imageId: string,
    maxDistance = 8,
    limit = 50,
    algorithm?: string,
): Promise<NearDuplicateImage[]> {
    return invoke('find_near_duplicates_by_phash', {
        imageId,
        maxDistance,
        limit,
        algorithm: algorithm ?? null,
    });
}

// Delete commands
export interface TrashImageResult {
    image_id: string;
    path: string | null;
    status: 'trashed' | 'missing' | 'not_found' | 'failed';
    error: string | null;
}

export interface TrashImagesDetailedResult {
    requested: number;
    succeeded: number;
    failed: number;
    results: TrashImageResult[];
}

export async function trashImages(imageIds: string[]): Promise<number> {
    const result = await invoke<number>('trash_images', { imageIds });
    emitSessionEventsRefresh();
    return result;
}

export async function trashImagesDetailed(imageIds: string[]): Promise<TrashImagesDetailedResult> {
    const result = await invoke<TrashImagesDetailedResult>('trash_images_detailed', { imageIds });
    emitSessionEventsRefresh();
    return result;
}

export async function deleteImagesPermanently(imageIds: string[]): Promise<number> {
    const result = await invoke<number>('delete_images_permanently', { imageIds });
    emitSessionEventsRefresh();
    return result;
}

export async function createActionProposal(request: CreateActionProposalRequest): Promise<AgentActionProposal> {
    return invoke<AgentActionProposal>('create_action_proposal', { request });
}

export async function listActionProposals(status: AgentProposalStatus | null = 'pending', limit = 20): Promise<AgentActionProposal[]> {
    return invoke<AgentActionProposal[]>('list_action_proposals', { status, limit });
}

export async function dismissActionProposal(proposalId: string): Promise<void> {
    return invoke<void>('dismiss_action_proposal', { proposalId });
}

export async function applyActionProposal(proposalId: string, approvedImageIds: string[], resultJson: string): Promise<ApplyActionProposalResult> {
    return invoke<ApplyActionProposalResult>('apply_action_proposal', { proposalId, approvedImageIds, resultJson });
}

export async function listAgentSelectionPresets(): Promise<AgentSelectionPreset[]> {
    return invoke<AgentSelectionPreset[]>('list_agent_selection_presets');
}

export async function upsertAgentSelectionPreset(request: UpsertAgentSelectionPresetRequest): Promise<AgentSelectionPreset> {
    return invoke<AgentSelectionPreset>('upsert_agent_selection_preset', { request });
}

export async function runClaudeAgentChatTurn(request: ClaudeAgentChatTurnRequest): Promise<ClaudeAgentChatTurnResult> {
    return invoke<ClaudeAgentChatTurnResult>('run_claude_agent_chat_turn', { request });
}

export async function cancelClaudeAgentChatTurn(requestId: string): Promise<boolean> {
    return invoke<boolean>('cancel_claude_agent_chat_turn', { requestId });
}

// Undo/Redo
export interface UndoStatus {
    can_undo: boolean;
    can_redo: boolean;
    undo_label: string | null;
    redo_label: string | null;
    stack_depth: number;
}

export interface UndoRecord {
    seq: number;
    id: string;
    action_type: string;
    label: string;
    before_json: string;
    after_json: string;
    affected_image_ids: string | null;
    group_id: string | null;
    has_file_backup: boolean;
    created_at: string;
}

export async function undo(): Promise<string | null> {
    return invoke<string | null>('undo');
}

export async function redo(): Promise<string | null> {
    return invoke<string | null>('redo');
}

export async function getUndoStatus(): Promise<UndoStatus> {
    return invoke<UndoStatus>('get_undo_status');
}

export async function listUndoHistory(limit?: number | null): Promise<UndoRecord[]> {
    return invoke<UndoRecord[]>('list_undo_history', { limit: limit ?? null });
}

// Settings
export async function getAppSetting(key: string): Promise<string | null> {
    return invoke('get_app_setting', { key });
}

export async function setAppSetting(key: string, value: string): Promise<void> {
    return invoke('set_app_setting', { key, value });
}

export async function applyAppIconVariant(variant: string): Promise<void> {
    return invoke('apply_app_icon_variant', { variant });
}

export interface StaticPublishCanvasItem {
    image_id: string;
    x?: number | null;
    y?: number | null;
    width?: number | null;
    height?: number | null;
    hidden?: boolean | null;
}

export interface StaticPublishLink {
    label: string;
    url: string;
}

export interface StaticPublishRequest {
    canvas_name: string;
    items: StaticPublishCanvasItem[];
    layout_json?: string | null;
    output_dir?: string | null;
    share_url?: string | null;
    site_title?: string | null;
    site_description?: string | null;
    indexable?: boolean;
    links?: StaticPublishLink[];
    include_thumbnails: boolean;
    include_web: boolean;
    include_full: boolean;
}

export interface StaticPublishResult {
    export_dir: string;
    site_dir: string;
    manifest_path: string;
    instructions_path: string;
    qr_svg_path: string;
    qr_svg_data_url: string;
    qr_target_url: string;
    access_phrase: string;
    image_count: number;
    skipped_count: number;
    warnings: string[];
}

export interface StaticPublishServerResult {
    url: string;
    host: string;
    port: number;
    site_dir: string;
}

export interface StaticPublishServerStopResult {
    stopped: boolean;
    url: string | null;
}

export async function exportStaticPublishPackage(request: StaticPublishRequest): Promise<StaticPublishResult> {
    return invoke('export_static_publish_package', { request });
}

export async function serveStaticPublishPackage(siteDir: string, host?: string | null, port?: number | null): Promise<StaticPublishServerResult> {
    return invoke('serve_static_publish_package', { siteDir, host: host ?? null, port: port ?? null });
}

export async function stopStaticPublishServer(): Promise<StaticPublishServerStopResult> {
    return invoke('stop_static_publish_server');
}

// Lineage commands
export interface LineageGroup {
    id: string;
    name: string;
    created_at: string;
    detection_method: string | null;
    detection_score: number | null;
    image_count: number;
}

export async function listLineageGroups(): Promise<LineageGroup[]> {
    return invoke('list_lineage_groups');
}

export async function getLineageGroupImages(groupId: string): Promise<ImageWithFile[]> {
    return invoke('get_lineage_group_images', { groupId });
}

export async function createLineageGroupManual(name: string, imageIds: string[]): Promise<string> {
    return invoke('create_lineage_group_manual', { name, imageIds });
}

export async function renameLineageGroup(groupId: string, name: string): Promise<void> {
    return invoke('rename_lineage_group', { groupId, name });
}

export async function mergeLineageGroups(keepId: string, mergeId: string): Promise<void> {
    return invoke('merge_lineage_groups', { keepId, mergeId });
}

export async function dissolveLineageGroup(groupId: string): Promise<void> {
    return invoke('dissolve_lineage_group', { groupId });
}

export async function addToLineageGroup(groupId: string, imageId: string): Promise<void> {
    return invoke('add_to_lineage_group', { groupId, imageId });
}

export async function removeFromLineageGroup(imageId: string): Promise<void> {
    return invoke('remove_from_lineage_group', { imageId });
}

export async function getBatchImages(batchId: string): Promise<ImageWithFile[]> {
    return invoke('get_batch_images', { batchId });
}

export async function scanLineage(): Promise<number> {
    return invoke('scan_lineage');
}

// MCP Token Management

export interface McpToken {
    id: string;
    name: string;
    role: string;
    scope_json: string | null;
    created_at: string;
    expires_at: string | null;
    last_used_at: string | null;
    revoked: boolean;
}

export interface TokenScope {
    collections?: string[];
    folders?: string[];
    tags?: string[];
}

export interface McpStatus {
    active_connections: number;
}

export async function getMcpStatus(): Promise<McpStatus> {
    return invoke('get_mcp_status');
}

export async function createMcpToken(name: string, role: string, scope?: TokenScope, expiresAt?: string): Promise<[McpToken, string]> {
    return invoke('create_mcp_token', { name, role, scope: scope || null, expiresAt: expiresAt || null });
}

export async function listMcpTokens(): Promise<McpToken[]> {
    return invoke('list_mcp_tokens');
}

export async function revokeMcpToken(tokenId: string): Promise<void> {
    return invoke('revoke_mcp_token', { tokenId });
}

export async function rotateMcpToken(tokenId: string): Promise<string> {
    return invoke('rotate_mcp_token', { tokenId });
}

export interface McpAuditEntry {
    id: number;
    token_id: string | null;
    tool_name: string;
    params_json: string | null;
    result_status: string;
    timestamp: string;
}

export async function getMcpAuditLog(limit: number): Promise<McpAuditEntry[]> {
    return invoke('get_mcp_audit_log', { limit });
}

export async function cropImage(imageId: string, x: number, y: number, width: number, height: number): Promise<string> {
    const result = await invoke<string>('crop_image', { imageId, x, y, width, height });
    emitSessionEventsRefresh();
    return result;
}

export async function rotateImage(imageId: string, degrees: number): Promise<string> {
    const result = await invoke<string>('rotate_image', { imageId, degrees });
    emitSessionEventsRefresh();
    return result;
}

export async function getGenerationRun(imageId: string): Promise<GenerationRun | null> {
    return invoke<GenerationRun | null>('get_generation_run', { imageId });
}

export async function rescanSidecars(): Promise<number> {
    return invoke<number>('rescan_sidecars');
}

export interface ResubmitPromptRequest {
    provider: string;
    source_image_id: string | null;
    prompt: string;
    n: number;
    model: string;
    size: string;
    quality: string;
}

export interface ResubmitPromptResponse {
    job_id: string;
}

export interface CostEstimate {
    estimated_cost: number;
    provider: string;
    model: string;
    size: string;
    quality: string;
    n: number;
}

export async function resubmitPrompt(request: ResubmitPromptRequest): Promise<ResubmitPromptResponse> {
    return invoke<ResubmitPromptResponse>('resubmit_prompt', { request });
}

export async function estimateGenerationCost(provider: string, model: string, size: string, quality: string, n: number): Promise<CostEstimate> {
    return invoke<CostEstimate>('estimate_generation_cost', { provider, model, size, quality, n });
}

// Sessions
export interface Session {
    id: string;
    name: string;
    description: string | null;
    folder_path: string;
    settings_json: string | null;
    created_at: string;
    image_count: number;
}

export interface Canvas {
    id: string;
    session_id: string;
    name: string;
    canvas_type: 'manual' | 'query';
    layout_json: string;
    filter_json: string | null;
    grid_config_json: string | null;
    sort_order: number;
    created_at: string;
    updated_at: string;
}

export interface SessionEvent {
    id: string;
    session_id: string | null;
    event_type: string;
    actor_type: 'user' | 'agent' | 'system';
    actor_id: string | null;
    subject_type: string | null;
    subject_id: string | null;
    payload_json: string;
    created_at: string;
}

export interface ActivityLibrarySummary {
    total_images: number;
    scoped_images: number;
    rated_images: number;
    accepted_images: number;
    rejected_images: number;
    import_batches: number;
}

export interface ActivityContext {
    session: Session | null;
    library: ActivityLibrarySummary;
    recent_events: SessionEvent[];
}

export async function createSession(name: string): Promise<Session> {
    return invoke<Session>('create_session', { name });
}
export async function listSessions(): Promise<Session[]> {
    return invoke<Session[]>('list_sessions');
}
export async function getActivityContext(sessionId?: string | null, limit?: number | null): Promise<ActivityContext> {
    return invoke<ActivityContext>('get_activity_context', { sessionId: sessionId ?? null, limit: limit ?? null });
}
export async function getSession(sessionId: string): Promise<Session> {
    return invoke<Session>('get_session', { sessionId });
}
export async function deleteSession(sessionId: string, deleteFiles: boolean): Promise<void> {
    return invoke<void>('delete_session', { sessionId, deleteFiles });
}
export async function convertSessionToCollection(sessionId: string): Promise<void> {
    return invoke<void>('convert_session_to_collection', { sessionId });
}
export async function validateSessionFolder(sessionId: string): Promise<boolean> {
    return invoke<boolean>('validate_session_folder', { sessionId });
}
export async function createCanvas(sessionId: string, name: string, canvasType: string): Promise<Canvas> {
    return invoke<Canvas>('create_canvas', { sessionId, name, canvasType });
}
export async function listCanvases(sessionId: string): Promise<Canvas[]> {
    return invoke<Canvas[]>('list_canvases', { sessionId });
}
export async function updateCanvasLayout(canvasId: string, layoutJson: string): Promise<void> {
    return invoke<void>('update_canvas_layout', { canvasId, layoutJson });
}
export async function deleteCanvas(canvasId: string): Promise<void> {
    return invoke<void>('delete_canvas', { canvasId });
}

// File management
export async function copyImageToClipboard(imageId: string): Promise<void> {
    return invoke<void>('copy_image_to_clipboard', { imageId });
}

export async function pasteImageFromClipboard(
    destinationFolder: string,
    sessionId: string | null = null,
): Promise<PastedImageResult> {
    const result = await invoke<PastedImageResult>('paste_image_from_clipboard', { destinationFolder, sessionId });
    emitSessionEventsRefresh();
    return result;
}

export async function moveImage(imageId: string, destinationFolder: string): Promise<string> {
    const result = await invoke<string>('move_image', { imageId, destinationFolder });
    emitSessionEventsRefresh();
    return result;
}

export async function renameImage(imageId: string, newName: string): Promise<string> {
    const result = await invoke<string>('rename_image', { imageId, newName });
    emitSessionEventsRefresh();
    return result;
}

export async function createSubfolder(parentPath: string, name: string): Promise<string> {
    const result = await invoke<string>('create_subfolder', { parentPath, name });
    emitSessionEventsRefresh();
    return result;
}

export async function shareImages(imageIds: string[]): Promise<void> {
    return invoke<void>('share_images', { imageIds });
}

export async function openImagesWithApplication(imageIds: string[], appPath: string): Promise<void> {
    return invoke<void>('open_images_with_application', { imageIds, appPath });
}

export async function listOpenWithApplications(imageId: string): Promise<OpenWithApplication[]> {
    return invoke<OpenWithApplication[]>('list_open_with_applications', { imageId });
}

export async function captureAgentWindowSnapshot(): Promise<string> {
    return invoke<string>('capture_agent_window_snapshot');
}

export async function completeAgentViewSnapshot(
    request: CompleteAgentViewSnapshotRequest,
): Promise<AgentSnapshotPackage> {
    return invoke<AgentSnapshotPackage>('complete_agent_view_snapshot', { request });
}

export async function getLastAgentViewSnapshot(snapshotId: string | null = null): Promise<AgentSnapshotPackage | null> {
    return invoke<AgentSnapshotPackage | null>('get_last_agent_view_snapshot', { snapshotId });
}

export async function requestAgentViewSnapshot(clipboard = false): Promise<AgentSnapshotPackage> {
    return invoke<AgentSnapshotPackage>('request_agent_view_snapshot', { clipboard });
}

export async function updateMenuState(state: MenuStatePayload): Promise<void> {
    return invoke<void>('update_menu_state', { state });
}

export async function backfillRawPreviews(): Promise<number> {
    return invoke<number>('backfill_raw_previews');
}

// Privacy & audit log
export interface AuditLogEntry {
    id: string;
    timestamp: string;
    provider: string;
    endpoint: string;
    data_type: string;
    data_size_bytes: number | null;
    prompt_preview: string | null;
    image_dimensions: string | null;
    model: string | null;
    response_status: number | null;
    jurisdiction: string;
}

export interface DataFlowEntry {
    feature: string;
    status: string;
    server: string;
    data_sent: string;
}

export async function getDataFlowStatus(): Promise<DataFlowEntry[]> {
    return invoke('get_data_flow_status');
}

export async function getApiAuditLog(limit: number): Promise<AuditLogEntry[]> {
    return invoke('get_api_audit_log', { limit });
}

export async function exportAuditLog(): Promise<string> {
    return invoke('export_audit_log');
}

// Local diagnostics
export interface AssetLoadEventRequest {
    view: string;
    imageId: string | null;
    assetKind: string;
    imageFormat: string | null;
    fallbackUsed: boolean;
    fallbackSucceeded: boolean | null;
    pathBasename: string | null;
    pathHash: string | null;
    errorKind: string;
    detailsJson: string | null;
}

export interface AssetLoadEvent {
    id: string;
    created_at: string;
    view: string;
    image_id: string | null;
    asset_kind: string;
    image_format: string | null;
    fallback_used: boolean;
    fallback_succeeded: boolean | null;
    path_basename: string | null;
    path_hash: string | null;
    error_kind: string;
    details_json: string | null;
}

export async function recordAssetLoadEvent(event: AssetLoadEventRequest): Promise<AssetLoadEvent> {
    return invoke<AssetLoadEvent>('record_asset_load_event', { event });
}

export async function getAssetLoadEvents(limit: number): Promise<AssetLoadEvent[]> {
    return invoke<AssetLoadEvent[]>('get_asset_load_events', { limit });
}

// Plugins (Track C2): registry fetch + checksum-verified install. All
// commands are module_plugins-gated in Rust; the consent dialog runs in the
// UI BEFORE installPlugin is invoked.
export interface PluginManifestInfo {
    id: string;
    name: string;
    version: string;
    description: string;
    entry: string;
    permissions: string[];
    minAppVersion: string;
    checksum: string;
    repo: string;
}

export interface RegistryPluginInfo {
    manifest: PluginManifestInfo;
    download: string;
}

export interface InstalledPluginInfo {
    manifest: PluginManifestInfo;
    granted: string[];
}

export async function fetchPluginRegistry(): Promise<RegistryPluginInfo[]> {
    return invoke('fetch_plugin_registry');
}

export async function installPlugin(pluginId: string): Promise<void> {
    return invoke('install_plugin', { pluginId });
}

export async function uninstallPlugin(pluginId: string): Promise<void> {
    return invoke('uninstall_plugin', { pluginId });
}

export async function listInstalledPluginInfo(): Promise<InstalledPluginInfo[]> {
    return invoke('list_installed_plugin_info');
}
