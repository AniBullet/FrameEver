---
status: accepted
---

# The input record is user content, drawn after recording from presets in input-overlay's format

FrameEver can show what was pressed on the keyboard, mouse and gamepad over a recording, mainly so a game bug video shows how to reproduce it (AniBullet/FrameEver#66). A Studio recording saves an input record and the editor draws the input display from it at preview and export; nothing is drawn into a Studio recording while it is captured, so the display can be restyled, moved or switched off afterwards. Instant recording has no editor, so there the input display is drawn into the frames, only when the user switched it on, and no input record is kept.

The input record is user content under ADR-0002. It lives only in the project, the CLI and the AI-agent surfaces never return it, diagnostics carry only whether recording it is on, and the editor can clear it from a project without deleting the project. An export contains only what the input display drew. When the recording target is one window, the display shows only what was pressed while that window was in front.

Input is read by polling, on the thread that already polls the cursor, and only while a recording runs. The mouse wheel has no state to poll, so its steps come from Windows Raw Input, registered only while a recording runs and drained on the same polling tick. We do not install a global hook or a driver and never inject into another process: people recording games must not trip anti-cheat, and a recorder that hooks the keyboard reads as a keylogger.

Input presets use the format of the OBS plugin input-overlay (a PNG atlas plus a JSON list of elements), so its community presets and its preset editor work with FrameEver. We wrote our own reader and ship only presets we drew: input-overlay is GPL-2.0 and FrameEver is AGPL-3.0, and its bundled presets carry mixed credits. Presets are imported from a local folder, copied into the app data folder, parsed as data with size limits, and never downloaded.

## Considered options

- Drawing into Studio recordings during capture, as input-overlay does in OBS: rejected, because it cannot be undone or restyled and a recorded password would be in the video for good.
- A global keyboard hook for exact timing: rejected for the reasons above. Polling every 16 ms can miss a tap shorter than one poll.
- Our own preset format: rejected, because users would lose the existing presets and editor for no gain.
- Bundling input-overlay's presets: rejected on licence grounds.
