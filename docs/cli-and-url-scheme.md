# CLI and URL Scheme Specification

Version: 0.1.0 — Draft, 2026-05-07

ImageView ships a single `imageview` binary. With no subcommand it launches the GUI. With a subcommand it runs headless and exits. Every operation available in the GUI has a CLI equivalent and a URL scheme equivalent.

---

## CLI

### Invocation

```
imageview [FLAGS] [SUBCOMMAND]
```

With no subcommand, launches the GUI. A bare path argument is shorthand for `open`.

```bash
imageview                        # launch GUI
imageview ~/photos               # open folder in GUI (same as: imageview open ~/photos)
imageview shot.png               # open file in GUI
```

### Global Flags

| Flag | Short | Description |
|------|-------|-------------|
| `--json` | `-j` | Emit output as JSON (default for `metadata`, `detect`) |
| `--quiet` | `-q` | Suppress non-error output |
| `--verbose` | `-v` | Verbose logging to stderr |
| `--db <path>` | | Use a specific SQLite database instead of the default |
| `--no-gui` | | Force headless mode (skip window creation even for `open`) |

### Subcommands

#### `open` — Open in viewer

```bash
imageview open <path|folder> [--view grid|loupe|compare|embeddings] [--fullscreen]
```

Opens the GUI with the given path focused. If the path is a folder, imports and displays its contents.

| Option | Default | Description |
|--------|---------|-------------|
| `--view` | `grid` for folders, `loupe` for files | Initial view mode |
| `--fullscreen` | off | Launch in fullscreen |
| `--focus <index>` | `0` | Focus Nth image |
| `--size <px>` | `160` | Thumbnail size in grid |
| `--zoom <percent>` | `100` | Loupe zoom level |

#### `import` — Import to library

```bash
imageview import <folder> [--recursive] [--dry-run]
```

Scans a folder for images, generates thumbnails, adds to the database. Prints count of imported files.

| Option | Default | Description |
|--------|---------|-------------|
| `--recursive` | on | Recurse into subdirectories |
| `--no-recursive` | | Only top-level files |
| `--dry-run` | off | List files that would be imported without importing |

#### `search` — Semantic text search

```bash
imageview search <query> [--top N] [--threshold FLOAT]
```

Search the library using CLIP text-to-image similarity. Requires embeddings to have been generated.

```bash
imageview search "sunset over mountains" --top 10
# /Users/me/photos/DSC_4021.jpg  0.34
# /Users/me/photos/DSC_4055.jpg  0.31
# ...
```

| Option | Default | Description |
|--------|---------|-------------|
| `--top` | `20` | Max results |
| `--threshold` | `0.0` | Minimum similarity score |

Output: one line per result — `<path>\t<score>`. With `--json`: array of `{"path", "score"}`.

#### `similar` — Find visually similar images

```bash
imageview similar <image-path> [--top N] [--threshold FLOAT]
```

Find images in the library visually similar to the given image (cosine similarity on CLIP embeddings).

Same output format as `search`.

#### `contact-sheet` — Generate contact sheet

```bash
imageview contact-sheet <folder|collection-name> [OPTIONS] --output <path.png>
```

Renders a grid of thumbnails as a single image. The flagship automation command.

| Option | Default | Description |
|--------|---------|-------------|
| `--columns` | `6` | Number of columns |
| `--size` | `200x200` | Thumbnail cell size (WxH) |
| `--gap` | `8` | Gap between cells in px |
| `--output` | required | Output file path (.png, .jpg, .webp) |
| `--labels` | `filename` | Label below each thumbnail: `filename`, `metadata`, `rating`, `none` |
| `--sort` | `name` | Sort order: `name`, `date`, `rating`, `size` |
| `--filter-stars` | | Minimum star rating to include |
| `--filter-status` | | `accepted`, `rejected`, `undecided` |
| `--bg` | `#1a1a1a` | Background color |
| `--font-size` | `11` | Label font size |
| `--title` | | Optional title rendered at top |

```bash
imageview contact-sheet ./shoot-042 --columns 8 --size 150x150 --labels metadata --output proof.png
imageview contact-sheet favorites --filter-stars 4 --output best-of.jpg
```

#### `export` — Batch export

```bash
imageview export <folder|collection> [OPTIONS] --output <dir>
```

Export images with optional format conversion and resizing.

| Option | Default | Description |
|--------|---------|-------------|
| `--format` | original | `png`, `jpg`, `webp`, `avif`, `tiff` |
| `--quality` | `90` | JPEG/WebP quality (1-100) |
| `--resize` | | `WxH`, `Wx0` (preserve aspect), `0xH` |
| `--output` | required | Output directory |
| `--naming` | `{original}` | Naming template: `{original}`, `{n}`, `{date}`, `{rating}` |
| `--flatten` | off | Ignore subfolder structure |

#### `resize` — Batch resize

```bash
imageview resize <path|glob> --width <W> [--height <H>] [--output <dir>]
```

Resize images. If only width or height is given, aspect ratio is preserved.

| Option | Default | Description |
|--------|---------|-------------|
| `--width` | | Target width in pixels |
| `--height` | | Target height in pixels |
| `--output` | in-place | Output directory; omit to overwrite originals |
| `--filter` | `lanczos3` | Resampling filter: `nearest`, `bilinear`, `lanczos3` |

#### `convert` — Batch format conversion

```bash
imageview convert <path|glob> --format <fmt> [--quality <Q>] [--output <dir>]
```

Convert image format. Supports all formats the app can read.

#### `metadata` — Read metadata

```bash
imageview metadata <path> [--format json|yaml|table]
```

Dump EXIF, IPTC, XMP as structured data. Default output is JSON.

```bash
imageview metadata DSC_4021.jpg
# {"camera": "Nikon Z6III", "lens": "24-70mm f/2.8", "focal_length": "35mm", ...}
```

#### `rate` — Set star rating

```bash
imageview rate <path|glob> --stars <0-5>
```

Set star rating in the library database. `--stars 0` clears the rating.

#### `accept` / `reject` / `undecide` — Set curation status

```bash
imageview accept <path|glob>
imageview reject <path|glob>
imageview undecide <path|glob>
```

#### `collection` — Manage collections

```bash
imageview collection create <name>
imageview collection add <name> <path|glob...>
imageview collection remove <name> <path|glob...>
imageview collection list [name]
imageview collection delete <name>
imageview collection export <name> --output <dir> [--format ...] [--resize ...]
```

`collection list` with no name lists all collections. With a name, lists images in that collection.

#### `detect` — Object detection

```bash
imageview detect <path|glob> [--model yolo|florence] [--threshold FLOAT]
```

Run object detection. Output: JSON array of `{"path", "objects": [{"label", "confidence", "bbox"}]}`.

#### `embed` — Generate embeddings

```bash
imageview embed <path|folder> [--provider clip|gemini|dinov2] [--force]
```

Generate or regenerate vector embeddings for images.

| Option | Default | Description |
|--------|---------|-------------|
| `--provider` | `clip` | Embedding model |
| `--force` | off | Regenerate even if embeddings exist |

#### `serve` — MCP server

```bash
imageview serve [--transport stdio|sse] [--port PORT]
```

Start the MCP (Model Context Protocol) server. Default transport is `stdio` for agent piping.

#### `pipe` — Stdin/stdout pipeline

```bash
imageview pipe [ACTION-FLAGS]
```

Read file paths from stdin (one per line), apply the specified action, write results to stdout. Composable with Unix tools.

```bash
find . -name "*.png" | imageview pipe --resize 800x0 --format webp --output ./web/
cat paths.txt | imageview pipe --rate --stars 5
find . -name "*.jpg" | imageview pipe --detect --json > detections.json
```

Action flags mirror the subcommands: `--resize WxH`, `--format fmt`, `--detect`, `--embed`, `--metadata`, `--rate --stars N`, `--accept`, `--reject`.

---

## URL Scheme

Scheme: `imageview://`

The host portion is the action verb. Parameters are query string key-value pairs. Every CLI subcommand maps 1:1 to a URL scheme action.

### Mapping

| CLI | URL Scheme |
|-----|-----------|
| `imageview open ~/photo.jpg --view loupe` | `imageview://open?path=~/photo.jpg&view=loupe` |
| `imageview import ~/photos --recursive` | `imageview://import?folder=~/photos&recursive=true` |
| `imageview search "sunset"` | `imageview://search?q=sunset` |
| `imageview similar ~/ref.jpg --top 5` | `imageview://similar?path=~/ref.jpg&top=5` |
| `imageview contact-sheet ~/folder -o out.png` | `imageview://contact-sheet?folder=~/folder&output=out.png` |
| `imageview export faves --format webp` | `imageview://export?collection=faves&format=webp` |
| `imageview rate ~/img.jpg --stars 4` | `imageview://rate?path=~/img.jpg&stars=4` |
| `imageview accept ~/img.jpg` | `imageview://accept?path=~/img.jpg` |
| `imageview collection create picks` | `imageview://collection/create?name=picks` |
| `imageview collection add picks ~/a.jpg` | `imageview://collection/add?name=picks&paths=~/a.jpg` |
| `imageview detect ~/img.jpg` | `imageview://detect?path=~/img.jpg` |
| `imageview embed ~/photos` | `imageview://embed?folder=~/photos` |

### URL Parameters

Paths must be URL-encoded. Multiple paths use comma-separated values: `paths=a.jpg,b.jpg,c.jpg`.

Headless actions (`contact-sheet`, `export`, `resize`, `convert`) triggered via URL scheme run in the background and produce no GUI unless `&gui=true` is appended.

GUI actions (`open`, `search`, `similar`) bring the window to front. If the app is not running, macOS launches it.

### Response

URL scheme calls are fire-and-forget from the caller's perspective. For programmatic use requiring return values, use the CLI or MCP server.

---

## Automation Patterns

### 1. Agent workflow (Claude Code)

An agent curating AI-generated images:

```bash
# Import a batch of generated images
imageview import ~/generations/run-042 --recursive

# Find the best sunset images
imageview search "dramatic sunset, warm colors" --top 20 --json | jq -r '.[].path'

# Auto-rate based on detection results
for img in ~/generations/run-042/*.png; do
    objects=$(imageview detect "$img" --json | jq '.objects | length')
    if [ "$objects" -gt 0 ]; then
        imageview rate "$img" --stars 3
    fi
done

# Create a proof sheet for review
imageview contact-sheet ~/generations/run-042 \
    --columns 8 --filter-stars 3 --output ~/proofs/run-042.png
```

### 2. Shell pipeline — bulk convert

```bash
# Convert all PNGs in a tree to WebP at 80% quality, preserving structure
find ~/assets -name "*.png" | imageview pipe --format webp --quality 80 --output ~/assets-web/

# Resize for social media
find . -name "*.jpg" | imageview pipe --resize 1200x0 --output ./social/
```

### 3. Contact sheet from folder

```bash
# Quick proof sheet
imageview contact-sheet ./photos --columns 6 --output overview.png

# Film strip (single row, wide)
imageview contact-sheet ./frames --columns 24 --size 120x80 --gap 2 --output strip.png

# Curated best-of with metadata labels
imageview contact-sheet my-collection \
    --filter-stars 4 \
    --labels metadata \
    --title "Best of 2026" \
    --output best-of-2026.png
```

### 4. Shortcuts.app integration

Create a macOS Shortcut that accepts image files from the share sheet:

1. **Receive input**: Files (images)
2. **Run Shell Script**: `imageview import "$@" && imageview contact-sheet imported --output ~/Desktop/sheet.png`
3. **Open File**: `~/Desktop/sheet.png`

Or use URL scheme directly via "Open URL" action:

```
imageview://import?folder=/path/to/files
imageview://contact-sheet?folder=/path/to/files&columns=4&output=~/Desktop/sheet.png
```

### 5. AppleScript

```applescript
-- Open a folder in ImageView
do shell script "/Applications/ImageView.app/Contents/MacOS/imageview open ~/photos --view grid"

-- Generate a contact sheet
do shell script "/Applications/ImageView.app/Contents/MacOS/imageview contact-sheet ~/photos --columns 5 --output /tmp/proof.png"

-- Get metadata
set meta to do shell script "/Applications/ImageView.app/Contents/MacOS/imageview metadata ~/photos/DSC_001.jpg"
```

### 6. MCP server for agents

```bash
# Start MCP server on stdio (for Claude Code, Cursor, etc.)
imageview serve

# The MCP server exposes all CLI operations as tools:
# - imageview_import(folder, recursive)
# - imageview_search(query, top)
# - imageview_similar(path, top)
# - imageview_contact_sheet(source, columns, output, ...)
# - imageview_rate(path, stars)
# - imageview_detect(path)
# - imageview_collection_*(...)
# - imageview_export(...)
```

### 7. Watch folder (combine with fswatch)

```bash
# Auto-import new images dropped into a folder
fswatch -0 ~/incoming-images | while IFS= read -r -d '' file; do
    imageview import "$file"
    imageview embed "$file" --provider clip
    imageview detect "$file"
done
```

---

## Design Principles

1. **CLI = GUI = URL = MCP**: Every operation has exactly one canonical implementation in Rust. The CLI, URL scheme, GUI actions, and MCP tools are thin wrappers that call the same functions.

2. **Composable over complete**: Small commands that chain well. `pipe` is the glue for Unix-style composition.

3. **JSON by default for machines**: `--json` flag for structured output. Human-readable table format is the default for interactive use.

4. **Headless by default for subcommands**: Running `imageview contact-sheet` does not open a window. Running `imageview open` does.

5. **Paths are first-class**: Commands accept individual files, globs, folders, and collection names. The app resolves what you mean.

6. **Non-destructive by default**: `resize` and `convert` without `--output` write to a new directory, not in-place. Explicit `--in-place` flag to overwrite.
