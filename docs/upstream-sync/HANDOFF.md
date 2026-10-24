# Handoff: FrameEver migration (state at 2026-10-24)

Read this first, then `docs/upstream-sync/BACKLOG.md` (what is left), `docs/upstream-sync/0.6.1.md` (full log, per-fix verdicts, removed-feature audit, roadmap), `docs/adr/0001`–`0005` and `docs/i18n.md`.

## Where things stand

- `main` now holds FrameEver. On 2026-10-24 the work on `merge/upstream-0.6.1` landed on `main` as one commit whose parents are the previous `main` (Cap-aniBullet 2.2.5) and upstream Cap 0.6.1, so our intermediate commits are gone but future upstream merges still find the common ancestor. The 2.2.2–2.2.5 recording fixes on the old `main` were re-judged first (`0.6.1.md`). The local branch `merge/upstream-0.6.1` keeps the full history and is not pushed.
- Upstream CapSoftware/Cap 0.6.1 is merged. The product is the GPUI app `apps/desktop-gpui` (ADR-0001); `apps/desktop` (Tauri) is frozen and only kept for assets the GPUI app reads by relative path.
- Hard line (ADR-0002): user content never leaves the machine. Allowed network: update checks and downloads from our GitHub Releases, Whisper/Parakeet model downloads, and links the user opens in their browser. AI-agent surfaces return metadata only.
- Done:
  - Cloud strip of the GPUI app and CLI; local rewrites of the updater (own feed), `frameever update`, changelog, export logs, report-issue link, local MCP and agents config.
  - Release pipeline (`.github/workflows/release.yml`) and updater signing key (public key id `7A6F5207B3A2600B`; secrets and variable are on GitHub; the owner holds the backup).
  - Branding as FrameEver · 帧刻 (ADR-0005, constants in `crates/utils/src/brand.rs`), with a one-time import of Cap-aniBullet 2.x settings. Verified by running the debug build against a fake 2.x store.
  - Welcome page redesigned to match the dark, hand-drawn onboarding steps.
  - English and Simplified Chinese UI (`docs/i18n.md`): ~1,700 strings, a language option in Settings → General, and tests that keep the tables complete.
  - HEVC and AV1 export (editor export panel and `frameever export --codec`). Measured on one 1080p60 clip: H.264 9.2 MB, HEVC 3.5 MB, AV1 2.4 MB, all SSIM ≈ 0.997 against H.264.
  - Compression and version management for Instant recordings (Settings → Recordings): compress to an HEVC copy, switch between original and compressed, delete either, plus "compress instant recordings automatically" and "then keep only the compressed version" (same store keys as 2.x). Clear error dialogs when a file is in use.
  - Brand: core colour Klein blue `#1F3FD8` (owner pick, 2026-10-24). Icon is a Klein blue plate with cream and sky-blue timeline clips and an iOS-style record button (white ring, navy gap, red dot). All brand assets come from `packaging/brand/render_brand.py`; 16/20/24 px are drawn pixel by pixel; taskbar, title bar and tray load exact frames from `icon.ico`. Installer sidebar is light with left-aligned type at 2×. Palette, method and pitfalls: `docs/brand/app-icon.md`. Owner confirmed taskbar and tray are crisp.
  - UI fonts: Inter for English, MiSans for Chinese, chosen by language (`theme::ui_font()`, details in `docs/i18n.md`). MiSans is fetched by `pnpm fe setup` and never committed (its license forbids redistributing the files); the settings sidebar credits it. Installer adds about 18 MB.
  - Text sharpness on Windows: settings and welcome windows are opaque there, so text uses ClearType like the main window. The Chinese installer uses Microsoft YaHei UI instead of NSIS's default 新宋体.
  - Proper nouns stay English in the Chinese UI (CLI etc.); the CLI page explains it in one line. The CLI PATH hint now names the real command, `frameever`.
  - Automations: upload, webhook and "organization is" items in old rules show a "removed" notice; their cloud fields are gone; rule data is kept.
- All GPUI tests pass (1059); `scripts/fe.test.mjs` passes (8); root crates touched (enc-ffmpeg, export, cli) pass their tests and clippy.

## Environment

- Everything goes through `pnpm fe <command>` (`scripts/fe.mjs`): `setup`, `doctor`, `dev [--watch] [--lang zh-CN]`, `logs`, `check`, `test [--all]`, `lint`, `fmt`, `export <codec>`, `package`, `release <version>`. CI (`release.yml`) runs the same `setup` and `package`. Human guide: `BUILD.md`; agent guide for debugging and fixing: `docs/agents/debugging.md`.
- The owner's machine has a Claude Desktop pane, "FrameEver 开发" (`.claude/skills/frameever-dev`, `/fe-dev`; local only, `.claude/` is gitignored), whose buttons call `pnpm fe`.
- Root workspace: Rust 1.88 (`rust-toolchain.toml`). GPUI app: Rust stable. Version numbers: both Cargo manifests say `3.0.0-dev`; CI stamps the tag's version.
- Never screenshot the owner's screen; ask them to look instead.
- Upstream `CapSoftware/Cap` is read-only: we merge from it and read it, and never push, open PRs or file issues there. The GitHub fork link was removed (2026-10-24), so PRs default to our repo. In this clone the `upstream` remote has a broken push URL and fetches no tags, and `gh repo set-default` is `AniBullet/FrameEver`; repeat that setup in any new clone (`docs/agents/issue-tracker.md`).

## What is left

Everything still open is in `docs/upstream-sync/BACKLOG.md`, the one list (P0 before the first release, missing 2.x features, UI redesign, leftovers, things to verify). Work from it and delete items as they land. The 2.x comparison behind several items is `docs/upstream-sync/2x-feature-audit.md`.

Already settled and not in the backlog: the "cap" rename of user-visible names (rule in ADR-0005); the GitHub repo rename to `AniBullet/FrameEver` on 2026-10-23 (GitHub redirects the old URL, so 2.x installs still reach the feed; release secrets and variables moved with the repo).

## Owner preferences (also in Claude memory)

- Rewrite upstream features to be local rather than delete them; delete only what has no local value.
- Repo docs mention only this project. No code comments (CLAUDE.md). UI languages: English and Simplified Chinese only.
- When naming, propose one best option and check collisions first.
- Never take screenshots of the owner's screen.
- Prefers flat design. Proper nouns are not translated in the Chinese UI.
