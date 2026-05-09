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
}

export interface ImportResponse {
    imported: number;
    skipped: number;
    errors: string[];
    batch_id: string | null;
    image_ids: string[];
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
