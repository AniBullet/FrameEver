# Handoff: FrameEver (state at 2026-10-25)

Read this first, then the open issues on `AniBullet/FrameEver` (what is left), `docs/upstream-sync/0.6.1.md` (full log, per-fix verdicts, removed-feature audit, roadmap, upstream watch list), `docs/adr/0001`–`0007` and `docs/i18n.md`. Work continues in Claude Code (Claude Desktop) and, when its quota runs out, in Cursor; both read this file.

## Where things stand

- `main` holds FrameEver. On 2026-10-24 the merge with upstream Cap 0.6.1 landed on `main` as one commit (parents: Cap-aniBullet 2.2.5 and upstream 0.6.1). The intermediate branch `merge/upstream-0.6.1` was deleted on 2026-10-25; its final tree equals that merge commit, so nothing was lost. Since ADR-0007 upstream is a read-only reference: we never merge it again and port fixes by hand.
- The product is the GPUI app `apps/desktop-gpui` (ADR-0001) plus the CLI `apps/cli`. The Tauri app and `packages/ui-solid` were deleted on 2026-10-25 (AniBullet/FrameEver#2); their assets now live in the GPUI app, and packaging reads the GPUI manifest for the version.
- Hard line (ADR-0002): user content never leaves the machine. Allowed network: update checks and downloads from our GitHub Releases, Whisper/Parakeet model downloads, and links the user opens in their browser. AI-agent surfaces return metadata only.
- Done before this batch:
  - Cloud strip of the GPUI app and CLI; local rewrites of the updater (own feed), `frameever update`, changelog, export logs, report-issue link, local MCP and agents config.
  - Release pipeline (`.github/workflows/release.yml`) and updater signing key (public key id `7A6F5207B3A2600B`; secrets and variable are on GitHub; the owner holds the backup).
  - Branding as FrameEver · 帧刻 (ADR-0005, constants in `crates/utils/src/brand.rs`). The first-launch import of Cap-aniBullet 2.x settings was removed from the app by AniBullet/FrameEver#9; bringing old data in is FrameEver Converter's job (ADR-0006).
  - Welcome page redesigned to match the dark, hand-drawn onboarding steps.
  - English and Simplified Chinese UI (`docs/i18n.md`): ~1,700 strings, a language option in Settings → General, and tests that keep the tables complete.
  - HEVC and AV1 export (editor export panel and `frameever export --codec`).
  - Compression and version management for Instant recordings (Settings → Recordings), including automatic compression.
  - Brand: core colour Klein blue `#1F3FD8`; icon and installer art from `packaging/brand/render_brand.py` (`docs/brand/app-icon.md`).
  - UI fonts: Inter for English, HarmonyOS Sans SC for Chinese, loaded only for the Chinese UI (`theme::ui_font()`, `docs/i18n.md`). HarmonyOS Sans is fetched by `pnpm fe setup` and never committed.
  - Dependency upgrades to current stable majors (2026-10-24); what is held back and why is under Phase 4 in `0.6.1.md`.
  - `pnpm fe privacy` and the pre-push hook (see Environment).
  - First release number set to 1.0.0 (ADR-0007); `CHANGELOG.md` has an unreleased `[1.0.0]` section.
- Done in the 2026-10-25 batch (all squash-merged into `main`):
  - AniBullet/FrameEver#33 (closes #32): Lucide (ISC/MIT) and Phosphor (MIT) icon licence notices beside the icons, in the installer and in Settings credits; 15 unused SVGs deleted; a test fails on any unreferenced icon. The installer was not rebuilt to look at the notice.
  - AniBullet/FrameEver#34 (closes #20): `FRAMEEVER_EXPORT_FORCE_SOFTWARE_ENCODER=1` forces libx265 and SVT-AV1 for HEVC/AV1 export and compression; measurements in `0.6.1.md`.
  - AniBullet/FrameEver#36 (closes #35): software HEVC uses CRF derived from bits per pixel instead of the NVENC bitrate; the test clip went from 6.2 MB to 0.75 MB at SSIM 0.9963.
  - AniBullet/FrameEver#37 (closes #18): Windows notifications when automatic compression succeeds or fails (log and folder buttons on failure). Not yet clicked through with a real Instant recording.
  - AniBullet/FrameEver#38 (closes #21): Chinese captions route to Whisper (Parakeet has no CJK vocabulary), split UTF-8 tokens are rebuilt, no spaces between CJK characters, full-width punctuation, a Simplified Chinese prompt. The long-trim crash did not reproduce.
  - AniBullet/FrameEver#39 (closes #2): Tauri app deleted. Sounds, music and backgrounds were not checked by hand in a running dev build.
- Storage module (AniBullet/FrameEver#9, 2026-10-25): `frameever_utils::storage` is the one place that knows the app data folder, the settings file, the known recordings and screenshots folders (custom, default, previous; `FRAMEEVER_GPUI_RECORDINGS_DIR` collapses recordings to one folder) and what counts as a project. The app's library, delete guards, recovery scan and the CLI's `recordings list` all go through it, so the CLI now lists the custom and previous folders too. Behaviour is otherwise unchanged: still `.cap`, still AppData; #10 (`.ever`) and #11 (`Videos\FrameEver`) change the rules inside the module. The recordings scan now skips folders without the project extension, as the CLI always did. `pnpm fe test --all` now also runs the `frameever-utils` tests.
- Storage folder (AniBullet/FrameEver#11, 2026-10-25): one setting, `storagePath` (default `Videos\FrameEver`), holds `Recordings` and `Screenshots`; screenshots follow it. Changing it keeps the old folder in `previousStoragePaths`, which Settings lists with a Remove button. An unavailable storage folder sends new projects to the default and stays in settings. `recordingsPath`/`previousRecordingsPaths` and AppData `recordings`/`screenshots` are no longer read. `FRAMEEVER_GPUI_STORAGE_DIR` replaces `FRAMEEVER_GPUI_RECORDINGS_DIR`; `fe dev` uses `target\testdata\storage`. Checked in a dev build on 2026-10-26: an area recording and an area screenshot landed in `Recordings` and `Screenshots` of the storage folder, both stamped with format version 1.
- Tests: `pnpm fe test --all` is green on `main` (GPUI 1067, enc-ffmpeg 180, export 45, CLI 48; GPUI dropped from 1075 when #5 deleted the Linux-only tests). After the folder rename, a stale `ffmpeg-sys-next` build cache pointed at the old folder and linking failed with `avcodec.lib` missing; `cargo clean -p ffmpeg-sys-next` in both workspaces fixed it.
- Crate names (AniBullet/FrameEver#6, 2026-10-25): every `cap-*` package is `frameever-*` (Rust paths `frameever_*`), the CLI package is `frameever`, and `crates/cap-muxer`, `crates/cap-muxer-protocol` and `crates/cap-test` are `crates/muxer`, `crates/muxer-protocol` and `crates/test`. Build outputs are `frameever-gpui`, `frameever` (CLI) and `frameever-muxer`; installed names are unchanged (`FrameEver.exe`, `frameever-cli`, `frameever-muxer`). App data files keep their names (`cap-gpui.pid`, `cap-gpui.log`, `cap-gpui.handoff`), and the installer still removes `cap-gpui.exe` and `cap-muxer.exe` left by old installs. Historical records (`docs/upstream-sync/0.6.1.md`, `docs/adr/`, `docs/superpowers/`, `docs/research/`, `CHANGELOG.md`) keep the old names.
- Platforms changed on 2026-10-25 (ADR-0008): Windows first, macOS kept for a later release, no Linux. The macOS-only crates deleted by #3 were restored from our history, and AniBullet/FrameEver#4 was redone to remove Linux only: the shared crates have no Linux `cfg` branches, modules, files, examples or target dependencies left (about 18,000 lines; `crates/workspace-hack` is generated and was left alone). Windows test results are unchanged. AniBullet/FrameEver#5 did the same for the GPUI app, the CLI and `scripts/`: no `target_os = "linux"` branches, Linux-only modules (`editor_modal.rs`), Linux target dependencies (`ashpd`, `wayland-client`, `x11rb`), the deb/AppImage updater kinds, the `bundle.linux` Tauri config, or Linux packaging scripts (`build-linux-*`, `finalize-linux-*`, `linux-bundle-config*`) are left. The updater-signing helpers moved into `scripts/finalize-macos-packages.mjs`.
- Still Linux-shaped after #5, left on purpose: the always-false "clean capture" stubs in `app_windows.rs` (`clean_capture_*`) and `hotkeys::editing_blocked`, with their callers in the quit gate, deep links and main window; `apps/desktop-gpui/patches/zed-linux.patch` (needs the zed checkout re-prepared); and the `linux` runners in `scripts/verify/`. `node --test scripts/*.test.mjs` fails only the three `bun-scripts` web-alias tests, because `bun` is not installed.

## What is left

The open issues on `AniBullet/FrameEver` are the one list (`gh issue list`); native blocking links give the order (`gh api repos/AniBullet/FrameEver/issues/<n>/dependencies/blocked_by`).

- Decisions of 2026-10-24: ADR-0006 (one visible storage folder `Videos\FrameEver` with `Recordings`, `Screenshots`, `Exports`; `.ever` projects with a format version; export records; no database; old data only through the separate FrameEver Converter) and ADR-0007 (upstream is a reference; internal names become `frameever-*` / `FRAMEEVER_*`; version starts at 1.0.0). ADR-0008 (2026-10-25) replaces the Windows-only scope: macOS code stays, Linux code goes.
- Next in the cleanup chain: #4 (Linux out of the shared crates), #5 (Linux out of the app, the CLI and the packaging scripts) and #6 (crates renamed to `frameever-*`) are done; #7 rename environment variables, #8 docs rewrite. The storage spec AniBullet/FrameEver#1: #9–#11 are done, #12 is next; then FrameEver Converter (#13, #14); then the first release (#15, owner).
- Owner tasks: #16 (phase 2 checks on real hardware), #17 (benchmark, after the release).
- Not ticketed on purpose: the in-app automations runner and upstream fixes to port later (upstream watch list in `0.6.1.md`); recording start latency (part of the benchmark ticket).

### Platform plan (ADR-0008)

1. Now until Windows 1.0.0: Windows is the only platform built, tested and released. macOS code stays in the tree but is not kept compiling; do not delete it, and do not spend time on it. Remove Linux code wherever it is found (#4 and #5 removed it from the crates, the app, the CLI and the packaging scripts).
2. After Windows 1.0.0 (#15): make the macOS build compile and run on the owner's Mac (start with frameever-rendering's Metal interop, which was not ported to wgpu 30), then add a macOS CI build and packaging job: AniBullet/FrameEver#40 (blocked by #15) and #41 (blocked by #40). Signing and notarization need an Apple Developer account, which the owner decides on.
3. macOS fixes from upstream are ported by hand (ADR-0007), from the upstream watch list in `0.6.1.md`.

### UI redesign and library (wayfinder map AniBullet/FrameEver#23)

The UI redesign and the library window are one effort, the wayfinder map AniBullet/FrameEver#23 ("Map: FrameEver 1.0 UI/UX redesign and library"). In Claude Code continue it with `/mattpocock-skills:wayfinder 23` (one human-in-the-loop ticket per session). Its four research tickets are resolved; their reports are on the remote branches `research/ui-references`, `research/gpui-animation`, `research/installer-design` and `research/icon-family` (`docs/research/*.md`), and each gist is in the map's "Decisions so far". Open decisions, all needing the owner: #29 main-window/library structure (clickable grayscale wireframes), #28 visual direction (three directions from the research: Graphite Studio, Paper and Ink, Klein Field; note `#1F3FD8` is only ~2.4:1 as text on dark, so dark mode needs a lighter accent tint for text), #31 installer route (own NSIS template vs a GPUI front end), then #30 library behaviour. The redesign ships in 1.0.0, so once the map clears it goes through `/to-spec` and `/to-tickets` and those tickets block #15. Prototypes are HTML files the owner opens; never screenshot the owner's screen.

## How to work

- One ticket at a time, in this checkout, committing to `main` (or a short-lived branch), the way `/mattpocock-skills:implement` works: read the ticket, work test-first (`tdd`), run `pnpm fe check` while iterating and `pnpm fe test` (or `--all`) and `pnpm fe lint` at the end, review the diff against the ticket and CLAUDE.md, commit, push, close the issue with a comment naming the commit. Do not give single tickets their own worktree: each worktree compiles the GPUI app from scratch (10–20 minutes) and parallel builds starve each other. Worktrees plus an integration branch are only for `/mattpocock-skills:implement-spec` over a whole spec.
- In Claude Code: the Matt Pocock skills are installed (`grill-with-docs`, `to-spec`, `to-tickets`, `implement`, `implement-spec`, `wayfinder` are typed by the owner; `tdd`, `code-review`, `research`, `prototype`, `domain-modeling` can be used by the agent). Claude's memory for this project holds the owner's standing preferences.
- In Cursor: there are no skills or session tools. Follow the same steps by hand: pick the next unblocked issue, read `CLAUDE.md`, `AGENTS.md`, `GLOSSARY.md` and the ADRs it touches, write a failing test first, use `pnpm fe` for everything, and use `gh` for issues and PRs (`docs/agents/issue-tracker.md`). Keep the rules below; Cursor does not know them otherwise.
- Rules that apply everywhere: no code comments of any kind; UI strings in English and Simplified Chinese only (every new string needs a zh-CN entry); proper nouns stay English in the Chinese UI; upstream `CapSoftware/Cap` is read-only (never push, open PRs, comment or file issues there; `gh` targets `AniBullet/FrameEver`); never screenshot the owner's screen; never bypass the pre-push privacy hook; commit messages in Chinese, ending with the co-author line the tool requires.

## Environment

- Everything goes through `pnpm fe <command>` (`scripts/fe.mjs`): `setup`, `doctor`, `dev [--watch] [--lang zh-CN]`, `logs`, `check`, `test [--all]`, `lint`, `fmt`, `export <codec>`, `package`, `release <version>`, `privacy`. CI (`release.yml`) runs the same `setup` and `package`. Human guide: `BUILD.md`; agent guide for debugging and fixing: `docs/agents/debugging.md`.
- Privacy: `pnpm fe setup` sets `core.hooksPath` to `.githooks`, so every `git push` first runs `pnpm fe privacy` over the commits being pushed (emails, local user and host names, keys, tokens, sensitive files). The allowlist is `scripts/privacy-allow.json`. `pnpm fe doctor` reports whether the hook is installed.
- The Claude Desktop pane "FrameEver 开发" (`.claude/skills/frameever-dev`, tracked in the repo) has buttons that call `pnpm fe`.
- Both workspaces build with Rust stable (`rust-toolchain.toml`). The GPUI manifest is the version source (`1.0.0-dev`); CI stamps the tag's version.
- Windows long paths are enabled (`LongPathsEnabled = 1`), but MSVC's linker, NSIS and some build scripts still fail past 260 characters. Packaging from a deep folder such as `.claude\worktrees\<name>` can fail; package from the main checkout.
- Leftover untracked build output from the deleted Tauri app may remain in `apps/desktop/` and `packages/ui-solid/` (`node_modules`, `gen`, `binaries`, `dist`); it is safe to delete. `.claude/worktrees/zed-cap` is the patched gpui checkout used by worktrees and can go once no worktree needs it.
- In this clone the `upstream` remote has a broken push URL and fetches no tags, and `gh repo set-default` is `AniBullet/FrameEver`; repeat that setup in any new clone (`docs/agents/issue-tracker.md`).

## Local folder

On 2026-10-25 the checkout was renamed to `D:\_Scripts\GitHub\frameever` (lower case). Claude Code's history and memory were copied to `%USERPROFILE%\.claude\projects\D---Scripts-GitHub-frameever*`; worktrees, `pnpm fe doctor` and memory were checked in the new folder. The first build there is a full rebuild because `target` holds absolute paths.

## Owner preferences (also in Claude memory)

- FrameEver is an independent product: no legacy weight from Cap-aniBullet 2.x or Cap; old data only through the converter.
- Repo docs mention only this project. No code comments. UI languages: English and Simplified Chinese only.
- When naming, propose one best option and check collisions first. The product name is written FrameEver (no space); in Chinese text put a space between Chinese and Latin words.
- Never take screenshots of the owner's screen.
- Pure flat design, no gradients or glass. Proper nouns are not translated in the Chinese UI.
- Merging ticket PRs is delegated to the agent after review; pushes are privacy-checked first.
