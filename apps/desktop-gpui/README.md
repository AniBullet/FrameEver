# frameever-desktop-gpui

The FrameEver desktop app, drawn with [gpui](https://www.gpui.rs/). No webview:
the whole UI is drawn by gpui and every pixel is Rust. It began as upstream
Cap's GPUI app at 0.6.1; upstream is now a read-only reference (ADR-0007).

The main recording window uses a fixed 330×432 layout with device controls
and recording and screenshot browsers. The expanded view is no longer available.

This workspace also holds FrameEver Converter's wizard (`converter/`); its
conversion rules live in the root workspace's `crates/converter`.

## Running it

```powershell
pnpm fe dev
```

`pnpm fe dev` uses the test profile in `target/testdata/`, puts FFmpeg on the
path and writes logs that `pnpm fe logs` reads. For more logging:
`RUST_LOG=frameever_gpui=debug pnpm fe dev` — note the filter is
`frameever_gpui` (the binary), not `frameever_desktop_gpui` (the package).
`BUILD.md` and `docs/agents/debugging.md` at the repo root cover the rest.

The first build takes about 11 minutes and 6.4 GB of `target/`: gpui pulls its
own revisions of the wgpu and font stacks. After that, an edit rebuilds only
the app crate, at `opt-level = 0` and incrementally, in about 22s with plain
`cargo build` or the dev loop below; the app crate's incremental cache is about
1.6 GB of `target/debug/incremental`. Dependencies build at `opt-level = 2`.
The app crate and its dependencies carry line-table debug info, so breakpoints
and backtraces keep their lines; for locals in a debugger, build with
`--config 'profile.dev.package.frameever-desktop-gpui.debug=true'`.

Loops over pixels or samples live in `kernels/` (`frameever-gpui-kernels`,
`opt-level = 3`): unoptimised, the editor's frame conversion alone costs 42.7ms
a frame against 1.0ms, a 23fps preview instead of a 60fps one. That includes
generic dependency calls over large data — `imageops::resize`, image encoders,
`serde_json::from_reader`, `md5` — because a generic compiles into the crate
that instantiates it, at that crate's opt-level. Put the call in `kernels/`
behind a plain-slice function and call that from the app.

### The dev loop

```powershell
pnpm fe dev --watch
```

Native Rust cannot hot-reload, so this rebuilds on save instead. It watches
the app's sources, assets and `kernels/` and the `frameever-*` crates the
editor depends on (`WATCH_DIRS` in `scripts/fe.mjs`), rebuilds, and swaps the running app
**only when the build succeeds** — a compile error leaves the previous build
running. The relaunch reopens exactly where the old process was: same windows
at the same frames, same settings page, same editor project with its sidebar
tab and scroll (`src/dev_restore.rs`, driven by `FRAMEEVER_GPUI_DEV_RESTORE`,
applied through the same entry points real clicks take). Relaunches
deliberately do not activate the app, so keyboard focus stays in your code
editor while the windows refresh beside it.

`dev.sh` is the same loop for macOS. It also watches the local gpui checkout and waits out an in-flight
recording rather than truncating it (the restore state file doubles as that
handshake — it will not kill the app while it reads `"recording":true`).

Both loops run a plain `cargo build`, so they share every artifact with one
run by hand.

### Why it is a separate workspace

`Cargo.toml` declares its own `[workspace]` with its own `Cargo.lock`. gpui
comes from the pinned `gpui-pre` crates, patched in `../../../gpui-pre-patched` (prepared by
`pnpm fe setup`), and gpui's tree pulls its own revisions of wgpu and the font
stacks that the root workspace's crates do not want. Keeping the two apart
keeps the root lock and the `cargo hakari` workspace-hack untouched by gpui.
Both workspaces build with Rust stable (`rust-toolchain.toml`).

## Shipping and updates

### How FrameEver is packaged

`tauri.conf.json` in this directory is a **packaging** config only — nothing
here links the Tauri runtime; the Tauri CLI is used as the NSIS bundler.
`scripts/bundle-gpui.mjs` builds `frameever-gpui`
(`scripts/build-gpui-binary.sh release <target>`), stages it as the main
binary `FrameEver.exe`, and runs `tauri bundle` against this crate
(`TAURI_APP_PATH`). The identity is `FrameEver`, `io.github.anibullet.frameever`
(`crates/utils/src/brand.rs`).

The bundle carries the sidecars `frameever-muxer` and `frameever-cli`.
Release sidecars are built without debug info and stripped.
`pnpm fe package` also builds FrameEver Converter
(`FrameEver-Converter_<version>_x64.exe`), a separate download, not part of
the installer.

`packaging/windows/installer-hooks.nsh` gives the NSIS installer a 128 MB
LZMA window, so the CLI compresses against the app, and its install hook
deletes `cap-gpui.exe`, `cap-exporter.exe`, `cap-cli.exe` and
`cap-muxer.exe` left by older installs. `scripts/finalize-macos-packages.mjs` holds the macOS packaging and
updater-signing steps for the later macOS release.

### The native updater

`src/updates.rs` decides when to check, and `src/installer.rs` fetches
`latest.json` from the newest release on `AniBullet/FrameEver` (GitHub
Releases) and does the rest:

1. **Download** streams the artifact to
   `<cache>/io.github.anibullet.frameever/updates/frameever/<version>/` and verifies its
   minisign signature *while* it downloads (`minisign_verify::StreamVerifier`),
   so a 200 MB update never sits in memory. A verified file is reused on the
   next attempt instead of being fetched again; anything that fails
   verification is deleted.
2. **Install** checks the file's format, then on Windows starts the NSIS
   installer with `/P /R /UPDATE` and quits (on macOS it unpacks the
   `.app.tar.gz` beside the running bundle and swaps it in).
3. **Relaunch** waits for this process to exit and reopens the new build.

Progress shows in the settings sidebar footer, and the update never installs
while a recording, export, import or transcription is running. A build without
an update signing key has updates disabled.

### Where the code came from

The Tauri app this code was ported from was deleted from the repository
(ADR-0001); comments and docs that cite a file as Tauri `src-tauri/...`,
Tauri `src/...` or Tauri `ui-solid/...` mean that file in commit `e94ffd0e5`.

## Where things live

| Area | Files |
|---|---|
| Startup, global hooks, single instance | `main.rs`, `single_instance.rs`, `deeplink.rs` (`frameever://` actions), `hotkeys.rs`, `menus.rs`, `tray.rs` |
| Main window and pickers | `main_window.rs`, `devices.rs`, `media_access.rs`, `target_overlay.rs`, `target_thumbnails.rs`, `mode_select_window.rs` |
| Recording | `session.rs` (the app-wide recording session), `recording.rs`, `controls_window.rs`, `camera_window.rs`, `feeds.rs`, `screenshot.rs` |
| Library | `library.rs`, `import.rs`, `auto_compress.rs` (compressed copies) |
| Editor | `editor_window.rs` and the `editor_*` modules: timeline, clips, edits and undo, config sidebar and tabs, crop, colour, export page |
| Screenshot editor | `screenshot_editor.rs`, `screenshot_annotations.rs`, `screenshot_crop.rs`, `screenshot_export.rs` |
| Captions | `transcription.rs` (Whisper and Parakeet models, downloaded on demand) |
| Settings and state | `settings_window.rs`, `settings_pages.rs`, `store.rs`, `presets.rs`, `diagnostics.rs` |
| Other windows | `onboarding_window.rs`, `teleprompter_window.rs`, `permissions_ui.rs`, `permission_guide.rs` |
| Updates | `updates.rs`, `installer.rs`, `http.rs` |
| Shared UI | `ui/` (components), `theme.rs` (colours and type), `i18n/` (Chinese tables, `docs/i18n.md`) |
| Platform glue | `platform.rs` and `platform/` |
| Dev-only | `dev_restore.rs` (the `--watch` relaunch), `snapshot.rs`, the `*_bench.rs` modules |

## Recording and the library

Recording runs through the root workspace's `frameever-recording` actors, the
same ones the CLI drives. A Studio recording and an Instant recording each
become a project: a `.ever` folder in the storage folder
(`frameever_utils::storage`), with `frameever.json`, `recording-meta.json` and,
for Studio, the `project-config.json` the editor edits. A project recorded in
the app opens and exports with `frameever` on the command line, and the other
way round.

Starting a recording opens the controls bar (320×150, `app_windows.rs`) in its
"Starting" state and hides the main window. The bar is a non-activating
`WindowKind::PopUp` window, so its buttons work without taking focus from the
app being recorded, and it is excluded from the capture.

The library (`library.rs`) is a scan of the storage folder, not an index: every
folder `frameever_utils::storage::is_project` accepts, plus screenshots, read
with `frameever-project`'s own loaders. Thumbnails are the images baked into
each project, decoded and downsampled on the background executor and cached.
Exports made from a project are recorded in its `frameever.json`
(`storage::record_export`), and the export dialog starts in the last exports
folder.

`FRAMEEVER_GPUI_STORAGE_DIR` and `FRAMEEVER_GPUI_APP_DATA_DIR` point the app at
another storage folder and app data folder; `pnpm fe dev` uses them for the
test profile in `target/testdata/`.

## Settings

Settings are one JSON file, `frameever_utils::storage::settings_file()`, read
and written by `store.rs`. A write merges one key into its section of the raw
JSON (`store::set_store_setting`), so keys the app does not know survive, and a
file that exists but does not parse is never replaced with a fresh one.
`FRAMEEVER_GPUI_TAURI_STORE` points every read and write at another file, which
is how probe runs avoid touching real settings.

## The component library

`src/ui/` is the one component set every window uses:

| Module | What it is |
|---|---|
| `button.rs` | `Button` (variants and sizes) and `IconButton` |
| `toggle.rs`, `slider.rs`, `segmented.rs` | switch, slider with its snapping maths, segmented control |
| `menu.rs`, `select.rs` | `MenuState` (the keyboard contract) and `Menu`; `Select`, the trigger a menu opens from |
| `text_input.rs` | `TextInputState` and `TextInput`, see [Text input](#text-input) |
| `surface.rs` | `Card`, `Popover`, `SettingRow`, `Section` |
| `collapsible.rs` | `Collapsible`, with measured-height animation |
| `tab_rail.rs`, `field.rs`, `editor_button.rs` | the editor sidebar's tab rail, labelled setting containers and header buttons |
| `radio_cards.rs`, `number_field.rs`, `position_pad.rs` | full-width radio cards, a number field with its raw-value state machine, the 2D focal-point pad |
| `selection_header.rs` | the Done / "N selected" / Delete row the segment panels open with |
| `progress.rs`, `kbd.rs`, `tooltip.rs` | circular progress, key chips, tooltips over gpui's `.tooltip()` |
| `color_picker.rs`, `microphone_level.rs`, `windows_caption.rs` | colour picker, live microphone meter, the Windows caption buttons |

Components have one named constructor per surface (`::plain`, `::body`,
`::settings`, `::glass`), and that surface's colours come from `theme.rs`. A
call site that needs a colour no surface provides sets it on the builder.
Handlers are `impl Fn(&ClickEvent, &mut Window, &mut App)`, which is what
`cx.listener(..)` produces for any window, so components need no generic
window parameter.

Behaviour worth knowing:

- **Menu.** Click-away and Escape dismiss; the arrow keys walk the rows and
  wrap; Home and End jump; Enter or Space commits; the value in force carries a
  check mark. `MenuState` is a plain struct, so all of it is unit-tested
  without a window.
- **Slider.** The track's rect comes back from a zero-sized `canvas` in
  prepaint, and a full-bleed transparent drag layer keeps a drag tracking
  after it leaves the control. `snap_to_step` pre-quantises in `f64`, because
  `0.1` is not representable and a naive quantisation snaps an exact half-step
  down.
- **Collapsible.** The content is measured by a `canvas` at its natural size
  and the height is animated over 200ms by a ticker task, because gpui only
  paints on invalidation. An interrupted transition restarts from where the
  panel is.

### Text input

`ui/text_input.rs` is a real text field on gpui's own input seam:
`EntityInputHandler`, a custom `Element` and `window.handle_input`, the same
three pieces as gpui's `examples/input.rs`, plus wrapping, word and line
motion, grapheme-aware deletion, selection, clipboard, field-scoped undo and
per-surface theming. Text arrives through `replace_text_in_range` and
`replace_and_mark_text_in_range`, so IME composition and dead keys work.

The design follows from gpui's key dispatch order:

1. Key bindings are matched first against the focused node's context stack. A
   matching action consumes the event, so no `on_key_down` on the path sees it.
2. Only an unhandled event reaches the platform input context, which composes
   it and calls back into the field.

So every editing chord is a bound action in the `TextInput` key context
(`ui::text_input::bind_keys`). Because a matched binding consumes the key, a
focused field structurally stops the editor's delete-the-selection and
undo-the-project handlers. A bare printable key can never be bound, or it
could never be typed, so `s`, `c` and Space still reach an ancestor's
`on_key_down`; the editor checks `ui::text_input_has_focus` before acting on
them.

The field emits `Changed`, `Confirmed`, `Cancelled` and `Blurred`, and the
window decides what they mean: Escape clears the filter in the main window,
reverts in settings and commits in the editor's project-name field.

## The editor

`editor_window.rs` opens one window per project path (canonicalised, so two
spellings of one folder share a window), 1275×800 fitted to the screen. The
main window hides while an editor is open and comes back when the last one
closes.

**Loading.** Everything expensive is off the UI thread. A pre-flight runs the
same synchronous construction `EditorInstance::new` would, on a background
thread under `catch_unwind`, so a damaged project lands on the window's error
state instead of taking the process down. `EditorInstance::new` then runs on
the `gpui_tokio` runtime. Opening a project is not read-only:
`EditorInstance::new` writes `project-config.json` back when it has to
synthesise a timeline or clip offsets, so tests work on copies.

**Frames.** The renderer's frame callback feeds a bounded channel. Each frame
is un-padded from wgpu's 256-byte row alignment and swapped RGBA → BGRA for
gpui's atlas in `frameever_gpui_kernels::frame::unpad_rgba_to_bgra`, built at
`opt-level = 3`, then painted with `window.paint_image`. A seek moves the
playhead only; the picture comes from a push on the preview channel, so a
paused seek pushes one.

**Playback.** The engine's state callback runs off the main thread, so it
stores the playhead in an `AtomicU32` and pokes a one-slot channel; the main
thread reads the newest value, so a burst coalesces instead of queueing. Play,
pause and seek write desired state (`playing`, `seek`, `seek_gen`) and wake a
driver task on the tokio runtime that applies the difference, so a drag scrub
never builds a backlog of positions already passed.

Measured when the editor was ported (`FRAMEEVER_GPUI_AUTO_PLAYBACK=16`, a 15.9s
Studio recording, 1080×702 frames):

```
playback fps=59.9 frames=949 dropped=0 (rendered=949 rendered_fps=59.9 paints=1570 convert_avg=933us over 15.84s)
```

With all eleven timeline rows drawn the run held 59.6 fps with no drops, and
the live editing layer added nothing measurable (59.9 fps, no drops). The same
frame conversion compiled at `opt-level = 0` took 30.1ms a frame and dropped
45% of frames, which is why it lives in `kernels/`.

## The timeline

`editor_timeline.rs` holds the view state: `zoom` is visible seconds and
`position` is the second at the left edge. Zoom is clamped to
`[MAX_ZOOM_IN = 3, min(total, 600)]`, position to
`[0, max(zoom-out limit, total) + 4 - zoom]`, and a zoom keeps its origin (the
playhead, or the hovered time for a ctrl-wheel) at the same fractional x.
Without ctrl the wheel pans; gpui's scroll delta has the opposite sign to a DOM
wheel delta and is negated before use. A pinch arrives as a `PinchEvent` and
maps onto the same zoom formula. Segments outside the viewport plus
`SEGMENT_RENDER_PADDING` are not built, and labels and waveform peaks are
shared (`SharedString`, `Arc`), so a 60fps playhead does not allocate per
segment.

Clip segments live in gapless recording time; every other track lives in
output time. A full-screen text segment pauses the recording clock, so the clip
track converts through `TimelineConfiguration::hold_windows`
(`effective_to_output`) and its box stretches across the pause.

### Editing

- **Undo** (`editor_edits.rs`, `ProjectHistory`): each entry is a whole
  `ProjectConfiguration` snapshot, the stack holds 100, a new change drops the
  redo tail, and a drag is one entry (`pause` and `resume` bracket it). The
  header's undo and redo buttons clear the timeline selection first.
- **Writes.** One change goes to the undo stack, to the renderer (with a
  preview push when paused) and to disk after a 250ms debounce through
  `ProjectConfiguration::write`. Closing the window flushes a pending write.
  Unknown keys in `project-config.json` are not kept: the file is the typed
  configuration.
- **Seeking.** A press on the ruler scrubs from mousedown; a press elsewhere in
  the timeline seeks once, on release, to the press position. Times within
  `START_SNAP_PX` of the start snap to zero.

## gpui traps

- **Do not touch the window from inside `open_window`'s builder closure.** The
  platform window is not finished: a resize there gives a viewport that
  disagrees with the scale factor, and a task spawned there never schedules a
  frame. Pass the size in the bounds and start work once the handle exists.
- **Do not change native window state from inside a gpui update.** Native
  move, resize and frame calls fire gpui's own callbacks synchronously, which
  re-borrow the `App` and log `RefCell already borrowed`. Take the handle in
  the update and make the native call from a spawned task.
- **`svg()` does not inherit `text_color`.** Set the colour on the `svg()`
  element itself, or it draws nothing.

## Checking changes

Anything visible is checked by the owner in the running app (`AGENTS.md`). For
measurements and repeatable runs, debug builds read `FRAMEEVER_GPUI_AUTO_*`
hooks, each going through the same handler a click takes. The common ones:

| Variable | Does |
|---|---|
| `FRAMEEVER_GPUI_AUTO_RECORD=studio:5` / `instant:4` / `screenshot` | records the primary display for N seconds, or takes one screenshot |
| `FRAMEEVER_GPUI_AUTO_SETTINGS=<page>` | opens settings on that page |
| `FRAMEEVER_GPUI_AUTO_EDITOR=<project>` / `=1` | opens the editor on a project, or on the newest Studio recording |
| `FRAMEEVER_GPUI_AUTO_PLAYBACK=<seconds>` | plays that long, then pauses and logs the `playback fps=…` line |
| `FRAMEEVER_GPUI_AUTO_SEEK=<fraction>` | seeks through the click path before the play hook |
| `FRAMEEVER_GPUI_AUTO_PLAYBACK_TORTURE=<cycles>` | play → pause → seek cycles |
| `FRAMEEVER_GPUI_AUTO_SIDEBAR=<tab>[:<scroll>]`, `FRAMEEVER_GPUI_AUTO_SELECT=<track>:<i>` | open a sidebar tab, select timeline segments |
| `FRAMEEVER_GPUI_MUTE_AUDIO=1` | swaps the editor's audio output for the headless one |

Each editor load logs one `editor timeline model rows=…` line naming every
track and its segment count, each zoom or pan logs a `timeline transform`
line, and each committed edit logs a `timeline edit` line, so a scripted run is
checked against the log rather than a screenshot.

`tests/editor_frame0.rs` renders frame 0 of a copied Studio project through a
real `EditorInstance` with the headless audio sink
(`FRAMEEVER_GPUI_E0_PROJECT=<project>`); with no project it skips.

