# Apple Photos Import and Reimport Policy

Status: Accepted implementation policy for `imageview-uz3` follow-on slices  
Date: 2026-08-08

## Problem Statement

Cull can now browse the macOS System Photo Library without downloading assets, but importing from Apple Photos crosses several user-data boundaries that the catalog slice intentionally avoided. A Photos asset may be edited, iCloud-only, compound (RAW+JPEG or Live Photo), or no longer visible under Limited Photos access. Cull's existing importer treats a durable filesystem path as canonical, so a temporary PhotoKit export cannot safely be handed to it and then deleted.

Users need an import that preserves the version they chose, survives restarts, does not overwrite earlier Cull work when the Photos asset changes, and reports partial network failures truthfully. The implementation also needs a stable definition of album mirroring, cancellation, reimport identity, and automation boundaries before it can write files or database records.

## Solution

Cull will provide a macOS-only, user-initiated Apple Photos import job. The job freezes the selected asset IDs, representation mode, and optional source album at start. PhotoKit materializes each selected still-image resource into Cull-owned durable storage, and Cull then routes that path through the existing import pipeline.

The unit of atomicity is one materialized still-image resource, not the whole batch. Successfully imported resources remain available if a later iCloud download fails or the user cancels. Retrying the same PhotoKit asset/version is idempotent. A changed Photos asset creates a new preserved version instead of overwriting an existing Cull image or its ratings, selections, metadata, or collection membership.

Album mirroring is an explicit snapshot/add operation. It never performs automatic deletion or continuous synchronization. Photos authorization, catalog access, and import remain native UI operations for the MVP; they are not exposed to MCP, CLI, plugins, or background startup code.

## User Stories

1. As a macOS user, I want to import selected Apple Photos assets, so that I can cull them alongside filesystem images.
2. As an iCloud Photos user, I want Cull to download an iCloud-only asset with visible progress, so that I know the app is still working.
3. As a user, I want to cancel an Apple Photos import, so that I can stop network and disk work without losing items already completed.
4. As a user, I want to retry a cancelled or failed import, so that Cull skips resources it already imported successfully.
5. As a user, I want to choose the current edited representation, so that my Cull image matches what Photos displays.
6. As a user, I want to choose original resources, so that Cull can preserve source files without silently baking Photos edits.
7. As a RAW+JPEG user, I want original mode to preserve both supported still resources, so that Cull does not silently discard part of the compound asset.
8. As a Live Photo user, I want the still image imported and the paired video reported as skipped, so that the still-image MVP is explicit rather than lossy or surprising.
9. As a user with HEIC or supported RAW originals, I want Cull to use its existing decoders, so that PhotoKit import behaves like filesystem import.
10. As a user with an unsupported still resource, I want a per-item error, so that Cull does not transcode or mislabel it silently.
11. As a user, I want imported bytes stored in Cull-controlled durable storage, so that removing a temporary export cannot break my library.
12. As a user, I want an imported copy to remain when the source asset is later removed from Photos, so that Photos is not an implicit deletion authority for Cull.
13. As a user, I want reimporting an unchanged representation to return the existing Cull image, so that duplicates are not created.
14. As a user, I want a changed Photos edit to create a new version, so that earlier ratings, selections, and file bytes are preserved.
15. As a user, I want provenance to retain the opaque Photos asset ID and imported representation, so that Cull can explain where each image came from.
16. As a user, I want provenance IDs kept out of filenames and ordinary logs, so that opaque library identifiers are not unnecessarily exposed.
17. As a user importing an album, I want Cull to create or reuse the mapped collection, so that the album becomes a useful culling scope.
18. As a user, I want a Photos album title collision handled without adopting an unrelated Cull collection, so that name equality is never treated as identity.
19. As a user who renamed the mapped Cull collection, I want my name preserved on later imports, so that source refreshes do not overwrite my edits.
20. As a user, I want later album imports to add newly imported members without removing Cull-only members, so that mirroring cannot destroy curation work.
21. As a user with Limited Photos access, I want Cull to import only currently granted assets and report withdrawn assets as inaccessible, so that the system permission boundary is respected.
22. As a user who revokes Photos access mid-job, I want the active request to fail clearly and completed imports to remain consistent.
23. As a user who runs out of disk space, I want the affected item to remain retryable and no partial file to appear in the library.
24. As a user who restarts Cull during an import, I want abandoned partials reconciled inside Cull's managed Photos directory, so that no incomplete image is presented as imported.
25. As a user, I want one import batch and one job identity for the selection, so that progress, cancellation, history, and the resulting grid agree.
26. As a user, I want completed, failed, skipped, inaccessible, and cancelled counts reported separately, so that a partial batch is never called fully successful.
27. As a privacy-conscious user, I want Photos permission requested only from the visible dialog action, so that automation cannot trigger a TCC prompt.
28. As an agent user, I want Photos operations unavailable to MCP and CLI by default, so that an automation token cannot browse or import my personal Photos library.
29. As a non-macOS user, I want the Photos UI hidden and commands to retain their typed unsupported behavior, so that the feature fails predictably.
30. As a developer, I want PhotoKit objects and callbacks isolated behind the existing platform-neutral adapter, so that native lifetime and threading rules do not leak through the app.

## Implementation Decisions

- **System Photo Library only.** Cull uses PhotoKit and never scrapes a modern `.photoslibrary` package. Legacy-library recovery is a separate product.
- **Still-image MVP.** Video assets are excluded. A Live Photo contributes its still resource; its paired video is counted as skipped with an explicit reason. Burst expansion and adjustment-data sidecars are out of scope.
- **Two representation modes.** `current` materializes one full-resolution current edited still representation. `original` materializes every supported original still-photo resource exposed for the selected asset. For RAW+JPEG this can create two Cull images. The job result maps each provider asset to one or more image IDs.
- **No implicit transcoding.** HEIC, JPEG, PNG, and RAW resources enter the existing importer in their supplied format. Unsupported formats fail per resource. A future explicit export/transcode option is separate.
- **Frozen request.** Asset IDs, representation mode, source album ID, and permission state are captured when the job starts. UI selection changes do not retarget an in-flight job.
- **Durable managed storage.** Canonical files live below Cull's application-data `imports/apple-photos` area. The path uses a hash of provider plus opaque asset ID, a representation/version key, and a sanitized resource filename. Opaque PhotoKit IDs are not embedded directly in paths.
- **Collision-free version paths.** The version key incorporates representation, provider modification metadata, and stable resource identity. Multiple same-named resources receive deterministic resource suffixes. Cull never overwrites a prior canonical file.
- **Same-directory atomic materialization.** PhotoKit writes to a unique `.part` file in the final directory. Cull closes and syncs the file, verifies non-zero/expected bytes where metadata permits, and atomically renames it without replacing an existing path.
- **Recoverable item journal.** Each resource has a durable import-item record before network materialization begins, with states such as `requested`, `materializing`, `materialized`, `imported`, `failed`, `cancelled`, and `skipped`. A final file that exists but is not yet linked remains retryable through this journal; it is never silently deleted or treated as imported.
- **Scoped cleanup.** Cancellation immediately removes incomplete `.part` files when safe. Startup reconciliation may clean stale `.part` files only inside the managed Apple Photos directory and only when no active journal item owns them. It never scans or deletes arbitrary user paths.
- **Per-resource atomicity.** After a final file exists, the existing import pipeline creates the image/file records. Provenance, import-batch linkage, and the resource journal's terminal state commit together. If database finalization fails, the durable file and journal remain visible as retryable; the batch does not claim success.
- **Idempotent identity.** The idempotency key is provider + opaque asset ID + representation + provider/resource version fingerprint. Repeating that key returns the existing image mapping. A changed fingerprint creates a new version and preserves the previous image.
- **No user-state overwrite.** Reimport never overwrites or clears prior ratings, decisions, collections, generation metadata, or image records. Byte-hash deduplication may point a new provenance version to an existing image, but the provenance record remains distinct.
- **One-to-many provenance.** The data model separates external assets, imported asset versions/resources, external collections, and collection membership. A provider asset can map to multiple Cull images; an asset can belong to multiple Photos albums. A single `provider_collection_id` column on an image is insufficient.
- **Opaque provider IDs.** IDs are stored for identity but are not interpreted, normalized, exposed in routine UI, or logged at normal verbosity.
- **Snapshot album mapping.** Importing from an album can create or reuse a Cull manual collection through an explicit provider-album mapping. Name matching alone never reuses a collection. A collision gets a disambiguated initial name such as `Holiday (Apple Photos)`.
- **Cull owns subsequent collection edits.** Later imports add successfully imported selected members to the mapped collection. They do not remove members absent from the latest Photos snapshot, do not remove Cull-only members, and do not overwrite a user-renamed Cull collection. Continuous sync and destructive reconciliation require a separate feature and confirmation policy.
- **Job semantics.** One Apple Photos selection creates one `import` job and one import batch. Progress includes the exact job ID and separate discovery/download/import phases, with per-item status and bytes when PhotoKit supplies them.
- **Cancellation is cooperative and truthful.** Cull cancels active PhotoKit request IDs, stops starting new resources, cleans uncommitted partials, and seals the job as cancelled after in-flight callbacks settle. Already imported resources remain committed. The response reports partial counts and the next retry skips completed identity keys.
- **Network and permission errors are item-specific.** Offline, iCloud unavailable, authorization revoked, asset missing, unsupported resource, disk full, and importer failure have stable reason codes plus user-facing messages. A batch is completed only when every requested resource is imported or explicitly skipped by policy; any unresolved error produces a partial/failed result.
- **No automation surface in the MVP.** Photos status, catalog, and import are available only to the first-party main window through the dedicated Photos capability. MCP, CLI, plugins, deep links, secondary windows, and background startup cannot request authorization, enumerate the catalog, or start imports. Any future headless surface requires a separate privacy/security decision and cannot trigger authorization UI.
- **Signed macOS packaging is part of the boundary.** The usage description and Photos entitlement must be present in the signed app. Source configuration alone is not sufficient release evidence.

## Testing Decisions

- The primary seam is the platform-neutral Photos import service using a fake Photos resource provider/materializer. Tests make assertions on job results, durable files, import/provenance records, and collection outcomes rather than private Objective-C calls.
- Service tests cover current and original modes, RAW+JPEG one-to-many results, Live Photo paired-video skip reporting, unsupported resources, Limited access, authorization revocation, offline failure, cancellation between and during resources, and retry after partial completion.
- Temporary-database tests cover the journal state machine, uniqueness/idempotency keys, one-to-many asset mappings, byte-hash deduplication with distinct provenance, per-resource rollback, and preservation of mixed prior ratings and collection membership.
- Filesystem tests use a dedicated temporary managed root and cover `.part` cleanup, no-replace rename, filename sanitization, resource-name collisions, disk-write failure, restart reconciliation, and proof that cleanup cannot escape the managed root.
- Album tests cover provider-ID mapping rather than name matching, same-title collisions, user-renamed collections, additive reimport, multiple album membership, and preservation of Cull-only members.
- Job tests use the existing JobRegistry seams and assert exact job-ID progress, monotonic phase totals, terminal event ordering, partial-cancel truthfulness, and that a second job cannot overwrite the first job's UI state.
- Rendered dialog tests use the injectable catalog/import client and cover frozen selection, representation choice, progress/cancel/error states, Limited access, partial completion, retry, stale events after close, and focus restoration.
- Native contract tests verify PhotoKit remains macOS-gated, the dedicated capability targets only the main window, no MCP/CLI registration exists, and no PhotoKit content request occurs during metadata-only catalog listing.
- Signed-canary verification inspects the bundled usage description and effective code-signing entitlements, then manually exercises allow, deny, Limited, iCloud download, cancellation, and offline recovery with a controlled Photos library.
- Tests must not require a developer's real Photos library in CI. Native callback/lifetime behavior is covered by compilation plus a small controlled manual canary; deterministic product behavior is covered through fakes.

## Out of Scope

- Video import, Live Photo paired-video import, burst expansion, adjustment data, sidecars, and edit-history reconstruction.
- Continuous Photos change observation, destructive two-way sync, automatic album-member removal, or automatic deletion when a source disappears.
- Browsing non-System Photos libraries or scraping `.photoslibrary`/legacy iPhoto packages.
- Transcoding, format conversion, cloud-provider login, or direct iCloud APIs.
- MCP, CLI, plugin, deep-link, secondary-window, or unattended/background Photos access.
- Replacing Cull's existing file importer, collection model, JobRegistry, or import-batch UI.
- Automatic deletion of durable imported copies or earlier imported versions.

## Further Notes

- The first implementation slice after this policy should materialize a small selected set in `current` mode with the journal, durable storage, existing importer handoff, progress, cancellation, and provenance. Original compound-resource mode and album snapshot mapping can then land as separate reviewable slices on the same model.
- The catalog slice (`imageview-uz3.1`) remains the read-only source of stable PhotoKit asset and album IDs. This policy does not broaden its permission or IPC surface.
- Existing imported images remain ordinary Cull images. Losing Photos permission later affects catalog/reimport access only; it never hides or invalidates the durable Cull copy.
