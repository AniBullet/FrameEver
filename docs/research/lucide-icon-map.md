# Map every icon to Lucide

Research for issue #50 (map #23). It builds on "Which icon family should FrameEver use?" (#27, report `docs/research/icon-family.md` on the branch `research/icon-family`) and on the Klein Field tokens decided in #48 (prototype `apps/desktop-gpui/prototype/design-tokens.prototype.html` on the branch `prototype/design-tokens`). Checked on 2026-10-30 against `main` at `d18974cd0`.

Sources used for every claim below:

- The app: `apps/desktop-gpui/assets/icons/*.svg`, the `ICONS` and `ONBOARDING` tables and the icon tests in `apps/desktop-gpui/src/assets.rs`, and every `"icons/<name>.svg"` / `"onboarding/<name>.svg"` literal in `apps/desktop-gpui/src` and `apps/desktop-gpui/converter/src`.
- Lucide: the `icons/` tree of [lucide-icons/lucide](https://github.com/lucide-icons/lucide) on `main` (commit `81875c8c42`, 2026-10-29; latest release **1.64.0**, 2026-10-29), 1,998 icons. Every Lucide name in this report was checked to exist as `icons/<name>.svg` there. Renames were read from each icon's `icons/<name>.json` `aliases` field (an old name kept as a deprecated alias).
- Tabler: the `icons/outline/` tree of [tabler/tabler-icons](https://github.com/tabler/tabler-icons) on `main` (latest release v3.50.0, 2026-10-27), 5,203 outline icons.
- Licences: Lucide's [`LICENSE`](https://github.com/lucide-icons/lucide/blob/main/LICENSE) and Tabler's [`LICENSE`](https://github.com/tabler/tabler-icons/blob/main/LICENSE) on `main`.

## Answer

- **141 SVG files** ship today (143 files in `assets/icons`, two of them licence notices).
- **125 map to Lucide.** Because several files are synonyms, they collapse to **112 Lucide glyphs** (13 files merge into another glyph, see §3).
- **3 have no good Lucide match**: `padding`, `shadow`, `bg-blur`. Tabler has `box-padding`, `shadow` and `blur`.
- **13 stay custom** and are not icon-family work: the 7 brand logo layers, the 4 Windows caption buttons, the macOS traffic-light close glyph, and the 125×84 monitor illustration in the target overlay.
- **0 unused SVGs.** The 15 dead files were deleted in #33, and two tests in `assets.rs` (`every_embedded_icon_is_referenced`, `every_icon_file_is_in_the_table_or_included_by_source`) now fail on any new one. What the swap deletes are the 13 merged files.
- **Stroke 1.75 on Lucide's 24 px grid, drawn at 16 px by default**, as in the #48 prototype. Use 14 px and 20 px as the other two steps, and keep 12 px for filled glyphs only.
- **Licence:** ship Lucide's `LICENSE` unchanged. It holds the ISC licence and the MIT licence for the Feather-derived icons, and the Feather part applies to FrameEver. `assets/icons/LICENSE-lucide.txt` already holds that text, byte-identical to Lucide's `main`. Add Tabler's MIT notice only if the three Tabler icons are used, and drop the Phosphor notice once `record-fill` and `warning-bold` are swapped.

## 1. What counts as an icon

Every icon the app draws is one of the SVG files in `assets/icons`. `svg()` loads them through the `ICONS` table, and `img()` loads one. Two files are also served under a second path: `onboarding/volume-2.svg` and `onboarding/volume-x.svg` (the `ONBOARDING` table), which is why `volume-2` has no `"icons/volume-2.svg"` literal in `src`. FrameEver Converter embeds the two Windows caption buttons itself (`converter/src/main.rs`).

Other things drawn in `src` that are not icons and stay as they are:

| What | Where | Why it stays |
| --- | --- | --- |
| Hand-drawn doodles (SCREEN, CURSOR, MICROPHONE, CAMERA, ALL_SET, the four permission marks) | `onboarding_doodles.rs`, painted by `doodle.rs` | #48 decision 6: the welcome page and onboarding keep their hand-drawn art and animation; only the ink colours change |
| OS cursor pictures | `assets/onboarding/cursor-windows.svg`, `cursor-macos.svg` | They depict the real system cursors (#27 §7) |
| Keycap glyphs `⌘ ⌃ ⇧ ⌥ ↩ ⌫ ↑ ↓ ← → ⇥ ⌦` | `ui/kbd.rs`, `settings_pages.rs` | Text on keycaps, not icons |
| `×` between width and height | `editor_crop.rs`, `screenshot_crop.rs`, `target_overlay.rs` | Typography |
| `←  Back` | `onboarding_window.rs` | Onboarding text, kept by #48 decision 6 |
| Camera loading sweep, sliders, position pad, collapsible | `camera_loading.rs`, `ui/*.rs` | Painted controls and animation, not glyphs |

## 2. The table

"Now" is the file's drawing system: **L** is a Lucide 24×24 original with stroke 2, **C** is Cap's 20×20 set with stroke 1.667, **P** is Phosphor 256×256 filled, and **D** is a one-off drawing (its viewBox in brackets). "Used in" lists the source files under `apps/desktop-gpui/src`. "Merge" means another file maps to the same Lucide glyph, so this file goes away in the swap.

### 2.1 Mapped to Lucide (125 files)

| File | Now | Used in | Lucide | Note |
| --- | --- | --- | --- | --- |
| `align-center` | L | `editor_panels.rs` | `text-align-center` | Renamed; `align-center` is a deprecated alias |
| `align-left` | L | `editor_panels.rs` | `text-align-start` | Renamed; `align-left` is a deprecated alias |
| `align-right` | L | `editor_panels.rs` | `text-align-end` | Renamed; `align-right` is a deprecated alias |
| `app-window-mac` | L | `editor_window/frame.rs` | `app-window-mac` | The editor's "macOS" frame style, available on Windows too |
| `app-window` | L | `editor_window/frame.rs`, `settings_pages.rs` | `app-window` | |
| `area` | C | `main_window.rs` | `scan` | Four corner brackets, same drawing |
| `arrow-left-right` | L | `editor_panels.rs` | `arrow-left-right` | |
| `arrow-left-to-line` | L | `editor_window.rs` | `arrow-left-to-line` | |
| `arrow-left` | L | `editor_sidebar.rs` | `arrow-left` | |
| `arrow-right-to-line` | L | `editor_window.rs` | `arrow-right-to-line` | |
| `arrow-up-right` | L | `screenshot_annotations.rs` | `arrow-up-right` | |
| `arrows` | C | `camera_window.rs` (mirror button) | `triangles-centerline-dashed-horizontal` | Merge with `flip-horizontal-2`: the same "mirror" meaning the teleprompter already uses |
| `audio-on` | C | `editor_sidebar.rs` (Audio tab) | `volume-2` | Merge with `volume-2` |
| `ban` | L | `editor_window/frame.rs` | `ban` | |
| `bell` | C | `main_window.rs`, `settings_window.rs` | `bell` | |
| `box-select` | L | `editor_timeline.rs` | `square-dashed` | Renamed; `box-select` is a deprecated alias |
| `camera` | C | `editor_sidebar.rs`, `main_window.rs`, `target_overlay.rs` | `webcam` | The camera input. Lucide `camera` is a photo camera; the #48 prototype uses `webcam` for this row |
| `captions` | L | `editor_timeline.rs`, `editor_window.rs`, `settings_window.rs` | `captions` | |
| `caret-down` | D (10×6) | `onboarding_window.rs`, `target_overlay.rs` | `chevron-down` | Merge with `chevron-down` |
| `check` | L | 11 files | `check` | |
| `chevron-down` | L drawn on 20×20 | 12 files | `chevron-down` | |
| `chevron-left` | L | `teleprompter_window.rs` | `chevron-left` | |
| `chevron-right` | L | `editor_panels.rs`, `editor_sidebar.rs`, `teleprompter_window.rs` | `chevron-right` | |
| `chevron-up` | L | `editor_export.rs`, `onboarding_window.rs`, `settings_pages.rs` | `chevron-up` | |
| `circle-check` | C | `editor_window/frame.rs`, `settings_pages.rs` | `circle-check` | |
| `circle-help` | C | `main_window.rs` | `circle-question-mark` | Renamed; `circle-help` is a deprecated alias |
| `circle-plus` | C | `editor_clips.rs`, `editor_window.rs` | `circle-plus` | |
| `circle-x` | C | 5 files | `circle-x` | |
| `circle` | C | `camera_window.rs`, `onboarding_window.rs`, `screenshot_annotations.rs` | `circle` | |
| `clapperboard` | L | `editor_clips.rs`, `editor_timeline.rs`, `settings_pages.rs` | `clapperboard` | |
| `cloud-upload` | L | `settings_pages.rs` | `cloud-upload` | |
| `columns-2` | L | `editor_panels.rs`, `editor_timeline.rs` | `columns-2` | |
| `copy` | L | 6 files | `copy` | |
| `corners` | C | `screenshot_editor.rs` (Rounding) | `square-round-corner` | Today a dashed circle; Lucide's glyph says "corner radius" directly |
| `crop` | C | `editor_window.rs`, `screenshot_editor.rs` | `crop` | |
| `cursor` | C | `editor_panels.rs`, `editor_sidebar.rs`, `onboarding_window.rs` | `mouse-pointer-2` | Same pointer drawing. Merge with `mouse-pointer-2` |
| `download` | L | `editor_tabs.rs`, `main_window.rs`, `settings_window.rs` | `download` | |
| `edit` | L | 4 files | `square-pen` | Renamed; `edit` is a deprecated alias |
| `enlarge` | C | `camera_window.rs`, `screenshot_editor.rs` | `maximize-2` | |
| `eye-off` | L | 4 files | `eye-off` | |
| `film-cut` | D (124×119) | `library.rs`, `main_window.rs`, `onboarding_window.rs`, `settings_window.rs`, `target_overlay.rs` (Studio mode); `editor_export.rs` (Video codec) | `clapperboard` for Studio, `film` for Video codec | Cap's clapperboard drawing. `settings_pages.rs` already uses `clapperboard` for "Studio recording finished", and the #48 prototype uses it for Studio. Merge |
| `film` | L | `settings_pages.rs` | `film` | |
| `flip-horizontal-2` | L | `editor_panels.rs`, `teleprompter_window.rs` | `triangles-centerline-dashed-horizontal` | Renamed; `flip-horizontal-2` is a deprecated alias |
| `flip-vertical-2` | L | `editor_panels.rs` | `triangles-centerline-dashed-vertical` | Renamed; `flip-vertical-2` is a deprecated alias |
| `folder-down` | L | `settings_pages.rs` | `folder-down` | |
| `folder-open` | L | `settings_pages.rs` | `folder-open` | |
| `folder` | L | 4 files | `folder` | |
| `gauge` | L | `editor_export.rs`, `settings_window.rs`, `teleprompter_window.rs` | `gauge` | |
| `gear` | D (16×16, filled) | `editor_window.rs`, `target_overlay.rs` | `settings` | Merge with `settings` |
| `gem` | L | `editor_export.rs` | `gem` | |
| `globe` | L | `editor_window/frame.rs`, `settings_pages.rs` | `globe` | |
| `grip-vertical` | L | `editor_window.rs`, `screenshot_annotations.rs` | `grip-vertical` | |
| `hotkeys` | C | `settings_window.rs` (Shortcuts page) | `command` | Same ⌘ drawing |
| `image` | C | 6 files | `image` | |
| `import` | L | 4 files | `import` | |
| `info` | D (20×20, filled outline) | 4 files | `info` | |
| `instant` | D (223×223) | `library.rs`, `main_window.rs`, `onboarding_window.rs`, `settings_window.rs`, `target_overlay.rs` | `zap` | `settings_pages.rs` already uses `zap` for "Instant recording finished", and the #48 prototype uses it for Instant. Merge; see open question 1 |
| `keyboard` | L | `editor_sidebar.rs`, `editor_timeline.rs` | `keyboard` | |
| `laptop` | L | `editor_window/frame.rs` | `laptop` | |
| `layers` | L | 4 files | `layers` | |
| `layout` | D (20×20, filled) | `editor_window.rs`, `screenshot_editor.rs` (aspect ratio buttons) | `ratio` | Merge with `ratio`, which the screenshot crop already uses for the same meaning |
| `link` | L (via Cap) | `settings_pages.rs` | `link` | |
| `maximize` | L | `screenshot_crop.rs` | `maximize` | |
| `message-bubble` | C | `editor_sidebar.rs` (Captions tab) | `message-square-more` | Same drawing. `captions` would match the timeline's captions track instead; see open question 3 |
| `message-square-plus` | L | `settings_window.rs` | `message-square-plus` | |
| `mic-off` | L | `controls_window.rs` | `mic-off` | |
| `microphone` | C | 5 files | `mic` | |
| `minus` | L | `teleprompter_window.rs` | `minus` | |
| `monitor-outline` | L | 4 files | `monitor` | Takes over the name `monitor.svg` once the illustration moves (§2.3) |
| `more-horizontal` | L | `screenshot_editor.rs` | `ellipsis` | Renamed; `more-horizontal` is a deprecated alias |
| `more-vertical` | C | `controls_window.rs`, `onboarding_window.rs` | `ellipsis-vertical` | Renamed; `more-vertical` is a deprecated alias |
| `mouse-pointer-2` | L | `screenshot_annotations.rs` | `mouse-pointer-2` | |
| `mouse-pointer-ban` | L | `editor_timeline.rs`, `editor_window.rs` | `mouse-pointer-ban` | |
| `move-left` | L | `editor_clips.rs`, `editor_export.rs`, `main_window.rs` | `move-left` | |
| `move-right` | L | `editor_panels.rs`, `onboarding_window.rs` | `move-right` | |
| `move` | L | `editor_panels.rs` | `move` | |
| `music` | L | `editor_timeline.rs` | `eighth-notes` | Renamed; `music` is a deprecated alias of `eighth-notes`. #27 named `sixteenth-notes`, but that one's alias is `music-4` |
| `next` | D (12×15, filled) | `editor_window.rs` | `skip-forward` | |
| `palette` | L | `editor_timeline.rs` | `palette` | |
| `panel-right` | L | `editor_panels.rs`, `editor_timeline.rs` | `panel-right` | |
| `pause-circle` | D (20×20, filled) | `controls_window.rs`, `onboarding_window.rs` | `circle-pause` | |
| `pause` | L (filled) | 4 files | `pause` | Keep `fill="currentColor"` as today |
| `pencil` | L | `editor_window.rs`, `screenshot_annotations.rs` | `pencil` | |
| `person-standing` | L | `camera_window.rs` (background blur) | `person-standing` | |
| `play-circle` | D (20×20, filled) | `controls_window.rs`, `main_window.rs`, `settings_pages.rs` | `circle-play` | |
| `play` | L (filled) | `editor_panels.rs`, `editor_window.rs`, `teleprompter_window.rs` | `play` | Keep `fill="currentColor"` as today |
| `plus` | L | 7 files | `plus` | |
| `presets` | D (20×20, filled) | `editor_window.rs` | `wand-sparkles` | Same wand-and-sparkles drawing |
| `prev` | D (12×15, filled) | `editor_window.rs` | `skip-back` | |
| `ratio` | L | `screenshot_crop.rs` | `ratio` | |
| `record-fill` | P | `settings_window.rs` (recording-in-progress pill) | `circle` | Filled variant (`fill="currentColor"`), kept as its own file, like `play` and `pause` |
| `rectangle-horizontal` | L | `camera_window.rs` | `rectangle-horizontal` | |
| `redo` | C | `editor_window.rs` | `redo-2` | Same hooked-arrow drawing |
| `refresh-cw` | L | `editor_panels.rs`, `settings_window.rs` | `refresh-cw` | |
| `restart` | C | `controls_window.rs`, `onboarding_window.rs` | `rotate-ccw` | Merge with `rotate-ccw` |
| `rotate-3d` | L | `editor_timeline.rs` | `rotate-3d` | |
| `rotate-ccw` | L | 4 files | `rotate-ccw` | |
| `save` | L | `editor_sidebar/animated_gradient.rs`, `screenshot_editor.rs` | `save` | |
| `scan-text` | C | `main_window.rs`, `settings_pages.rs` | `scan-text` | |
| `scissors` | D (20×20, filled) | `editor_window.rs` | `scissors` | |
| `screen` | C | `main_window.rs` (Display target) | `monitor` | Merge with `monitor-outline` |
| `screenshot` | D (24×24, filled) | `main_window.rs`, `onboarding_window.rs` | `image` | `settings_pages.rs` already uses `image` for "Screenshot taken", and the #48 prototype uses it for screenshots. Merge; see open question 3 |
| `search` | C | `editor_timeline.rs`, `ui/text_input.rs` | `search` | |
| `settings-2` | L | `main_window.rs`, `settings_window.rs`, `teleprompter_window.rs` | `settings-2` | |
| `settings` | C | 5 files | `settings` | |
| `shuffle` | L | `editor_sidebar/animated_gradient.rs`, `editor_sidebar.rs` | `shuffle` | |
| `sliders-horizontal` | L | `editor_export.rs` | `sliders-horizontal` | |
| `square-play` | L | `main_window.rs`, `settings_window.rs` | `square-play` | |
| `square` | C | 4 files | `square` | |
| `stop-circle` | D (20×20, filled) | `controls_window.rs`, `main_window.rs`, `onboarding_window.rs` | `circle-stop` | |
| `terminal` | L | `settings_window.rs` | `terminal` | |
| `trash` | C | 11 files | `trash` | `trash-2` is now a deprecated alias of `trash` |
| `triangle-alert` | L | 5 files | `triangle-alert` | |
| `type` | L | `editor_timeline.rs`, `onboarding_window.rs`, `screenshot_annotations.rs` | `type` | |
| `undo` | C | `editor_window.rs` | `undo-2` | Same hooked-arrow drawing |
| `upload-arrow` | C | `editor_export.rs`, `editor_window.rs` | `upload` | |
| `video` | L | 4 files | `video` | |
| `volume-2` | L | `onboarding_window.rs` (as `onboarding/volume-2.svg`) | `volume-2` | |
| `volume-x` | L | `editor_timeline.rs`, `editor_window.rs`, onboarding | `volume-x` | |
| `warning-bold` | P | `settings_window.rs` (recording-failed pill) | `triangle-alert` | Merge. A filled Lucide triangle would hide the `!`, so use the stroke glyph |
| `window` | C | `main_window.rs` (Window target) | `app-window` | Same drawing. Merge with `app-window` |
| `x` | D (9×9, filled) | 11 files | `x` | |
| `zap` | L | `settings_pages.rs`, `settings_window.rs` | `zap` | |
| `zoom-in` | L | `editor_window.rs`, `screenshot_editor.rs` | `zoom-in` | |
| `zoom-out` | L | `editor_window.rs`, `screenshot_editor.rs` | `zoom-out` | |

### 2.2 No good Lucide match (3 files)

All three are fields in the screenshot editor's background section (`screenshot_editor.rs`). The editor's own background panel has the same settings.

| File | Now | Nearest Lucide, and why it is not good enough | Tabler candidate |
| --- | --- | --- | --- |
| `padding` | C | `square-dashed` is the same dashed square, but it already means box-select on the timeline | `box-padding` |
| `shadow` | C (half sun over two lines) | No shadow glyph; `sunset` reads as weather | `shadow` |
| `bg-blur` | D (16×15) | No blur glyph; `droplet` and `blend` mean something else | `blur` |

Tabler's outline set uses the same 24 px grid, 2 px stroke and round caps and joins (Tabler README), so these sit with Lucide once their stroke is set to the same width. The alternative is to draw the three in Lucide's own rules (see open question 2).

### 2.3 Stays custom (13 files)

| File | Now | Used in | Why |
| --- | --- | --- | --- |
| `logo-wordmark`, `logo-tile`, `logo-record`, `logo-record-dot`, `logo-record-gap`, `logo-clip-top`, `logo-clip-bottom` | D (180×40) | `logo.rs` | Brand layers generated by `packaging/brand/render_brand.py` (`docs/brand/app-icon.md`) |
| `caption-minimize-windows`, `caption-maximize-windows`, `caption-restore-windows`, `caption-close-windows` | D (10×10) | `ui/windows_caption.rs`, `main_window.rs`, FrameEver Converter | They match the Windows caption glyphs, not an icon family |
| `traffic-close` | D (8×8) | `main_window.rs`, `onboarding_window.rs` | The macOS traffic-light close glyph. It is only drawn off Windows (`main_window.rs` puts the traffic lights under `!cfg!(target_os = "windows")`), and macOS code is kept (AGENTS.md) |
| `monitor` | D (125×84) | `target_overlay.rs` via `img()` | An 80 px illustration, not an icon. Move it out of `icons/` (e.g. to `images/`) so `monitor.svg` can be the Lucide glyph |

### 2.4 Files that go away in the swap

There are no unused files today. These 13 files go away because their glyph merges with another file's: `arrows`, `audio-on`, `caret-down`, `cursor`, `film-cut`, `gear`, `instant`, `layout`, `restart`, `screen`, `screenshot`, `warning-bold`, `window`. `monitor-outline` is renamed to `monitor`. Eleven files are renamed to their current Lucide names: `align-*` ×3, `box-select`, `circle-help`, `edit`, `flip-*-2` ×2, `more-*` ×2 and `music`. The rest of the renames are by choice, such as `microphone` → `mic`.

## 3. Today's Lucide files are out of date

77 files are Lucide originals on the 24 px grid. Compared element by element (with stroke and fill attributes ignored) against the same icon on Lucide's `main`, 40 are identical and 37 differ. Most of the differences are only in encoding: `download` used `<polyline>` and `<line>`, and `type` used one combined path. Some are real redraws: `zap` is now a rounded bolt with different path data, and `music`'s replacement `eighth-notes` was redrawn. So the swap should re-vendor every file from one pinned Lucide release rather than keep the old copies. As #27 recommended, record the Lucide version and source name of each file, because Lucide renames icons (11 of the files above already carry an old name).

## 4. Stroke and sizes for the Klein Field tokens

What the tokens say: the #48 prototype draws every icon as Lucide with `svg.lucide{width:var(--icon);height:var(--icon);stroke-width:1.75}`. Its `--icon` is 15 px for the compact density and 16 px for the default and comfortable ones. #48 decision 2 keeps only the default density for 1.0 (34 px rows, 32 px controls, 13.5 px body text), so **16 px** is the base icon size.

What the app asks for today: of the `svg()` calls where a size could be read from the code, there are 28 at 16 px, 28 at 12 px, 26 at 14 px, 9 at 20 px, 7 at 10 px and 5 at 24 px, plus one or two each at 8, 9, 13, 15, 18, 22 and 32 px.

Recommendation:

- **Stroke 1.75 on the 24 px grid**, written into each vendored file (`stroke-width="1.75"`). gpui's `svg()` rasterises the file as written and has no equivalent of Lucide's `absoluteStrokeWidth`, so the width has to live in the files. Filled glyphs (`play`, `pause`, the filled `circle`) keep `fill="currentColor"`.
- **Three sizes for stroke icons:**

| Token | px | Stroke at 1× (1.75 × size / 24) | Use |
| --- | --- | --- | --- |
| `icon_sm` | 14 | 1.02 px | Dense editor toolbars, icons beside 12 px small text |
| `icon` | 16 | 1.17 px | Default: rows, buttons, inputs, menus (the prototype's `--icon`) |
| `icon_lg` | 20 | 1.46 px | Mode tiles, the recording bar, empty-state actions |

- **12 px and smaller only for filled glyphs** (the status pills, the playback transport). A 1.75 stroke at 12 px is 0.875 px, under one device pixel at 100 % scaling, which #27 §4 flagged as faint and blurry. The 10 px and 8–9 px stroke icons used today should move to 12 px filled or to 14 px.
- 24 px and larger only for illustrations and empty states.

At 16 px an icon leaves 8 px above and below it in a 32 px control.

## 5. Licence notice to ship

**Lucide (required).** Lucide's `LICENSE` is the ISC licence (`Copyright (c) 2026 Lucide Icons and Contributors`), followed by a list of icons derived from Feather and the MIT licence that covers them (`Copyright (c) 2013-present Cole Bemis`). The Feather part applies to FrameEver. The list names many icons this map uses, under their old or current names, among them `check`, `chevron-down/left/right/up`, `circle`, `download`, `info`, `link`, `maximize`, `minus`, `monitor`, `more-horizontal`, `more-vertical`, `move`, `music`, `plus`, `search`, `square`, `terminal`, `trash`, `type`, `upload`, `x`, `zoom-in` and `zoom-out`. So ship the whole file unchanged.

The repository already does this: `apps/desktop-gpui/assets/icons/LICENSE-lucide.txt` is a one-line header followed by Lucide's `LICENSE`, and that text is identical to Lucide's `main` as of 2026-10-30. `tauri.conf.json` copies `assets/icons/LICENSE-*.txt` into `licenses/icons/` in the installer, and the test `icon_licence_notices_ship_beside_the_icons_and_in_the_installer` checks both the file and the bundle rule. Nothing needs to change for Lucide, except refreshing the file if a later Lucide release changes `LICENSE` when the icons are re-vendored.

**Tabler (only if §2.2 uses Tabler).** Add `assets/icons/LICENSE-tabler.txt` with the same kind of one-line header naming the three files, then Tabler's `LICENSE`: the MIT licence with `Copyright (c) 2020-2026 Paweł Kuna`. Add it to the licence test.

**Phosphor (drop after the swap).** `LICENSE-phosphor.txt` covers only `record-fill.svg` and `warning-bold.svg`. Once both are Lucide glyphs, delete the notice and its entry in the licence test, and remove the "Icons: Phosphor (MIT)" credit in `settings_window.rs` and its zh-CN entry. If Tabler is added, add a matching credit.

## Open questions for the owner

1. **Instant symbol.** Lucide `zap` already stands for Instant in Settings and in the #48 prototype. Is that the final Instant symbol, or does the map still want the hand-drawn one it lists as an open item? If it does, it is the only new drawing this swap needs, drawn to Lucide's rules (24 px grid, round caps and joins, 1.75 stroke).
2. **Padding, shadow and blur.** Use Tabler's `box-padding`, `shadow` and `blur`, which adds a third licence notice, or draw three icons to Lucide's rules so the app has one family and one notice? Recommendation: Tabler. It is less work and MIT, and those three fields are low-traffic.
3. **Merges that change a meaning's picture.** The screenshot mode becomes `image`, matching Settings and the prototype, though `camera` would read more as "take a shot". The editor's Captions tab becomes `message-square-more`, the same drawing as today, though `captions` would match the timeline track. Recommendation: `image` and `captions`, so one meaning has one picture everywhere.
