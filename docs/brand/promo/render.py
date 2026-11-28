import pathlib
import sys

from PIL import Image
from playwright.sync_api import sync_playwright

REPO = pathlib.Path(__file__).resolve().parents[3]
DESIGN = REPO / "docs" / "design"
PROMO = pathlib.Path(__file__).resolve().parent
PARTS = PROMO / "parts"
IMAGES = REPO / ".github" / "images"

MAIN = "surfaces/main-window-library.html"
EDITOR = "surfaces/editor.html"
INPUT = "surfaces/input-display.html"
BAR = "surfaces/recording-controls.html"
AREA = "surfaces/area-picker.html"
SHOT = "surfaces/screenshot-editor.html"

PAGE_ONLY = "html, body { background: transparent !important; }"
BARE = (
    PAGE_ONLY
    + ".stage, .proto-stage, .desk { background: transparent !important; border-color: transparent !important;"
    + " box-shadow: none !important; overflow: visible !important; }"
    + "#stage > .ground, #stage > .block, #stage > .hud, #stage > .hint, .io-wrap > .grip { display: none !important; }"
)


def window_size(width, height):
    return f"#states-stage .win {{ width: {width}px !important; height: {height}px !important; }}"


def part(name, page, target, theme="dark", scale=2, css=BARE, clicks=(), languages=("zh", "en")):
    return {
        "folder": PARTS,
        "name": name,
        "page": page,
        "target": target,
        "theme": theme,
        "scale": scale,
        "css": css,
        "clicks": clicks,
        "languages": languages,
        "pad": False,
    }


def picture(name, page, target, scale=2, css=BARE, clicks=()):
    item = part(name, page, target, scale=scale, css=css, clicks=clicks, languages=("zh",))
    item["folder"] = IMAGES
    item["pad"] = True
    return item


PARTS_LIST = [
    part("main-dark-{lang}", MAIN, ["#live-main"]),
    part("main-light-{lang}", MAIN, ["#live-main"], theme="light"),
    part("library-{lang}", MAIN, ["#states-stage .win"], css=BARE + window_size(900, 600)),
    part("editor-{lang}", EDITOR, ["#ed"]),
    part("bar", BAR, ["#dock .rb"], scale=3, languages=("zh",)),
    part("keys", INPUT, ["#stage .io-wrap"], scale=4, languages=("zh",)),
    part("pad", INPUT, ["#stage .io-wrap"], scale=4, clicks=['#tools [data-p="used:pad"]'], languages=("zh",)),
]

PICTURES = [
    picture("input-display", INPUT, ["#stage", "#inspector"], css=PAGE_ONLY),
    picture("transcript", EDITOR, ["#ed"], clicks=["#ed [data-act=side][data-v=transcript]"]),
    picture("library", MAIN, ["#states-stage .win"], css=BARE + window_size(1200, 760)),
    picture("screenshot-editor", SHOT, ["#stage > div"]),
    picture("area-picker", AREA, ["#stage"], css=PAGE_ONLY),
    picture("main-window", MAIN, ["#live-main"]),
]


def union(page, selectors):
    boxes = [page.locator(selector).first.bounding_box() for selector in selectors]
    left = min(box["x"] for box in boxes)
    top = min(box["y"] for box in boxes)
    right = max(box["x"] + box["width"] for box in boxes)
    bottom = max(box["y"] + box["height"] for box in boxes)
    scrolled = page.evaluate("() => [scrollX, scrollY]")
    return {"x": left + scrolled[0], "y": top + scrolled[1], "width": right - left, "height": bottom - top}


def pad_to_card(path):
    image = Image.open(path).convert("RGBA")
    width = max(image.width, round(image.height * 1.6))
    height = max(image.height, round(image.width / 1.6))
    card = Image.new("RGBA", (width, height), (0, 0, 0, 0))
    card.paste(image, ((width - image.width) // 2, (height - image.height) // 2))
    card.save(path)


def capture(browser, item):
    for language in item["languages"]:
        context = browser.new_context(viewport={"width": 1760, "height": 1000}, device_scale_factor=item["scale"])
        page = context.new_page()
        page.goto((DESIGN / item["page"]).as_uri() + f"#theme={item['theme']}&lang={language}")
        page.wait_for_timeout(500)
        page.add_style_tag(content=item["css"])
        for selector in item["clicks"]:
            page.click(selector)
        page.wait_for_timeout(400)
        path = item["folder"] / (item["name"].format(lang=language) + ".png")
        page.screenshot(path=str(path), clip=union(page, item["target"]), omit_background=True, full_page=True)
        if item["pad"]:
            pad_to_card(path)
        context.close()
        print(path.relative_to(REPO))


def hero(browser):
    for language, name in (("zh", "hero.png"), ("en", "hero.en.png")):
        context = browser.new_context(viewport={"width": 1400, "height": 900}, device_scale_factor=2)
        page = context.new_page()
        page.goto((PROMO / "promo.html").as_uri() + f"#{language}")
        page.wait_for_timeout(800)
        page.locator("#hero").screenshot(path=str(IMAGES / name), omit_background=True)
        context.close()
        print(f".github/images/{name}")


def main():
    wanted = set(sys.argv[1:])
    with sync_playwright() as playwright:
        browser = playwright.chromium.launch(channel="chrome")
        for item in PARTS_LIST + PICTURES:
            if not wanted or item["name"].split("-{")[0] in wanted:
                capture(browser, item)
        if not wanted or "hero" in wanted:
            hero(browser)
        browser.close()


if __name__ == "__main__":
    main()
