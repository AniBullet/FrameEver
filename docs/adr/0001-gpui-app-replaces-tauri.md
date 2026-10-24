---
status: accepted
---

# GPUI app replaces the Tauri app

Upstream Cap ships `apps/desktop-gpui` (native GPUI, every pixel drawn in Rust, no webview) as its desktop app from 0.6.1; the Tauri app last shipped in 0.6.0 and survives upstream only as a dev harness. We adopt `apps/desktop-gpui` as aniBullet's main app and retire the Tauri app, because our goal is the best-performing local recorder on the newest architecture and staying on Tauri would mean maintaining a UI layer upstream has abandoned.

## Considered Options

- **Stay on Tauri + SolidJS**: larger ecosystem and our existing UI work is reusable, but upstream UI improvements stop flowing to us and the webview/IPC layer costs startup time, memory and preview throughput.
- **Keep both during migration**: rejected by the owner; no adaptation effort goes into the Tauri app.

## Consequences

- The Tauri app is frozen at its pre-merge state and may not compile against the new `crates/*`; it is deleted once the GPUI app reaches parity.
- Everything only the Tauri app has must be rebuilt in Rust: a bilingual UI (English and Simplified Chinese only — the GPUI app has no i18n, and the Tauri app's ja/ko locales are deliberately dropped), video compression, version management, the library delete flow.
- GPUI is pinned to a third-party fork (`wingleeio/zed` at a fixed rev, plus Cap's patches applied by `scripts/prepare-gpui-dependency.sh` into `../zed-cap`). Windows is upstream's secondary platform (roughly 3:1 macOS-to-Windows `cfg` branches), so Windows-specific gaps are ours to fix.
