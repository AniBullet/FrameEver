# Can the library play video in a small preview and full-screen with the editor's playback?

Research for [#45](https://github.com/AniBullet/FrameEver/issues/45) (map: #23). The library's detail panel should hold a small player (play/pause, progress, mute), and Space should open a full-screen Quick Look-style preview (←/→ next item, K play/pause), for Studio projects, Instant recordings, export files and screenshots.

Sources are primary only: FrameEver's code at commit `bb6195105`, plus the gpui fork it builds against (`wingleeio/zed` at `5d1f83d9f`, checked out in `../zed-cap`; paths starting `gpui/` and `gpui_windows/` are under `zed-cap/crates/`). Nothing was built or run for this report. Where a number is missing, the report says so and says how to measure it.

## Answer in short

The editor's playback works, but it is the wrong engine for a preview. It is a full compositor (decode, then a wgpu render of background, cursor, camera and zoom, then a GPU readback, then a CPU copy into gpui's sprite atlas). It only opens complete Studio projects, it may write `project-config.json` when it opens one, and its caches allow a few hundred MB per open project. A preview needs a separate, small player: FFmpeg decode straight to a BGRA `RenderImage` at the size it is shown, with a streaming audio decode into a cpal stream. That path already exists in pieces (the clip-thumbnail decoder and the editor's audio output). Studio projects play their latest export when they have one. Otherwise they play the raw screen recording, or they show the poster and an "Open in editor" action.

## 1. How the editor plays a project today (Windows)

| Stage | What happens | Evidence |
|---|---|---|
| Validate | Background-executor pre-flight of the `.ever`, then `EditorInstance::new_with_startup_inputs` on the `gpui_tokio` runtime | `apps/desktop-gpui/src/app_windows.rs:3778-3790, 3936-3975` |
| Project gate | Only Studio metadata is accepted (`"Cannot edit non-studio recordings"`). With no timeline or no clip offsets it builds them and **writes them to the project** | `crates/editor/src/editor_instance.rs:398-400, 420-500` |
| Decoders | FFmpeg is forced on Windows (`force_ffmpeg_for_editor = cfg!(windows)`). It opens with D3D11VA, and each hardware frame is copied back to system memory with `av_hwframe_transfer_data` as NV12 | `editor_instance.rs:508-512`; `crates/rendering/src/decoder/ffmpeg.rs:479-495`; `crates/ffmpeg-hw-device/src/lib.rs:54-74` |
| Audio | Every mic and system track is decoded progressively into RAM as 48 kHz f32 (`ProgressiveAudio::spawn`). The cpal output stream is prewarmed when the project has audio | `editor_instance.rs:540-556, 1758-1790`; `crates/audio/src/progressive.rs:242`; `crates/audio/src/audio_data.rs:31` |
| GPU | `RenderVideoConstants` on the shared wgpu device (created once per app run and cached), renderer layers built on their own thread (cursor assets preloaded) | `app_windows.rs:3596-3645`; `editor_instance.rs:600-631`; `crates/editor/src/editor.rs:86-126` |
| Render | `Renderer` composites each frame into an `Rgba8Unorm` target at the preview size: 1248x702 by default ("Half"), 1920x1080 "Full", 480x270 "Quarter" | `editor_window.rs:112, 118-133, 12583-12594` |
| Hand-off | GPU readback, then a bounded flume channel (4 frames), then a background task un-pads rows and swaps RGBA to BGRA (`frame_image`), then `RenderImage`, then `paint_image` into the gpui sprite atlas. The previous frame is removed with `window.drop_image` | `app_windows.rs:3925, 4040-4060`; `editor_window.rs:576-610, 6374-6380` |
| Playback loop | `Playback` prefetches with 4 parallel decode tasks into a 10-90 frame buffer capped at 128 MB (192 MB when boosted after a stall), plus a 64 MB rendered-frame cache. The decoder keeps its own cache of up to 90 frames or 128 MB | `crates/editor/src/playback.rs:35-50, 298-327`; `crates/rendering/src/decoder/mod.rs:652`; `decoder/ffmpeg.rs:79` |

On Windows there is no zero-copy path. `paint_surface` exists only for macOS (`gpui/src/window.rs:4655-4670`), and the DirectX renderer's `draw_surfaces` does nothing (`gpui_windows/src/directx_renderer.rs:728-733`). So every displayed frame is a CPU buffer uploaded into the window's own atlas (`directx_renderer.rs:163`).

### What is measured

- Editor playback: **59.9 fps, 0 dropped frames, frame conversion 578 µs on average.** This was a debug build on one 5 s 830x470 Studio recording, 2026-10-21 (`docs/upstream-sync/0.6.1.md:53`). That is the only playback number in the repo.
- A comment in the code records that an inactive editor window which refreshed the whole window on every frame dropped 224 of 298 frames. That is why only the preview entity is notified per frame (`editor_window.rs:6392-6399`).
- Time to first frame, CPU, GPU and memory while playing: **not measured anywhere in the repo.** The code above sets only upper bounds: decoder cache ≤128 MB per video file, prefetch ≤128-192 MB, frame cache ≤64 MB, and audio about 23 MB per minute per stereo track. So one open project may hold a few hundred MB before the wgpu textures are counted.

### How to measure it later with `pnpm fe`

- **Time to first frame and fps.** `pnpm fe dev` runs a debug build and passes the environment through (`scripts/fe.mjs:477-501`). Set `FRAMEEVER_GPUI_AUTO_EDITOR=<a .ever in the test storage folder>`, `FRAMEEVER_GPUI_AUTO_PLAYBACK=10` and `FRAMEEVER_GPUI_MUTE_AUDIO=1` (`main.rs:640-652`, `app_windows.rs:4430-4445`). Then `pnpm fe logs --grep editor` shows the timestamps of `editor project validated`, `editor instance ready` and the first `editor frame size`, and the perf line printed when playback stops (`editor_window.rs:3170-3180, 894-910`). Debug numbers are pessimistic, so record the build type with them.
- **Startup stages.** `crates/editor/examples/editor-startup-benchmark.rs` times each stage (metadata, render constants, segments, layers, renderer). `scripts/capbench.py` drives it, but neither is a `pnpm fe` command yet. Add `pnpm fe bench editor-startup` before relying on it.
- **Memory and CPU.** No counter exists. Read the process working set and CPU time (Task Manager or `Get-Process frameever-gpui`) before opening, after the first frame, and after 10 s of playback.

## 2. Can it be reused outside the editor, or for plain MP4s?

**Outside the editor window: technically yes.** `EditorInstance` is a library type. The editor window only supplies a frame callback, a state callback, the shared device and an `AudioOutput`. A library player could build one the same way and paint `EditorPreviewFrame` into a small box. It is still a poor fit for the detail panel:

- **Studio only.** `EditorInstance` refuses Instant recordings, and nothing in it opens a plain MP4 (`editor_instance.rs:398-400`). An export file or an Instant `content/output.mp4` (`library/versions.rs:3-6`) would need a different path anyway.
- **It writes.** Opening a project with no timeline or clip offsets saves them (`editor_instance.rs:488, 497`). Selecting a card in the library must not change the project.
- **It is heavy to start.** For each selection it would open decoders per segment and camera, decode every audio track, build renderer layers and cursor assets, and spawn a renderer thread. These are the caches from section 1, sized for editing and scrubbing, not for a 360 px box.
- **It renders at editor size.** Output is 1248x702 by default. Rendering at the panel's size works by passing a smaller `resolution_base`, but the decode and composite work stays the same.

**The lighter path is the better fit for a preview.** Decode with FFmpeg (D3D11VA plus the transfer the editor already uses, or software decode for small sizes), convert with `sws_scale` straight to BGRA at the size shown, and wrap the result in a `RenderImage`. The clip-thumbnail decoder already does exactly this for one frame: it seeks, decodes to the target time, scales to BGRA at 192 px and builds a `RenderImage` (`apps/desktop-gpui/src/editor_clips.rs:2114-2230`). A player is that loop kept open, driven by a clock and showing one frame at a time with `drop_image` on the previous one, the way the editor's pump does. At the panel's size (for example 640x360 BGRA, 0.9 MB per frame) the atlas upload is a quarter of what the editor already sustains at 60 fps (1248x702, 3.5 MB). Full screen at 1080p is 8.3 MB per frame. That upload rate and the NV12→BGRA conversion cost on the CPU are **not measured**. Measure them with a perf line like the editor's `PumpStats` in the prototype before committing to 60 fps full-screen.

**What a Studio project plays without the editor.** The library cannot show an edited Studio project (background, camera, zoom, cursor) without the compositor. The options, cheapest first:

1. The project's latest export, if it has one (export paths are recorded inside the project, ADR-0006). This is exactly what the user made. It is a plain MP4, so the light player plays it.
2. The raw screen recording of segment 1 (`display.mp4`), labelled as the original recording. With several segments, play them in order or only the first.
3. The poster (`screenshots/preview.jpg`, which the editor renders from the edited project when it closes, `crates/editor/src/thumbnail.rs`) plus "Open in editor".

A later option is a "preview" `EditorInstance` that opens read-only, never writes and uses smaller caches. It costs changes in `crates/editor` and gives a full-fidelity Studio preview. Keep it out of 1.0.

**Screenshots** need no player: they are PNGs and already decode through the thumbnail cache (`library.rs:1089-1130`). Full screen loads the original PNG through the same decode at screen size.

## 3. Several windows and players; audio

- **Several editors already coexist.** `AppWindows::editors` is a `Vec` of open editor windows (`app_windows.rs:214`). They share one cached wgpu device (`app_windows.rs:3596-3645`), and each has its own cpal stream (`editor_audio_output`, `app_windows.rs:3654-3662`). Windows' shared-mode audio mixes the streams.
- **Images are per window.** gpui's DirectX atlas belongs to each window's renderer (`gpui_windows/src/directx_renderer.rs:163, 198-200`), and `drop_image` removes a frame from that window's atlas only (`gpui/src/window.rs:4712-4723`). A frame shown in the detail panel and in a separate full-screen window is uploaded twice. Pause the small player and hand its playhead to the full-screen player when Space opens it, then hand it back on close. Do not run both.
- **One player at a time is a product rule, not a limit.** Nothing stops two light players from running together, since each is a decoder thread plus a frame per paint. But the library should pause the current one when another starts or when the editor starts playing, the way Quick Look does.
- **Audio path for the preview.** gpui has no audio. The app's `rodio` is built with only `mp3` and `vorbis` (`apps/desktop-gpui/Cargo.toml:63`), so it cannot decode the AAC in MP4s. Two routes:
  - Reuse `frameever_editor::AudioOutput` (one cpal stream, sources installed into a live callback, mute through `new_headless`, `crates/editor/src/audio_output.rs:1-60, 342-430`). Its `PlaySpec` wants `AudioSegment`s and a `ProjectConfiguration` and plays fully decoded RAM tracks: about 23 MB per minute for a stereo track, decoded before the first sound plays. That is workable for short clips and wasteful for a 30-minute export.
  - **Recommended:** a small streaming decoder. FFmpeg audio decode and resample to the device format, into a ring buffer read by a cpal callback, with the video frames timed against the audio clock. This is the size of `onboarding_audio.rs` plus a resampler (`crates/editor/src/audio.rs:1391-1445` already has one on FFmpeg). Mute stops filling the ring buffer, or plays silence, without stopping the clock.

## 4. Hover-scrub thumbnails

**How thumbnails are made today.**

- Library cards use the project's `screenshots/preview.jpg` (the edited render) or `screenshots/display.jpg` (the first frame of the recording at full resolution, written by `create_screenshot` with FFmpeg and `sws_scale`). The card image is downscaled to 392x224 on a pool of at most 8 background workers and cached as a JPEG of about 25 KB in the app's thumbnail cache, keyed by the source's mtime (`apps/desktop-gpui/src/library.rs:810-830, 899-1130, 1156-1235`).
- The editor's clip list decodes one frame per clip on demand. It opens the file, seeks, decodes forward to the time (at most 4096 packets or 2 s) and scales to 192 px BGRA. The result stays in memory only (`editor_clips.rs:700-725, 2114-2230`).
- `target_thumbnails.rs` holds live capture-target thumbnails for the display and window pickers. It has nothing to do with library media.

**A scrub strip is affordable if it is made once, in the background, and cached.** Every piece exists: the clip decoder for frames at chosen times, the decode pool for background work, and `ThumbnailCache::keyed` (`library.rs:990-1000`) for a cache entry per source and mtime.

- Cost per strip: N seeks, each decoding from the previous keyframe to the target. The FFmpeg H.264 encoder sets a 2 s GOP (`crates/enc-ffmpeg/src/video/h264.rs:1154-1170`), so one seek decodes at most about 120 frames at 60 fps. The Media Foundation encoder sets no GOP (no match in `crates/enc-mediafoundation/src`), so its keyframe spacing is the encoder's default and is **unknown**. Snapping each sample to the nearest keyframe (FFmpeg `skip_frame = AVDISCARD_NONKEY`, not used in the repo today) makes each sample one decode, which is enough for a hover preview.
- Storage: for example 20 frames at 196x112 in one JPEG strip is far below 100 KB per item, and one atlas image per hovered card.
- When to make it: on first hover (show the poster until it is ready) or during idle time after the grid's thumbnails are done. Never during recording.
- **Not measured:** time per strip. Measure it with an `#[ignore]` test that reads a project path from an environment variable, the pattern of `auto_zoom_click_loading_benchmark` (`editor_window.rs`). Run it through `pnpm fe test <name>`, which passes its extra arguments to `cargo test` (`scripts/fe.mjs:559-565`), with `-- --ignored`.

## 5. Recommendation

- **Build one light library player and do not reuse the editor's playback.** It is FFmpeg video decode to BGRA at the size shown, wrapped in a `RenderImage`, plus a streaming FFmpeg audio decode into one cpal stream, with the audio clock as the master. It serves the detail panel and full screen; full screen decodes at source size.
- **What each item plays.** Instant recording: `content/output.mp4` (or the compressed version in use). Export: the file. Studio project: its latest export if there is one, otherwise the raw screen recording labelled as such, or the poster with "Open in editor". Screenshot: the PNG.
- **One active player.** Space hands the playhead from the small player to the full-screen one and back. Selecting another item or starting editor playback pauses the current player.
- **Scrub strips** are generated lazily on hover in the background, cached beside the card thumbnails, and sampled at keyframes.
- **Rough work.** Video player core and its GPUI element: 3-5 days. Streaming audio and A/V clock: 2-3 days. Full-screen window, keys and hand-off: 2-3 days. Scrub strips: 1-2 days. Measurement lines: 1 day. A read-only preview `EditorInstance` for edited Studio projects would be extra and is not needed for 1.0.
- **Risks.**
  - Full-screen 60 fps upload through the atlas is unmeasured: 1080p BGRA is 8.3 MB per frame, and 4K is 4x that. The fallback is 30 fps or a decode scaled to fit.
  - Studio previews without an export do not show the edits, which may surprise the user, so label them.
  - Media Foundation's GOP is unknown, so seek cost may vary.
  - A/V sync in a new player needs its own tests.
  - Long exports must not be decoded whole into RAM, which is why the audio streams.
