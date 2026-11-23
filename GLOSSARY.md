# FrameEver · 帧刻

A local-only Windows screen recorder and editor by aniBullet, forked from Cap (see ADR-0005). Everything it captures, edits and exports stays on the user's machine.

**FrameEver**:
The product name (Chinese: 帧刻). Tagline: 一刻，凝成永恒.
_Avoid_: Cap, Cap - aniBullet Edition

**aniBullet**:
The publisher behind FrameEver.
_Avoid_: using it as the product name

## Recording

**Studio recording**:
A recording captured as separate editable tracks (display, camera, audio, cursor) that opens in the editor before anything is exported.
_Avoid_: Studio mode file, raw recording

**Instant recording**:
A recording captured straight to a single playable video, without an editing step.
_Avoid_: Quick recording, fast mode

**Recording mode**:
The way a video is recorded: Studio or Instant, and nothing else.
_Avoid_: Capture mode, Screenshot mode, GIF mode

**Screenshot**:
A still image captured from a display, window or area; an action of its own beside recording, not a recording mode.
_Avoid_: Screenshot mode, snapshot, capture

**Studio Sound**:
FrameEver's voice clean-up for recorded audio: it lowers background noise and evens out loudness. Capitalised as a proper noun in English; 录音棚音质 in Chinese.
_Avoid_: noise reduction, audio enhancement

**Project**:
The on-disk folder holding one recording's media tracks, metadata and edit configuration.
_Avoid_: Recording folder, session

**Audio only recording**:
A recording of the microphone and system audio with no screen and no camera, started in Studio or Instant mode. Audio is its recording target, beside display, window, area and camera; it is not a recording mode. In either mode it is saved as a Studio project, which opens in the editor and exports as MP3 or WAV; once it is given visuals (Background › Visuals: bars, wave or circle over the background), it also exports as MP4 or GIF. 仅音频录制 in Chinese.
_Avoid_: Audio mode, voice memo, podcast mode

**Chapter mark**:
A named point in time the user drops while recording or adds in the editor, to find a moment again and split clips at it. 章节标记 in Chinese.
_Avoid_: Marker, bookmark, recording marker

## Editing

**Editor preset**:
A saved look for a whole project (background, cursor, camera, captions and the rest) that can be applied to any project or set as the default for new recordings. 编辑器预设 in Chinese.
_Avoid_: Preset (alone), project template, theme

**Style preset**:
A saved look for one item on the timeline, such as a text, mask, caption or shape, offered when the user adds or restyles that kind of item. 样式预设 in Chinese.
_Avoid_: Preset (alone), item template

**Filler word**:
A hesitation or verbal tic in speech, such as "um", "you know", 嗯 or 那个, that captions can hide and transcript clean-up can cut. 语气词 in Chinese.
_Avoid_: Filler, disfluency

## Input

**Input record**:
The key presses, mouse buttons, wheel steps and gamepad buttons, sticks and triggers saved with a Studio recording, each with its time. It is user content and stays in the project.
_Avoid_: Key log, input log, keystrokes

**Input display**:
A picture of a keyboard, mouse or gamepad drawn over the video whose keys and buttons light up as they were pressed. Off by default; 操作显示 in Chinese.
_Avoid_: Input overlay, key overlay, keyboard overlay

**Input preset**:
One look for the input display: a layout of keys and buttons with its artwork. FrameEver ships a few and the user can import more from a folder.
_Avoid_: Skin, theme, layout file

## Output

**Export**:
The video or GIF file rendered from a project, whole or from a chosen stretch; the file the user keeps and shares by hand. GIF is only ever an export format.
_Avoid_: Render, output file, GIF recording

**Enhance quality**:
A choice saved with the project that makes zoomed-in and scaled-down screen content sharper in exports and on the paused preview; playback and tracks exports stay plain. Compare shows the paused frame without it while held. 增强画质 in Chinese.
_Avoid_: AI upscale, super resolution

**Follow hover highlights**:
A cursor choice, on in new projects, that puts the smoothed cursor on the pointer where the recorded screen reacted under it (a button lit up, a menu row highlighted). The moments are found while the editor is open and idle and kept beside the screen video. 跟随悬停高亮 in Chinese.
_Avoid_: Hover anchors, hover snapping, magnetic cursor

**Tracks export**:
An export of a Studio recording as separate files, one per chosen track (screen, camera, microphone, system audio), each following the cuts and speed of the edit and without background or effects. 分轨导出 in Chinese.
_Avoid_: Stems, raw export, multitrack

**Result card**:
The small card that appears in a corner of the display after a recording or a screenshot, with a thumbnail and the next actions, when After a recording is set to Show card. 结果卡片 in Chinese.
_Avoid_: Recordings overlay, toast, popup

**Compressed copy**:
A smaller re-encoded version of an Instant recording kept beside the original, so the user can switch between or delete either one.
_Avoid_: Compressed version, compact file

**Library**:
The in-app list of the user's projects and screenshots.
_Avoid_: Gallery, recordings list

## Storage

**Storage folder**:
The one user-visible folder that holds everything FrameEver keeps: projects, screenshots and exports.
_Avoid_: Save path, base folder, recordings folder

**Exports folder**:
The part of the storage folder where exports land by default; the files there are the user's own to move and share.
_Avoid_: Output folder

**Converter**:
FrameEver Converter, the separate program that brings older data (Cap-aniBullet 2.x or official Cap settings, projects, loose videos, screenshots and rules) into the storage folder as FrameEver projects; the only FrameEver code that understands older layouts.
_Avoid_: 2.x import, migration, legacy scan

## Codebase lineage

**Upstream**:
The original CapSoftware/Cap repository FrameEver comes from; a read-only reference, no longer merged (see ADR-0007).
_Avoid_: Origin, original repo

**GPUI app**:
The native desktop app in `apps/desktop-gpui`, aniBullet's main app (see ADR-0001).
_Avoid_: New app, native app

**Cap-aniBullet 2.x**:
The previous release line of this product, built on the Tauri app, whose users upgrade to FrameEver.
_Avoid_: Old version, aniBullet edition

**Tauri app**:
The legacy webview desktop app, deleted from the repository; FrameEver ships only the GPUI app.
_Avoid_: Old app, desktop app
