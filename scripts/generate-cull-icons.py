#!/usr/bin/env python3
import binascii
import json
import shutil
import struct
import subprocess
import tempfile
import zlib
from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]
TAURI_ICONS = ROOT / "src-tauri" / "icons"
TAHOE_DIR = ROOT / "design" / "icons" / "tahoe"
TAHOE_MASTERS = TAHOE_DIR / "masters-unmasked"
TAHOE_MASKED = TAHOE_DIR / "previews-masked"
TAHOE_LAYERS = TAHOE_DIR / "icon-composer-layers"
TAHOE_DOCUMENTS = TAHOE_DIR / "icon-composer-documents"
TAHOE_RENDERS = TAHOE_DIR / "icon-composer-renders"
TAHOE_ICNS = TAHOE_DIR / "icns"
VARIANT_DIRS = [
    TAHOE_MASTERS,
    TAURI_ICONS / "variants",
    ROOT / "static" / "icon-variants",
]

VARIANTS = {
    "primary": {
        "label": "Primary Mono",
        "background": (246, 246, 242),
        "mark": (10, 10, 10),
    },
    "red": {
        "label": "Signal Red",
        "background": (225, 0, 0),
        "mark": (246, 246, 242),
    },
    "blue": {
        "label": "Bauhaus Blue",
        "background": (0, 94, 255),
        "mark": (246, 246, 242),
    },
    "dark": {
        "label": "Dark Mono",
        "background": (10, 10, 10),
        "mark": (246, 246, 242),
    },
    "yellow": {
        "label": "Archive Yellow",
        "background": (255, 212, 0),
        "mark": (10, 10, 10),
    },
}

BUNDLE_SIZES = {
    "32x32.png": 32,
    "64x64.png": 64,
    "128x128.png": 128,
    "128x128@2x.png": 256,
    "256x256.png": 256,
    "512x512.png": 512,
    "icon.png": 1024,
}

WINDOWS_SIZES = {
    "Square30x30Logo.png": 30,
    "Square44x44Logo.png": 44,
    "Square71x71Logo.png": 71,
    "Square89x89Logo.png": 89,
    "Square107x107Logo.png": 107,
    "Square142x142Logo.png": 142,
    "Square150x150Logo.png": 150,
    "Square284x284Logo.png": 284,
    "Square310x310Logo.png": 310,
    "StoreLogo.png": 50,
}


def smoothstep(edge0, edge1, x):
    if edge0 == edge1:
        return 1.0 if x >= edge1 else 0.0
    t = min(1.0, max(0.0, (x - edge0) / (edge1 - edge0)))
    return t * t * (3.0 - 2.0 * t)


def rect_alpha(x, y, left, top, right, bottom, aa):
    return (
        smoothstep(left - aa, left + aa, x)
        * (1.0 - smoothstep(right - aa, right + aa, x))
        * smoothstep(top - aa, top + aa, y)
        * (1.0 - smoothstep(bottom - aa, bottom + aa, y))
    )


def rounded_rect_alpha(x, y, left, top, right, bottom, radius, aa):
    cx = (left + right) / 2.0
    cy = (top + bottom) / 2.0
    half_w = (right - left) / 2.0 - radius
    half_h = (bottom - top) / 2.0 - radius
    dx = abs(x - cx) - half_w
    dy = abs(y - cy) - half_h
    outside_x = max(dx, 0.0)
    outside_y = max(dy, 0.0)
    outside = (outside_x * outside_x + outside_y * outside_y) ** 0.5
    inside = min(max(dx, dy), 0.0)
    signed_distance = outside + inside - radius
    return 1.0 - smoothstep(-aa, aa, signed_distance)


def mark_alpha(x, y, size):
    scale = size / 1024.0
    aa = max(0.75, 1.45 * scale)

    cx = 410.0 * scale
    cy = 512.0 * scale
    outer = 240.0 * scale
    inner = 128.0 * scale
    d = ((x - cx) ** 2 + (y - cy) ** 2) ** 0.5

    ring = (
        (1.0 - smoothstep(outer - aa, outer + aa, d))
        * smoothstep(inner - aa, inner + aa, d)
    )
    notch = rect_alpha(
        x,
        y,
        (cx + 98.0 * scale),
        (cy - 62.0 * scale),
        (cx + outer + 20.0 * scale),
        (cy + 62.0 * scale),
        aa,
    )
    c_shape = ring * (1.0 - notch)

    bar_top = 274.0 * scale
    bar_bottom = 712.0 * scale
    bar_1 = rect_alpha(
        x,
        y,
        672.0 * scale,
        bar_top,
        748.0 * scale,
        bar_bottom,
        aa,
    )
    bar_2 = rect_alpha(
        x,
        y,
        776.0 * scale,
        bar_top,
        852.0 * scale,
        bar_bottom,
        aa,
    )

    return min(1.0, max(c_shape, bar_1, bar_2))


def render_icon(size, background, mark):
    pixels = bytearray(size * size * 4)
    for y in range(size):
        for x in range(size):
            alpha = mark_alpha(x + 0.5, y + 0.5, size)
            rgb = tuple(
                int(round(background[i] * (1.0 - alpha) + mark[i] * alpha))
                for i in range(3)
            )
            offset = (y * size + x) * 4
            pixels[offset : offset + 4] = bytes((rgb[0], rgb[1], rgb[2], 255))
    return bytes(pixels)


def render_background(size, background):
    return bytes((background[0], background[1], background[2], 255)) * (size * size)


def render_foreground(size, mark):
    pixels = bytearray(size * size * 4)
    for y in range(size):
        for x in range(size):
            alpha = int(round(mark_alpha(x + 0.5, y + 0.5, size) * 255.0))
            offset = (y * size + x) * 4
            pixels[offset : offset + 4] = bytes((mark[0], mark[1], mark[2], alpha))
    return bytes(pixels)


def tahoe_mask_alpha(x, y, size):
    center = size / 2.0
    half = size * 0.403
    dx = abs(x - center) / half
    dy = abs(y - center) / half
    distance = (dx**4.6 + dy**4.6) ** (1.0 / 4.6)
    aa = 1.8 / half
    return 1.0 - smoothstep(1.0 - aa, 1.0 + aa, distance)


def render_masked_preview(size, background, mark):
    pixels = bytearray(render_icon(size, background, mark))
    for y in range(size):
        for x in range(size):
            offset = (y * size + x) * 4
            pixels[offset + 3] = int(round(tahoe_mask_alpha(x + 0.5, y + 0.5, size) * 255.0))
    return bytes(pixels)


def png_chunk(kind, data):
    return (
        struct.pack(">I", len(data))
        + kind
        + data
        + struct.pack(">I", binascii.crc32(kind + data) & 0xFFFFFFFF)
    )


def encode_png(size, rgba):
    rows = []
    stride = size * 4
    for y in range(size):
        rows.append(b"\x00" + rgba[y * stride : (y + 1) * stride])
    raw = b"".join(rows)
    return b"".join(
        [
            b"\x89PNG\r\n\x1a\n",
            png_chunk(b"IHDR", struct.pack(">IIBBBBB", size, size, 8, 6, 0, 0, 0)),
            png_chunk(b"IDAT", zlib.compress(raw, 9)),
            png_chunk(b"IEND", b""),
        ]
    )


def write_png(path, size, background, mark):
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_bytes(encode_png(size, render_icon(size, background, mark)))


def write_png_pixels(path, size, rgba):
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_bytes(encode_png(size, rgba))


def icon_composer_tool():
    candidates = [
        Path("/Applications/Xcode.app/Contents/Applications/Icon Composer.app/Contents/Executables/ictool"),
        Path("/Applications/Icon Composer.app/Contents/Executables/ictool"),
    ]
    for candidate in candidates:
        if candidate.exists():
            return str(candidate)
    return shutil.which("ictool")


def normalize_png_depth(path):
    magick = shutil.which("magick")
    if not magick:
        return

    with tempfile.NamedTemporaryFile(suffix=".png") as tmp:
        subprocess.run(
            [magick, str(path), "-depth", "8", tmp.name],
            check=True,
            stdout=subprocess.DEVNULL,
            stderr=subprocess.DEVNULL,
        )
        path.write_bytes(Path(tmp.name).read_bytes())


def write_icon_composer_document(name, variant):
    document = TAHOE_DOCUMENTS / f"cull-{name}.icon"
    asset_dir = document / "Assets"
    asset_dir.mkdir(parents=True, exist_ok=True)

    write_png_pixels(
        asset_dir / "background",
        1024,
        render_background(1024, variant["background"]),
    )
    write_png_pixels(
        asset_dir / "foreground",
        1024,
        render_foreground(1024, variant["mark"]),
    )
    (document / "icon.json").write_text(
        json.dumps(
            {
                "groups": [
                    {
                        "layers": [
                            {"image-name": "foreground"},
                            {"image-name": "background"},
                        ],
                    }
                ],
            },
            indent=2,
        )
        + "\n"
    )
    return document


def export_icon_composer_image(document, output, size):
    ictool = icon_composer_tool()
    if not ictool:
        return False

    output.parent.mkdir(parents=True, exist_ok=True)
    subprocess.run(
        [
            ictool,
            str(document),
            "--export-image",
            "--output-file",
            str(output),
            "--platform",
            "macOS",
            "--rendition",
            "Default",
            "--width",
            str(size),
            "--height",
            str(size),
            "--scale",
            "1",
        ],
        check=True,
        stdout=subprocess.DEVNULL,
    )
    normalize_png_depth(output)
    return True


def write_ico(path, pngs):
    path.parent.mkdir(parents=True, exist_ok=True)
    count = len(pngs)
    header = struct.pack("<HHH", 0, 1, count)
    directory = bytearray()
    payload = bytearray()
    offset = 6 + count * 16
    for size, data in pngs:
        directory.extend(
            struct.pack(
                "<BBBBHHII",
                0 if size >= 256 else size,
                0 if size >= 256 else size,
                0,
                0,
                1,
                32,
                len(data),
                offset,
            )
        )
        payload.extend(data)
        offset += len(data)
    path.write_bytes(header + directory + payload)


def generate_variant_icons():
    for name, variant in VARIANTS.items():
        for directory in VARIANT_DIRS:
            write_png(
                directory / f"cull-{name}.png",
                1024,
                variant["background"],
                variant["mark"],
            )
        layer_dir = TAHOE_LAYERS / name
        write_png_pixels(
            layer_dir / "background.png",
            1024,
            render_background(1024, variant["background"]),
        )
        write_png_pixels(
            layer_dir / "foreground.png",
            1024,
            render_foreground(1024, variant["mark"]),
        )
        write_png_pixels(
            TAHOE_MASKED / f"cull-{name}-masked.png",
            1024,
            render_masked_preview(1024, variant["background"], variant["mark"]),
        )


def generate_tahoe_icon_documents():
    for name, variant in VARIANTS.items():
        document = write_icon_composer_document(name, variant)
        render_path = TAHOE_RENDERS / f"cull-{name}.png"
        if export_icon_composer_image(document, render_path, 1024):
            for directory in (TAURI_ICONS / "variants", ROOT / "static" / "icon-variants"):
                directory.mkdir(parents=True, exist_ok=True)
                shutil.copyfile(render_path, directory / f"cull-{name}.png")
            shutil.copyfile(render_path, TAHOE_MASKED / f"cull-{name}-masked.png")


def generate_icns_variant(name, variant):
    iconutil = shutil.which("iconutil")
    if not iconutil:
        return

    TAHOE_ICNS.mkdir(parents=True, exist_ok=True)
    write_icns(
        TAHOE_ICNS / f"cull-{name}.icns",
        variant,
        TAHOE_DOCUMENTS / f"cull-{name}.icon",
    )


def write_icns(path, variant, document=None):
    iconutil = shutil.which("iconutil")
    if not iconutil:
        return

    files = {
        "icon_16x16.png": 16,
        "icon_16x16@2x.png": 32,
        "icon_32x32.png": 32,
        "icon_32x32@2x.png": 64,
        "icon_128x128.png": 128,
        "icon_128x128@2x.png": 256,
        "icon_256x256.png": 256,
        "icon_256x256@2x.png": 512,
        "icon_512x512.png": 512,
        "icon_512x512@2x.png": 1024,
    }
    with tempfile.TemporaryDirectory(suffix=".iconset") as tmp:
        iconset = Path(tmp)
        for filename, size in files.items():
            output = iconset / filename
            if not document or not export_icon_composer_image(document, output, size):
                write_png(output, size, variant["background"], variant["mark"])
        subprocess.run(
            [iconutil, "-c", "icns", str(iconset), "-o", str(path)],
            check=True,
        )


def generate_tahoe_icns():
    for name, variant in VARIANTS.items():
        generate_icns_variant(name, variant)


def generate_bundle_icons():
    primary = VARIANTS["primary"]
    for filename, size in BUNDLE_SIZES.items():
        output = TAURI_ICONS / filename
        if filename != "icon.png" or not export_icon_composer_image(
            TAHOE_DOCUMENTS / "cull-primary.icon",
            output,
            size,
        ):
            write_png(output, size, primary["background"], primary["mark"])
    for filename, size in WINDOWS_SIZES.items():
        write_png(TAURI_ICONS / filename, size, primary["background"], primary["mark"])

    ico_pngs = []
    for size in (16, 24, 32, 48, 64, 128, 256):
        data = encode_png(size, render_icon(size, primary["background"], primary["mark"]))
        ico_pngs.append((size, data))
    write_ico(TAURI_ICONS / "icon.ico", ico_pngs)
    write_icns(TAURI_ICONS / "icon.icns", primary, TAHOE_DOCUMENTS / "cull-primary.icon")


def main():
    generate_variant_icons()
    generate_tahoe_icon_documents()
    generate_tahoe_icns()
    generate_bundle_icons()


if __name__ == "__main__":
    main()
