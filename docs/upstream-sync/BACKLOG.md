# Backlog (state at 2026-10-24)

The one list of everything still open in the FrameEver migration. When an item is done, delete it here and record the result in `HANDOFF.md` or `0.6.1.md`. Sources swept: `HANDOFF.md`, `0.6.1.md` (roadmap and re-audit), `2x-feature-audit.md`, open upstream issues that touch the GPUI app on Windows, and a search of the code and docs for leftovers.

Tracker note: GitHub Issues are on for `AniBullet/FrameEver` (2026-10-24). Items can move to issues as they are picked up; until an item has an issue, this file holds it.

## P0: before the first release

1. **2.x library not found after upgrade.** Recordings, exported videos (every 2.x Instant recording ended up there), `_compressed` copies and screenshots under `Videos\Cap Recordings` or the custom save folder are never scanned. Fix the import and scanning; details in `2x-feature-audit.md`. Done when a fake 2.x layout (default folder and custom folder) shows every item in the library, with tests.
2. **Settings keys not mapped from 2.x:** `recordingsSavePath` → `recordingsPath` (2.x meant the base folder), `recordingQuality` → `studioRecordingQuality`. Same tests as item 1.
3. **First release.** `pnpm release 3.0.0`; expect to fix CI on the first run. After installing, check that `frameever://` links open the app, the installer shows the new artwork, and the upgrade from an installed 2.x keeps settings and the library.
4. **Owner review in the running app:** the welcome page, the export codec option, the Recordings compress buttons.

## P1: missing 2.x features

5. **Library window.** Rebuild the 2.x window (filters with counts, search, three views, multi-select, batch delete and compress, detail panel with version switch and three-way delete, context menu). Build it on the new UI system (item 9). Multi-session: write a spec, then tickets.
6. **Open the library after recording** (`openLibraryAfterRecording`), and point the tray at the library window. Blocked by item 5.
7. **Notification when auto-compress finishes or fails** (2.x had it; the GPUI auto-compress path has none).
8. **Exported videos in the library.** 2.x exported to a known folder and showed "exported" status; the GPUI app asks for a path each time and forgets it. Owner decision: a default export folder the library scans, or remember exported paths in the project.

## P1: UI redesign

9. **New UI system.** Owner wants pure flat, current, premium, a good palette and smooth animated transitions. Settle the direction with an HTML prototype, then put colour, type, radius, spacing and motion tokens in `src/theme.rs` and restyle `src/ui/`. Replaces "switch the accent to Klein blue". Remove the glass and blur material for the owner's windows (`theme::MaterialTokens`).
10. **Move screens over one at a time:** main window, settings, editor, recording controls, tray menus. Blocked by item 9.
11. **Hand-drawn touches** from the welcome page in a few places (empty states, settings heading underline, mode doodles in the main window hover cards, a drawn tick when an export or recording finishes). Replace the Instant mode upload-cloud doodle with a local symbol. Decide together with item 9.

## P2: smaller gaps and leftovers

12. **Automations runner in the app.** Rules only run in the CLI (`apps/cli/src/automation.rs` is the model). Owner decision. Upstream tracks the same gap as CapSoftware/Cap#2959.
13. **Excluded windows:** add the 2.x note that remote-desktop windows cannot be excluded.
14. **Editor frame placeholder URL** is still `cap.so` (`editor_window/frame.rs`); use the brand domain or a neutral one.
15. **Recording settings popover** in the recording controls is an upstream TODO (`controls_window.rs`); check whether upstream ships it, otherwise build it.
16. **Delete `apps/desktop`** (roadmap phase 3), after moving the assets the GPUI app reads from it by relative path; drop `packages/ui-solid` and the Tauri scripts with it.
17. **Docs that still describe the old app or upstream:** `CLAUDE.md` and `AGENTS.md` describe the Tauri app as primary; `docs/saml-sso.md` is upstream cloud documentation; `docs/superpowers/` holds 2.x-era plans. Rewrite or delete them.

## Verify

18. **Phase 2 test checklist** was run once (one 5 s Studio clip). Still to check: Studio at 4K and 60 fps, no freeze around 2 s, frame pacing over a long recording, crash recovery of Studio recordings, pause/resume (the 2.2.5 fix).
19. **HEVC/AV1** with software encoders (SVT-AV1, libx265) and with AMD and Intel hardware; only NVIDIA NVENC was exercised.
20. **Recording start latency:** first frame about 1.9 s after the start cue (owner: follow upstream for now; check again before calling it a bug).
21. **Captions for Chinese.** Upstream reports Chinese speech transcribed as English gibberish (CapSoftware/Cap#2831) and a crash on trimmed recordings over about 6.5 minutes (CapSoftware/Cap#2719). Check both with the local models; Chinese users are the main audience.
22. **Fonts:** whether the MiSans variable font works with GPUI weight matching on Windows (saves about 7 MB), and loading MiSans only for the Chinese UI (saves RAM).

## Later

23. **Phase 4:** dependency upgrades, one group at a time, inspecting deliberate pins first (gpui rev, `vendor/*`, CapSoftware forks of cpal/rust-ffmpeg/nokhwa, `specta` rc).
24. **Phase 5:** benchmark against the top open-source recorders on quality, efficiency and UI/UX.
25. **Upstream issues worth taking when they land:** seek hover preview shows the wrong frame (CapSoftware/Cap#2936), 1.25x playback speed (CapSoftware/Cap#2842), editor scroll bar size (CapSoftware/Cap#2874), themed keycaps (CapSoftware/Cap#2861), floating screenshot preview (CapSoftware/Cap#2944), CLI recordings list missing custom folders (CapSoftware/Cap#2942; check our CLI against item 1).
