# Handoff: FrameEver migration (state at 2026-10-24)

Read this first, then the open issues on `AniBullet/FrameEver` (what is left), `docs/upstream-sync/0.6.1.md` (full log, per-fix verdicts, removed-feature audit, roadmap, upstream watch list), `docs/adr/0001`–`0007` and `docs/i18n.md`.

## Where things stand

- `main` now holds FrameEver. On 2026-10-24 the work on `merge/upstream-0.6.1` landed on `main` as one commit whose parents are the previous `main` (Cap-aniBullet 2.2.5) and upstream Cap 0.6.1, so our intermediate commits are gone. Since ADR-0007 (2026-10-24) we no longer merge upstream: it is a read-only reference, and fixes worth having are ported by hand. The 2.2.2–2.2.5 recording fixes on the old `main` were re-judged first (`0.6.1.md`). The local branch `merge/upstream-0.6.1` keeps the full history and is not pushed.
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
- Both workspaces build with Rust stable (`rust-toolchain.toml`; the root was on 1.88 only for the frozen Tauri app and moved to stable on 2026-10-24 so dependencies can follow their latest releases). Version numbers: both Cargo manifests say `3.0.0-dev`; CI stamps the tag's version.
- Never screenshot the owner's screen; ask them to look instead.
- Upstream `CapSoftware/Cap` is read-only: we merge from it and read it, and never push, open PRs or file issues there. The GitHub fork link was removed (2026-10-24), so PRs default to our repo. In this clone the `upstream` remote has a broken push URL and fetches no tags, and `gh repo set-default` is `AniBullet/FrameEver`; repeat that setup in any new clone (`docs/agents/issue-tracker.md`).

## What is left

The open issues on `AniBullet/FrameEver` are the one list (`gh issue list`); native blocking links give the order. On 2026-10-24 the owner went through the old backlog item by item (grilling) and chose an independent, legacy-free product:

- ADR-0006: one visible storage folder (`Videos\FrameEver` with `Recordings`, `Screenshots`, `Exports`), projects named `.ever` with a format version, export paths recorded per project, no database; the app holds no code for Cap-aniBullet 2.x or `.cap` data. Older data comes in through FrameEver Converter, a separate download.
- ADR-0007: upstream is a reference, not a merge baseline (supersedes ADR-0003). Internal names are renamed to `frameever-*` / `FRAMEEVER_*`, and macOS/Linux code is removed, before the first release.
- Order before 3.0.0: delete the Tauri app → remove macOS/Linux crates and code paths → rename crates and environment variables → docs rewrite; storage work from the spec AniBullet/FrameEver#1 (its sub-issues) → FrameEver Converter → first release (owner). Small independent tickets (compression notification, Excluded Windows note, software-encoder export check, Chinese captions, MiSans) can be taken any time.
- Not ticketed on purpose: dependency upgrades beyond version ranges (another session is doing them; remaining groups are listed under Phase 4 in `0.6.1.md`); the in-app automations runner and upstream fixes to port later (upstream watch list in `0.6.1.md`); recording start latency (part of the benchmark ticket).

### UI redesign and library (next: `/wayfinder`)

The UI redesign and the library window are one effort, too big and foggy for tickets yet. The owner will chart it with `/wayfinder` in a fresh session; when the map clears it goes through `/to-spec` and `/to-tickets`. Starting brief the owner approved:

1. The library is a first-class surface, reachable in one obvious step from the main window, after a recording finishes, and from the tray. 2.x hid it behind an unlabelled 16 px grid icon in the main window header (`new-main/index.tsx` in the old Tauri app), so users did not find it.
2. The library is designed together with the main window, settings, editor, recording controls (including the recording settings popover upstream left as a TODO) and tray menus, on one token system (colour, type, radius, spacing, motion) in `theme.rs`, replacing the glass and blur material.
3. Direction: pure flat, no gradients or glass, clean, current and premium; core colour Klein blue `#1F3FD8`; smooth animated transitions; a few hand-drawn touches from the welcome page (empty states, a drawn tick when an export or recording finishes, a local symbol instead of the Instant mode upload-cloud doodle). Settle it with HTML prototypes first.
4. Every 2.x library capability returns in a better form: filters by type and status with counts, search, several views, multi-select, batch delete and compress, a detail panel (original/compressed switch, three-way delete, copy file, open folder), context menu, refresh on focus and on new recording, open the library after recording; plus "exported" status from the export record. Evidence: `2x-feature-audit.md`.
5. The data layer is settled by ADR-0006 and is not reopened in the map. The UI/UX half of the open-source benchmark feeds this effort.

## Owner preferences (also in Claude memory)

- Rewrite upstream features to be local rather than delete them; delete only what has no local value.
- Repo docs mention only this project. No code comments (CLAUDE.md). UI languages: English and Simplified Chinese only.
- When naming, propose one best option and check collisions first.
- Never take screenshots of the owner's screen.
- Prefers flat design. Proper nouns are not translated in the Chinese UI.
