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

**Project**:
The on-disk folder holding one recording's media tracks, metadata and edit configuration.
_Avoid_: Recording folder, session

## Output

**Export**:
The video file rendered from a project's edits; the file the user keeps and shares by hand.
_Avoid_: Render, output file

**Compressed copy**:
A smaller re-encoded version of a recording kept beside the original, so the user can switch between or delete either one.
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
The legacy webview desktop app in `apps/desktop`, frozen and due for retirement.
_Avoid_: Old app, desktop app
