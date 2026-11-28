# Promo image

`promo.html` is the source of the picture at the top of the README: `.github/images/hero.png` (Chinese) and `.github/images/hero.en.png` (English). Open it in a browser as `promo.html#zh` or `promo.html#en`.

## Rules it follows

- Tiles are all the same light neutral; dark belongs only to the app's own windows. A tile is never dark behind a light window or the reverse.
- One tile is Klein blue, and nothing else is. It carries the app icon's shapes (the two clips and the record button) at their brand colours (`../app-icon.md`).
- Recording leads: both main windows and the recording bar get the largest tile. The editor is cropped and shares a tile with its feature list.
- Each tile has one label, top left: the name in ink, then a few words in grey.
- Text, including the labels, is in the `text` table in the page's script, once per language.

## Parts

`parts/` holds the pictures the page places. All of them are rendered from the design pages in `../../design/surfaces` at 2×, in the dark theme unless named otherwise:

| File | Design page | What |
| --- | --- | --- |
| `main-dark-*.png`, `main-light-*.png` | `main-window-library.html` | The main window |
| `library-*.png` | `main-window-library.html` | The library, window narrowed to 900 × 600 |
| `editor-*.png` | `editor.html` | The editor |
| `bar.png` | `recording-controls.html` | The recording bar |
| `keys.png`, `pad.png` | `input-display.html` | The keyboard and mouse preset, and the gamepad preset |

When one of those surfaces changes, render the part again from its design page, then render the two README pictures from `promo.html`.

## Rendering

`render.py` does all of it: the parts, the six pictures in the README's highlights table (`.github/images/*.png`, each padded to 8:5 on a transparent background) and the two hero pictures (the `#hero` element, 1280 × 720 at device scale 2). It needs Python with Playwright and Pillow, and Chrome.

```powershell
python docs/brand/promo/render.py
```

Pass names to render only some of them, for example `render.py library hero`.
