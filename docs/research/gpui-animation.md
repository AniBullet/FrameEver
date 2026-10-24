# What GPUI can animate on Windows, and at what cost

Research for [#25](https://github.com/AniBullet/FrameEver/issues/25) (map: #23).

Sources are primary only: the gpui checkout FrameEver builds against, which is the `wingleeio/zed` fork at rev `5d1f83d9f27a19bec1fb241dc33b42238af9cf8d` (pinned in `apps/desktop-gpui/Cargo.toml:33-40`, checked out by `scripts/prepare-gpui-dependency.sh` into `../zed-cap`), and FrameEver's own code in `apps/desktop-gpui/src`. Paths starting `gpui/`, `gpui_windows/` are under `zed-cap/crates/`. Zed publishes no separate animation guide; the rustdoc in these files is the official documentation. Line numbers are at the pinned rev and at FrameEver commit `0cc5c2e67`.

## How animation works in GPUI

There is no animation engine, compositor layer or retained property animation. An animation is a re-render: every frame the element tree is rebuilt, laid out (taffy), prepainted and painted again with a different value.

- `Animation { duration, oneshot, easing }`, built with `Animation::new(d)`, `.repeat()`, `.with_easing(f)` (`gpui/src/elements/animation.rs:13-48`).
- `AnimationExt::with_animation(id, animation, |el, delta| el)` and `with_animations(id, Vec<Animation>, |el, ix, delta| el)` for a chained sequence (`animation.rs:57-93`). The closure receives the eased delta and returns a restyled element; anything `Styled` can be changed.
- The start time is element state keyed by the element id (`animation.rs:155-160`). An animation starts the first frame the id appears and never restarts while the id stays. To replay it, change the id; FrameEver does this with a generation counter, e.g. `("add-track-tray", generation)` (`editor_window.rs:8627-8629`) and `("screenshot-selection", self.demo_cycle)` (`onboarding_window.rs:2075-2077`).
- While unfinished it calls `window.request_animation_frame()` (`animation.rs:193-195`), which queues a next-frame callback that does `cx.notify(current_view)` (`gpui/src/window.rs:2368-2371`). So the **whole enclosing view re-renders every frame**, not just the animated element.
- `App::reduce_motion()` is honoured by `with_animation` (oneshot jumps to the end, repeating ones freeze at delta 0, no frames scheduled; `animation.rs:161-168`) and by animated images (`elements/img.rs:321`). Hand-driven animations must check it themselves (`window.rs:2362-2367`).
- Lower-level alternative: in `render` or a `canvas` paint closure, compute a value from an `Instant` and call `window.request_animation_frame()` while not done. FrameEver uses this for the camera loading shader-like glass (`camera_loading.rs:80-120`), doodles (`doodle.rs:575-586`), onboarding welcome (`onboarding_window.rs:828-838`) and the crop tween (`editor_crop.rs:940-950`).

## Capability table

| Area | What exists at the pinned rev | Evidence | Cost / caveat on Windows |
|---|---|---|---|
| Element animation API | `with_animation`, `with_animations` (sequence), `.repeat()`. No keyframes, delays, stagger, reverse, pause or interruption API. Delay/stagger are done by mapping delta inside the closure. | `animation.rs:13-93`; stagger by hand in `editor_window.rs:1693-1698` | One full view render + layout + paint per frame |
| Retargeting / interruption | None. A running animation cannot be redirected mid-flight; a new id restarts from the closure's start value. Smooth retargeting needs a hand-driven tween that stores from/to/started, like `CropAnim`. | `editor_crop.rs:940-950` | Same as above |
| Easing | Built in: `linear`, `quadratic`, `ease_in_out` (quad), `ease_out_quint()`, `bounce(f)` (there-and-back, not a physical bounce), `pulsating_between(min,max)`. Any `Fn(f32)->f32` is accepted. | `animation.rs:236-290` | Free |
| Overshoot easing in `with_easing` | Not allowed: the eased delta hits `debug_assert!((0.0..=1.0).contains(&delta))`, so back/elastic curves panic in debug builds. Apply overshoot inside the animator closure instead, as `ease_out_back` does. | `animation.rs:185-188`; `editor_window.rs:1678-1698, 8782-8790` | Free |
| Springs | No spring or physics type anywhere in gpui. A spring must be a closed-form curve of time (fixed duration) or a hand-driven integrator per frame. | grep of `gpui/src` | Free |
| Opacity | `.opacity(f)`, multiplied down the tree. It is per-primitive alpha, not group opacity: overlapping children of a fading parent show through each other mid-fade. | `styled.rs:745-748`; `window.rs:3524-3540` | Cheap (no re-raster) |
| Position | Only through layout: `.left/.top/.margin` etc. on absolute or relative elements. No translate transform for divs, text or images. | `scene.rs:577-587` (`Quad` has no transform); `editor_window.rs:8633`, `target_overlay.rs:1832-1840` | Re-layout of the view each frame; text glyphs snap to 1/4 px horizontally and whole px vertically (`text_system.rs:45-48`), so slow slides of text step |
| Size | `.w/.h/.size` with px or `relative()`. Causes re-layout; children reflow. | `onboarding_window.rs:2075-2078`, `target_overlay.rs:1822-1826` | Wrapped text whose width changes is re-shaped every frame (wrap width is part of the line-layout cache key, `text_system/line_layout.rs:833-839`); SVG size changes re-rasterize a new atlas tile per size (`window.rs:4509-4523`) |
| Scale / rotate | Only `svg().with_transformation(Transformation::rotate/scale/translate)`; sprites carry a matrix, quads do not. No scale/rotate for divs, text or `img`. | `elements/svg.rs:44, 187-252`; `scene.rs:765-796`; `gpui/examples/animation.rs:76-82` | SVG transform is cheap (matrix on an existing tile) |
| Font size | Animatable via `.text_size`, but the size is in the line-layout cache key, so every frame is a cache miss and a DirectWrite shape. | `line_layout.rs:833-839` | Expensive; avoid |
| Color / background / border | Any style value, including gradients, can be set per frame from the closure. | `target_overlay.rs:1810-1826` | Cheap |
| Edge fades | `window.with_edge_fade(EdgeFade{..})` ramps primitive alpha near region edges (scroll fades over translucent backgrounds). Static, but can be animated by changing the fade params. | `window.rs:745-790, 3548` | Cheap; per-primitive granularity |
| Backdrop blur | `BackdropBlur` primitives exist in the scene but only the Metal renderer draws them; the DirectX renderer's batch loop has no case for them. | `scene.rs:56, 92`; `gpui_macos/src/metal_renderer.rs:937`; `gpui_windows/src/directx_renderer.rs:348-367` | Not available on Windows |
| List insert / remove / reorder | `list()` / `ListState::splice`, `reset`, `remeasure_items`, `scroll_to*` only re-measure; no item transitions, no layout (FLIP) animation, no smooth scroll. Enter animation = wrap new rows in `with_animation` with an id per row; exit needs keeping the row alive until its fade ends; reorder needs hand-computed offsets. | `elements/list.rs:355-412, 503-515, 603-664` | Per frame cost of the whole list's view; keep animated rows few |
| Animated images | `img()` plays multi-frame images with their own frame delays, respecting reduce motion. | `elements/img.rs:316-332` | One atlas tile per frame decoded |
| Window show / hide | Instant. Windows path is `ShowWindowAsync(SW_SHOW/SW_HIDE/SW_SHOWNOACTIVATE)`; no fade or slide. FrameEver's `set_window_alpha` is a no-op on Windows that returns 1.0, and window material is `None`. | `platform/windows.rs:188-206, 490-506, 590-606` | A fade-in must be painted inside the window (content opacity) on a transparent background |
| Window resize / move | `PlatformWindow::resize` spawns an async `SetWindowPos`; each `WM_SIZE` calls the DirectX `ResizeBuffers` and recreates render targets. FrameEver sets frames with `set_window_logical_frame`. Not built for per-frame tweening. | `gpui_windows/src/window.rs:671-723`; `gpui_windows/src/events.rs:162-212`; `directx_renderer.rs:386-430`; `platform/windows.rs:408-440` | Swap-chain realloc every step; expect flicker or lag. Animate content inside a fixed-size window instead |
| Window background | Opaque, Transparent, Blurred (accent policy), Mica, MicaAlt; DirectComposition swap chain with premultiplied alpha. Not animatable. | `gpui_windows/src/window.rs:975-1000`; `directx_renderer.rs:1204-1230` | Static choice |
| Reduce motion source | GPUI never reads the OS setting. FrameEver feeds it on macOS only; on Windows `follow_system_reduce_motion` does nothing, so `reduce_motion()` is always false. | `platform.rs:596-603` | Gap: a Windows hook (e.g. SPI_GETCLIENTAREAANIMATION) is needed |

## Frame loop and per-frame cost on Windows

- **Clock.** A single `VSyncProvider` thread waits on `DwmFlush` (falling back to sleeping one refresh interval read from `DwmGetCompositionTimingInfo`) and posts `RedrawWindow` to every gpui window each vblank (`gpui_windows/src/vsync.rs:31-55`; `gpui_windows/src/platform.rs:306-345`). Animations therefore run at the monitor refresh rate, with deltas computed from `Instant`, so a missed frame skips ahead rather than slowing down.
- **Idle cost.** The frame pacer that stops idle frame requests needs `PlatformWindow::frame_request_toggle`; the Windows backend does not implement it (default `None`, `gpui/src/platform.rs:858-863`), so every window still receives a WM_PAINT per vblank. A clean window returns early without drawing (`gpui/src/window.rs:1653-1676`).
- **Throttling.** Inactive windows are capped to about 30 fps only when they are not dirty; a dirty, unfocused window animates at full rate. Serious or Critical thermal state caps at 60 fps (`window.rs:1594-1628`).
- **What one animated frame does.** Notify the enclosing view, run its `render`, taffy layout of everything that view returns, prepaint, paint, upload scene buffers, draw batches, present (`window.rs:2831` onward; `directx_renderer.rs:322-367`). Cached child views (`Entity::cached` / `AnyView::cached`, which need a definite size) are replayed without re-rendering (`gpui/src/view.rs:225-275`).
- **Text.** Line layouts are cached across consecutive frames keyed by text, font size, runs and wrap width (`line_layout.rs:400-460, 833-839`). Moving or fading text costs nothing extra; changing its size, content or wrap width costs a DirectWrite shape per frame.
- **Images and SVG.** A raster image is one atlas tile scaled on the GPU, so moving or resizing it is cheap. An `svg()` is rasterized at its device pixel size (`window.rs:4509-4523`), so animating its size fills the atlas with new tiles; rotate/scale it with `with_transformation` instead.
- **Paths.** Vector paths render through an MSAA intermediate texture (`directx_renderer.rs:28, 528-580`); many animated paths are the most expensive primitive.
- **No measurement.** Nothing in this research measured frame times; FrameEver's `picker_ui_benchmark.rs` shows the `on_next_frame` pattern that would be needed to do so.

## Constraints a motion spec must respect

1. Express every motion as a pure function of elapsed time over a fixed duration: `value = f(delta)`. There are no springs, velocities or interruptible animations unless hand-built.
2. Easing passed to `with_easing` must stay within 0..1. Overshoot (back, elastic, spring-like) is applied in the animator closure, not as the easing.
3. Animatable properties are opacity, colors, layout position (left/top/margins) and layout size. Scale and rotation exist only for SVG icons. Do not specify scale-up/zoom transitions for cards, text or images; use opacity plus a short positional slide, or a size change of a container with clipped content.
4. Opacity fades are per-primitive. Fading a stack of overlapping layers shows the lower layers through the upper ones mid-fade; keep fading groups non-overlapping or fade a single top layer.
5. Do not animate font size, text content width on wrapped text, or SVG size.
6. Windows open, close, hide and resize instantly. Entrance and exit motion must be painted inside an already-shown window (transparent background plus content opacity/slide). No per-frame window resizing; size the window to its final bounds first and animate the content.
7. No backdrop blur on Windows; designs relying on blur-behind must have a flat fallback (which matches the flat-only style preference anyway).
8. Lists have no built-in insert/remove/reorder transitions. Specify at most a per-row enter fade/slide; exits and reorders require explicit design and implementation cost.
9. Every animation re-renders its whole enclosing view each frame. Keep motion short (FrameEver's current range is 180-700 ms for transitions, 1.2-1.95 s for loops), avoid perpetual loops in large views (editor, main window), and isolate long-running loops in small cached child views.
10. Replaying an animation means changing its element id; the spec should say what event bumps the generation.
11. Every non-essential animation must have a static end state for reduce motion. `with_animation` handles this; hand-driven ones must check `cx.reduce_motion()`. On Windows the flag is currently never set from the OS, which is a separate gap to close.
12. Frame rate follows the monitor's refresh rate; do not specify motion in frame counts.
