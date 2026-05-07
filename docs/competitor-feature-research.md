# Competitor Feature Research: Daily-Driver Image Viewers on macOS

Research compiled 2026-05-07. Sources verified via web search.

---

## 1. macOS Preview.app

The built-in image and PDF viewer. The baseline any replacement must match or exceed.

### Image Format Support

| Format | Status | Notes |
|--------|--------|-------|
| JPEG | Native | Full read/write |
| PNG | Native | Full read/write |
| TIFF | Native | Full read/write, multi-page |
| GIF | Native | Animated playback |
| HEIC/HEIF | Native | Read/write since macOS High Sierra |
| JPEG 2000 | Native | Read/write |
| OpenEXR | Native | Read/write (HDR) |
| BMP | Native | Read/write |
| PSD | Read only | Flattened composite only |
| PDF | Native | Full read/write/edit |
| RAW (camera) | Partial | Supported via ImageIO framework; macOS Sequoia supports 800+ camera models. Uses system-level RAW processing, not per-app. |
| WebP | Limited | macOS Sequoia added partial support; users report inconsistencies. No WebP write support. |
| AVIF | Limited | Can convert AVIF to other formats; native viewing is inconsistent as of Sequoia. |
| SVG | No | Not supported natively |
| JPEG XL | No | Not supported |

**Export formats** (visible with Option key held in Export dialog): HEIC, JPEG, JPEG-2000, OpenEXR, PDF, PNG, TIFF, plus additional formats.

### OS Integration (Gold Standard)

- **Default handler** for images, PDFs via Launch Services
- **Quick Look**: Preview IS the Quick Look renderer for images/PDFs
- **"Open With"**: Always present, always first in the list
- **Services menu**: Provides and consumes macOS Services
- **Share sheet**: Full macOS Share sheet integration
- **Spotlight**: System handles image indexing; Preview benefits automatically
- **Drag and drop**: Full Finder integration, drag images in/out
- **AppleScript**: Scriptable via `sdef`; basic open/export/print commands
- **Continuity Camera**: Scan/photograph documents directly into Preview
- **Handoff**: Resume viewing on another Apple device
- **File associations**: Registered as handler for all system-supported image UTTypes
- **Sidebar thumbnails**: Multi-image/multi-page documents in sidebar

### Viewing Features

- **Zoom**: Cmd+Plus/Minus, pinch-to-zoom trackpad, scroll wheel with modifier
- **Pan**: Click-drag when zoomed, two-finger trackpad scroll
- **Rotate**: Cmd+L (left), Cmd+R (right), two-finger trackpad rotation gesture
- **Actual Size**: Cmd+0 (actual pixels), Option+Cmd+0 (all images to actual size)
- **Fit to Window**: Cmd+9
- **Slideshow**: Cmd+Shift+F; full-screen slideshow of all open images
- **Multi-image**: Open multiple images, navigate via sidebar thumbnails
- **Multi-page**: PDF and multi-page TIFF in sidebar
- **Contact sheet**: View all pages/images as thumbnail grid
- **Full screen**: Native macOS full screen (Ctrl+Cmd+F)
- **Continuous scroll**: PDF pages scroll continuously

### Basic Editing

- **Crop**: Rectangular selection then Cmd+K
- **Resize**: Tools > Adjust Size (width, height, resolution, units, resampling method)
- **Color adjust**: Tools > Adjust Color (exposure, contrast, highlights, shadows, saturation, temperature, tint, sharpness, sepia)
- **Annotations**: Text, shapes, arrows, speech bubbles, signatures, magnifier loupe
- **Markup toolbar**: Pen, highlighter, shapes, text, signatures
- **Instant Alpha**: Remove background color (magic wand-like)
- **Flip**: Horizontal and vertical flip

### Batch Operations

- Select multiple images in Finder, open all in Preview
- **Batch resize**: Select all in sidebar > Tools > Adjust Size
- **Batch export**: File > Export Selected Images (change format, quality)
- **Batch rotate/flip**: Works across all selected images
- **No batch color adjustment**: Color changes apply to single image only
- **Destructive**: Saves changes to original files by default (no undo after close)

### PDF Capabilities (not needed in ImageView, but for reference)

- View, annotate, fill forms, sign
- Reorder pages via drag in sidebar
- Insert/delete pages
- Merge PDFs (drag pages between documents)
- Export pages as images
- Print with layout options
- Password-protect/encrypt PDFs

### Key Keyboard Shortcuts

| Shortcut | Action |
|----------|--------|
| Space | Quick Look (Finder) |
| Cmd+Plus/Minus | Zoom in/out |
| Cmd+0 | Actual size |
| Cmd+9 | Fit to window |
| Cmd+L/R | Rotate left/right |
| Cmd+K | Crop |
| Cmd+Shift+F | Slideshow |
| Cmd+Shift+A | Annotate (Markup toolbar) |
| Arrow keys | Next/prev image in sidebar |
| Cmd+P | Print |
| Cmd+Shift+S | Export (Save As) |

### Weaknesses as a Daily Driver

- No folder browsing / directory navigation
- No ratings, tags, or culling workflow
- No histogram, no EXIF display
- Limited format support (no SVG, no JPEG XL, inconsistent WebP/AVIF)
- No batch rename
- Destructive edits by default
- Slow with very large files (100+ MP, large RAW)
- No color management controls
- No filmstrip/thumbnail strip while viewing single image
- No keyboard-driven navigation between files in a folder

---

## 2. FastRawViewer

RAW-focused viewer built on LibRaw. Aimed at photographers who need to cull shoots fast.

### RAW Format Support and Rendering

- Supports 800+ cameras and virtually all RAW formats (CR2, CR3, NEF, ARW, DNG, ORF, RAF, RW2, PEF, SRW, etc.)
- **Renders actual RAW data**, not embedded JPEG previews (critical differentiator)
- Also reads JPEG, TIFF, PNG, HEIC
- Frequent updates for new camera support
- Uses LibRaw library (same codebase as dcraw successor)

### Speed and Performance

- Opens RAW files nearly instantly (sub-second for most files)
- Optimized for rapid sequential viewing (culling hundreds of images)
- Lightweight memory footprint compared to Lightroom
- Grid mode with thumbnails rendered from RAW data

### Histogram and Exposure Analysis

- **RAW histogram**: Based on actual sensor data, not JPEG preview
- **Over/underexposure indication**: Visual overlay showing clipped highlights and crushed shadows
- **Exposure statistics**: Numerical percentage of over/underexposed pixels
- **Shadow Boost**: Temporarily brighten shadows to evaluate detail
- **Highlight Inspection**: Check highlight detail without adjusting exposure
- **Per-channel view**: View individual R/G/B channels
- **Focus Peaking**: Highlight in-focus areas for sharpness evaluation

### EXIF Display

- Customizable EXIF panel (show/hide individual fields)
- EXIF displayed in Grid mode above or below thumbnails
- Comprehensive EXIF data: camera, lens, focal length, aperture, shutter, ISO, GPS, date, etc.
- Sortable by EXIF fields

### Culling and Rating Workflow

- **Star ratings**: 0-5 stars, written to XMP sidecar files
- **Color labels**: Customizable, also XMP-compatible
- **Move/copy to folders**: Assign keepers to output folders
- **Reject marking**: Flag images for deletion
- **XMP sidecar compatibility**: Ratings/labels readable by Adobe Lightroom, Camera Raw, Bridge
- **Series propagation**: Copy white balance, exposure, orientation from one shot to entire panorama/bracket series

### On-the-Fly Adjustments (Non-Destructive)

- Exposure correction
- White balance (temperature + tint)
- Contrast curves
- Shadow boost
- Sharpening for display (two presets)
- Black and white preview
- All adjustments saved to XMP sidecars

### Color Management

- ICC profile-aware display
- Monitor profile matching
- Configurable working color space
- Soft proofing not available (not its purpose)

### Key Strengths for Daily Use

- Extremely fast culling workflow
- Honest RAW evaluation (not misleading JPEG preview)
- Fully customizable keyboard/mouse shortcuts
- Low resource usage

### Weaknesses

- Not a general-purpose image viewer (RAW-focused)
- No annotation or markup tools
- No image editing beyond basic adjustments
- No batch export/conversion
- No PDF support
- UI is functional but not polished
- No macOS Services or Share sheet integration
- No Quick Look extension

---

## 3. ACDSee Photo Studio for Mac

Full photo management and editing suite. Current version: 26 (released November 2025).

### Browse / Thumbnail Mode (Manage Mode)

- **Folder tree navigation**: Browse entire filesystem via folder panel
- **Thumbnail grid**: Configurable thumbnail sizes
- **List view**: Alternative to thumbnails with file details
- **Group By**: Sort and group by camera, file type, date captured, rating, label, size, author
- **Filter bar**: Filter visible files by type, rating, label, tag
- **Preview pane**: See larger preview alongside thumbnails
- **Dual-pane**: Manage mode layout with folder tree + file list + preview + properties

### Metadata / EXIF Editing

- **EXIF viewing**: Full EXIF data display
- **IPTC editing**: Description, creator, copyright, keywords, location
- **XMP support**: Read and write XMP metadata
- **Batch metadata**: Copy/paste metadata from one file to many
- **Strip metadata**: Remove EXIF, IPTC, ACDSee metadata in bulk
- **GPS/location data**: View and edit geographic metadata

### Categories, Ratings, Tags

- **Star ratings**: 1-5 stars
- **Color labels**: Customizable colors
- **Categories**: Hierarchical category system
- **Keywords**: Hierarchical keyword trees
- **Tags**: Quick tagging for organization
- **Smart collections**: Auto-populated based on criteria
- **AI Keywords** (v26): AI-powered automatic keyword generation for search

### Quick View vs Full Management Mode

- **Manage Mode**: Full browser with folder tree, metadata panel, thumbnail grid
- **View Mode**: Full-screen image viewing from Manage Mode
- **Develop Mode**: Non-destructive RAW/image editing (exposure, color, detail, geometry)
- **Edit Mode**: Pixel-level editing (layers, selections, clone, healing)

### Batch Processing

- **Batch rename**: Pattern-based file renaming
- **Batch resize**: Multiple images at once
- **Batch convert**: Format conversion across files
- **Batch rotate**: Based on EXIF orientation or manual
- **Batch metadata editing**: As noted above
- **Batch develop**: Apply develop settings across images

### AI Features (v26)

- **AI Denoise**: Noise removal preserving detail
- **AI Presets**: Auto-analyze and apply adjustments per photo
- **AI Keywords**: Auto-tag images for search without manual tagging
- **AI Actions** (Edit Mode): Generate masks, selections via AI

### Format Support

- All common image formats (JPEG, PNG, TIFF, BMP, GIF, PSD)
- RAW: Supports major camera RAW formats via their own decoder
- HEIC/HEIF: Supported
- WebP: Not prominently listed

### Weaknesses

- macOS version historically lags behind Windows version in features
- Subscription pricing ($70-90/year)
- Can feel heavyweight for quick viewing
- Not a lean viewer; it is a full DAM/editor
- No Quick Look or Spotlight integration
- Performance with very large libraries can degrade

---

## 4. XnView MP

Cross-platform (Windows/macOS/Linux) image viewer, converter, and organizer. Free for personal use.

### Browser Mode

- **Folder tree panel**: Navigate filesystem
- **Thumbnail view**: Configurable grid with adjustable sizes
- **List view**: Detailed file list with columns
- **Preview panel**: Quick preview alongside browser
- **Filmstrip**: Strip of thumbnails alongside full-size view
- **Contact sheet creation**: Generate printable contact sheets
- **Tab interface**: Multiple folders in tabs

### Viewer Mode

- **Full-screen viewing**: Dedicated viewer with overlays
- **Slideshow**: Configurable timing, transitions
- **Loupe/magnifier**: Inspect details at zoom
- **Compare mode**: Side-by-side image comparison
- **Screen capture**: Built-in screenshot tool

### Format Support (Industry-Leading Breadth)

- **Reads 500+ formats** including: JPEG, PNG, TIFF, GIF, BMP, PSD, WebP, JPEG XL, AVIF, HEIC/HEIF, SVG, PDF, DNG, OpenEXR, ICO, TGA, PCX, and virtually every legacy and modern format
- **Writes ~70 formats**: JPEG, PNG, TIFF, GIF, BMP, WebP, JPEG XL, AVIF, HEIC/HEIF, PSD, JPEG 2000, OpenEXR, PDF
- **RAW support**: CR2, CR3, NEF, ARW, DNG, ORF, RAF, RW2, and more
- **HEIC/HEIF**: Native, no codec installation needed on any platform
- **Video thumbnails**: Can display video file thumbnails in browser

### Batch Conversion

- **Batch convert**: Tools > Batch Convert with output format, quality, and destination
- **80+ configurable actions** via XnConvert (companion app): resize, watermark, rotate, crop, filter, text overlay, color adjustments, DPI change, canvas resize
- **Batch rename**: Pattern-based (date, EXIF, sequence numbers, regex)
- **Lossless JPEG transforms**: Rotate, crop without recompression
- **Batch process mixed formats**: Input various formats, output to unified format

### Metadata Handling

- **EXIF**: Full read, selective write
- **IPTC-IIM**: Read and edit all fields
- **XMP**: Read and edit; dedicated XMP tab in properties
- **ExifTool integration**: Properties panel includes ExifTool output tab
- **Batch metadata editing**: Edit IPTC/XMP across multiple files
- **Metadata template**: Save and apply metadata presets
- **GPS map**: View photo location on map

### Organization

- **Star ratings**: 0-5
- **Color labels**: Multiple colors
- **Categories**: Assign categories to files
- **Tags/keywords**: Keyword assignment
- **Filter by**: Rating, label, color, metadata fields
- **Icons on thumbnails**: Show which metadata categories exist, ratings, labels

### Other Notable Features

- **Print layouts**: Multiple images per page, custom layouts
- **Duplicate finder**: Find duplicate images
- **TWAIN/WIA scanning**: Acquire from scanner
- **Hex viewer**: Raw file data inspection
- **Screenshot capture**: Built-in
- **Plugin system**: Extensible

### Weaknesses

- UI looks dated / non-native on macOS (Qt-based)
- No macOS Share sheet, Services, Quick Look, or Spotlight integration
- Not a native macOS app (cross-platform compromises)
- No non-destructive editing (destructive only)
- Performance with very large files (100+ MP) can be sluggish
- No AppleScript support

---

## 5. macOS OS Integration Requirements

What an app needs to do to be a credible Preview.app replacement.

### 5.1 File Type Registration (Info.plist)

**CFBundleDocumentTypes** array in Info.plist:

```xml
<key>CFBundleDocumentTypes</key>
<array>
  <dict>
    <key>CFBundleTypeName</key>
    <string>JPEG Image</string>
    <key>CFBundleTypeRole</key>
    <string>Viewer</string>  <!-- or Editor -->
    <key>LSItemContentTypes</key>
    <array>
      <string>public.jpeg</string>
    </array>
    <key>LSHandlerRank</key>
    <string>Alternate</string>  <!-- Owner | Alternate | None | Default -->
  </dict>
  <!-- Repeat for each image type -->
</array>
```

**LSHandlerRank values:**
- `Owner`: App created this file type (highest priority)
- `Default`: App is the preferred handler
- `Alternate`: App can open these files (appears in "Open With")
- `None`: Can handle via drag-and-drop but doesn't appear in "Open With"

**Minimum image UTTypes to declare** for a daily-driver viewer:
- `public.image` (parent type, catches all images)
- `public.jpeg`, `public.png`, `public.tiff`, `com.compuserve.gif`
- `public.heic`, `public.heif`
- `public.webp` (org.webmproject.webp)
- `public.avif` (if supported)
- `public.svg-image`
- `com.adobe.raw-image` (parent for all RAW)
- `public.camera-raw-image`
- `com.adobe.photoshop-image`

### 5.2 UTType Declarations (Modern macOS 11+)

For custom or non-system types, declare in Info.plist:

**UTImportedTypeDeclarations** — for types your app can open but did not invent:
```xml
<key>UTImportedTypeDeclarations</key>
<array>
  <dict>
    <key>UTTypeIdentifier</key>
    <string>org.webmproject.webp</string>
    <key>UTTypeConformsTo</key>
    <array>
      <string>public.image</string>
    </array>
    <key>UTTypeTagSpecification</key>
    <dict>
      <key>public.filename-extension</key>
      <array>
        <string>webp</string>
      </array>
      <key>public.mime-type</key>
      <array>
        <string>image/webp</string>
      </array>
    </dict>
  </dict>
</array>
```

**UTExportedTypeDeclarations** — only for types your app invents/owns.

In Swift code (modern approach), types are declared using the `UTType` struct from `UniformTypeIdentifiers` framework.

### 5.3 "Open With" Context Menu

Automatic once `CFBundleDocumentTypes` is properly declared. The app appears in Finder's right-click > "Open With" submenu for matching file types. No additional code needed.

Users set default via: Right-click file > Get Info > "Open With" > Change All.

### 5.4 Quick Look Extension

Modern approach (macOS 10.15+): **QLPreviewingController** appex (app extension).

- Create a "Quick Look Preview" extension target in Xcode
- Implement `QLPreviewingController` protocol
- Declare supported content types in extension's Info.plist
- The extension runs sandboxed, separate from the main app

Legacy approach (deprecated): `.qlgenerator` bundles in `/Library/QuickLook/` — no longer recommended.

A Quick Look extension lets the user press Space in Finder and see your app's rendering of the file.

### 5.5 Finder Preview Panel

The Finder preview panel (View > Show Preview) uses Quick Look thumbnails. If your app provides a **Thumbnail Extension** (`QLThumbnailProvider`), Finder will use it to generate thumbnails in:
- Icon view
- Gallery view
- Preview panel
- Get Info window

### 5.6 Spotlight Importer

A **Spotlight Importer** (`.mdimporter`) plugin extracts metadata from files for Spotlight indexing.

- Create a "Spotlight Importer" target in Xcode
- Return metadata keys (image dimensions, color space, EXIF data)
- Declare supported UTTypes
- Gets invoked automatically by `mds` (metadata server)

For standard image formats, macOS already has importers. Only needed if supporting custom/non-standard formats.

### 5.7 Share Extension

A **Share Extension** lets your app appear in the system Share sheet of other apps.

- Create a "Share Extension" target
- Handle incoming images/URLs
- Appears in Photos.app share sheet, Safari, etc.

The main app can also invoke `NSSharingServicePicker` to present the system Share sheet from within the app.

### 5.8 Services Menu Integration

Register services in Info.plist under `NSServices`:

```xml
<key>NSServices</key>
<array>
  <dict>
    <key>NSMessage</key>
    <string>openImageInImageView</string>
    <key>NSSendTypes</key>
    <array>
      <string>NSFilenamesPboardType</string>
    </array>
    <key>NSMenuItem</key>
    <dict>
      <key>default</key>
      <string>Open in ImageView</string>
    </dict>
  </dict>
</array>
```

This adds an entry in the right-click > Services menu system-wide.

### 5.9 Drag and Drop

For a Tauri/web-based app:
- **Inbound**: Handle file drops from Finder (Tauri provides `tauri://file-drop` events)
- **Outbound**: Initiate drags of images to other apps (requires native integration or Tauri plugin)
- **Drag promise**: For large files, provide a file promise instead of copying data eagerly

Native macOS APIs: `NSPasteboardItem`, `NSDraggingSource`, `NSDraggingDestination`.

### 5.10 Apple Events / AppleScript

To be scriptable:
- Define an `.sdef` (scripting definition) file
- Register Apple Event handlers
- Support basic commands: `open`, `close`, `print`, `quit`
- Image-specific: `export`, `resize`, `rotate`, `get metadata`

Tauri apps would need a native plugin bridge for Apple Event support.

### 5.11 Default App Association

**User-facing methods:**
1. Right-click any image > Get Info > "Open With" dropdown > select app > "Change All..."
2. System Settings > (no centralized setting for file associations)

**Programmatic methods:**
- `LSSetDefaultRoleHandlerForContentType()` (deprecated but functional)
- `duti` CLI tool (`brew install duti`): `duti -s com.imageview.app public.jpeg viewer`
- The app CANNOT set itself as default without user consent (macOS security)

**For Tauri apps specifically:**
- The bundle identifier in `tauri.conf.json` > `identifier` becomes the app's CFBundleIdentifier
- File associations are configured in `tauri.conf.json` > `bundle` > `fileAssociations`
- Tauri generates the correct Info.plist entries from this configuration

### 5.12 Priority Ranking for ImageView

Based on this research, here is the priority order for OS integration features:

| Priority | Feature | Impact | Effort |
|----------|---------|--------|--------|
| P0 | CFBundleDocumentTypes for all image UTTypes | Appears in "Open With", can be set as default | Low (config) |
| P0 | Drag and drop from Finder | Basic file opening | Low (Tauri built-in) |
| P1 | Quick Look extension | Space-bar preview in Finder | Medium (native appex) |
| P1 | Thumbnail extension | Finder thumbnails for custom formats | Medium (native appex) |
| P1 | System Share sheet (outbound) | Share images from ImageView | Low (NSSharingServicePicker) |
| P2 | Services menu | Right-click > Services > Open in ImageView | Low (Info.plist config) |
| P2 | AppleScript/Apple Events | Automation support | Medium (sdef + handlers) |
| P3 | Spotlight importer | Only for non-standard formats | Medium (only if needed) |
| P3 | Share extension (inbound) | Receive images from other apps | Medium (appex) |

---

## Feature Comparison Matrix

| Feature | Preview | FastRawViewer | ACDSee Mac | XnView MP | ImageView Target |
|---------|---------|---------------|------------|-----------|-----------------|
| **Format breadth** | ~15 formats | RAW-focused + common | ~30 formats | 500+ read, 70 write | Broad (via native + libs) |
| **RAW rendering** | Via ImageIO | True RAW (LibRaw) | Own decoder | Via LibRaw/dcraw | TBD |
| **WebP** | Partial | No | No | Yes | Yes |
| **AVIF** | Partial | No | No | Yes | Yes |
| **JPEG XL** | No | No | No | Yes | Yes |
| **SVG** | No | No | No | Yes | Yes |
| **Folder browsing** | No | Grid mode | Full DAM | Full browser | Yes |
| **Thumbnail grid** | Sidebar only | Grid mode | Full grid | Full grid | Yes |
| **Histogram** | No | RAW histogram | Basic | Basic | Yes |
| **EXIF display** | No | Customizable | Full | Full + ExifTool | Yes |
| **Star ratings** | No | XMP-compatible | Full + AI | Yes | Yes |
| **Color labels** | No | Yes (XMP) | Yes | Yes | Yes |
| **Culling workflow** | No | Core feature | Via ratings | Basic | Yes |
| **Batch resize** | Yes | No | Yes | Yes (80+ actions) | Yes |
| **Batch convert** | Basic | No | Yes | Yes (industry-best) | Yes |
| **Annotations** | Yes | No | Yes (Edit mode) | No | TBD |
| **Non-destructive edit** | No | XMP adjustments | Develop mode | No | TBD |
| **Quick Look** | IS Quick Look | No | No | No | P1 goal |
| **"Open With"** | Default | Yes | Yes | Yes | P0 goal |
| **Share sheet** | Full | No | Limited | No | P1 goal |
| **Services menu** | Full | No | No | No | P2 goal |
| **AppleScript** | Basic | No | No | No | P2 goal |
| **Drag and drop** | Full | Basic | Basic | Basic | P0 goal |
| **Native macOS feel** | Perfect | Adequate | Adequate | Poor (Qt) | Goal: native-feeling |
| **Performance (large files)** | Slow | Excellent | Moderate | Moderate | Goal: fast |
| **AI features** | No | No | Yes (v26) | No | Agent-friendly |
| **Keyboard-driven** | Limited | Customizable | Standard | Standard | Core design principle |
| **Dark theme** | System | Custom | Custom | Custom | Terminal-inspired |

---

## Key Takeaways for ImageView

1. **The gap Preview leaves**: No folder browsing, no EXIF, no histogram, no ratings, no culling, limited format support, no keyboard-driven workflow. These are all table-stakes for a power-user image viewer.

2. **What XnView MP proves**: 500+ format support is achievable and valued. Batch conversion with 80+ actions is a beloved feature. But cross-platform UI compromises make it feel alien on macOS.

3. **What FastRawViewer proves**: RAW histogram and true RAW rendering are transformative for photographers. Speed matters enormously for culling. XMP sidecar compatibility with Adobe is essential.

4. **What ACDSee proves**: AI-powered features (auto-keywords, denoise, smart presets) are becoming expected in 2026. Full DAM capability is valued but heavyweight.

5. **The OS integration moat**: No competitor has meaningful macOS integration (Quick Look, Share, Services, Spotlight). An app that combines power-user features WITH native macOS integration would be genuinely differentiated.

6. **P0 for credibility**: CFBundleDocumentTypes registration + broad format support + "Open With" appearance. Without these, the app cannot be a daily driver regardless of other features.

---

## Sources

- [Apple Preview User Guide](https://support.apple.com/guide/preview/welcome/mac)
- [Apple Preview Supported File Formats](https://fileinfo.com/software/apple/preview)
- [Preview Keyboard Shortcuts - Apple Support](https://support.apple.com/guide/preview/keyboard-shortcuts-cpprvw0003/mac)
- [macOS Sequoia RAW Format Support](https://support.apple.com/en-us/120534)
- [FastRawViewer Features](https://www.fastrawviewer.com/features)
- [FastRawViewer 2.0 User Manual](https://updates.fastrawviewer.com/data/FastRawViewer2-Manual-ENG.pdf)
- [FastRawViewer Review - Photography Life](https://photographylife.com/reviews/fastrawviewer)
- [ACDSee Photo Studio for Mac Features](https://www.acdsee.com/en/products/photo-studio-mac/features/)
- [ACDSee Photo Studio for Mac 26 Release Notes](https://www.acdsee.com/en/support/photo-studio-mac/release-notes/acpm26en/)
- [ACDSee Ultimate Review 2026](https://silentpeakphoto.com/photo-editing-apps/photo-editing-app-reviews/acdsee-ultimate-review/)
- [XnView MP Official](https://www.xnview.com/en/xnview/)
- [XnView MP - Toolify Overview](https://www.toolify.ai/ai-news/xnview-mp-the-ultimate-image-viewer-and-converter-3438865)
- [XnView MP for Creatives - Persone Design](https://www.personedesign.com/blog/2025/06/xnviewmp-image-manager-for-creatives.html)
- [CFBundleDocumentTypes - Apple Developer](https://developer.apple.com/documentation/bundleresources/information-property-list/cfbundledocumenttypes)
- [UTType - Apple Developer](https://developer.apple.com/documentation/uniformtypeidentifiers/uttype-swift.struct)
- [UTExportedTypeDeclarations - Apple Developer](https://developer.apple.com/documentation/BundleResources/Information-Property-List/UTExportedTypeDeclarations)
- [Defining File and Data Types - Apple Developer](https://developer.apple.com/documentation/uniformtypeidentifiers/defining-file-and-data-types-for-your-app)
- [How macOS Recognises File Types - Eclectic Light](https://eclecticlight.co/2025/10/25/explainer-how-does-macos-recognise-file-types/)
- [Modern Quick Look Extensions List](https://github.com/Oil3/List-of-modern-Quick-Look-extensions)
- [Uniform Type Identifiers Tech Talk - Apple](https://developer.apple.com/videos/play/tech-talks/10696/)
- [Batch Process Images with Preview - MacMost](https://macmost.com/how-to-batch-process-images-with-mac-preview.html)
- [How to Change Default Apps on Mac](https://www.simplymac.com/macos/how-to-change-default-open-with-mac)
