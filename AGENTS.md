# Repository Guidelines

FrameEver · 帧刻 is a local-only screen recorder and editor by aniBullet (`GLOSSARY.md`). Everything it captures, edits and exports stays on the user's machine. It ships on Windows first; macOS code is kept for a later release and Linux is not supported (ADR-0008).

Read `docs/upstream-sync/HANDOFF.md` first when resuming work: it holds the current state and what is next. The open issues on `AniBullet/FrameEver` are the one list of remaining work (`docs/agents/issue-tracker.md`).

## What ships

- **GPUI app** `apps/desktop-gpui` (`frameever-desktop-gpui`, binary `frameever-gpui`, installed as `FrameEver.exe`): recording, the editor, the library and settings, drawn natively with gpui (ADR-0001).
- **CLI** `apps/cli` (package `frameever`, installed as `frameever-cli`, run as `frameever`): recording, export and project commands for scripts, automation rules and AI agents (`apps/cli/README.md`).
- **FrameEver Converter** `apps/desktop-gpui/converter` (`frameever-converter-app`) on the core `crates/converter` (`frameever-converter`): the separate wizard that brings Cap-aniBullet 2.x or official Cap data into the storage folder. It is the only code that knows older layouts (ADR-0006).

There is no server, no account, no database and no cloud storage (ADR-0002). Settings are a JSON store in the app data folder; projects, screenshots and exports live in the storage folder (`Videos\FrameEver` by default), each project a `.ever` folder with a `frameever.json` (ADR-0006). The only network use is update checks and downloads from our GitHub Releases, caption model downloads, and links the user opens in a browser.

## Layout

Two Cargo workspaces, both on Rust stable (`rust-toolchain.toml`):

- Root workspace: `apps/cli` and `crates/*`.
  - `crates/recording`, `crates/rendering`, `crates/editor`, `crates/export`, `crates/project`: the recording and editing pipeline; an edit here rebuilds everything above it.
  - `crates/enc-*`, `crates/media*`, `crates/video-decode`, `crates/muxer*`: encoding, decoding and muxing (FFmpeg, Media Foundation; AVFoundation for macOS).
  - `crates/scap-*`, `crates/camera*`, `crates/audio`, `crates/cursor-*`: screen, camera, audio and cursor capture.
  - `crates/utils`: brand constants (`brand.rs`) and `storage`, the one place that knows the app data folder, the settings file, the storage folder and what counts as a project.
  - `crates/converter`: FrameEver Converter's core.
- GPUI workspace `apps/desktop-gpui` (members: the app, `kernels`, `converter`). It has its own `Cargo.lock`, and `pnpm fe setup` prepares the patched gpui checkout it builds against in `../zed-cap`.
  - `src/`: the app. `src/i18n/` holds the Chinese tables (`docs/i18n.md`). `kernels/` holds pixel and sample loops built at `opt-level = 3`.
  - `assets/`: fonts, icons, sounds, editor music and backgrounds.
  - `packaging/` and `tauri.conf.json`: installer packaging only. The Tauri CLI is used as the NSIS bundler; nothing links the Tauri runtime.
- `scripts/`: `fe.mjs` (the `pnpm fe` entry point), packaging, privacy scan and benchmarks.
- `docs/`: `adr/` (decisions), `agents/` (agent guides), `upstream-sync/` (handoff, roadmap and history), `i18n.md`, `brand/`.

`docs/upstream-sync/0.6.1.md`, `docs/adr/` and `CHANGELOG.md` are history and keep old names. Upstream `CapSoftware/Cap` is a read-only reference: fixes are ported by hand, never merged (ADR-0007).

## Build, test, run

Everything goes through `pnpm fe <command>` (`scripts/fe.mjs`; `pnpm fe` lists them). Local work, CI and the "FrameEver 开发" pane in Claude Desktop all call it. `BUILD.md` is the human guide; `docs/agents/debugging.md` is how to debug and fix things. Don't use other build commands.

| Command | Does |
|---|---|
| `pnpm fe setup` / `doctor` | Prepare the machine / say what is missing and how to fix it |
| `pnpm fe dev [--watch] [--lang zh-CN]` | Build and run the app on the test profile (`apps/desktop-gpui/target/testdata/`) |
| `pnpm fe test [filter]` / `--all` | GPUI tests / plus converter, encoder, export, utils, CLI and script tests |
| `pnpm fe check` | Compile the GPUI app, FrameEver Converter and the CLI with tests |
| `pnpm fe lint` / `fmt` | Format check, clippy, translation tests / format |
| `pnpm fe converter [--lang zh-CN]` | Run FrameEver Converter against a fresh fake 2.x install |
| `pnpm fe package` | Build the installer and FrameEver Converter into `dist/` |

When asked to "build", "package" or "打包" the app, run `pnpm fe package`.

## Coding style

- **No code comments of any kind** (`//`, `/* */`, `///`, `//!`, `#`, JSDoc). Code explains itself through names, types and structure; explanations go in docs.
- Rust: `rustfmt` and the workspace lints below. Modules snake_case, crates kebab-case and named `frameever-*`; environment variables `FRAMEEVER_*`.
- Scripts (`scripts/*.mjs`, the Claude Desktop pane): tab indent, Biome (`pnpm format`), files kebab-case.
- UI text: English and Simplified Chinese only. Every `t!` string needs a zh-CN entry; proper nouns (FrameEver, HEVC, Whisper) stay English in the Chinese UI (`docs/i18n.md`). How copy reads (voice, sentence case, errors, confirmations) is in `docs/copy-style.md`.
- UI design: flat only, no gradients or glass. Colours and type come from `src/theme.rs`.
- Use `GLOSSARY.md` terms in code, UI and docs: project, storage folder, export, Studio recording, Instant recording, library.
- Platforms: keep `#[cfg(target_os = "macos")]` and `cfg(unix)` code even though it is not kept compiling until the macOS work after Windows 1.0.0; delete Linux code wherever it turns up.

## Rust lints

These are denied in the workspace `Cargo.toml` and fail CI:

- `unused_must_use`: handle every `Result`, `Option` and `#[must_use]` value.
- `dbg_macro`: use `tracing::debug!` instead of `dbg!`.
- `let_underscore_future`: await futures instead of `let _ = async_fn()`.
- `unchecked_duration_subtraction`: `a.saturating_sub(b)` for `Duration`.
- `collapsible_if`: `if a && b {}` instead of nested `if`s.
- `clone_on_copy`: copy `Copy` values instead of `.clone()`.
- `redundant_closure`: `iter.map(foo)` instead of `iter.map(|x| foo(x))`.
- `ptr_arg`: take `&[T]` / `&str`, not `&Vec<T>` / `&String`.
- `len_zero`: `.is_empty()` instead of comparing `.len()` with 0.
- `let_unit_value`: `foo();` instead of `let _ = foo();` for unit returns.
- `unnecessary_lazy_evaluations`: `.unwrap_or(value)` for cheap defaults.
- `needless_range_loop`: iterate the collection (with `.enumerate()` when the index matters).
- `manual_clamp`: `value.clamp(min, max)`.

## Testing

- Work test-first. Rust tests sit beside the code (`#[cfg(test)]`) or in a crate's `tests/`; script tests are `scripts/*.test.mjs` (`node --test`).
- While iterating run `pnpm fe test <filter>`; before committing run `pnpm fe test --all` and `pnpm fe lint` once.
- Prove a fix by reproducing it first and checking it with a test or a measured result. Anything visible is checked by the owner in the running app: never take screenshots of the owner's screen.

## Commits and PRs

- Conventional prefixes (`feat:`, `fix:`, `improve:`, `refactor:`, `chore:`, `docs:`); messages in Chinese, naming the issue (`（#12）`).
- One ticket at a time, in the main checkout. Keep scope tight and update docs (and `HANDOFF.md`) when behaviour or the state of the work changes.
- `git push` runs `pnpm fe privacy` through the `.githooks` pre-push hook; never bypass it.
- `gh` targets `AniBullet/FrameEver`. Never push, open PRs, comment or file issues on `CapSoftware/Cap`.
