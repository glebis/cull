# PDF import, catalogization, and media-format support research

Date: 2026-06-16
Issue: imageview-6g39.1
Parent epic: imageview-6g39

## Executive recommendation

Support PDF as a first-class media asset, not as "just another image extension".
Cull's current library contract is image-shaped: `images`, `image_files`,
`ImageWithFile`, thumbnails, selections, collections, quality analysis, CLIP
embeddings, source detection, and color metrics all assume one visual image per
library row. A PDF can contain many pages, embedded text, active content, forms,
annotations, encryption, and page dimensions that differ from page to page.

Recommended implementation path:

1. Add a media layer that can represent both existing images and imported PDFs.
2. Keep existing image APIs stable during the first implementation phase.
3. Import each PDF as one parent media asset with first-page thumbnail/preview.
4. Store page metadata in `pdf_pages`, but do not make every page independently
   rateable/selectable in v1.
5. Use PDFium through Rust for import-time rendering and page metadata.
6. Use a lightweight pure-Rust parser for metadata/text only if it proves useful
   after the PDFium spike; do not add AGPL/GPL renderers.
7. Use PDF.js only for an in-app reader if native raster previews are not enough.

## License-safe stack choice (preferred)

For this Apache-2.0 codebase, the safest default stack is:

- `pdfium-render` as the import-time PDF parser/rendering engine.
- Dynamic/system PDFium loading first, with optional bundled binaries only after adding
  explicit source-attribution, checksums, and packaging policy.
- No AGPL/GPL native PDF dependencies for ingest (avoid MuPDF / Poppler in the first pass).
- No JS execution in backend PDF rendering.

The first useful feature should be: "I can import a PDF folder, see each PDF in
the grid with a stable thumbnail, open it, inspect metadata/text summary, and
search by filename/metadata/extracted text."

## Current Cull constraints

The important current code boundaries are:

- `src-tauri/src/extensions.rs` lists image and RAW extensions only. PDF is not a
  supported import path today.
- `src-tauri/src/commands/import.rs` filters folder imports by that extension
  list before calling `db_core::import::import_file`.
- `src-tauri/src/db_core/import.rs` computes a hash, creates an `Image`, inserts
  an `image_files` row, and then runs image-specific post-processing only if the
  format is decodable.
- `src-tauri/src/db_core/schema.sql` has `images`, `image_files`,
  `image_metadata`, `selections`, and `collection_items`, all keyed around
  `image_id`.
- `src-tauri/src/db_core/models.rs` exposes `ImageWithFile`, and
  `src/lib/api.ts` mirrors that type to the Svelte frontend.
- Existing PDF support is export-only through `printpdf`, used to assemble PNG
  slides into a PDF. That does not help parse or render incoming PDFs.

Implication: adding `"pdf"` to `BASE_IMAGE_EXTENSIONS` would create records with
`format = "pdf"` but no reliable thumbnail, no page count, no extracted text, and
unsafe participation in image-only jobs. That shortcut would make later media
generalization harder.

## Stack options

### Recommended rendering core: PDFium via `pdfium-render`

`pdfium-render` is the strongest fit for import-time thumbnails and previews. It
is an idiomatic Rust binding to PDFium, the C++ PDF library used by Chromium. Its
docs describe page rendering to bitmaps plus document loading and text/image
extraction capabilities.

Use cases for Cull:

- render first page to the existing thumbnail sizes;
- render current page to a loupe/preview bitmap;
- read page count and page dimensions;
- detect encrypted/password-protected PDFs cleanly;
- optionally extract page text after validating quality.

Packaging decision:

- Start with dynamic PDFium in development.
- For release builds, bundle pinned PDFium binaries per platform only after
  adding source, license, checksum, architecture, and update policy to
  `docs/OPEN_SOURCE_AUDIT.md`, `NOTICE`, and the release process.
- Prefer non-V8 PDFium builds unless JavaScript/form execution is explicitly
  needed. Cull should not execute PDF JavaScript.

Sources:

- `pdfium-render` docs: <https://docs.rs/crate/pdfium-render/latest>
- PDFium source: <https://pdfium.googlesource.com/pdfium/>
- PDFium binary distribution reference: <https://github.com/bblanchon/pdfium-binaries>

### Viewer option: PDF.js

PDF.js is appropriate only for an interactive reader surface. It is Apache-2.0
licensed and provides core, display, and viewer layers. The viewer is useful if
Cull needs scrolling pages, text selection, zoom, outline navigation, and
find-in-document behavior sooner than a native custom PDF viewer.

Use cases for Cull:

- a PDF tab or loupe mode that opens the PDF with page navigation;
- client-side page rendering if native PDFium page previews are too limiting;
- future annotation/read mode experiments.

Reasons not to use PDF.js for import-time catalogization:

- import currently lives in Rust and runs outside the Svelte view layer;
- using browser rendering for background thumbnails complicates jobs and
  reproducibility;
- the app still needs native metadata and security decisions before display.

Sources:

- PDF.js home: <https://mozilla.github.io/pdf.js/>
- PDF.js getting started: <https://mozilla.github.io/pdf.js/getting_started/>

### Metadata/text option: `lopdf` and `pdf-extract`

`lopdf` is a pure-Rust PDF manipulation/parser library. It is useful for reading
document structure and metadata without rendering. `pdf-extract` exposes direct
text extraction helpers, including by-page extraction.

Recommended use:

- Treat these as optional helpers after a PDFium spike.
- Use them only if PDFium text extraction is insufficient, too awkward, or too
  hard to package for text-only work.
- Do not use them for thumbnails or visual rendering.

Sources:

- `lopdf` docs: <https://docs.rs/lopdf>
- `pdf-extract` docs: <https://docs.rs/pdf-extract>

### Avoid for core shipping path: MuPDF and Poppler

MuPDF is technically strong, but its open-source license is AGPL and the official
docs state that systems built with it must comply with AGPL conditions unless
they use a commercial license. Poppler is GPL-family software. Cull is Apache-2.0
open source, so either dependency would require a deliberate license decision and
is not a good default.

Sources:

- MuPDF license docs: <https://mupdf.readthedocs.io/en/1.27.0/license.html>
- Poppler source mirror: <https://gitlab.com/freedesktop-sdk/mirrors/freedesktop/poppler/poppler>

### OS-native thumbnailing

macOS Quick Look / ImageIO-style thumbnailing can be a useful fallback or spike,
but it should not be the canonical implementation:

- it is platform-specific;
- it makes tests less deterministic;
- it does not solve metadata/text extraction;
- it does not support Linux/Windows packaging if Cull broadens distribution.

Use OS-native thumbnailing only as a tactical fallback after the PDFium path is
implemented and tested.

## Proposed data model

Do not rename `images` globally in one migration. Too many query surfaces rely on
`ImageWithFile`.

Instead add a media layer:

```sql
CREATE TABLE media_assets (
    id TEXT PRIMARY KEY,
    media_type TEXT NOT NULL CHECK (media_type IN ('image', 'pdf')),
    primary_image_id TEXT NOT NULL REFERENCES images(id) ON DELETE CASCADE,
    sha256_hash TEXT NOT NULL,
    format TEXT NOT NULL,
    file_size INTEGER NOT NULL,
    page_count INTEGER,
    title TEXT,
    created_at TEXT NOT NULL,
    imported_at TEXT NOT NULL
);

CREATE TABLE media_files (
    id TEXT PRIMARY KEY,
    media_asset_id TEXT NOT NULL REFERENCES media_assets(id) ON DELETE CASCADE,
    path TEXT NOT NULL,
    last_seen_at TEXT NOT NULL,
    last_seen_size INTEGER,
    last_seen_mtime TEXT,
    missing_at TEXT
);

CREATE TABLE pdf_pages (
    id TEXT PRIMARY KEY,
    media_asset_id TEXT NOT NULL REFERENCES media_assets(id) ON DELETE CASCADE,
    page_index INTEGER NOT NULL,
    width_points REAL,
    height_points REAL,
    thumbnail_path TEXT,
    preview_path TEXT,
    extracted_text TEXT,
    text_extracted_at TEXT,
    UNIQUE(media_asset_id, page_index)
);
```

Backfill rule:

- Every existing `images` row gets a `media_assets` row with `media_type =
  'image'` and `primary_image_id = images.id`.
- Existing `image_files` can stay in place for image APIs. A later migration can
  backfill `media_files` if the frontend moves fully to media queries.

PDF import rule:

- PDF gets one `media_assets` parent row.
- PDF gets one `media_files` path row.
- First page render creates a thumbnail and preview path.
- If the UI still needs an `ImageWithFile` compatibility object in phase 1,
  create a generated representative image record for the first page and link it
  through `primary_image_id`. Mark it as generated/derived in metadata so ML
  pipelines can skip or opt into it deliberately.

Page curation decision:

- v1: rating/selection applies to the parent PDF asset.
- v2: add page-level selection only if there is a concrete workflow for choosing
  individual pages from a multi-page PDF.

This avoids a bad v1 where a 60-page PDF silently becomes 60 imported "images"
and pollutes image curation, embeddings, import counts, and collections.

## Import pipeline

Add a media-aware import path beside the existing image path:

1. Split extension policy into media groups:
   - image import extensions;
   - RAW extensions;
   - document/media extensions, starting with `pdf`.
2. Add `db_core::pdf_import` or `db_core::media_import`.
3. For a PDF:
   - validate extension and MIME/header enough to reject obvious non-PDF files;
   - hash with existing streaming hash pattern;
   - reject oversized files with a separate PDF limit;
   - open through PDFium with JavaScript/action execution disabled or unavailable;
   - capture page count, first-page size, encryption status, and metadata;
   - render first page thumbnail/preview;
   - store per-page rows with dimensions;
   - extract text in a capped, best-effort pass or defer to a background job.
4. Do not run image quality, CLIP embedding, color metrics, perceptual hash, or
   AI source detection against the PDF file itself.
5. For first-page representative images, only run visual jobs if a future issue
   explicitly opts PDFs into page visual embedding.

Error handling:

- password-protected PDF: import the file row, mark status as encrypted, no
  thumbnail unless PDFium can render without password;
- malformed PDF: skip import and report path-specific error;
- huge page count: import metadata and first page only, defer page expansion;
- render failure: keep catalog entry with a generic PDF thumbnail and error state.

## Frontend UX

Minimal v1 UI:

- Grid card can display image or PDF asset.
- PDF badge shows `PDF` and page count.
- Status bar shows filename, format, page count, file size, and selected/rating
  state.
- Loupe opens PDF preview with page stepper, page number, zoom, and "open
  original" action.
- Search can match filename, document title, author/producer fields, and
  extracted text snippets.

Do not expose a full PDF editor. This is catalogization, not Preview.app parity.

Frontend type path:

- Add `MediaAsset`, `MediaWithFile`, and `MediaType = 'image' | 'pdf'` to
  `src/lib/api.ts`.
- Keep `ImageWithFile` for existing image-specific commands.
- Add new media list commands instead of changing every `list_images` caller at
  once.

## Security posture

PDFs are hostile-input documents. Treat rendering and text extraction as unsafe
parsing work.

Required controls:

- no PDF JavaScript execution;
- no network fetches from PDF content;
- bounded file size, page count, render resolution, and extraction text length;
- thumbnail/render jobs run through existing job/progress infrastructure, not
  synchronous UI blocking;
- failures are recorded per file/page and surfaced as catalog warnings;
- fuzz/regression fixtures include malformed, encrypted, huge-page, and image-only
  scanned PDFs;
- document license/source/checksum for any bundled PDFium binary.

## Licensing and release audit impact

Cull is Apache-2.0. The safe default stack is:

- `pdfium-render`: Rust wrapper; verify crate license before adding.
- PDFium binary/source: permissive/notice-bearing upstream, but must be audited
  because prebuilt binary packaging introduces third-party notices and checksums.
- PDF.js: Apache-2.0, acceptable if bundled through npm with notices.
- `lopdf` / `pdf-extract`: verify crate licenses in `cargo metadata` and
  `npm run audit:licenses` before committing.

Avoid:

- MuPDF/PyMuPDF unless accepting AGPL or buying commercial license.
- Poppler wrappers unless accepting GPL implications.

Before publishing PDF support, update:

- `docs/OPEN_SOURCE_AUDIT.md`;
- `NOTICE`;
- `README.md` supported import formats;
- `docs/USER_GUIDE.md`;
- `src-tauri/Cargo.toml` license/dependency notes as needed;
- About dialog dependency/license view if it lists bundled components.

## Testing strategy

Rust unit tests:

- extension routing includes PDF as media but not image;
- PDF import creates `media_assets`, `media_files`, and `pdf_pages`;
- duplicate PDF path/hash handling matches existing image import semantics;
- encrypted/malformed/oversized PDFs produce stable errors;
- representative thumbnail path is generated deterministically;
- schema migration from version 22 backfills existing image media assets.

Frontend tests:

- media API types do not import `tauri-mock` into production API code;
- grid renders a PDF card with badge/page count;
- loupe chooses PDF preview controls for PDF assets and image controls for images;
- search result text distinguishes PDF text hits from image metadata hits.

Manual/browser gate when UI changes:

- run the existing Vite/browser smoke flow on `localhost:1420`;
- import a folder with mixed PNG/JPEG/PDF files;
- verify image-only workflows still work after PDF assets appear.

Preflight:

- `npm run check`;
- `npm test`;
- `cd src-tauri && cargo fmt`;
- `cd src-tauri && cargo test --lib`;
- `npm run audit:licenses` after adding dependencies.

## Proposed bd implementation tasks

Under `imageview-6g39`, create these children after this research is accepted:

1. `Spike PDFium packaging and rendering`
   - Add a small Rust-only spike that loads a fixture PDF, reports page count,
     and renders first page to PNG.
   - Decide dynamic vs bundled binary release policy.

2. `Add media asset schema and backfill`
   - Add migration from schema version 22.
   - Add `media_assets`, `media_files`, and `pdf_pages`.
   - Backfill existing images.

3. `Implement PDF import pipeline`
   - Add PDF extension routing.
   - Add PDF hash/dedupe/import logic.
   - Generate first-page thumbnail and page rows.

4. `Add media API contracts`
   - Add Rust/Svelte `MediaWithFile` types.
   - Add list/get commands for media assets.
   - Keep image commands stable.

5. `Render PDF assets in grid and loupe`
   - Add PDF card badge/page count.
   - Add page navigation in loupe.
   - Preserve image keyboard behavior.

6. `Add PDF metadata and text search`
   - Extract title/author/producer/page text with caps.
   - Index snippets for search/smart collections.

7. `Update docs, license audit, and release notes`
   - Update supported import format docs.
   - Update third-party audit/NOTICE/checksums.
   - Run license audit.

## Open decisions

1. Should page-level ratings/selections exist in v1?
   - Recommendation: no. Parent PDF selection only.

2. Should PDFs generate CLIP/DINO embeddings from the first page?
   - Recommendation: no in v1. Add explicit opt-in later.

3. Should Cull bundle PDFium binaries in the first release?
   - Recommendation: do a spike first. For user-ready PDF import, bundled pinned
     binaries are likely necessary, but they must go through the license/checksum
     release process.

4. Should PDF.js ship in v1?
   - Recommendation: not required for import/catalogization. Add only if the
     first-page preview plus page raster navigation is not enough.

## Acceptance mapping

- Recommended PDF parsing/rendering stack: PDFium via `pdfium-render` for
  import-time rendering and page metadata; optional `lopdf` / `pdf-extract` for
  text/metadata after a spike; PDF.js only for an interactive reader.
- Database/model changes: add `media_assets`, `media_files`, and `pdf_pages`;
  keep existing `images` / `ImageWithFile` APIs stable during phase 1; backfill
  existing image rows into the media layer.
- Import/catalogization UX: import a PDF as one parent asset with first-page
  thumbnail/preview, page count, metadata, and capped extracted text; do not
  create one curation item per page in v1.
- Error handling and risk: explicitly covers encrypted, malformed, huge-page,
  and render-failure cases; treats PDF parsing as hostile-input work with
  bounds and no JavaScript/network execution.
- Licensing: keeps Apache-2.0 alignment by avoiding AGPL/GPL renderers in the
  core path; requires audit/NOTICE/checksum work for any bundled PDFium binary.
- Phased implementation: proposes seven bd child tasks under `imageview-6g39`
  spanning spike, schema, import, API, UI, search, and docs/license audit.
