// Copyright (c) 2025-present Gleb Kalinin. Architecture and design by author.
// Implementation assisted by Claude (Anthropic). See AUTHORSHIP.md.

import { invoke } from '@tauri-apps/api/core';

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

const RAW_EXTENSIONS = ['cr2', 'cr3', 'nef', 'arw', 'dng', 'orf', 'raf', 'rw2'];

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

export async function listImages(limit: number, offset: number): Promise<ImageWithFile[]> {
    return invoke<ImageWithFile[]>('list_images', { limit, offset });
}

export async function getImageCount(): Promise<number> {
    return invoke<number>('get_image_count');
}

export async function importFolder(folderPath: string): Promise<ImportResponse> {
    return invoke<ImportResponse>('import_folder', { folderPath });
}

export async function importFiles(filePaths: string[]): Promise<ImportResponse> {
    return invoke<ImportResponse>('import_files', { filePaths });
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

export async function rescanSources(): Promise<number> {
    return invoke<number>('rescan_sources');
}

export async function setRating(imageId: string, rating: number): Promise<void> {
    return invoke<void>('set_rating', { imageId, rating });
}

export async function setDecision(imageId: string, decision: string): Promise<void> {
    return invoke<void>('set_decision', { imageId, decision });
}

export async function getImagesByIds(imageIds: string[]): Promise<ImageWithFile[]> {
    return invoke<ImageWithFile[]>('get_images_by_ids', { imageIds });
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
    return invoke('delete_folder', { folder });
}

export async function listImagesFiltered(minWidth: number | null, minHeight: number | null, limit: number, offset: number): Promise<ImageWithFile[]> {
    return invoke('list_images_filtered', { minWidth, minHeight, limit, offset });
}

export async function createCollection(name: string): Promise<string> {
    return invoke('create_collection', { name });
}

export async function listCollections(): Promise<[string, string, number][]> {
    return invoke('list_collections');
}

export async function addToCollection(collectionId: string, imageIds: string[]): Promise<void> {
    return invoke('add_to_collection', { collectionId, imageIds });
}

export async function listCollectionImages(collectionId: string): Promise<ImageWithFile[]> {
    return invoke('list_collection_images', { collectionId });
}

export async function removeFromCollection(collectionId: string, imageIds: string[]): Promise<void> {
    return invoke('remove_from_collection', { collectionId, imageIds });
}

export async function deleteCollectionApi(collectionId: string): Promise<void> {
    return invoke('delete_collection', { collectionId });
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
    return invoke('create_smart_collection', { name, filterJson, nlQuery });
}

export async function evaluateSmartCollection(filterJson: string): Promise<ImageWithFile[]> {
    return invoke('evaluate_smart_collection', { filterJson });
}

export async function deleteSmartCollectionApi(id: string): Promise<void> {
    return invoke('delete_smart_collection', { id });
}

export async function updateSmartCollectionApi(
    id: string,
    name: string,
    filterJson: string,
    nlQuery?: string,
): Promise<void> {
    return invoke('update_smart_collection', { id, name, filterJson, nlQuery });
}

export async function parseNlQuery(query: string): Promise<string> {
    return invoke('parse_nl_query', { query });
}

export async function backfillImageMetadata(): Promise<number> {
    return invoke('backfill_image_metadata');
}

// Embedding commands
export async function downloadClipModel(): Promise<string> {
    return invoke('download_clip_model');
}

export async function isModelAvailable(): Promise<boolean> {
    return invoke('is_model_available');
}

export async function generateEmbeddings(imageIds: string[]): Promise<number> {
    return invoke('generate_embeddings', { imageIds });
}

export async function getAllEmbeddings(model?: string): Promise<[string, number[]][]> {
    return invoke('get_all_embeddings', { model: model ?? null });
}

export async function findSimilarImages(imageId: string, topK: number, model?: string): Promise<[string, number][]> {
    return invoke('find_similar_images', { imageId, topK, model: model ?? null });
}

export async function getEmbeddingCount(model?: string): Promise<number> {
    return invoke('get_embedding_count', { model: model ?? null });
}

export async function setApiKey(provider: string, key: string): Promise<void> {
    return invoke('set_api_key', { provider, key });
}

export async function getApiKey(provider: string): Promise<string | null> {
    return invoke('get_api_key', { provider });
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
    gap?: number;
}): Promise<void> {
    return invoke('open_with_params', params);
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

// Delete commands
export async function trashImages(imageIds: string[]): Promise<number> {
    return invoke('trash_images', { imageIds });
}

export async function deleteImagesPermanently(imageIds: string[]): Promise<number> {
    return invoke('delete_images_permanently', { imageIds });
}

// Undo/Redo
export async function undo(): Promise<string | null> {
    return invoke('undo');
}

export async function redo(): Promise<string | null> {
    return invoke('redo');
}

// Settings
export async function getAppSetting(key: string): Promise<string | null> {
    return invoke('get_app_setting', { key });
}

export async function setAppSetting(key: string, value: string): Promise<void> {
    return invoke('set_app_setting', { key, value });
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

export async function createMcpToken(name: string, role: string, scope?: TokenScope): Promise<[McpToken, string]> {
    return invoke('create_mcp_token', { name, role, scope: scope || null });
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

export async function cropImage(imageId: string, x: number, y: number, width: number, height: number, saveAsCopy: boolean): Promise<string> {
    return invoke<string>('crop_image', { imageId, x, y, width, height, saveAsCopy });
}

export async function rotateImage(imageId: string, degrees: number): Promise<void> {
    return invoke<void>('rotate_image', { imageId, degrees });
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

export async function createSession(name: string): Promise<Session> {
    return invoke<Session>('create_session', { name });
}
export async function listSessions(): Promise<Session[]> {
    return invoke<Session[]>('list_sessions');
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
export async function moveImage(imageId: string, destinationFolder: string): Promise<string> {
    return invoke<string>('move_image', { imageId, destinationFolder });
}

export async function renameImage(imageId: string, newName: string): Promise<string> {
    return invoke<string>('rename_image', { imageId, newName });
}

export async function createSubfolder(parentPath: string, name: string): Promise<string> {
    return invoke<string>('create_subfolder', { parentPath, name });
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
