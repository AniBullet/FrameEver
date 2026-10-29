# GPUI component libraries and a design system to borrow (research for #47)

Question: which existing GPUI component library and published design system FrameEver can build on instead of the 23 hand-made controls in `apps/desktop-gpui/src/ui/`, and what adopting them costs on our gpui pin.

Standing rules this is measured against: pure flat (no gradients, glass or blur), Klein blue `#1F3FD8` accent, dark first with light derived, Inter + MiSans, OKLCH colours in three token layers (brand, semantic, component), WCAG AA, full keyboard reach, crisp at 100–200 % scaling. Earlier reports this builds on: `docs/research/ui-references.md` (branch `research/ui-references`) and `docs/research/gpui-animation.md` (branch `research/gpui-animation`).

Sources are primary: GitHub repositories and their API (stars, licences, commit activity, compare views), crates.io API, the source trees of `longbridge/gpui-kit` at `944803984` (2026-10-29) and `zed-industries/zed` at `08cbb88a1` (2026-10-25), our patched checkout `../zed-cap` at `5d1f83d9f`, and FrameEver at `bb6195105`. Numbers were read on 2026-10-30. Nothing was compiled for this report.

## Short answer

- **Adopt GPUI Kit** (`longbridge/gpui-kit`, the project formerly called `gpui-component`): Apache-2.0, 18.8k stars, weekly releases, 75+ components with shadcn-style semantic theme tokens, AccessKit semantics, keyboard focus handling and a headless UI test harness. It is the only maintained library with real coverage of what we hand-made (text input with IME, select, menu, slider, number input, switch, tooltip, kbd, colour picker, progress ring, tabs/segmented, popover, settings rows, title bar, virtual list, resizable panes, dialogs, notifications).
- **It does not build against our pin.** Every GPUI Kit release since 0.6.0 depends on `gpui-pre`, a crates.io snapshot of upstream Zed that its maintainer republishes weekly, pinned exactly (`=0.3.10` = `zed@08cbb88`). Our pin is the `wingleeio/zed` fork, 17 commits ahead of and 1,559 behind upstream `main`; the oldest `gpui-pre` (0.3.1) is already 727 upstream commits past our base. Adoption therefore starts with moving gpui to upstream `08cbb88`, rebasing our own three patches and consuming the result under the `gpui-pre` package names.
- **The move costs little in features**: on Windows, nothing FrameEver calls comes from the wingleeio fork. The fork's additions are EdgeFade, BackdropBlur and macOS Metal fixes; the CJK closing-punctuation wrap and `ObjectFit` it carried are upstream now. What matters is our own patch stack (custom frame, mixed-DPI, idle frame pacing, `paint_surface_fitted`, clipboard), which does not apply cleanly to `08cbb88` and must be rebased. The move also brings upstream's new `SpringAnimation`, gestures and profiler, which `gpui-animation.md` found missing.
- **Design system: shadcn/ui for tokens and component anatomy**, because GPUI Kit's theme is literally shadcn's token set and shadcn v4 is OKLCH-native and flat. Use **Radix Colors' 12-step scale** as the method for the brand layer (generated in OKLCH around Klein blue and a cool neutral), and **Fluent 2** for Windows interaction: keyboard model, focus visuals, motion durations and caption buttons. Do not take Fluent's Mica/Acrylic materials or anything from Material 3.

## 1. Candidate libraries

| Library | Licence | Popularity | Maintenance | gpui it builds on | Components | Theming model |
|---|---|---|---|---|---|---|
| [longbridge/gpui-kit](https://github.com/longbridge/gpui-kit) (crates `gpui-kit`, `gpui-component`, `gpui-base`) | Apache-2.0 (code, `LICENSE-APACHE`, crate metadata); docs prose also CC BY 4.0 (`LICENSE-DOCS.md`) | 18,767 stars, 1,163 forks; `gpui-component` 378,873 downloads (crates.io) | v0.8.1 on 2026-10-26; 12 releases between 0.6.0 (2026-09-03) and 0.8.1; 248 commits since 2026-09-30 by at least 6 regular authors, led by huacnlee. Ships Longbridge Pro (README). | `gpui-pre =0.3.10` (snapshot of `zed@08cbb88`), exact pin enforced by `script/check-gpui-pin.ts` (root `Cargo.toml`) | 75+ (see §3); `gpui-base` holds the unstyled behaviour | `Theme` global with `mode`, `font_family`, `font_size`, `radius`, `radius_lg`, `shadow: bool`, and a `ThemeColor` of 136 named colours; JSON theme files; 20+ bundled themes; default theme derived from shadcn `neutral` |
| Zed's `ui` crate ([zed-industries/zed `crates/ui`](https://github.com/zed-industries/zed/tree/main/crates/ui)) | GPL-3.0-or-later (`crates/ui/Cargo.toml`) | part of Zed (92,387 stars) | very active, but `publish = false` (workspace), no API promise; built for Zed only | same tree as gpui, so the copy in `../zed-cap` matches our pin | 52 files in `components/`: button, toggle (checkbox, switch), context/dropdown/right-click menus, popover, tooltip, tab, tab bar, list, modal, keybinding, notification, progress, data table, scrollbar, indicator. **No text input** (that is Zed's `editor` crate), **no slider, select, number field or colour picker.** | `theme` crate: `ThemeColors` with editor-oriented fields (685-line `styles/colors.rs`) and Zed's settings-driven theme loading |
| [crabtalk/bezel](https://github.com/crabtalk/bezel) | MIT | 193 stars | created 2026-08-15, 20 commits since 2026-09-30; README warns "APIs are still evolving" | its own fork, published as `bezel-gpui 0.3.21` | SwiftUI-style widgets, small catalogue | `Brand { tint, accent, radius }` flowing through an environment |
| [ELY-Open-Source/Ely-GPUI-Components](https://github.com/ELY-Open-Source/Ely-GPUI-Components) | MIT | 113 stars | created 2026-10-19 (11 days old) | `gpui-pre =0.3.10`, same as GPUI Kit | early | — |
| [pierreaubert/gpui-toolkit](https://github.com/pierreaubert/gpui-toolkit) (`gpui-ui-kit`) | ISC | 86 stars, `gpui-ui-kit` 1,009 downloads | last crate 2026-01-11 | own crates | audio/plot oriented | — |
| [gpui-ce/gpui-ce](https://github.com/gpui-ce/gpui-ce) | Apache-2.0 | 1,424 stars | active | is a gpui fork, not a component library ("mostly API compatible with upstream, but that is changing", README); adds `backdrop_blur`, transitions | — | — |
| [longbridge/gpui-fast](https://github.com/longbridge/gpui-fast) | Apache-2.0 | 307 stars | v0.1.12, 2026-10-26 | gpui with a reworked layout cache; an optional GPUI Kit backend (`gpui-fast` feature) | — | — |

Licence fit: FrameEver is AGPL-3.0 (`apps/desktop-gpui/Cargo.toml`). Apache-2.0 and MIT code can be combined into it. Zed's GPL-3.0-or-later `ui` could be too (GPLv3 §13 allows combining with AGPLv3), but it is unpublished, Zed-shaped and missing the controls we most need, so it is a reference, not a dependency.

GPUI Kit is three crates under one facade (README, "Framework Architecture"): `gpui-base` (unstyled behaviour, state, focus, AccessKit, motion; "Base UI" in the web analogy), `gpui-component` (the styled layer; "shadcn" in the analogy) and `gpui-shell` (JavaScript extensions, which we do not need). An app may use the styled components and drop to `gpui-base` where it wants its own look. It also ships a normative design guide (`website/docs/design-guides.md`), an accessibility guide built on AccessKit (`website/docs/accessibility.md`) and a styling-and-motion contract (`docs/STYLING-AND-MOTION.md`).

## 2. Our pin and what moving it costs

### What we build against today

- `apps/desktop-gpui/Cargo.toml` and `converter/Cargo.toml`: `gpui`, `gpui_platform`, `gpui_tokio` from `https://github.com/wingleeio/zed` at `5d1f83d9f`, redirected by `[patch."https://github.com/wingleeio/zed"]` to `../zed-cap`.
- `scripts/prepare-gpui-dependency.sh` (run by `pnpm fe setup`) shallow-fetches that rev and applies four patches from `apps/desktop-gpui/patches/`:

| Patch | Lines | Touches | What it adds |
|---|---|---|---|
| `zed-gpui.patch` | 1,327 | `gpui/{app,platform,scene,window}.rs`, `gpui_macos/*`, `gpui_platform`, `gpui_wgpu` | `try_write_to_clipboard`, retained window visibility, `paint_surface_fitted` (used 7× in FrameEver), Metal renderer hardening |
| `zed-windows.patch` | 844 | `gpui_windows/{directx_atlas,directx_renderer,display,events,shaders.hlsl,window}.rs` | custom frame insets and window moves, mixed-DPI work-area fitting, minimum size, polychrome atlas reuse, shader stride checks |
| `zed-idle-frames.patch` | 438 | `gpui/{platform,window}.rs`, `gpui_macos/window.rs` | `FramePacer`: stop frame requests while a window is idle |
| `zed-linux.patch` | 1,965 | `gpui_linux/*`, `gpui_wgpu/*` | Linux only; Linux is dropped (ADR-0008), so this one can be deleted |

### What the wingleeio fork adds over upstream

GitHub compare `zed-industries/zed main...wingleeio:zed:5d1f83d9f`: diverged, **17 ahead, 1,559 behind**, merge base `f14fea9bf` (2026-07-19). The 17 commits touch only `gpui` (6 files), `gpui_macos` (5) and `gpui_wgpu` (1), **nothing in `gpui_windows`**. Their content: EdgeFade and BackdropBlur primitives, macOS 26 blur material, macOS GPU memory bounds and destination-alpha fix, `ImageSource::evict` and `App::peek_asset`, object-fit crops, and a closing-punctuation line-wrap rule.

FrameEver's `src/` calls none of `EdgeFade`, `BackdropBlur`, `evict` or `peek_asset` (grep); its image dropping uses upstream's `Window::drop_image`. BackdropBlur would break the flat rule anyway. Upstream `08cbb88` already has a closing-punctuation rule (UAX #14 LB13, `gpui/src/text_system/line_wrapper.rs:530`) and `ObjectFit` (`gpui/src/elements/img.rs`). The Porter-Duff fix that the `Cargo.toml` comment says the rounded transparent shell needs lives in the Metal and wgpu renderers, so it matters for macOS later, not for the Windows DirectX path. On Windows, **leaving the fork loses nothing FrameEver uses.**

### Distance to GPUI Kit's gpui

`gpui-pre` is published by huacnlee (crates.io owners) from `script/bump-gpui.ts` in GPUI Kit, which renames Zed's Apache-2.0 crates (`gpui` → `gpui-pre`, `gpui_windows` → `gpui-pre-windows`, …), keeps `[lib] name = "gpui"` so `use gpui::*` still works, and audits that no GPL crate enters the closure. Every version records its Zed commit in the description:

| gpui-pre | Date | Zed commit | Distance from our base `f14fea9` |
|---|---|---|---|
| 0.3.1 | 2026-09-03 | `801c087` | 727 commits ahead |
| 0.3.10 | 2026-10-26 | `08cbb88` | 1,488 commits ahead |

No GPUI Kit release matches our base. `gpui-component 0.5.1` (2026-02) uses crates.io `gpui ^0.2.2` (2025-10), which is older than our base, and 0.6.0 onward uses `gpui-pre`. Between `f14fea9` and `08cbb88` the gpui crates (`gpui`, `gpui_windows`, `gpui_platform`, `gpui_wgpu`, `gpui_tokio`) changed in 148 files, +37,889 / −4,433 lines. The largest changes are in `window.rs` (+3,872), the new `profiler/` (+5,400), `gestures.rs` (+2,232), `wgpu_renderer.rs`, `elements/list.rs`, `key_dispatch.rs` and `div.rs`, plus a new `spring.rs` (`SpringConfig`, `SpringAnimation`, `sampled_easing`). `git apply --check` of our three live patches against `08cbb88` **fails in every patched file**, so they need a manual rebase.

### The migration path, and what it costs

1. Re-base `../zed-cap` on upstream Zed `08cbb88` (the `gpui-pre 0.3.10` snapshot) instead of wingleeio. Rebase `zed-gpui.patch`, `zed-windows.patch` and `zed-idle-frames.patch`; delete `zed-linux.patch`. The main conflicts will be in `window.rs` and `platform.rs`, which upstream rewrote heavily. Rough size: one ticket of several days, test-first against the patches' own unit tests, which travel with them.
2. Produce local `gpui-pre-*` crates from that checkout with GPUI Kit's own `script/bump-gpui.ts --zed ../zed-cap --stage-only`, which writes the renamed workspace to `target/gpui-pre/workspace` without publishing. Point `[patch.crates-io]` at it. The script needs Bun; `prepare-gpui-dependency.sh` would call it or carry the equivalent renaming.
3. Depend on gpui through the same package names in the app and converter (`gpui = { package = "gpui-pre", version = "=0.3.10" }`, likewise `gpui_platform`), or Cargo builds two incompatible gpui copies. `gpui_tokio` is not in the `gpui-pre` set: vendor its 100-line crate (Apache-2.0) with its `gpui` dependency renamed. FrameEver uses it in 54 places.
4. Fix FrameEver's own breakage from 1,488 upstream commits across a 160k-line app. This is unmeasured; a `pnpm fe check` on the rebased checkout is the first step of the ticket.
5. Add `gpui-kit = "=0.8.x"` with default features (styled components and icons) and keep it pinned exactly. GPUI Kit moves its `gpui-pre` pin almost weekly, so each GPUI Kit upgrade is a planned ticket that rebases our patches onto the new snapshot. It should not be an automatic bump.

Upstreaming the generally useful parts of our patches (custom frame insets, idle frame pacing, `paint_surface_fitted`) to Zed would shrink that recurring cost, but it is optional.

## 3. Mapping our 23 components

"Mentions" counts lines outside `src/ui/` that name the type. It is a rough grep, shown only to rank migration effort. GPUI Kit names are `gpui_kit::component::…` modules at 0.8.1.

| Ours (`src/ui/`) | Lines | Mentions | GPUI Kit replacement | Verdict |
|---|---|---|---|---|
| `text_input.rs` (`TextInput`, IME, undo, key bindings) | 2,680 | 171 | `input::{Input, InputState}`, `textarea`, `search`, `input::group` (input with prefix and suffix) | **Replace.** Biggest win; GPUI Kit's input implements `EntityInputHandler`, so IME composition, selection and undo are maintained upstream |
| `button.rs` (`Button`, `IconButton`, variants, sizes) | 701 | 174 | `button::{Button, ButtonGroup, DropdownButton, Toggle}` | **Replace** |
| `menu.rs` (`Menu`, `MenuItem`, `MenuKey`) | 564 | 143 | `menu` (popup, context and dropdown menus), `native_menu` | **Replace** |
| `slider.rs` | 489 | 341 | `slider::Slider` (horizontal and vertical, range, step) | **Replace** |
| `color_picker.rs` (HSV square, hue, hex) | 463 | 16 | `color_picker::{ColorPicker, ColorSelect}` (HSV, hex, alpha, featured colours) | **Replace**, after checking that its saturation/value square matches the editor's needs |
| `segmented.rs` (`SegmentedControl`) | 397 | 20 | `tab::TabBar` segmented variant (`tab_bar_segmented` token) or `ButtonGroup` | **Replace** |
| `field.rs` (`Field`, `Subfield`) | 360 | 186 | `form::Field`, `form::Form`, `label` | **Replace** |
| `number_field.rs` | 349 | 6 | `input::NumberInput` (`base::number_input`, decimal feature) | **Replace** |
| `collapsible.rs` | 306 | 35 | `collapsible`, `accordion` | **Replace** |
| `surface.rs` (`Card`, `Popover`, `Section`, `SettingRow`) | 298 | ~70 | `popover::Popover`, `group_box::GroupBox`, `setting::{SettingPage, SettingGroup, SettingItem}` | **Replace.** The settings window maps onto `setting` almost one to one |
| `editor_button.rs` | 285 | 38 | `button::Button` (ghost, selected) with `kbd` and `tooltip` | **Thin wrapper** over GPUI Kit's button |
| `position_pad.rs` | 231 | 5 | — | **Keep** (domain control: nine-point anchor pad) |
| `selection_header.rs` | 193 | 4 | — | **Keep** (library selection bar; built from GPUI Kit buttons) |
| `progress.rs` (`CircularProgress`) | 189 | 2 | `progress::{Progress, ProgressCircle}`, `spinner` | **Replace** |
| `select.rs` | 188 | 17 | `select::Select`, `combobox` (searchable) | **Replace** |
| `tab_rail.rs` (`TabRail`) | 187 | 2 | `sidebar::Sidebar` or vertical `tab::TabBar` | **Replace** |
| `radio_cards.rs` | 185 | 1 | `radio::{Radio, RadioGroup}` with a custom card body | **Thin wrapper** |
| `toggle.rs` | 160 | 38 | `switch::Switch`; `checkbox` | **Replace** |
| `kbd.rs` (`KbdChip`) | 154 | 1 | `kbd::Kbd` | **Replace** |
| `tooltip.rs` | 122 | 28 | `tooltip::Tooltip`, `hover_card` | **Replace** |
| `windows_caption.rs` | 114 | 10 | `title_bar::TitleBar` (draws Windows caption buttons itself), `window_border` | **Decide in the spike.** Our caption sits on the custom-frame patch; adopt `TitleBar` only if it works with that frame and snap layouts |
| `microphone_level.rs` | 79 | 4 | `progress` at a pinch | **Keep** (live audio meter) |
| `mod.rs` | 128 | — | — | Becomes a re-export of GPUI Kit plus FrameEver's own controls |

Result: 17 replaced, 3 thin wrappers, 3 kept, plus `windows_caption` to be decided. Roughly 7,000 of the 10,000 lines in `src/ui/` go away.

### Gaps GPUI Kit does not fill

| Need | What GPUI Kit offers | Plan |
|---|---|---|
| Video preview / player | nothing video-specific; `image` for stills | Keep our `paint_surface` / `paint_surface_fitted` preview; build the transport (play, scrub, time) from GPUI Kit buttons, slider and kbd |
| Editor timeline (tracks, clips, zoom, playhead, waveform) | `virtual_list`, `resizable`, `plot`, `chart`, `scroll` | **Custom**, as today; take scroll and resize behaviour from `gpui-base` |
| Recording area selection, overlays, countdown, camera bubble | — | Custom (platform windows) |
| Audio level meter | — | Keep `microphone_level` |
| Hotkey recorder | `kbd` to show the keys | Custom capture; GPUI Kit for display |
| Library grid with date scrubber (Immich-style, see `ui-references.md` §6) | `virtual_list` (variable item sizes), `list`, `table` | Custom grid on `virtual_list` |
| Colour picker for backgrounds | `color_picker` | Covered |
| Command palette (Ctrl+K) | `command` | Covered |
| Dialogs, sheets, toasts | `dialog`, `alert_dialog`, `sheet`, `notification` | Covered; FrameEver has none of its own today |
| Editor sidebars and split panes | `resizable`, `dock` | Covered |

## 4. Theming on three OKLCH token layers

GPUI Kit's theme already has two of the three layers. Its `ThemeColor` (`crates/component/src/theme/theme_color.rs`, 136 fields) mixes shadcn's **semantic** names (`background`, `foreground`, `muted`, `muted_foreground`, `primary`, `primary_foreground`, `secondary`, `accent`, `border`, `input`, `ring`, `popover`, `selection`, `danger`, `success`, `warning`, `info` with `_hover` and `_active`) with **component** names (`button_primary_hover`, `switch_thumb`, `slider_bar`, `slider_thumb`, `tab_active`, `tab_bar_segmented`, `sidebar_*`, `title_bar`, `list_*`, `table_*`, `scrollbar_thumb`, `progress_bar`, `caret`, `overlay`, `window_border`). `gpui-base` adds a smaller `SemanticThemeTokens` with colour, radius, spacing, typography and shadow scales (`crates/base/src/theme_tokens.rs`). Colours are stored as `Hsla`. `theme/color.rs` provides `oklch(l, c, h) -> Hsla`, `mix_oklab` and lighten/darken. Theme JSON accepts hex or Tailwind-style palette references (`neutral-200`, `red-500/60`) from `default-colors.json`, whose entries are defined in OKLCH. JSON cannot take an `oklch(...)` string (`try_parse_color`).

Proposed mapping, all in Rust code, not JSON:

| Layer | Lives in | Contents |
|---|---|---|
| 1 Brand | `theme.rs` constants as `(L, C, h)` | 12-step OKLCH ramps (Radix method): `klein-1…12` around `oklch(0.463 0.233 266.4)`; `neutral-1…12` at hue 266 with very low chroma; `red`, `amber`, `green` for recording, warning and success. Dark and light ramps are generated separately, as Radix does |
| 2 Semantic | a `Semantic` struct built from layer 1 per appearance | shadcn names: `background`, `surface`, `foreground`, `muted`, `primary` = Klein, `accent`, `border`, `ring`, `selection`, `danger`, `recording`, … with hover and active steps chosen by ramp step, never by ad-hoc lightening |
| 3 Component | GPUI Kit's `ThemeColor` component fields, plus a FrameEver `EditorTokens` for what GPUI Kit has no name for (timeline track, clip, playhead, waveform, stage, crop handles) | each field assigned from layer 2 only |

Rules that keep it within our standing rules:

- Set `Theme.shadow = false`, and keep `radius` and `radius_lg` at our values. Do not use the theme's `linear-gradient(...)` background support. GPUI Kit's dialog shadow (`blur_radius` in `dialog/dialog.rs`) needs checking against the flat rule in the spike.
- Set `font_family` to Inter. Check in the spike how gpui font fallback reaches MiSans for Chinese glyphs. Note that `pnpm fe setup` currently installs HarmonyOS Sans (`scripts/fe.mjs`, `chineseFontDir`), not MiSans; the owner should settle which one.
- Write a unit test that walks every semantic foreground/background pair and asserts WCAG AA (4.5:1 text, 3:1 UI). `ui-references.md` §5 already computes these contrasts for the Klein palettes.
- This replaces today's `Theme` (Radix `gray_1…12`, `blue_*`, legacy Tailwind greys) and the per-surface constructors (`::body`, `::settings`, `::glass`, `::plain`) described in `src/ui/mod.rs`. The `::glass` surface is against the flat rule and goes away.

## 5. Which published design system to borrow from

| System | Licence | Fit with our rules | Take | Leave |
|---|---|---|---|---|
| [shadcn/ui](https://github.com/shadcn-ui/ui) | MIT, 126,894 stars | Flat by default; v4 tokens are OKLCH surface/`-foreground` pairs with one `--radius` (`ui-references.md` §3, [theming](https://ui.shadcn.com/docs/theming)). **GPUI Kit's theme is shadcn's token set** (`default-theme.json`: `"author": "shadcn"`, `_ref` to shadcn's neutral palette), and its design guide credits shadcn's methods | Token names, semantic/foreground pairing, component anatomy and variants (default/secondary/ghost/outline/destructive), form and settings layouts | Its web-only behaviour; its default near-black primary (ours is Klein blue) |
| [Radix Colors](https://github.com/radix-ui/colors) | MIT | Our current `Theme` already uses its steps | The 12-step role scale (1–2 backgrounds, 3–5 component states, 6–8 borders, 9–10 solids, 11–12 text) as the shape of the brand layer, generated in OKLCH | Its sRGB palettes themselves |
| [Fluent 2](https://fluent2.microsoft.design) / [microsoft/fluentui](https://github.com/microsoft/fluentui) | MIT | Native Windows feel. Global → alias token split (`packages/tokens/src/global`, `alias/darkColor.ts`, 159 alias colour names such as `colorNeutralBackground1`, `colorBrandBackgroundHover`, `colorStrokeFocus2`) matches brand → semantic. Elevation by lightness, accent used sparingly (`ui-references.md` §1) | Keyboard model (Tab between regions, arrows inside groups, Esc, access keys), focus-visible ring, motion ladder 83/167/250 ms and curves, caption-button layout, density and control heights, high-contrast mode as a later target | Mica, Acrylic and any material or blur; Segoe as the typeface |
| [Material 3](https://m3.material.io) / [material-color-utilities](https://github.com/material-foundation/material-color-utilities) | Apache-2.0 | Tonal surfaces, ripple states and expressive shapes sit badly with "flat, calm, premium". HCT tonal palettes do what OKLCH ramps already do for us | Nothing directly; springs as a motion idea are now native in upstream gpui | Everything |

So: **shadcn/ui is the reference system** the HTML design reference copies (tokens, components, states). Radix Colors supplies the ramp method, and Fluent 2 supplies Windows interaction and motion. All three are MIT. Because shadcn maps one to one onto GPUI Kit's `ThemeColor`, whatever the HTML reference specifies can be implemented without inventing names on either side.

## 6. Recommendation

1. **Build on GPUI Kit** (`gpui-kit`, Apache-2.0), using its styled `gpui-component` layer and dropping to `gpui-base` behaviour for FrameEver-specific controls. Pin it exactly and upgrade it only in planned tickets.
2. **First ticket, before any UI work: move gpui to upstream Zed `08cbb88`** (the `gpui-pre 0.3.10` snapshot) and leave the wingleeio fork. Rebase `zed-gpui`, `zed-windows` and `zed-idle-frames` patches, delete `zed-linux.patch`, stage the crates under `gpui-pre-*` names with GPUI Kit's `bump-gpui.ts --stage-only`, vendor `gpui_tokio`, and get `pnpm fe check` green. Finish it with a spike: one window with GPUI Kit's `Root`, input, select and title bar on our custom frame.
3. **Rebuild `theme.rs` as three OKLCH layers** that fill GPUI Kit's `ThemeColor` (shadow off, our radii and fonts, a WCAG AA contrast test), then migrate controls in order of payoff: `text_input`, `button`, `menu`, `slider`, `select`/`segmented`/`toggle`/`tooltip`/`kbd`, the settings window onto `setting`, then the rest. Keep `position_pad`, `selection_header` and `microphone_level`, and build the video preview, timeline and library grid ourselves.
4. **Borrow from shadcn/ui** for tokens and component anatomy, **Radix Colors** for the 12-step ramp method and **Fluent 2** for Windows keyboard, focus, motion and caption patterns. Take nothing from Material 3 or from Fluent's materials.

Open points for the owner: whether to accept a weekly-moving upstream gpui (a pinned GPUI Kit upgraded every month or two keeps it bounded); MiSans or HarmonyOS Sans as the Chinese face; and whether to upstream the custom-frame and idle-frame patches to Zed to cut the recurring rebase.
