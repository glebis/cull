import { invoke } from '@tauri-apps/api/core';
import type {
    ExportManifest,
    JsonPatch,
    PatchResult,
    ValidationResult,
    PresetInfo,
    AssetResponse,
} from './export-types';

export async function createExportManifest(
    imageIds: string[],
    targetPresets: string[],
    collectionId?: string,
    template?: string,
): Promise<ExportManifest> {
    return invoke<ExportManifest>('create_export_manifest', {
        imageIds,
        targetPresets,
        collectionId: collectionId ?? null,
        template: template ?? null,
    });
}

export async function validateExportManifest(
    manifest: ExportManifest,
): Promise<ValidationResult> {
    return invoke<ValidationResult>('validate_export_manifest', { manifest });
}

export async function applyExportPatches(
    manifest: ExportManifest,
    patches: JsonPatch[],
): Promise<PatchResult> {
    return invoke<PatchResult>('apply_export_patches', { manifest, patches });
}

export async function listExportPresets(): Promise<PresetInfo[]> {
    return invoke<PresetInfo[]>('list_export_presets');
}

export async function getExportAsset(
    uri: string,
    variant?: 'original' | 'preview' | 'thumbnail',
    maxWidth?: number,
    maxHeight?: number,
): Promise<AssetResponse> {
    return invoke<AssetResponse>('get_export_asset', {
        uri,
        variant: variant ?? null,
        maxWidth: maxWidth ?? null,
        maxHeight: maxHeight ?? null,
    });
}
