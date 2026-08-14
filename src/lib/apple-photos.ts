import {
    photosAuthorizationStatus,
    photosLoadLocalPreview,
    photosListAlbums,
    photosListAssets,
    photosRequestAuthorization,
    photosStartImportAssets,
    type ApplePhotosAlbum,
    type ApplePhotosAsset,
    type ApplePhotosAssetFilter,
    type ApplePhotosAssetSort,
    type ApplePhotosAuthorization,
    type ApplePhotosImportStarted,
    type ApplePhotosPage,
} from './api';

export type {
    ApplePhotosAlbum,
    ApplePhotosAsset,
    ApplePhotosAssetFilter,
    ApplePhotosAssetSort,
    ApplePhotosAuthorization,
    ApplePhotosImportStarted,
    ApplePhotosPage,
};

export interface ApplePhotosCatalogClient {
    authorizationStatus(): Promise<ApplePhotosAuthorization>;
    requestAuthorization(): Promise<ApplePhotosAuthorization>;
    listAlbums(offset: number, limit: number): Promise<ApplePhotosPage<ApplePhotosAlbum>>;
    listAssets(
        albumId: string | null,
        offset: number,
        limit: number,
        filter: ApplePhotosAssetFilter,
        sort: ApplePhotosAssetSort,
    ): Promise<ApplePhotosPage<ApplePhotosAsset>>;
    loadPreview(assetId: string, size: number): Promise<string | null>;
    startImport(assetIds: string[], sourceAlbumId: string | null): Promise<ApplePhotosImportStarted>;
}

export const tauriApplePhotosCatalogClient: ApplePhotosCatalogClient = {
    authorizationStatus: photosAuthorizationStatus,
    requestAuthorization: photosRequestAuthorization,
    listAlbums: photosListAlbums,
    listAssets: photosListAssets,
    loadPreview: photosLoadLocalPreview,
    startImport: (assetIds, sourceAlbumId) => photosStartImportAssets(assetIds, sourceAlbumId),
};
