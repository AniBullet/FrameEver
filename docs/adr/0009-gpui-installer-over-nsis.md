---
status: accepted
---

# The installer is a GPUI front end over an embedded NSIS installer

FrameEver's Windows installer is a small GPUI program that draws every window the user sees with the app's own tokens, fonts and motion, and runs an embedded NSIS installer silently underneath (`/S /D=<dir>`). The owner wants the installer to look like the app, and only a GPUI front end reaches that: NSIS and Inno Setup can't use Inter or HarmonyOS Sans SC, and NSIS buttons always draw in Windows grey. NSIS stays the engine because it already handles files, the registry, the uninstaller and files in use, and it is what the Tauri bundler builds today. Decided on the installer ticket (AniBullet/FrameEver#31); research in `docs/research/installer-design.md` on the `research/installer-design` branch.

## Considered Options

- **Our own NSIS template**: flat Klein-blue pages at low cost with the updater untouched, but system fonts, grey buttons and no full dark theme.
- **Inno Setup**: skinned buttons and dark mode off the shelf, but it leaves the Tauri bundler, needs new updater switches and a migration for existing NSIS installs, and still can't use the brand fonts.
- **GPUI installer that copies files itself**: full control, but we would own uninstall, registry, file-in-use handling and upgrades that NSIS already solves.
- **WiX Burn custom bootstrapper, Velopack, NSIS skin plug-ins**: no more freedom than a GPUI front end at higher cost, replace the updater, or have no licence.

## Consequences

- The one download, `FrameEver_<version>_x64-setup.exe`, is the front end with the NSIS installer inside it. The front end has three modes: install, update and uninstall. The in-app updater launches its update mode, a progress-only window that relaunches FrameEver, in place of calling NSIS with `/P /R /UPDATE`. The Windows uninstall entry points at its uninstall mode.
- The progress bar must show real progress, so NSIS has to report progress to the front end.
- GPUI needs a working GPU path. When the front end can't start, it runs the NSIS installer with its own UI so installing always works; that fallback UI keeps today's bitmaps and isn't designed further.
- The front end is one more executable to build and sign. Its size and startup time haven't been measured yet; they are measured during implementation, with no fixed budget.
