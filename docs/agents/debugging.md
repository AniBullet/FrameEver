# Debugging and fixing FrameEver (for agents)

Everything goes through one entry point, `pnpm fe <command>` (`scripts/fe.mjs`). Local work, CI and the "FrameEver 开发" pane in Claude Desktop all call it, so a fix that works with `pnpm fe` works everywhere.

## Rules

- Never take screenshots of the owner's screen. You cannot see the app: work from logs, command output, tests and the source. When something has to be looked at, ask the owner to look (the pane's "运行应用" button starts the app for them).
- Use the test profile. `pnpm fe dev` keeps settings, recordings and logs in `apps/desktop-gpui/target/testdata/`, so you never touch the owner's real data. Only `--real` uses their data; don't use it unless they ask.
- Prove the fix: reproduce first, then check with a test or a measurable result, never "it should work now".

## Commands

| Command | Use it to |
|---|---|
| `pnpm fe doctor` | Check the environment first when anything fails to build or start; each ✗ line says the fix |
| `pnpm fe setup` | Fix a ✗ from doctor (fetches FFmpeg, ONNX Runtime, DXC, writes `.cargo/config.toml`, prepares `../zed-cap`) |
| `pnpm fe dev [--lang zh-CN] [--watch]` | Build and run the app on the test profile |
| `pnpm fe logs [--grep <text>] [--lines N]` | Read the app log after a run (`--real` reads the installed app's log) |
| `pnpm fe check` | Compile the GPUI app, FrameEver Converter and the CLI with tests |
| `pnpm fe test [filter]` / `--all` | GPUI tests; `--all` adds converter, encoder, export, CLI and script tests |
| `pnpm fe lint` | Format check, clippy, translation tests: run before committing |
| `pnpm fe fmt` | Format what FrameEver changed |
| `pnpm fe export h264\|hevc\|av1` | Export the newest test recording to `target/testdata/exports/` |
| `pnpm fe converter [--lang zh-CN]` | Run FrameEver Converter against a fresh fake Cap-aniBullet 2.x install in `target/testdata/converter/` |
| `pnpm fe package` | Build the installer and `FrameEver-Converter_<version>_x64.exe` into `dist/` exactly as CI does |

Test cadence: while iterating run `pnpm fe test <filter>` (an app-only edit rebuilds in about 20 s); run `pnpm fe check` only when you want errors without running anything, since `test` compiles the same code. Run `pnpm fe test --all` and `pnpm fe lint` once, before committing. An edit in a shared crate (`frameever-project`, `-rendering`, `-recording`, `-editor`, `-export`) rebuilds everything above it, 30–45 s; renaming or upgrading crates rebuilds everything. `dev`, `check`, `test` and `lint` keep the four newest incremental caches per crate and delete older ones.

## Where things are

| What | Where |
|---|---|
| Test profile settings | `apps/desktop-gpui/target/testdata/appdata/store` (JSON) |
| Test recordings | `apps/desktop-gpui/target/testdata/recordings/*.ever` |
| App log (test profile) | `apps/desktop-gpui/target/testdata/logs/frameever.log.<date>` |
| App log (installed app) | `%LOCALAPPDATA%\io.github.anibullet.frameever\logs\` |
| Installer output | `dist/` |
| Translations | `apps/desktop-gpui/src/i18n/zh_cn/*.rs`, rules in `docs/i18n.md` |
| Brand constants | `crates/utils/src/brand.rs` |

Environment variables the app reads: `FRAMEEVER_GPUI_APP_DATA_DIR` (also moves the default storage folder to `<that folder>\FrameEver`, so a test profile never touches `Videos\FrameEver`), `FRAMEEVER_GPUI_STORAGE_DIR` (the only storage folder written and scanned), `FRAMEEVER_GPUI_LOG_DIR` (set by `fe dev`), `FRAMEEVER_GPUI_DEV_RESTORE` (window state across `--watch` relaunches), `RUST_LOG` (for example `RUST_LOG=frameever_gpui=debug,frameever_recording=debug pnpm fe dev`).

## Debugging by area

**The app won't start or crashes at launch.** Run `pnpm fe doctor`, then `pnpm fe dev` and read the console, then `pnpm fe logs`. Exit code `0xc0000135` / `STATUS_DLL_NOT_FOUND` means FFmpeg DLLs weren't on the path: start the app through `pnpm fe dev`, not by double-clicking the debug exe.

**Recording, export, or file handling.** Reproduce without the UI through the CLI: `cargo run -p frameever -- doctor --json`, `cargo run -p frameever -- targets --json`, `cargo run -p frameever -- record start --screen <id> --json --detach` then `record stop`, and `pnpm fe export <codec>` (set `FRAMEEVER_EXPORT_FORCE_SOFTWARE_ENCODER=1` to skip the GPU encoders and use libx264, libx265 or SVT-AV1; it also applies to HEVC compression). Inspect results with `ffprobe` from `target/ffmpeg/bin` (codec, frame count, duration) and compare quality with `ffmpeg -i a.mp4 -i b.mp4 -lavfi "[0:v][1:v]ssim" -f null -`. Unit tests for encoders live in `crates/enc-ffmpeg/src/mux/mp4.rs`, `crates/enc-ffmpeg/src/video/hevc.rs` (encoder selection) and `crates/export/src/compress.rs`; add one that fails before your fix.

**UI layout or text.** Read the render code (`src/*_window.rs`, `src/settings_pages.rs`, `src/editor_*.rs`) and the theme (`src/theme.rs`). Change it, run `pnpm fe check`, then ask the owner to look with `pnpm fe dev` (or `--lang zh-CN` for Chinese layout).

**Translations.** `pnpm fe test i18n::` names the exact problem: a `t!` string without a Chinese entry (add it to the area table), a stale entry (delete it), placeholders that differ, or one English string with two Chinese translations (pick one, or use `tc!` for real context differences). Window titles stay English on purpose.

**Settings or data import.** Edit or delete `target/testdata/appdata/store` and rerun `pnpm fe dev`. Import from Cap-aniBullet 2.x is tested in `store.rs` (`cap_anibullet_settings_are_imported_once…`).

**Packaging.** `pnpm fe package` prints each step. Missing DLL folders → `pnpm fe setup`. A Tauri bundle error names the file it couldn't find (paths in `apps/desktop-gpui/tauri.conf.json`). Without `TAURI_SIGNING_PRIVATE_KEY` it builds an unsigned installer on purpose. CI runs the same command in `.github/workflows/release.yml`.

## Common errors

| Symptom | Cause and fix |
|---|---|
| `bash: ... WSL ... execvpe(/bin/bash) failed` | PowerShell's `bash` is WSL's. Scripts need Git Bash; `pnpm fe` finds it for you |
| `std::hint::cold_path` / gpui compile errors | Wrong Rust for the GPUI app; it needs stable (`apps/desktop-gpui/rust-toolchain.toml`), the root workspace 1.88 |
| `cargo build` fails with "Access is denied" on `frameever-gpui.exe` | The app is still running; stop it (pane "停止", or close it) and rebuild |
| `libclang` / `bindgen` errors | `.cargo/config.toml` lost `LIBCLANG_PATH`; run `pnpm fe setup` |
| clippy `deref which would be done by auto-deref` on `tr(*x)` | Write `tr(x)` |
| `makensis` `Internal compiler error #12345: deflateInit() failed` | NSIS (32-bit) couldn't get ~1.4 GB for the 128 MB LZMA window that lets the app and CLI share data. Close memory-heavy programs and rerun `pnpm fe package --skip-sidecars` (no recompile of the helpers). Don't shrink the window: the installer grows |
| A test asserting English text fails after wrapping it in `t!` | It shouldn't: tests run in English. If code compares a translated string, compare an id instead |

## Before you say it's done

1. `pnpm fe lint` and `pnpm fe test --all` pass.
2. For anything visible, the owner has looked at it in the running app.
3. Update `docs/upstream-sync/HANDOFF.md` if the state of the work changed.
