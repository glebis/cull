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

export async function getAllEmbeddings(): Promise<[string, number[]][]> {
    return invoke('get_all_embeddings');
}

export async function findSimilarImages(imageId: string, topK: number): Promise<[string, number][]> {
    return invoke('find_similar_images', { imageId, topK });
}

export async function getEmbeddingCount(): Promise<number> {
    return invoke('get_embedding_count');
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
