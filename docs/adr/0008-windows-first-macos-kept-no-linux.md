---
status: accepted
---

# Windows first, macOS kept, no Linux

FrameEver ships on Windows first and keeps its macOS code so a macOS release can follow; Linux is not supported. Windows is where the product is built, tested and released today, and nothing waits on macOS until Windows 1.0.0 is out and working. The owner has a Mac to build and test on, so macOS code paths, the macOS-only crates and their dependencies stay in the tree. Nobody can build or test Linux, so every Linux branch, module, crate, dependency, example and packaging script is removed.

This replaces the Windows-only scope written into the 2026-10-24 cleanup tickets (#3 to #5), the handoff and the README. The macOS-only crates removed by #3 come back from our own history, not from upstream, because they match the rest of our tree; later upstream macOS fixes are ported by hand as ADR-0007 describes.

## Considered Options

- **Windows only**: the smallest codebase, but a later macOS release would have to rebuild what is deleted now, while the owner can already test it.
- **Windows, macOS and Linux**: Linux code nobody can build or test only rots.
- **Restore macOS from upstream**: upstream is 900+ commits past our 0.6.1 base and its structure no longer matches ours.

## Consequences

- The macOS build is not compiling yet: the 2026-10-24 dependency upgrades only kept Windows building (for example, cap-rendering's Metal interop was not ported to wgpu 30). Making it build, run and package is its own work, done on the owner's Mac after Windows 1.0.0.
- macOS gets a CI build and packaging job when that work starts. Signing and notarization need an Apple Developer account; without one, users have to open the app through the Gatekeeper prompt.
- Code keeps `#[cfg(windows)]` / `#[cfg(target_os = "macos")]` branches; `cfg(unix)` now means macOS.
