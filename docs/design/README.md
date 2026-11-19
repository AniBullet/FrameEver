# Design reference

The single visual source of truth for FrameEver's UI. Open `index.html` by double-clicking it; nothing needs a server or the network. Decided in "Design reference page: how is it assembled and kept as the source of truth?" (AniBullet/FrameEver#49). It stays after 1.0.

## What is here

| File | Holds |
|---|---|
| `tokens.json` | The one source for every token: brand ramps, semantic and component layers, type, space, radius, size, motion |
| `tokens.css`, `tokens.js` | Generated from `tokens.json` by `pnpm fe design`. Never edit them by hand; a script test fails when they drift |
| `design.css` | Component styles shared by every page |
| `design.js` | The page shell (navigation and toggles), token helpers and the hand-drawn doodles (`FE.doodle`) |
| `icons.js` | The Lucide glyphs these pages use, named as in `apps/desktop-gpui/assets/icons/sources.json` and copied from the pinned release |
| `index.html` | Tokens and components |
| `surfaces/<surface>.html` | One page per surface, with its own `.css` and `.js` when it needs them |

The "设计页" row in the "FrameEver 开发" pane opens each page in your default browser; add a surface there when it lands.

Inter comes from `apps/desktop-gpui/assets/fonts/`. HarmonyOS Sans is read from the same folder after `pnpm fe setup`; its licence forbids committing it, so a machine without it falls back to a system font and the page says so.

## Toggles

The bar at the top sets theme (dark by default), language and scale (100, 150, 200%). They live in the URL hash, so a link keeps them: `surfaces/main-window-library.html#theme=light&lang=en&scale=150`. Every piece of copy is written in both languages with `FE.L(en, zh)`, following `docs/copy-style.md`. In Chinese, library is 资源库 (`docs/i18n.md`).

## Changing a token

1. Change `tokens.json`.
2. Run `pnpm fe design`.
3. Check `index.html`: the contrast table recomputes from the tokens and must stay free of failures.
4. Change `apps/desktop-gpui/src/theme/tokens.rs` in the same commit, or `theme/motion.rs` for motion. Their tests compare the brand, semantic and component layers, sizes, radii, durations and curves with `tokens.json`, and check the contrast pairs, so a drift fails in CI. The doodles in `design.js` are tested against `onboarding_doodles.rs` the same way.

## Landing a surface

1. Read the background first: the Klein blue lists in `index.html`, `docs/copy-style.md`, `GLOSSARY.md`, the tickets that touched the surface, and upstream's code for it (`upstream/main`). Then compare with the most popular open-source products that have the same surface.
2. Build the page directly in `surfaces/<surface>.html` of the main checkout, with its own `.css` and `.js`. No prototype branch, worktree or ignored folder. Reuse `design.css` and the shell; a new page is added to `surfaces` in `design.js` and to the "设计页" row of the dev pane.
3. Load the page in a browser in both themes and both languages and check the console. `pnpm fe test` and `pnpm fe lint` do not load these pages, and `FE.icon` throws on a name `icons.js` lacks, which leaves the page empty.
4. Close the ticket with a link to the page. The owner reviews it there; changes go in as new commits on the same files.

A shared style goes into `design.css`; a token goes into `tokens.json`.

## Rules for a surface page

- Every capability the surface has upstream keeps a place, cloud and account features aside. The page ends with a "Where everything went" table that names the place of each one; implementation tickets check against it.
- The surface holds controls and state, not the manual (`docs/copy-style.md`, "Progressive disclosure"). What a hint would say goes into a tooltip, a popover or that table.
- Icons: only names listed in `apps/desktop-gpui/assets/icons/sources.json`, with the path data copied from the pinned file in `apps/desktop-gpui/assets/icons/`. A test keeps an icon out of that list until the app draws it, so a design that needs a new glyph names it in its ticket and uses a listed one until the feature lands.
- A page shows the states a press can reach; what it cannot show (drags, hover-only controls, shortcuts) is written in its tables.

## Where a setting lives

A ticket that adds a setting or a preset decides its places with these rules before it is built, and its design page shows every one of them.

1. **The settings window holds every preference.** Anything the app saves and the person can choose has a row in the settings window, so settings search finds it. A value the app only remembers (a window position, the last export folder, the last preset used in the editor) is not a preference and has no row.
2. **A control beside the work is a copy, never the only place.** The recording options popover, the picker's options and the recording bar hold only what a person changes from one recording to the next. Each of them writes the same stored value as its settings row, and the popover's link opens the settings group that holds the rest.
3. **Settings are grouped by what they act on.** A row that works in only one recording mode sits in that mode's group (Studio recording, Instant recording), not in a shared group where it reads as if it covered both.
4. **A preset is managed in one place and offered wherever it is chosen.** Every control that chooses a preset ends with New…, Import… and Manage presets…. The list they open is in the settings window and does not need an open project. Presets that only the editor uses (style presets) are managed in the editor.
5. **State shows where the person starts.** When an option changes what a recording will contain (Show input, Click highlight), the main window says so without opening anything.

AniBullet/FrameEver#180 adds the test that keeps rule 1: every key of the settings store either has a settings row or is named in the list of values the app only remembers. Until spec AniBullet/FrameEver#179 lands, the app does not follow these rules everywhere; its Problem Statement lists where.

## During implementation

- Every implementation ticket links to the section it implements.
- A PR that changes how something looks changes this page in the same PR.
- The owner accepts the work by comparing the running app with this page.
