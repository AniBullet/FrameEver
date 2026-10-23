import io
import struct
import sys
from pathlib import Path

from PIL import Image, ImageDraw, ImageFont

PLATE = "#1F3FD8"
CLIP_TOP = "#FFFDED"
CLIP_BOTTOM = "#87BFFF"
RECORD_RING = "#FFFFFF"
RECORD_GAP = "#000F5C"
RECORD_DOT = "#FF3B30"
INK = "#1A1B24"
INK_SECONDARY = "#5D5E68"
SURFACE = "#F0EFFF"

GRID = 48
PLATE_RADIUS = GRID * 0.225
CLIP_HEIGHT = 7
CLIP_TOP_BOX = (9, 25, 39, 25 + CLIP_HEIGHT)
CLIP_BOTTOM_BOX = (9, 34, 26, 34 + CLIP_HEIGHT)
RECORD_CENTRE = (30.5, 13.5)
RECORD_RADIUS = 9.5
RECORD_GAP_RADIUS = RECORD_RADIUS * 0.8
RECORD_DOT_RADIUS = RECORD_RADIUS * 0.64
RECORD_DOT_RADIUS_SMALL = RECORD_RADIUS * 0.66
GAP_MIN_SIZE = 24

ICO_SIZES = [16, 20, 24, 30, 32, 36, 40, 48, 60, 64, 72, 80, 96, 128, 256]
SUPERSAMPLE = 8


def rgb(hex_):
    hex_ = hex_.lstrip("#")
    return tuple(int(hex_[i:i + 2], 16) for i in (0, 2, 4))


def draw_marks(draw, unit, size, origin=(0, 0)):
    ox, oy = origin

    def at(value, offset):
        scaled = offset + value * unit
        if size <= 48:
            return round(scaled / SUPERSAMPLE) * SUPERSAMPLE
        return scaled

    for box, colour in ((CLIP_TOP_BOX, CLIP_TOP), (CLIP_BOTTOM_BOX, CLIP_BOTTOM)):
        x0, y0, x1, y1 = at(box[0], ox), at(box[1], oy), at(box[2], ox), at(box[3], oy)
        draw.rounded_rectangle((x0, y0, x1, y1), radius=(y1 - y0) / 2, fill=rgb(colour))
    cx, cy = ox + RECORD_CENTRE[0] * unit, oy + RECORD_CENTRE[1] * unit

    def disc(radius, colour):
        r = radius * unit
        draw.ellipse((cx - r, cy - r, cx + r, cy + r), fill=rgb(colour))

    disc(RECORD_RADIUS, RECORD_RING)
    if size >= GAP_MIN_SIZE:
        disc(RECORD_GAP_RADIUS, RECORD_GAP)
        disc(RECORD_DOT_RADIUS, RECORD_DOT)
    else:
        disc(RECORD_DOT_RADIUS_SMALL, RECORD_DOT)


PIXEL_FRAMES = {
    16: {"ring": (8, 2, 6), "gap": None, "dot": (9, 3, 4), "top": (3, 9, 12, 10), "bottom": (3, 12, 8, 13)},
    20: {"ring": (10, 2, 8), "gap": None, "dot": (11, 3, 6), "top": (4, 11, 15, 13), "bottom": (4, 15, 10, 17)},
    24: {"ring": (12, 2, 10), "gap": (13, 3, 8), "dot": (14, 4, 6), "top": (5, 14, 18, 16), "bottom": (5, 18, 12, 20)},
}


def pixel_disc(pixels, x0, y0, n, colour, samples=8):
    centre = n / 2
    target = rgb(colour)
    for j in range(n):
        for i in range(n):
            hits = sum(
                (i + (a + 0.5) / samples - centre) ** 2 + (j + (b + 0.5) / samples - centre) ** 2 <= centre ** 2
                for a in range(samples)
                for b in range(samples)
            )
            coverage = hits / samples ** 2
            if coverage == 0:
                continue
            under = pixels[x0 + i, y0 + j]
            mixed = tuple(round(target[k] * coverage + under[k] * (1 - coverage)) for k in range(3))
            pixels[x0 + i, y0 + j] = (*mixed, max(under[3], round(255 * coverage)))


def pixel_bar(pixels, x0, y0, x1, y1, colour):
    for y in range(y0, y1 + 1):
        for x in range(x0, x1 + 1):
            pixels[x, y] = (*rgb(colour), 255)


def render_pixel_icon(size):
    spec = PIXEL_FRAMES[size]
    canvas = size * SUPERSAMPLE
    plate = Image.new("RGBA", (canvas, canvas), (0, 0, 0, 0))
    ImageDraw.Draw(plate).rounded_rectangle(
        (0, 0, canvas - 1, canvas - 1), radius=PLATE_RADIUS * canvas / GRID, fill=rgb(PLATE)
    )
    image = plate.resize((size, size), Image.LANCZOS)
    pixels = image.load()
    pixel_bar(pixels, *spec["top"], CLIP_TOP)
    pixel_bar(pixels, *spec["bottom"], CLIP_BOTTOM)
    pixel_disc(pixels, *spec["ring"], RECORD_RING)
    if spec["gap"]:
        pixel_disc(pixels, *spec["gap"], RECORD_GAP)
    pixel_disc(pixels, *spec["dot"], RECORD_DOT)
    return image


def render_icon(size):
    if size in PIXEL_FRAMES:
        return render_pixel_icon(size)
    canvas = size * SUPERSAMPLE
    unit = canvas / GRID
    image = Image.new("RGBA", (canvas, canvas), (0, 0, 0, 0))
    draw = ImageDraw.Draw(image)
    draw.rounded_rectangle((0, 0, canvas - 1, canvas - 1), radius=PLATE_RADIUS * unit, fill=rgb(PLATE))
    draw_marks(draw, unit, size)
    return image.resize((size, size), Image.LANCZOS)


def write_ico(path, sizes):
    frames = []
    for size in sizes:
        buffer = io.BytesIO()
        render_icon(size).save(buffer, format="PNG")
        frames.append((size, buffer.getvalue()))
    header = struct.pack("<HHH", 0, 1, len(frames))
    offset = 6 + 16 * len(frames)
    entries, data = b"", b""
    for size, png in frames:
        side = 0 if size >= 256 else size
        entries += struct.pack("<BBBBHHII", side, side, 0, 0, 1, 32, len(png), offset + len(data))
        data += png
    Path(path).write_bytes(header + entries + data)


def svg_shapes():
    x0, y0, x1, y1 = CLIP_TOP_BOX
    b0, c0, b1, c1 = CLIP_BOTTOM_BOX
    cx, cy = RECORD_CENTRE
    return {
        "tile": f'<rect width="{GRID}" height="{GRID}" rx="{PLATE_RADIUS}"/>',
        "clip-top": f'<rect x="{x0}" y="{y0}" width="{x1 - x0}" height="{y1 - y0}" rx="{CLIP_HEIGHT / 2}"/>',
        "clip-bottom": f'<rect x="{b0}" y="{c0}" width="{b1 - b0}" height="{c1 - c0}" rx="{CLIP_HEIGHT / 2}"/>',
        "record": f'<circle cx="{cx}" cy="{cy}" r="{RECORD_RADIUS}"/>',
        "record-gap": f'<circle cx="{cx}" cy="{cy}" r="{RECORD_GAP_RADIUS}"/>',
        "record-dot": f'<circle cx="{cx}" cy="{cy}" r="{RECORD_DOT_RADIUS}"/>',
    }


LAYERS = [
    ("tile", PLATE),
    ("clip-top", CLIP_TOP),
    ("clip-bottom", CLIP_BOTTOM),
    ("record", RECORD_RING),
    ("record-gap", RECORD_GAP),
    ("record-dot", RECORD_DOT),
]


def write_svgs(app):
    shapes = svg_shapes()
    lockup_scale = 40 / GRID
    head = '<svg xmlns="http://www.w3.org/2000/svg" width="180" height="40" viewBox="0 0 180 40">'
    for name, _ in LAYERS:
        body = f'<g transform="scale({lockup_scale})">{shapes[name]}</g>'
        (app / "assets" / "icons" / f"logo-{name}.svg").write_text(head + body + "</svg>", encoding="utf-8")
    groups = "".join(f'<g fill="{colour}">{shapes[name]}</g>' for name, colour in LAYERS)
    source = (
        f'<svg xmlns="http://www.w3.org/2000/svg" width="1024" height="1024" viewBox="0 0 {GRID} {GRID}">'
        f"{groups}</svg>"
    )
    (app / "packaging" / "brand" / "icon-source.svg").write_text(source, encoding="utf-8")


def font(app, name, size):
    return ImageFont.truetype(str(app / "assets" / "fonts" / name), size)


def text(image, xy, value, face, colour, alpha=255):
    layer = Image.new("RGBA", image.size, (0, 0, 0, 0))
    ImageDraw.Draw(layer).text(xy, value, font=face, fill=(*rgb(colour), alpha))
    image.alpha_composite(layer)


def write_installer_images(app, scale=2):
    out = app / "packaging" / "windows"
    w, h = 164 * scale, 314 * scale
    left = 20 * scale
    side = Image.new("RGBA", (w, h), (*rgb(SURFACE), 255))
    side.alpha_composite(render_icon(40 * scale), (left, 24 * scale))
    text(side, (left, 196 * scale), "FrameEver", font(app, "Inter-SemiBold.ttf", 19 * scale), INK)
    text(side, (left, 222 * scale), "帧刻", font(app, "misans/MiSans-Medium.ttf", 11 * scale), INK, 220)
    ImageDraw.Draw(side).rectangle((left, 246 * scale, left + 16 * scale, 247 * scale), fill=rgb(PLATE))
    text(side, (left, 254 * scale), "一刻，凝成永恒", font(app, "misans/MiSans-Regular.ttf", 9 * scale), INK_SECONDARY)
    text(side, (left, 290 * scale), "by aniBullet", font(app, "Inter-Regular.ttf", 7 * scale), INK_SECONDARY, 200)
    side.convert("RGB").save(out / "nsis-sidebar.bmp")

    hw, hh = 150 * scale, 57 * scale
    header = Image.new("RGBA", (hw, hh), (255, 255, 255, 255))
    icon = render_icon(36 * scale)
    header.alpha_composite(icon, (hw - icon.width - 10 * scale, (hh - icon.height) // 2))
    header.convert("RGB").save(out / "nsis-header.bmp")


def main(app_dir):
    app = Path(app_dir)
    write_ico(app / "icons" / "icon.ico", ICO_SIZES)
    for name, size in [("32x32.png", 32), ("64x64.png", 64), ("128x128.png", 128),
                       ("128x128@2x.png", 256), ("icon.png", 512)]:
        render_icon(size).save(app / "icons" / name)
    render_icon(1024).save(app / "packaging" / "brand" / "icon-source.png")
    render_icon(512).save(app / "assets" / "dock-icon.png")
    write_svgs(app)
    write_installer_images(app)


if __name__ == "__main__":
    main(sys.argv[1])
