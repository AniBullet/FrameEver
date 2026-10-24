# UI references: look, motion and interaction (research for #24)

Research for [AniBullet/FrameEver#24](https://github.com/AniBullet/FrameEver/issues/24), under the map [#23](https://github.com/AniBullet/FrameEver/issues/23). Gathered 2026-10-24 from primary sources: official guidelines, the products' own changelogs and blogs, and source code. Star counts were read with `gh api` on the same day.

Standing preferences this is measured against (from #23): pure flat (no glass, blur or gradients), calm and premium, Klein blue `#1F3FD8` accent, dark first with a complete light theme, Inter + MiSans, English + Simplified Chinese, WCAG AA, every action reachable by keyboard, crisp at 100–200% scaling, OKLCH tokens in three layers.

## Short answer

- The platforms agree on four things FrameEver can take without glass: neutrals carry the interface and the accent is rare; lighter surfaces mean more important; entering motion decelerates hard and exits accelerate; motion is short, optional and interruptible.
- The most praised recent product interfaces (Linear's 2026 refresh, Raycast and Zed arriving on Windows in 2025, PowerToys Command Palette) are calm, dense, near-neutral and keyboard-first, with a command palette and shortcuts shown next to actions.
- `#1F3FD8` is `oklch(0.463 0.233 266.4)`. White text on it reaches 7.41:1, but as text on a dark background it reaches only about 2.4:1. So the dark theme needs a separate, lighter accent for text and focus rings (about `L 0.72–0.76`), while `#1F3FD8` stays the solid fill.
- GPUI's `Animation::with_easing` accepts any `Fn(f32) -> f32`, so Fluent cubic-bezier curves and Material-style springs can both be written as plain functions. Neither platform's motion system is out of reach.
- Three directions: **A. Graphite Studio** (near-neutral, dense, Linear/Zed-like), **B. Paper and Ink** (warm neutrals, airy, hand-drawn touches, Excalidraw-like), **C. Klein Field** (blue-tinted neutrals, bolder accent moments, spring motion, inspired by Material 3 Expressive).

## 1. Platform design languages

### Windows 11 / Fluent 2

| Aspect | What the source says | Take for FrameEver |
| --- | --- | --- |
| Colour | Each theme is "a set of neutral color values"; "darker colors indicate background surfaces of less importance", and important surfaces are lighter ([Color in Windows](https://learn.microsoft.com/en-us/windows/apps/design/signature-experiences/color)). Accent colours are "used sparingly" to highlight important elements and show state (same page). Fluent 2 warns against brand colours "on large surfaces", and says semantic colours are not for decoration ([Fluent 2 Color](https://fluent2.microsoft.design/color)). | Elevation by lightness steps, not shadows or blur. The accent belongs to primary buttons, selection and focus. |
| Tokens | Global tokens carry no context; alias tokens add it ([Fluent 2 Color](https://fluent2.microsoft.design/color)). | Matches the brand → semantic → component layering on the map. |
| Durations | WinUI control durations: 83 / 167 / 250 ms ([Timing and easing](https://learn.microsoft.com/en-us/windows/apps/design/motion/timing-and-easing)). Fluent UI tokens: 50, 100, 150, 200, 250, 300, 400 and 500 ms ([durations.ts](https://github.com/microsoft/fluentui/blob/master/packages/tokens/src/global/durations.ts)). | Two short ladders to choose from; 83/167/250 is the native Windows feel. |
| Curves | Entering: `cubic-bezier(0, 0, 0, 1)` ("Fast Out, Slow In"). Exiting: `cubic-bezier(1, 0, 1, 1)` ([Timing and easing](https://learn.microsoft.com/en-us/windows/apps/design/motion/timing-and-easing)). Full set: decelerate max/mid/min `(0.1,0.9,0.2,1)` / `(0,0,0,1)` / `(0.33,0,0.1,1)`, easy-ease `(0.33,0,0.67,1)` ([curves.ts](https://github.com/microsoft/fluentui/blob/master/packages/tokens/src/global/curves.ts)). | Use decelerate for things appearing and accelerate for things leaving. |
| Principles | Motion is functional, natural, consistent and appealing ([Fluent 2 Motion](https://fluent2.microsoft.design/motion)). | — |
| Keyboard | PowerToys Command Palette (Win+Alt+Space) is "designed for keyboard-first workflows". It uses prefixes (`>` commands, `$` settings, `??` web, `=` calculator), Alt+1–9 to run visible results, and Ctrl+K for a result's context menu ([Command Palette overview](https://learn.microsoft.com/en-us/windows/powertoys/command-palette/overview)). | A current Windows-native model for a FrameEver command palette, including Ctrl+K for per-item actions. |

Avoid: Mica and Acrylic backdrops. They are translucency materials, which the no-glass rule excludes.

### Material 3 Expressive (Google, announced around I/O 2025)

- Research claims (Google's own): 46 studies, more than 18,000 participants, key UI elements spotted "up to four times faster", and the strongest preference among 18–24-year-olds ([Google Design](https://design.google/library/expressive-material-design-google-research)). Its tactics are colour, shape, size, motion and containment (same source).
- Motion is spring-based and split into **spatial** springs (position, size) and **effects** springs (colour, opacity), each with fast, default and slow speeds. The Compose source gives the values:
  - Expressive spatial: damping 0.6 / 0.8 / 0.8, stiffness 800 / 380 / 200 (fast / default / slow). Effects: damping 1.0, stiffness 3800 / 1600 / 800 ([ExpressiveMotionTokens.kt](https://github.com/androidx/androidx/blob/androidx-main/compose/material3/material3/src/commonMain/kotlin/androidx/compose/material3/tokens/ExpressiveMotionTokens.kt)).
  - Standard spatial: damping 0.9, stiffness 1400 / 700 / 300. Effects are the same as expressive ([StandardMotionTokens.kt](https://github.com/androidx/androidx/blob/androidx-main/compose/material3/material3/src/commonMain/kotlin/androidx/compose/material3/tokens/StandardMotionTokens.kt)).
- Take for FrameEver: **colour and opacity never overshoot (critically damped); only movement may overshoot slightly.** The standard scheme (damping 0.9) suits a calm tool. The expressive one (0.6) is for one or two signature moments at most.
- Avoid: the playful shape morphing and large tonal colour fields, which go against calm and flat.

### Apple HIG (Liquid Glass era, WWDC 2025)

- Apple's stated aims are "greater focus to content", controls as "a distinct functional layer that sits above apps", and shapes "concentric with the rounded corners of modern hardware and app windows" ([Apple Newsroom, 2025-06](https://www.apple.com/newsroom/2025/06/apple-introduces-a-delightful-and-elegant-new-software-design/)).
- The HIG restricts glass to the control layer ("Don't use Liquid Glass in the content layer"; "Use Liquid Glass effects sparingly") and says its appearance changes under settings that "reduce transparency or increase contrast" ([HIG Materials](https://developer.apple.com/design/human-interface-guidelines/materials)).
- On motion: "Add motion purposefully", "Make motion optional", "Aim for brevity and precision in feedback animations", avoid motion on interactions that happen often, and "Let people cancel motion" ([HIG Motion](https://developer.apple.com/design/human-interface-guidelines/motion)).
- Take for FrameEver: a separate control layer drawn flat (opaque raised surface plus a 1 px border instead of glass), concentric radii (inner radius = outer radius − padding), and no animation on high-frequency actions such as list hover or timeline scrubbing.

## 2. Products launched or redesigned in 2025–26

| Product | Colour and neutrals | Type and density | Motion | Interaction |
| --- | --- | --- | --- | --- |
| **Linear**, UI refresh 2026-03-12 | Default palettes moved from cool blue-leaning grey to a "warmer gray that is less saturated"; going too warm looked muddy. The sidebar is "a few notches dimmer" so content leads. Borders are softer and many were removed ([blog](https://linear.app/now/behind-the-latest-design-refresh), [changelog](https://linear.app/changelog/2026-03-12-ui-refresh)). Themes come from three inputs (base, accent, contrast) in LCH instead of 98 variables, and the blue was kept out of the neutrals for a "timeless" look ([2024 redesign](https://linear.app/now/how-we-redesigned-the-linear-ui)). | Inter Display for headings, Inter for the rest ([2024 redesign](https://linear.app/now/how-we-redesigned-the-linear-ui)). A more compact tab bar, smaller icons, and icon-only pills for the first tabs; density kept "without overwhelming" ([blog](https://linear.app/now/behind-the-latest-design-refresh)). | Not described in either post. | Headers and view controls made consistent across every object type; an Enter or Ctrl+Enter send preference; Ctrl+number shortcuts that work on non-US layouts ([changelog](https://linear.app/changelog/2026-03-12-ui-refresh)). |
| **Raycast for Windows**, public beta 2025-11-20 | "A design that fits right in" on Windows, not "ported over" ([blog](https://www.raycast.com/blog/raycast-for-windows)). | — | — | "Familiar keyboard shortcuts"; a hotkey launch where you type and press Tab for Quick AI; keyboard window management; its own real-time file indexer ([blog](https://www.raycast.com/blog/raycast-for-windows)). |
| **Zed for Windows**, 2025-10-15 (GPUI, the same framework as FrameEver) | One Dark theme: editor `#282c33`, panels `#2f343e`, chrome `#3b414d`, border `#464b57`, focused border `#47679e`, text `#dce0e5`, muted `#a9afbc` ([one.json](https://github.com/zed-industries/zed/blob/main/assets/themes/one/one.json)). Neutrals are slightly blue-tinted, and hover and selection are a single lightness step. | — | GPUI `Animation` has a duration, repeat and `with_easing(Fn(f32)->f32)`. Built-in easings: `linear`, `quadratic`, `ease_in_out`, `ease_out_quint`, `bounce`, `pulsating_between` ([animation.rs](https://github.com/zed-industries/zed/blob/main/crates/gpui/src/elements/animation.rs); FrameEver's pinned fork has the same API: [fork rev](https://github.com/wingleeio/zed/blob/5d1f83d9f27a19bec1fb241dc33b42238af9cf8d/crates/gpui/src/elements/animation.rs)). | Renders natively with DirectX 11 and DirectWrite rather than Electron ([blog](https://zed.dev/blog/zed-for-windows-is-here)). Its command palette and keymap are the GPUI reference for keyboard-first navigation. |
| **PowerToys Command Palette** (docs updated 2026-10) | Theme follows the user's settings. | Compact mode: a small search box that grows when it has results ([docs](https://learn.microsoft.com/en-us/windows/powertoys/command-palette/overview)). | — | Prefix commands, a Home page with pinned and recent items, Alt+number to run, Ctrl+K for per-item context, a tip when the shelf is empty (same source). |
| **Screen Studio** (a Mac screen recorder known for its automatic motion; included as the closest product in FrameEver's own category) | — | — | Automatic zoom on clicks, cursor smoothing ("Shaky movement becomes a steady glide"), and optics-based motion blur ([site](https://screen.studio/)). | Presets for backgrounds, padding and shadow; cursor size and style can be changed after recording (same source). |

What these have in common: near-neutral greys with a single accent; content brighter than the chrome; one family of type with display cuts used for headings; compact but aligned layouts; a command palette with prefixes; shortcuts shown next to actions. Motion is rarely mentioned in their write-ups at all, which suggests it stays quiet.

## 3. Most-starred open-source desktop and web apps (second source)

Stars and last push date on 2026-10-24 (`gh api repos/<owner>/<repo>`); the list was cross-checked against `gh search repos --topic desktop-app|electron|gpui --sort stars`.

| Repo | Stars | Last push | What its code shows |
| --- | --- | --- | --- |
| [microsoft/vscode](https://github.com/microsoft/vscode) | 194,262 | 2026-10-24 | A command palette (Ctrl+Shift+P) that gives access to all functionality ([docs](https://code.visualstudio.com/docs/getstarted/userinterface)). |
| [microsoft/PowerToys](https://github.com/microsoft/PowerToys) | 139,757 | 2026-10-23 | Command Palette and its Dock: keyboard-first Windows patterns (see section 2). |
| [excalidraw/excalidraw](https://github.com/excalidraw/excalidraw) | 134,466 | 2026-10-23 | A hand-drawn look made readable. Its own font, Excalifont, replaced Virgil to keep the sketch feel with better legibility ([Excalifont](https://plus.excalidraw.com/excalifont)). The model for FrameEver's few hand-drawn touches. |
| [shadcn-ui/ui](https://github.com/shadcn-ui/ui) | 126,412 | 2026-10-24 | Tokens in OKLCH as surface/`-foreground` pairs (`background`, `card`, `popover`, `primary`, `muted`, `accent`, `border`, `ring`) with one `--radius` that the other radii derive from. Its dark default is near-neutral, for example `background oklch(0.145 0 0)` and `border oklch(1 0 0 / 10%)` ([theming](https://ui.shadcn.com/docs/theming)). |
| [immich-app/immich](https://github.com/immich-app/immich) | 117,069 | 2026-10-24 | A large-library timeline: virtual scrolling, a draggable scrubber to jump through big libraries, folder view, stacks, archive and favourites ([README](https://github.com/immich-app/immich/blob/main/README.md)). The closest reference for the new library window. |
| [zed-industries/zed](https://github.com/zed-industries/zed) | 92,126 | 2026-10-24 | GPUI itself: theme JSON, the animation API, keymaps (see section 2). |
| [localsend/localsend](https://github.com/localsend/localsend) | 95,399 | 2026-10-21 | A calm single-purpose desktop tool (Flutter). |
| [obsproject/obs-studio](https://github.com/obsproject/obs-studio) | 78,060 | 2026-10-23 | Mostly a counter-example: dense, panel-heavy recording UI. |
| [longbridge/gpui-kit](https://github.com/longbridge/gpui-component) (formerly gpui-component) | 18,577 | 2026-10-24 | More than 75 GPUI components with semantic themes, dock layouts and virtual lists ([README](https://github.com/longbridge/gpui-component)). Reference code for building lists, tables and docks in GPUI. |

Also useful as a token method: Radix Colors' 12-step scale. Steps 1–2 are backgrounds, 3–5 are component normal/hover/pressed, 6–8 are borders (8 is "stronger borders on interactive components and focus rings"), 9–10 are solid fills (9 has the "highest chroma"), and 11–12 are text ([Radix scale](https://www.radix-ui.com/colors/docs/palette-composition/understanding-the-scale)). Vercel Geist uses the same idea with a 100–1000 grey ladder plus alpha greys ([design.md](https://vercel.com/design.md)).

## 4. Facts that constrain any direction

- `#1F3FD8` = `oklch(0.463 0.233 266.4)`. Contrast (WCAG 2): 7.41:1 with white text on it; 2.76:1 as text on black; about 2.35:1 on a `#17171A` surface. So it passes AA as a fill with white text in both themes, and as text on light backgrounds (about 7.1–7.3:1), but **fails as text, icon or focus colour on dark backgrounds**. Every direction below adds a dark-theme `accent-text` at `L ≈ 0.72–0.76`, same hue.
- Hover on a `#1F3FD8` fill should go lighter in dark mode (L 0.52–0.53, about 5.5:1 with white) and darker in light mode (L 0.42, 8.9:1), which follows the Windows "lighter = more important" rule.
- Inter 4.0 ships "Display" designs on an `opsz` axis and as a separate family, Inter Display ([Inter v4.0 release](https://github.com/rsms/inter/releases/tag/v4.0)). Linear uses it for headings (section 2). FrameEver can use Inter Display for large titles without adding a font family.
- GPUI motion: any easing function is allowed, so `cubic-bezier(0,0,0,1)` and a damped spring can each be written once as a helper. The pinned fork has no `max_fps`, so long looping animations, such as a recording pulse, cost a frame every refresh.

## 5. Three candidate directions

The contrast values below were computed from the OKLCH values (sRGB, WCAG 2 formula). Hex values are rounded and given only for orientation. "Accent" is always `oklch(0.463 0.233 266.4)` = `#1F3FD8`, with white (`oklch(0.99 0 0)`) on it. Borders are around 1.5–1.75:1 and decorative; focus rings use `accent-text` (dark) or `accent` (light), both above 7:1.

### A. Graphite Studio: calm, dense, keyboard-first

The professional-tool reading: Linear and Zed on Windows. The neutrals are almost achromatic with a faint cool cast at hue 266, so the blue looks deliberate; the accent appears only on the primary action, the selection and focus. Compact density (32 px rows, 13 px body), and one command palette (Ctrl+K) with prefixes, modelled on PowerToys. Motion follows Fluent: 83/167/250 ms, decelerate `(0,0,0,1)` to enter, accelerate to exit, no overshoot. Hand-drawn touches only in empty states and onboarding.

| Token | Dark | Light |
| --- | --- | --- |
| bg | `oklch(0.165 0.004 266)` `#0D0E10` | `oklch(0.985 0.002 266)` `#F9FAFB` |
| surface | `oklch(0.195 0.005 266)` `#141517` | `oklch(0.965 0.003 266)` `#F2F3F5` |
| raised | `oklch(0.230 0.006 266)` `#1C1D20` | `oklch(1 0 0)` `#FFFFFF` |
| border-subtle | `oklch(0.270 0.006 266)` `#252629` | `oklch(0.920 0.004 266)` `#E3E4E7` |
| border | `oklch(0.330 0.008 266)` `#33353A` | `oklch(0.860 0.006 266)` `#CFD1D5` |
| text-muted | `oklch(0.720 0.010 266)` `#A2A5AB` (7.4:1 on surface) | `oklch(0.500 0.012 266)` `#60636A` (5.4:1) |
| text | `oklch(0.940 0.004 266)` `#EAEBEE` (16.2:1) | `oklch(0.205 0.008 266)` `#15171B` (17.2:1) |
| accent / hover | `#1F3FD8` / `oklch(0.520 0.220 266.4)` `#2F56E5` | `#1F3FD8` / `oklch(0.420 0.215 266.4)` `#1935BF` |
| accent-text | `oklch(0.720 0.140 266.4)` `#7CA1FC` (7.7:1) | `#1F3FD8` (7.3:1) |

Risk: it can look like every other dark developer tool. Its identity rests on the Klein blue and the hand-drawn touches.

### B. Paper and Ink: warm, airy, crafted

The creator's-notebook reading: Excalidraw's readable hand-drawn style combined with Linear's 2026 warm-grey move, kept low in chroma so it does not go "muddy". The neutrals sit at hue 75–85 (warm paper and graphite) and Klein blue is the "ink". It is the only cool colour on screen, so it stands out at small sizes. More relaxed density (36 px rows, 14 px body, generous gutters), Inter Display for large titles, hand-drawn underlines and arrows in onboarding, empty states and the Instant mode symbol. Motion is gentle: 200–300 ms and Fluent's `decelerate-min (0.33,0,0.1,1)`, with crossfades rather than slides.

| Token | Dark | Light |
| --- | --- | --- |
| bg | `oklch(0.185 0.006 75)` `#141210` | `oklch(0.975 0.008 85)` `#F9F6F1` |
| surface | `oklch(0.215 0.007 75)` `#1B1916` | `oklch(0.955 0.010 85)` `#F3F0E9` |
| raised | `oklch(0.250 0.008 75)` `#24211D` | `oklch(0.995 0.004 85)` `#FFFDFA` |
| border-subtle | `oklch(0.290 0.008 75)` `#2E2B27` | `oklch(0.905 0.010 80)` `#E3DFD8` |
| border | `oklch(0.360 0.010 75)` `#403C37` | `oklch(0.840 0.012 80)` `#CFCAC2` |
| text-muted | `oklch(0.740 0.012 75)` `#AFAAA3` (7.6:1) | `oklch(0.500 0.015 75)` `#68625A` (5.3:1) |
| text | `oklch(0.945 0.010 85)` `#F0ECE5` (15.9:1) | `oklch(0.230 0.012 75)` `#201C17` (15.7:1) |
| accent / hover | `#1F3FD8` / `oklch(0.520 0.220 266.4)` `#2F56E5` | `#1F3FD8` / `oklch(0.420 0.215 266.4)` `#1935BF` |
| accent-text | `oklch(0.740 0.130 266.4)` `#85A8FD` (8.0:1) | `#1F3FD8` (7.1:1) |

Risk: warm neutrals change how screen-recording previews and thumbnails look next to them. The preview area should stay true neutral (chroma 0) so colour judgements in the editor are not skewed.

### C. Klein Field: confident, blue-tinted, springy

The brand-forward reading, inspired by Material 3 Expressive but kept flat: the neutrals themselves carry the Klein hue (chroma 0.02–0.045 at hue 266), so the whole app feels "made of" the brand colour. Large flat blue moments (a solid accent-tint selection, a full-width primary bar on welcome and export-complete) replace gradients. Medium density. Motion uses springs: M3 standard spatial springs (damping 0.9) for panels and the library grid, expressive (damping 0.6) only for the record button and the countdown, and critically damped effects springs for colour and opacity.

| Token | Dark | Light |
| --- | --- | --- |
| bg | `oklch(0.175 0.022 266)` `#0C101A` | `oklch(0.975 0.010 266)` `#F4F7FE` |
| surface | `oklch(0.210 0.028 266)` `#121825` | `oklch(0.950 0.016 266)` `#E9EFFA` |
| raised | `oklch(0.250 0.034 266)` `#1A2132` | `oklch(0.993 0.003 266)` `#FCFDFF` |
| border-subtle | `oklch(0.290 0.036 266)` `#232B3E` | `oklch(0.900 0.022 266)` `#D7DEED` |
| border | `oklch(0.360 0.045 266)` `#323D55` | `oklch(0.830 0.035 266)` `#BDC7DF` |
| text-muted | `oklch(0.760 0.040 266)` `#A5B1CB` (8.2:1) | `oklch(0.500 0.040 266)` `#58637B` (5.2:1) |
| text | `oklch(0.955 0.012 266)` `#ECF0F9` (16.7:1) | `oklch(0.220 0.040 266)` `#121A2D` (16.2:1) |
| accent / hover | `#1F3FD8` / `oklch(0.530 0.215 266.4)` `#325BE6` | `#1F3FD8` / `oklch(0.420 0.215 266.4)` `#1935BF` |
| accent-text | `oklch(0.760 0.120 266.4)` `#8EAFFE` (8.8:1) | `#1F3FD8` (7.1:1) |
| accent-tint (selection fill) | `oklch(0.300 0.110 266.4)` `#142764` (text 12.2:1, accent-text 6.5:1) | `oklch(0.920 0.038 266.4)` `#D9E5FF` (accent 6.0:1) |

Risk: blue-tinted chrome around a video preview colours the user's perception of their recording, and bold blue moments can tip from calm into loud. Like B, it needs a true-neutral preview area and a strict list of the places where blue may appear.

## 6. Open points for the next tickets

- Pick one direction (or a blend, for example A's density and motion with B's neutrals) in a prototype ticket, viewed by the owner himself.
- Decide the motion ladder: Fluent's 83/167/250 ms with bezier curves, or M3 springs. Both can be built in GPUI with a helper easing function.
- Command palette: the evidence (VS Code, Zed, Raycast, PowerToys) favours one Ctrl+K palette with prefixes and shortcuts shown inline. This answers the map's "if research shows it fits".
- Library: Immich's virtual-scrolling timeline with a scrubber is the strongest open-source reference; gpui-component has the virtual list primitive.
