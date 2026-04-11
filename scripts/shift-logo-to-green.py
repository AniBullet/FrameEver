import os
import math
from PIL import Image

ICONS_DIR = os.path.join("apps", "desktop", "src-tauri", "icons")

SKIP_PREFIXES = ("tray-",)

OUTER_BLUE = (71, 133, 255)
INNER_BLUE = (173, 201, 255)

OUTER_GREEN = (0x35, 0xC8, 0xB4)
INNER_GREEN = (0x9A, 0xE3, 0xD9)

MAX_DISTANCE = 80


def rgb_distance(c1, c2):
    return math.sqrt(sum((a - b) ** 2 for a, b in zip(c1, c2)))


def interpolate_color(original, src_anchor, dst_anchor, distance, max_dist):
    blend = 1.0 - (distance / max_dist)
    blend = max(0.0, min(1.0, blend))

    result = []
    for o, s, d in zip(original, src_anchor, dst_anchor):
        shift = d - s
        result.append(int(max(0, min(255, o + shift * blend))))
    return tuple(result)


def shift_pixel(r, g, b, a):
    if a == 0:
        return r, g, b, a

    pixel = (r, g, b)

    d_outer = rgb_distance(pixel, OUTER_BLUE)
    d_inner = rgb_distance(pixel, INNER_BLUE)

    if d_outer <= MAX_DISTANCE:
        nr, ng, nb = interpolate_color(pixel, OUTER_BLUE, OUTER_GREEN, d_outer, MAX_DISTANCE)
        return nr, ng, nb, a

    if d_inner <= MAX_DISTANCE:
        nr, ng, nb = interpolate_color(pixel, INNER_BLUE, INNER_GREEN, d_inner, MAX_DISTANCE)
        return nr, ng, nb, a

    return r, g, b, a


def shift_image(img):
    img = img.convert("RGBA")
    pixels = img.load()
    width, height = img.size

    for y in range(height):
        for x in range(width):
            r, g, b, a = pixels[x, y]
            pixels[x, y] = shift_pixel(r, g, b, a)

    return img


def process_png(path):
    print(f"  Processing: {path}")
    img = Image.open(path)
    shifted = shift_image(img)
    shifted.save(path)


def generate_ico(source_png, ico_path):
    print(f"  Generating ICO: {ico_path}")
    img = Image.open(source_png)
    sizes = [(16, 16), (32, 32), (48, 48), (64, 64), (128, 128), (256, 256)]
    icon_images = []
    for size in sizes:
        resized = img.copy()
        resized.thumbnail(size, Image.LANCZOS)
        canvas = Image.new("RGBA", size, (0, 0, 0, 0))
        offset = ((size[0] - resized.size[0]) // 2, (size[1] - resized.size[1]) // 2)
        canvas.paste(resized, offset)
        icon_images.append(canvas)

    icon_images[0].save(
        ico_path,
        format="ICO",
        sizes=[s for s in sizes],
        append_images=icon_images[1:],
    )


def collect_logo_pngs():
    result = []
    for root, _dirs, files in os.walk(ICONS_DIR):
        for f in files:
            if not f.lower().endswith(".png"):
                continue
            if any(f.startswith(prefix) for prefix in SKIP_PREFIXES):
                continue
            result.append(os.path.join(root, f))
    return result


def main():
    os.chdir(os.path.dirname(os.path.dirname(os.path.abspath(__file__))))

    print("=== Shifting Logo icons: blue -> green (precise matching) ===")
    print(f"  Outer: {OUTER_BLUE} -> {OUTER_GREEN}")
    print(f"  Inner: {INNER_BLUE} -> {INNER_GREEN}")
    print(f"  Max RGB distance: {MAX_DISTANCE}\n")

    logo_pngs = collect_logo_pngs()
    print(f"Found {len(logo_pngs)} Logo PNG files:\n")

    for png in logo_pngs:
        process_png(png)

    print("\n=== Generating ICO ===\n")
    icon_png = os.path.join(ICONS_DIR, "icon.png")
    icon_ico = os.path.join(ICONS_DIR, "icon.ico")
    if os.path.exists(icon_png):
        generate_ico(icon_png, icon_ico)

    print("\n=== Done! ===")


if __name__ == "__main__":
    main()
