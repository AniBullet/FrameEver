# 2.x feature audit (2026-10-24)

What Cap-aniBullet 2.x added on top of upstream, checked against the GPUI app. Source: the 73 fork-only commits on the old `main` (`2b543a008^1 --not 0da8f6d4d`) and the diff of the Tauri app from the fork point `78dd5c9f3`. Features that came from upstream syncs (keyboard overlay, captions track, gradient editor) are not listed; upstream's GPUI app has its own versions.

The rule: a 2.x feature does not have to be copied as it was, but it must exist in a form that fits the GPUI app, and a 2.x user's data and settings must carry over.

## P0: 2.x users' files are not found after upgrading

2.x stored everything under one base folder: `recordingsSavePath` if set, else `%USERPROFILE%\Videos\Cap Recordings`, else the app data folder.

| 2.x location | Holds | GPUI app |
|---|---|---|
| `<base>\recordings\*.cap` | Studio projects | Not scanned unless `<base>` is the app data folder |
| `<base>\exports\video\*.mp4` | Every exported video, and every Instant recording: 2.x moved `content/output.mp4` out of the project to `<name>.mp4` here | Not scanned; the GPUI library only lists `.cap` folders |
| `<base>\exports\video\*_compressed.mp4` | HEVC copies made by compression | Not recognised; GPUI keeps versions inside the project (`content/output_compressed.mp4`) |
| `<base>\exports\screenshot\` | Screenshots | Not scanned; the import only copies `<app data>\screenshots` |

`store::import_legacy_data_into` only adds `<legacy app data>\recordings` to `previousRecordingsPaths`. A 2.x user with default settings opens FrameEver to an empty library. Fix before the first release:
- Work out the 2.x base folder during import, then register its `recordings`, `exports\video` and `exports\screenshot` folders as places the library scans.
- List loose `.mp4` files from `exports\video` in the library, as 2.x did ("exported, no project"), pairing `<name>_compressed.mp4` as the compressed version.
- Map `recordingsSavePath` to `recordingsPath`, which means `<recordingsSavePath>\recordings`, because 2.x treated the setting as the base folder.

## Settings keys not carried over

| 2.x key | GPUI | Action |
|---|---|---|
| `recordingsSavePath` | `recordingsPath` (different meaning) | Map as above |
| `recordingQuality` (ultra/high/standard/low) | `studioRecordingQuality` | Map on import; check what Instant mode uses |
| `recordingCodec` (H.264/HEVC while recording) | None; ADR-0004 moved HEVC to export | Drop on purpose; no action |
| `openLibraryAfterRecording` | None | Port together with the library window |
| `language`, `autoCompressInstant`, `autoCompressDeleteOriginal`, `transcriptionHints`, `captureKeyboardEvents`, `excludedWindows` | Same keys | Done |
| `disableContentProtection` | None | 2.x already removed it; no action |

## Features

| 2.x feature | GPUI app | Action |
|---|---|---|
| Library window: sidebar filters by type and status with counts, search, grid/list/compact views, Ctrl/Shift multi-select, batch delete and compress, detail panel (metadata, original/compressed switch, three-way delete, copy file, open folder), context menu, refresh on focus and on new recording | Settings → Recordings/Screenshots lists and a 20-item panel in the main window | Rebuild as a GPUI window on the new UI system |
| Tray "Open library" | Tray has "View all recordings/screenshots" | Point it at the library window once it exists |
| Open the library after a recording finishes | Missing | Port with the library |
| Notifications when auto-compress finishes or fails | Not found in `recording.rs` auto-compress path | Check and add |
| Exports go to a known folder by default and show in the library | Export asks for a path each time; exported files are not tracked | Decide with the owner: a default export folder the library scans, or track exported paths in the project |
| Excluded windows: note that remote-desktop windows cannot be excluded | Missing | Add the one-line note to the Excluded Windows section |
| Recording quality presets ultra/high/standard/low | Studio quality enum | Check that the presets match in meaning |
| Screenshot hotkeys and tray items (display/window/area) | Present | Done |
| Compression, version switch, auto-compress Instant | Present (Settings → Recordings) | Done; move into the library detail panel later |
| Crates fixes from 2.2.x (pause/resume segments, 2 s freeze, gray frames, MediaFoundation for Studio) | Re-judged in `0.6.1.md` | Done |
