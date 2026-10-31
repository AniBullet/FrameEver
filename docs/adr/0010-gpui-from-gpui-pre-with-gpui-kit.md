---
status: accepted
---

# gpui comes from the pinned `gpui-pre` snapshot, and shared controls come from GPUI Kit

FrameEver moves gpui from the wingleeio fork to `gpui-pre 0.3.10` (upstream Zed `08cbb88`), the snapshot GPUI Kit 0.8.1 pins, and takes shared controls from GPUI Kit, pinned exactly. FrameEver Converter moves to the same gpui but keeps its own controls. Upgrades are planned tickets that rebase our patch onto the next snapshot, never routine bumps.

This records the result of the trial in #86. The owner decided on 2026-10-31 from a phone: adopt if it helps how the app looks and is worth having. The owner has not run the branch; the decision rests on the numbers and on the window comparison below.

## What the trial measured

Time: one working session, about 3.6 hours, well inside the limit of two or three sessions. Most of it was release builds and packaging retries caused by stale local caches. The rule's early stop never came close.

| Measurement | Result |
|---|---|
| App compile errors on unpatched `gpui-pre`, Windows | 5 in about 164,000 lines, all argument counts (`paint_image` ×3, `ArenaClearNeeded::clear` ×2); first check took 272 s. macOS was not compiled |
| Compile errors after the fixes, tests included | 0 |
| Renames | 9 deprecation warnings (`on_scroll_wheel`, `ScrollWheelEvent`) |
| Dependency resolution with GPUI Kit | no conflicts; 1,108 → 1,129 packages |
| Patch stack for Windows | 2,609 lines in three patches → 783 lines in one |
| `zed-windows.patch` | 36 hunks: 23 applied, 6 rebased by hand, 7 dropped (EdgeFade shaders, which only existed in the fork) |
| `zed-gpui.patch`, `zed-idle-frames.patch` | one Windows-relevant change kept (unfocused windows no longer capped at 30 fps when they have new content); the rest is macOS or Linux, or an API the app doesn't call |
| `zed-linux.patch` | deleted |
| `pnpm fe test --all` | passed on the first run |
| The patch's own tests in `gpui_windows` | 28 of 28 pass, 9 of them ours, after adapting the atlas test to upstream. `test --all` didn't run them before; it does now |
| GPUI Kit start-up cost | `init` 3.2 ms; a window with input, select, slider, colour picker, tabs and title bar opened in about 230 ms in a debug build |

Download size, from `pnpm fe package` on both branches:

| Artifact | `main` | Branch | Change |
|---|---|---|---|
| Installer | 149,326,369 B | 151,529,580 B | +2.2 MB, +1.5% |
| App exe | 110,690,304 B | 121,118,720 B | +10.4 MB, +9.4% |
| FrameEver Converter, own controls | 13,568,000 B | 13,859,840 B | +0.3 MB, +2.2% |
| FrameEver Converter, linking GPUI Kit | 13,568,000 B | 20,134,912 B | +6.6 MB, +48% |

The branch sizes were measured with a trial window that linked GPUI Kit's controls, so they are a fair picture of adoption. That window was removed when the move landed.

Behaviour checked on the branch, reading window state through Win32 and recordings through ffprobe, with a test-owned window as the target:

- Studio recording: 7.27 s, 354 frames; stopping opened the editor with its timeline and first frame.
- Instant recording: 7.24 s, 182 frames, with `output.mp4` and a thumbnail.
- Target overlays covered both monitors. Their styles and sizes, the recording bar's, and the main window's frame match `main`. The custom frame's insets are 8 px at the sides and 1 px at the top at 100%, as the rebased tests expect.
- FrameEver's own windows, captured alone on both branches in the same state: main window 0.000% of pixels differ, recording bar 0.04% (one 3×9 px area), editor 0.138% (caption buttons, top edge and a few digits, mostly foreground state). The translucent recording bar shows no visible change from the blend state below.
- The same run on `main` showed the same `window not found` log line, and the `FRAMEEVER_GPUI_AUTO_RECORD` stop that never takes effect, so neither comes from this move. Logs from 23 to 25 October already held the `0x80070578` handle error seen when the editor opens.

## Considered Options

- **Stay on the fork and restyle our own controls**: no migration risk, but 23 hand-made controls stay ours, including a 2,680-line text input, and the fork falls further behind. The trial's numbers removed the reason to accept that.
- **`gpui-pre` without GPUI Kit**: smallest binary, but it pays the move and gets only half the benefit.
- **GPUI Kit's `bump-gpui.ts` over a Zed checkout**: needs Bun and a full Zed clone. Downloading the two published crates at pinned checksums and applying one patch does the same job (`scripts/prepare-gpui-dependency.sh`).

## Consequences

- `pnpm fe setup` prepares `../gpui-pre-patched` from `apps/desktop-gpui/patches/gpui-pre.pins` and `gpui-pre-windows.patch`. `gpui_tokio` isn't published, so `apps/desktop-gpui/gpui_tokio` vendors it.
- GPUI Kit moves its pin almost weekly. Staying on one snapshot until a planned upgrade keeps that bounded. Ports from upstream Cap are adapted by hand, because Cap stays on the old gpui.
- GPUI Kit's defaults don't all fit our rules. `TitleBar` paints a gradient by default, so its background is always set; `Theme.shadow` stays off; dialog shadows are checked when dialogs are used; its default theme is light until the tokens fill it.
- Each window that uses GPUI Kit is wrapped in its `Root`, so windows migrate one at a time.
- The DirectX blend state changes with upstream: `DestBlendAlpha` goes from `ONE` to `INV_SRC_ALPHA`, the correct OVER blend. Translucent layers in transparent windows (overlays, the recording bar, the rounded main shell) therefore compose differently. The recording bar was compared pixel by pixel; the full-screen overlay scrim wasn't.
- The macOS work (#40) costs more than a rebase. `zed-gpui.patch` (Metal changes) and `zed-idle-frames.patch` (frame pacing) stay in `patches/`, unapplied, and they apply to wingleeio/zed at `5d1f83d9f27a19bec1fb241dc33b42238af9cf8d`. `PaintSurface` needs `corner_radii` and `source_uv` again for `paint_surface_fitted`, and upstream's `paint_surface` takes `CFRetained<CVPixelBuffer>` where the app passes `core_video`'s type, so `editor_window.rs`, `camera_window.rs` and `camera_blur.rs` change too. Upstream's `schedule_frame` and `wake_platform` may replace part of the pacing patch. On Windows `frame_request_toggle` was never implemented, so nothing is lost there.

## Not checked by the trial

These are accepted on each surface's ticket, because the automated run didn't cover them: how the controls feel by hand, the full-screen overlay scrim against `main`, Chinese IME input in GPUI Kit's input, maximise and snap layouts, mixed-DPI moves, window fitting at screen edges, area and display overlays by hand, the camera bubble, the unfocused-window frame rate, and idle CPU compared with `main`.
