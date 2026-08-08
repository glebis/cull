import {
    photosAuthorizationStatus,
    photosListAlbums,
    photosListAssets,
    photosRequestAuthorization,
    type ApplePhotosAlbum,
    type ApplePhotosAsset,
    type ApplePhotosAuthorization,
    type ApplePhotosPage,
} from './api';

export type {
    ApplePhotosAlbum,
    ApplePhotosAsset,
    ApplePhotosAuthorization,
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
    ): Promise<ApplePhotosPage<ApplePhotosAsset>>;
}

export const tauriApplePhotosCatalogClient: ApplePhotosCatalogClient = {
    authorizationStatus: photosAuthorizationStatus,
    requestAuthorization: photosRequestAuthorization,
    listAlbums: photosListAlbums,
    listAssets: photosListAssets,
};
