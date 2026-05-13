# iCloud Photos Import Research

Date: 2026-05-13

## Summary

Cull can support iCloud Photos import, but the safe path is through Apple Photos on macOS and PhotoKit. We should treat iCloud Photos as part of the user's System Photo Library, not as a standalone cloud service that Cull logs into directly.

The recommended product is a read-only Apple Photos catalog source:

- Request Photos permission from macOS.
- Enumerate PhotoKit assets and albums.
- Let users browse/import albums or selected assets.
- Materialize originals or current rendered versions into Cull-controlled storage before running the existing import pipeline.
- Preserve Apple asset IDs and album IDs so we can refresh without losing Cull ratings, selections, or collections.

## Current Cull Fit

Cull's current import path is file-system based:

- `src-tauri/src/db_core/import.rs` reads a local path, hashes bytes, creates an `images` record, creates an `image_files` path record, generates thumbnails, runs source detection, and parses adjacent sidecars.
- `src-tauri/src/commands/import.rs` exposes folder and file import commands.
- `src-tauri/src/db_core/db.rs` already has manual collections via `projects` and `collection_items`.

That means a PhotoKit importer should export or copy each selected asset to a temporary/app-data location, then call the existing import code rather than bypassing it.

## Important Findings

PhotoKit is the supported API for Photos access. `PHPhotoLibrary` covers assets and collections managed by Photos, including assets backed by iCloud Photos. It also provides authorization, change observation, and persistent change tokens.

iCloud-only originals are not always local. PhotoKit request options include a network access flag so Photos can download an asset from iCloud when needed. The importer needs progress, cancellation, and clear error states for offline, metered, or stalled downloads.

The practical scope is the user's System Photo Library. Apple supports multiple Photos libraries, but the System Photo Library is the one routinely exposed to the system and iCloud Photos. Users must switch libraries in Photos to work with another library.

There is no product-safe reason to scrape modern `.photoslibrary` internals for the main flow. The package database and file layout are private implementation details. Scraping risks stale previews, missing cloud originals, broken albums, and data corruption assumptions.

Legacy iPhoto libraries are different. A one-time recovery importer can inspect package folders such as `Originals`, `Modified`, `Masters`, and `Previews`, and may read `AlbumData.xml` or `AlbumData2.xml` when present. This should be labelled legacy recovery, not the normal iCloud Photos path.

## Proposed MVP

1. Add macOS PhotoKit authorization commands:
   - `photos_authorization_status`
   - `photos_request_authorization`

2. Add catalog listing commands:
   - `photos_list_albums`
   - `photos_list_assets(album_id?: string)`

3. Add import command:
   - `photos_import_assets(asset_ids: string[], version: "original" | "current", album_id?: string)`

4. Persist provenance:
   - Add a small `external_assets` or `photo_assets` table keyed by provider plus asset ID.
   - Suggested fields: `provider`, `provider_asset_id`, `image_id`, `provider_collection_id`, `filename`, `creation_date`, `modification_date`, `favorite`, `media_subtypes`, `imported_version`, `last_seen_at`.

5. Mirror albums:
   - On import, create or update Cull manual collections matching selected Photos albums.
   - Preserve Cull-only ratings and selections in `selections`.

6. Handle iCloud download states:
   - Show per-asset progress.
   - Allow cancellation.
   - Surface assets that require network download.
   - Keep imports resumable and idempotent.

## Native Integration Notes

The repo already depends on `objc2`, `objc2-foundation`, and `block2` on macOS for dictation. The `objc2-photos` crate provides PhotoKit bindings for `PHPhotoLibrary`, `PHAsset`, `PHAssetCollection`, `PHAssetResourceManager`, and related request options, so a Rust-native bridge is plausible without adding a Swift build step.

The app currently has microphone and speech usage strings in `src-tauri/Info.plist`; a PhotoKit implementation must add `NSPhotoLibraryUsageDescription`.

## User-Facing Flow

Add a File menu item or Import view action:

- Import from Apple Photos...
- Prompt for Photos permission if needed.
- Show albums, smart albums where exposed by PhotoKit, and recent assets.
- Let users choose original or current edited version.
- Import selected assets into Cull, then open the resulting import batch.

For users with old `.iphoto` or `.photoslibrary` files:

- Prefer: open or migrate the library in Photos, make it the active/System Photo Library if they want iCloud-backed access, then import through Cull.
- Fallback: legacy package recovery that imports originals from package folders, with a warning that album/edit metadata may be incomplete.

## Sources

- Apple PhotoKit `PHPhotoLibrary`: https://developer.apple.com/documentation/photos/phphotolibrary
- Apple PhotoKit iCloud download option: https://developer.apple.com/documentation/photokit/phcontenteditinginputrequestoptions/1624049-networkaccessallowed
- Apple Photos privacy access on Mac: https://support.apple.com/guide/mac-help/allow-apps-to-use-your-photos-mchl325bd573/mac
- Apple Photo Library overview: https://support.apple.com/en-au/guide/photos/pht211de786/mac
- Apple import from another photo library: https://support.apple.com/en-euro/guide/photos/phtcca765b2b/mac
- Apple export originals/current versions: https://support.apple.com/en-euro/guide/photos/pht6e157c5f/mac
- objc2 PhotoKit bindings: https://docs.rs/objc2-photos/latest/objc2_photos/
- iPhoto package internals reference: https://www.fatcatsoftware.com/iplm/Help/iphoto%20library%20internals.html
