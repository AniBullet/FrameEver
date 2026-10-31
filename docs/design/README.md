# Design reference

The single visual source of truth for FrameEver's UI. Open `index.html` by double-clicking it; nothing needs a server or the network. Decided in "Design reference page: how is it assembled and kept as the source of truth?" (AniBullet/FrameEver#49). It stays after 1.0.

## What is here

| File | Holds |
|---|---|
| `tokens.json` | The one source for every token: brand ramps, semantic and component layers, type, space, radius, size, motion |
| `tokens.css`, `tokens.js` | Generated from `tokens.json` by `pnpm fe design`. Never edit them by hand; a script test fails when they drift |
| `design.css` | Component styles shared by every page |
| `design.js` | The page shell (navigation and toggles), token helpers and the hand-drawn doodles (`FE.doodle`) |
| `icons.js` | The Lucide glyphs these pages use |
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
4. Change the code afterwards. `theme.rs` gets a test against `tokens.json` during implementation, so a drift fails in CI.

## Landing a surface

1. A surface prototype ticket explores on its own `prototype/<name>` branch, with several variants if needed.
2. The owner picks one.
3. Only the chosen variant is tidied into `surfaces/<surface>.html` on the final tokens and merged to main. Reuse `design.css` and the shell, and add the page to `surfaces` in `design.js` by setting it to `true`.
4. Close the ticket with a link to the page.

Each surface ticket edits its own files, so they can run in parallel. A shared style goes into `design.css`; a token goes into `tokens.json`. Rejected variants stay on their branch.

## During implementation

- Every implementation ticket links to the section it implements.
- A PR that changes how something looks changes this page in the same PR.
- The owner accepts the work by comparing the running app with this page.
