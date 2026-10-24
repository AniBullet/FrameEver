# Which icon family should FrameEver use?

Research for issue #27 (map #23). Facts were checked on 2026-10-24 against the GPUI app's source and the icon families' own repositories through the GitHub API. Everything not marked "judgement" has a source next to it.

## Answer

Use **Lucide** as FrameEver's one icon family, and keep a short list of hand-made icons for things no family can cover (brand marks, Windows caption buttons, the Instant mode symbol). Tabler Icons is the fallback if Lucide ever stops fitting. Lucide is the right base for four reasons:

- It is already the app's main style. 92 of the 156 shipped SVGs are Lucide originals, and 33 more are Cap icons drawn on a 20 px grid with a 1.667 px stroke. That is the same weight as Lucide's 2 px stroke on 24 px, so moving everything to Lucide means redrawing or swapping about 60 files, not starting over.
- It covers everything FrameEver needs. Lucide had a match for every one of the 87 meanings checked below.
- Its licence is ISC (Feather-derived glyphs are MIT). Both are GPL-compatible, so bundling them is fine as long as the licence notice ships with the app, which it does not today.
- It is the most active candidate: a release two days before this research and icon commits in the same week.

## 1. What the app ships today

Source: `apps/desktop-gpui/assets/icons/*.svg` (156 files, 247 KB on disk), the `ICONS` table and its provenance comments in `apps/desktop-gpui/src/assets.rs`, and `"icons/<name>.svg"` path literals in `apps/desktop-gpui/src`.

How GPUI uses them: `svg()` rasterises the file, keeps only the alpha channel and tints it with the element's text colour, so every icon must be a single-colour glyph (`assets.rs`, doc comment on `ICONS`). The sizes the app asks for are mostly small: of all `svg()…size(px(n))` calls, 35 are at 14 px, 34 at 16 px, 28 at 12 px, 11 at 20 px and 6 at 24 px. A handful use other sizes between 8 and 32 px.

### Where the files come from (grid and stroke read from each file)

| Origin | Count | Grid / stroke | Notes |
| --- | --- | --- | --- |
| Lucide originals | 92 | 24×24, stroke 2, round caps/joins | `assets.rs` comments: "the Lucide 24x24 originals `~icons/lucide/*` resolves to" |
| Cap's own set (`packages/ui-solid/icons`) | 33 | 20×20, stroke 1.667 (one at 1.713, `chevron-down` at 2) | 1.667/20 = 2/24, so these look the same weight as Lucide |
| Phosphor originals | 2 | 256×256, filled | `record-fill`, `warning-bold` (`assets.rs`: "Phosphor's 256x256 originals") |
| One-off filled or drawn art | 29 | mixed (8×8 to 223×223) | logos ×7, Windows caption buttons ×4, `traffic-close`, `caret-down`, `x` (9×9), `gear` (16×16), `instant`, `film-cut`, `monitor` (125×84), `next`/`prev`, `play-circle`/`pause-circle`/`stop-circle`, `presets`, `layout`, `info`, `scissors`, `screenshot`, `bg-blur` |

So the set mixes four drawing systems. Some paired states even come from different grids: `microphone` is a Cap 20 px icon while `mic-off` is Lucide 24 px, and `audio-on` is Cap 20 px while `volume-x` is Lucide 24 px.

Dead files: 15 SVGs are neither listed in the `ICONS` table nor referenced from `src`: `clock`, `ease-curve`, `fast-forward`, `grid`, `grip`, `image-off`, `moon`, `mouse-pointer-click`, `rabbit`, `rotate-cw`, `shield`, `sparkles`, `timer`, `wind`, `x-mark`.

### Inventory by meaning

| Meaning | Files today |
| --- | --- |
| Recording transport | `record-fill`, `circle`, `pause`, `pause-circle`, `stop-circle`, `square`, `restart`, `rotate-ccw`, `play`, `play-circle`, `square-play`, `next`, `prev`, `fast-forward` |
| Capture targets and modes | `screen`, `monitor`, `monitor-outline`, `laptop`, `window`, `app-window`, `app-window-mac`, `area`, `box-select`, `screenshot`, `image`, `instant`, `zap`, `timer` |
| Inputs | `camera`, `video`, `microphone`, `mic-off`, `audio-on`, `volume-2`, `volume-x`, `music` |
| Cursor | `cursor`, `mouse-pointer-2`, `mouse-pointer-ban`, `mouse-pointer-click` |
| Editing tools | `scissors`, `film-cut`, `crop`, `zoom-in`, `zoom-out`, `captions`, `scan-text`, `type`, `layers`, `undo`, `redo`, `move`, `move-left`, `move-right`, `flip-horizontal-2`, `flip-vertical-2`, `rotate-3d`, `rotate-cw`, `ratio`, `gauge`, `rabbit`, `clock`, `ease-curve`, `align-left`, `align-center`, `align-right` |
| Look and background | `padding`, `corners`, `shadow`, `bg-blur`, `wind`, `palette`, `shuffle`, `image-off`, `sparkles`, `gem` |
| Layout and panels | `layout`, `columns-2`, `panel-right`, `grid`, `grip`, `grip-vertical`, `presets`, `sliders-horizontal`, `settings-2` |
| Library and files | `folder`, `folder-open`, `folder-down`, `import`, `download`, `upload-arrow`, `cloud-upload`, `save`, `copy`, `link`, `trash`, `edit`, `pencil`, `film`, `clapperboard`, `search`, `arrow-up-right` |
| App, settings, feedback | `settings`, `gear`, `hotkeys`, `keyboard`, `globe`, `bell`, `moon`, `shield`, `terminal`, `message-bubble`, `message-square-plus`, `person-standing`, `refresh-cw` |
| Status | `check`, `circle-check`, `info`, `circle-help`, `triangle-alert`, `warning-bold`, `ban`, `circle-x`, `eye-off` |
| Generic controls | `x`, `x-mark`, `plus`, `circle-plus`, `minus`, `chevron-up/down/left/right`, `caret-down`, `arrow-left`, `arrow-left-right`, `arrow-left-to-line`, `arrow-right-to-line`, `arrows`, `more-horizontal`, `more-vertical`, `maximize`, `enlarge` |
| Window chrome | `caption-minimize/maximize/restore/close-windows`, `traffic-close` |
| Brand | `logo-wordmark`, `logo-tile`, `logo-record`, `logo-record-dot`, `logo-record-gap`, `logo-clip-top`, `logo-clip-bottom` |

Synonyms that one family would collapse into a single glyph each: `settings` / `settings-2` / `gear`, `x` / `x-mark` / `circle-x`, `screen` / `monitor` / `monitor-outline`, `edit` / `pencil`, `maximize` / `enlarge`, `restart` / `rotate-ccw` / `refresh-cw`, `warning-bold` / `triangle-alert`, `record-fill` / `circle`.

## 2. Candidates

Repository data comes from `gh api repos/<repo>`, `…/releases` and `…/commits?path=<icon dir>`. Icon counts come from the icon directory trees, fetched on 2026-10-24.

| Family | Repo | Stars | Licence | Latest release | Last change to icon files | Icons | Grid / weight |
| --- | --- | --- | --- | --- | --- | --- | --- |
| Lucide | lucide-icons/lucide | 25,254 | ISC (+ MIT for Feather-derived glyphs) | 1.61.0, 2026-10-21 | 2026-10-20 | 1,903 | 24×24, stroke **must** be 2 px, round joins and caps, 2 px corner radius ([specification](https://github.com/lucide-icons/lucide/blob/main/docs/contribute/icons/specification.md)) |
| Tabler Icons | tabler/tabler-icons | 22,440 | MIT | v3.49.0, 2026-10-05 | 2026-10-05 | 5,184 outline + 1,054 filled | "designed on a 24x24 grid with a 2px stroke"; stroke width adjustable ([README](https://github.com/tabler/tabler-icons/blob/main/README.md)) |
| Fluent UI System Icons | microsoft/fluentui-system-icons | 10,916 | MIT | no GitHub releases (Maven artefact `1.1.344` per README) | 2026-10-22 | 2,994 icons, each at 12/16/20/24/28/32/48 px, regular + filled | Filled outline paths, drawn per size (e.g. `ic_fluent_record_20_regular.svg`, a 1-unit ring on a 20 px canvas) |
| Phosphor | phosphor-icons/core | 393 (homepage repo 7,710) | MIT | v2.0.8, 2024-02-01 | 2024-03-29 | 1,512 per weight | 256×256 filled outlines, six weights; Regular's line is 16/256 (1.5 px at 24) (e.g. `assets/regular/record.svg`) |
| Iconoir | iconoir-icons/iconoir | 4,605 | MIT | v7.12.1, 2026-08-12 | 2024-12-27 | 1,383 | 24×24, stroke 1.5 (e.g. `icons/regular/video-camera.svg`) |
| Heroicons | tailwindlabs/heroicons | 23,878 | MIT | v2.2.0, 2024-11-18 | — | 324 (24 outline) | 24×24, stroke 1.5 (`optimized/24/outline/video-camera.svg`) |
| Material Symbols | google/material-design-icons | 54,151 | Apache-2.0 | 4.0.0 (2020) for the old set; Symbols updated by commit ("Update Symbols", 2026-10-23) | 2026-10-23 | — | Variable font first, weight 100–700 ([README](https://github.com/google/material-design-icons/blob/master/README.md)) |
| MingCute | mingcute-design/mingcute-icons | 1,652 | Apache-2.0 | v3.0.2, 2026-07-31 | — | — | — |
| Remix Icon | Remix-Design/RemixIcon | 8,430 | Remix Icon License v1.0 (custom) | v4.9.1, 2026-01-29 | — | — | — |
| Hugeicons | hugeicons/hugeicons | 1,222 | MIT for the repo; "6,000+ free" out of "60,000+" ([README](https://github.com/hugeicons/hugeicons/blob/main/README.md)) | — | — | — | 24×24 |

Dropped early:

- **Remix Icon.** Its custom licence forbids distributing the icons "as a standalone product" or as "an independent icon pack" (License §3.1). An AGPL source repository must let anyone redistribute it, so this restriction is a poor fit. Every other candidate uses a standard permissive licence.
- **Hugeicons.** It is freemium: most of the set is paid, and FrameEver's earlier one-off use (`ease-curve`) came from it.
- **Heroicons.** Only 324 outline icons, too few for an editor.
- **Phosphor and Iconoir.** Neither has changed its icon files since 2024 (2024-03-29 and 2024-12-27).
- **Material Symbols.** Actively maintained, but it is designed as a variable font with Material's own shapes. Judgement: it would read as Google/Android rather than calm Windows-native.
- **MingCute.** Small and less proven than the shortlist.
- **Segoe Fluent Icons.** Microsoft names it the Windows 11 system icon font ([Icons in Windows apps](https://learn.microsoft.com/en-us/windows/apps/develop/ui/controls/icons)). It ships with Windows and is not an openly licensed asset set, so it was not considered for bundling. Its terms were not checked here.

## 3. Coverage of FrameEver's needs

87 meanings were checked: everything in the inventory, plus what the library and the redesign add (grid/list view, library, project, export, convert, teleprompter, reveal in folder). Each was matched to a file name in the four strongest families' icon trees.

| Family | Meanings without a match |
| --- | --- |
| Lucide | 0. Two names in the checklist turned out to be renamed recently: `music` became `sixteenth-notes` (PR #4072, 2026-10-15) and `flip-horizontal-2` / `flip-vertical-2` became `triangles-centerline-dashed-vertical` / `-horizontal` (PR #4833, 2026-09-09). |
| Tabler | 2 have only near matches. There is no webcam glyph (nearest: `camera-selfie`) and no gradient glyph (nearest: `color-swatch`). |
| Fluent | 3 have only near matches. "Rounded Corner", "Shadow" and "Spline" are missing; the nearest are "Square Shadow" and "Bezier Curve Square". |
| Phosphor | 1 has only a near match: no "reveal in folder" glyph (nearest: `folder-open`). It has also had no new icons since 2024. |

Recording-specific glyphs in Lucide include `circle-dot`, `screen-share`, `monitor-play`, `webcam`, `square-dashed-mouse-pointer`, `audio-waveform`, `captions`, `scan-text`, `clapperboard`, `library`, `gallery-vertical-end`, `wand-sparkles` and `timer` (all present in `icons/`). Every family covers the basics, so coverage is not what decides between them.

## 4. Fit with Inter, MiSans and the flat, calm style

- **One adjustable stroke.** Lucide and Tabler are stroke icons. Their weight is a single `stroke-width` attribute, and Tabler's README documents changing it. Fluent and Phosphor are filled outlines, so their weight is fixed by the drawing.
- **Line weight at FrameEver's sizes.** Most icons render at 12–16 px. A 2 px stroke on a 24 px grid becomes about 1.0–1.33 px there. Phosphor Regular and the 1.5-stroke families (Iconoir, Heroicons) come out at 0.75–1.0 px. Judgement: 1.0–1.33 px lines sit beside Inter and MiSans at regular/medium text weights without looking heavier or lighter than the type. Sub-pixel lines look faint and blurry at 100% scaling, which matters because of the map's "crisp at 100–200%" floor.
- **Shape language.** Lucide requires round caps, round joins and 2 px corner radii (specification §Strokes, §Corners). Judgement: that matches a calm, rounded, flat look and the rounded terminals of Inter and MiSans. Fluent's per-size drawings are the most pixel-exact for Windows. Judgement: Fluent's friendly Office look would make FrameEver resemble a Microsoft app rather than its own product.
- **Filled states.** Lucide is stroke-only, so states such as a filled record dot or a filled play triangle use `fill="currentColor"` on a Lucide glyph, as `play.svg` and `pause.svg` already do. Tabler ships 1,054 dedicated filled icons, which is its main advantage.

## 5. Licence and bundling in an AGPL app

- The FSF licence list calls ISC "compatible with the GNU GPL" and Expat/MIT "compatible with the GNU GPL". It calls Apache 2.0 "compatible with version 3 of the GNU GPL" ([FSF licence list](https://www.gnu.org/licenses/license-list.en.html)). FrameEver is AGPLv3 (root `LICENSE`). AGPLv3 §7 explicitly allows notice-preservation terms like the ones these licences add.
- ISC and MIT both require keeping "the above copyright notice and this permission notice" in copies (Lucide `LICENSE`). **Gap found:** the repository ships no Lucide or Phosphor notice today. `licenses/` contains only `LICENSE-MIT` for Cap's MIT crates, and `assets/icons/` has no licence file. Whichever family is chosen, its licence text should go next to the icons and into the installer's notices.

## 6. Size

Size does not matter for this choice. Icons are embedded one file at a time with `include_bytes!` (`assets.rs`), so only the icons actually used ship, however big the family is. A Lucide file is a few hundred bytes (`play.svg` 235 B, `check.svg` 260 B), while some one-off drawings are much larger (`gear.svg` 3,977 B, `monitor.svg` 2,774 B).

## 7. Custom icons still needed with Lucide

| Icon | Why custom |
| --- | --- |
| Brand marks (`logo-*`, 7 files) | Brand is settled in `docs/brand/app-icon.md` |
| Windows caption buttons (4) | Should match Windows' own caption glyphs, not an icon family |
| Instant mode symbol | The map has it as an open hand-drawn item ("the Instant mode symbol that replaces the upload-cloud doodle") |
| `film-cut` | Cap illustration. Either redraw it in Lucide's rules or replace it with `scissors` + `film`. |
| Cursor previews (`packages/ui-solid/icons/cursor-macos.svg`, `cursor-windows.svg`) | They depict real OS cursors |

Probably dropped instead of redrawn (Windows-only app): `traffic-close`, `app-window-mac`. Any new glyph should follow Lucide's specification (24 px canvas, 2 px stroke, round caps and joins) so it matches the rest.

## 8. Shortlist

1. **Lucide (recommended).** Already about 60% of the set, no coverage gaps, ISC, most active. Watch-out: Lucide renames icons (two renames in the last two months, see §3). Vendor the SVG files and record each file's Lucide source name and version, instead of tracking names.
2. **Tabler Icons.** Same 24/2 system, MIT, active, 5,184 outline icons plus dedicated filled versions. Pick it if the editor ends up needing many filled/outline pairs.
3. **Fluent UI System Icons.** The most Windows-native, with per-size drawings and MIT. It ranks third because it would give FrameEver Microsoft's look and has a fixed stroke.

## Open questions for the design reference page

- Should the stroke stay at Lucide's 2 px, or be thinned slightly (for example 1.75) for a calmer look at 16–20 px? This is a prototype question: compare both beside Inter and MiSans text at 100% and 150% scaling.
- Which states get filled glyphs (record, play, active tool), and does the Klein-blue accent apply to them?
- Delete the 15 dead SVGs when the icon swap lands.
