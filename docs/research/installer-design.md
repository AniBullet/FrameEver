# Installer design: how far, and with what technology

Research for issue #26 (map #23). Question: how far can the FrameEver installer be styled with the NSIS setup it has today, and how do the realistic alternatives compare on visual freedom, size, signing, silent install for the in-app updater, English and Simplified Chinese, and effort?

Researched 2026-10-24. Every external claim links to its primary source; repo claims name the file.

## What we ship today

- `pnpm fe package` (`scripts/fe.mjs`, `packageInstaller`) downloads Tauri's NSIS 3.11 toolset plus `nsis_tauri_utils.dll`, then runs `scripts/bundle-gpui.mjs --bundles nsis`, which drives the Tauri bundler. The current dev installer is about 146 MB; the app binary alone is about 111 MB (`dist/`, `target/.../FrameEver.exe`, local build of 2026-10-24).
- `apps/desktop-gpui/tauri.conf.json` sets `headerImage`, `sidebarImage`, `installerIcon`, `installMode: currentUser`, `languages: ["English", "SimpChinese"]`, `installerHooks`, `compression: lzma`. No custom `template`.
- `packaging/windows/installer-hooks.nsh` adds `SetFont /LANG=2052 "Microsoft YaHei UI" 9`, a 128 MB LZMA dictionary and pre/post-install cleanup.
- The updater verifies the download with minisign (`src/installer.rs`) and starts it through `ShellExecuteW` with `/P /R /UPDATE` (`src/updates.rs`, `launch_installer`). Any replacement must keep an equivalent "progress only, relaunch after, keep shortcuts" mode, or the updater changes with it.
- What `/P /R /UPDATE` mean comes from Tauri's template ([installer.nsi](https://github.com/tauri-apps/tauri/blob/dev/crates/tauri-bundler/src/bundle/windows/nsis/installer.nsi)): `/P` skips every page except the progress page (`SkipIfPassive`); `/R` relaunches the app from `.onInstSuccess` via `nsis_tauri_utils::RunAsUser`; `/UPDATE` skips the reinstall page and shortcut changes. The template uses Modern UI 2 pages (welcome, reinstall choice built with nsDialogs, directory, progress, finish) and already declares `ManifestDPIAware true` and `ManifestDPIAwareness PerMonitorV2`.

## How far NSIS goes

Three levels, each a superset of the previous one.

### Level 1: configuration only (what we do now)

Tauri exposes images, icon, languages, install mode, hooks and compression ([Tauri Windows installer docs](https://v2.tauri.app/distribute/windows-installer/)). Underneath, Modern UI 2 gives one welcome/finish bitmap (recommended 164x314), one header bitmap (150x57), `MUI_BGCOLOR`/`MUI_TEXTCOLOR` for header, welcome and finish, and progress-bar style (`smooth` or `colored`) ([MUI2 readme](https://nsis.sourceforge.io/Docs/Modern%20UI%202/Readme.html)). Tauri does not pass the colour settings through, so today the look is the classic wizard frame with our two bitmaps and system controls.

### Level 2: our own template (`bundle.windows.nsis.template`)

Tauri lets us replace the whole `.nsi` with our own handlebars file ([Tauri docs](https://v2.tauri.app/distribute/windows-installer/)). Starting from a copy of Tauri's template keeps the `/P /R /UPDATE` contract. What that unlocks:

- Every page can be an nsDialogs page: labels, inputs, checkboxes, radio buttons, progress bars, bitmaps (`NSD_SetBitmap`, `NSD_SetStretchedBitmap`), icons, timers (`NSD_CreateTimer`), silent AVI clips (`NSD_Anim_*`); each control is a Win32 HWND open to `SendMessage` and `SetCtlColors` ([nsDialogs readme](https://nsis.sourceforge.io/Docs/nsDialogs/Readme.html)).
- `ChangeUI` swaps the dialog resources themselves, so the frame layout (where the buttons, header and branding line sit) can be redrawn in a resource editor ([NSIS manual 4.8.1.8](https://nsis.sourceforge.io/Docs/Chapter4.html)).
- Per-language fonts with `SetFont /LANG` ([4.8.1.38](https://nsis.sourceforge.io/Docs/Chapter4.html)).
- The title bar can follow dark mode by calling `DwmSetWindowAttribute(DWMWA_USE_IMMERSIVE_DARK_MODE)` through the System plug-in; caption and border colours (`DWMWA_CAPTION_COLOR`, `DWMWA_BORDER_COLOR`) are settable too, all from Windows 11 build 22000 ([DWMWINDOWATTRIBUTE](https://learn.microsoft.com/en-us/windows/win32/api/dwmapi/ne-dwmapi-dwmwindowattribute)).

Hard limits at this level:

- Push buttons ignore colour messages and are always drawn in system colours; a custom button needs owner-draw ([WM_CTLCOLORBTN](https://learn.microsoft.com/en-us/windows/win32/controls/wm-ctlcolorbtn)), which plain nsDialogs does not provide. Next/Cancel stay Windows-grey, so a full dark theme is not reachable without a plug-in.
- Fonts must already be installed on the user's machine ([4.8.1.38](https://nsis.sourceforge.io/Docs/Chapter4.html)), so Inter and MiSans cannot be used; Segoe UI and Microsoft YaHei UI are the realistic choices.
- Animation means timers swapping bitmaps or an AVI clip; no smooth transitions between pages.
- DPI: the template is per-monitor aware, so text stays sharp, but bitmaps are single files scaled by the control (we ship them at 2x, `docs/brand/app-icon.md`); NSIS itself warns that some built-in bitmaps are not scaled ([4.8.1.29](https://nsis.sourceforge.io/Docs/Chapter4.html)).

Result: a clean, flat light installer with Klein-blue accents, our own layout and type in system fonts, and native-grey buttons. Close to the brand, not identical to the app.

### Level 3: a skin plug-in that draws everything

Some NSIS installers replace the Win32 dialogs with a DirectUI engine. The best-documented one, nsNiuniuSkin, describes itself as a DirectUI UI plug-in with XML-defined skins ([repo](https://github.com/leeqia/nsNiuniuSkin)). It gives full visual freedom, but the repository has no licence file (GitHub reports none), its last push was 2023-12-10, and it is a closed binary plug-in. Not usable for an open-source project without a licence; writing our own equivalent is option F below.

## Alternatives

### D. Inno Setup 6.6+ / 7

- Visual freedom: built-in light, dark and dynamic (follows Windows) modes for Setup and Uninstall, built-in styles (polar, slate, windows11, zircon) and custom VCL style files via `WizardStyleFile` with a separate dark variant (6.6.0, 2025-11-11); PNG wizard images (6.5.2); custom background colour and image with separate dark-mode variants and multiple files per DPI (6.7.0, 2026-01-06) ([Inno Setup 6 what's new](https://jrsoftware.org/files/is6-whatsnew.htm)). Inno Setup 7.1.0 is current (2026-08-12) and added another style, "stellar" ([Inno Setup 7 what's new](https://jrsoftware.org/files/is7-whatsnew.htm)). VCL styles skin buttons too, so this is the only off-the-shelf tool here that reaches a real dark theme.
- Same font limit as NSIS in practice (system fonts; custom fonts would need installing at runtime).
- Silent install: `/SILENT` (progress only), `/VERYSILENT`, `/SUPPRESSMSGBOXES`, `/CLOSEAPPLICATIONS`, `/RESTARTAPPLICATIONS`, `/LANG=`, `/DIR=` ([command line](https://jrsoftware.org/ishelp/topic_setupcmdline.htm)). Covers our updater's needs; relaunch would be our own `[Run]`/code step.
- Signing: `SignTool` signs Setup, and the uninstaller with `SignedUninstaller=yes` ([SignTool](https://jrsoftware.org/ishelp/topic_setup_signtool.htm)).
- Languages: Simplified Chinese is an official bundled translation ([translations](https://jrsoftware.org/files/istrans/)).
- Real use: Zed ships a per-user (`PrivilegesRequired=lowest`) Inno installer with English and Simplified Chinese ([zed.iss](https://github.com/zed-industries/zed/blob/main/crates/zed/resources/windows/zed.iss)); VS Code uses Inno with per-DPI wizard images (`inno-big-100.bmp` … `-250.bmp`) ([code.iss](https://github.com/microsoft/vscode/blob/main/build/win32/code.iss)).
- Licence: free; commercial users above USD 5,000 revenue are asked, not required, to buy a licence ([ordering](https://jrsoftware.org/isorder.php)). FrameEver is free and non-commercial.
- Cost for us: leaves the Tauri bundler (it only builds NSIS and WiX), so `fe package` needs its own Inno step; `updates.rs` switches to Inno switches; existing NSIS installs need a migration path (uninstall the NSIS entry or install over the same per-user folder).

### E. WiX v5/v6 Burn with a custom bootstrapper application

- Visual freedom: WixStdBA themes are XML-defined, five built-in themes, and a theme can be replaced entirely ([WixStdBA](https://docs.firegiant.com/wix/tools/burn/wixstdba/)); beyond that, a custom BA in native or .NET code draws anything ([Burn bundles](https://docs.firegiant.com/wix/tools/burn/)).
- Signing: detach the engine, sign it, reattach, sign the bundle ([signing](https://docs.firegiant.com/wix/tools/signing/)).
- Licence: from v6 the Open Source Maintenance Fee applies to organisations with over USD 10,000 revenue ([OSMF](https://docs.firegiant.com/wix/osmf/)).
- Cost: an MSI chain underneath a custom UI exe, two build systems, Tauri's MSI template needs VBScript on the build machine ([Tauri docs](https://v2.tauri.app/distribute/windows-installer/)). The freedom equals option F at a much higher cost. Not recommended.

### F. Our own front end drawn in GPUI

A small Rust setup executable that draws its window with GPUI, the same toolkit, tokens and fonts as the app (Inter and MiSans can be embedded, unlike in NSIS or Inno), with real animation. Two shapes:

- F1, GPUI front end over the NSIS engine: the front end collects choices and runs the existing NSIS installer with `/S` and NSIS's `/D=` directory switch ([NSIS manual 4.2.2 and 4.8.1.41](https://nsis.sourceforge.io/Docs/Chapter4.html)). Keeps registry, uninstaller and the updater path unchanged (the updater keeps calling the NSIS installer directly). NSIS reports no progress to a parent in silent mode, so progress is indeterminate unless a hook writes progress somewhere the front end reads.
- F2, GPUI installer that does the copying itself: full control and real progress, but we own uninstall, registry, file-in-use handling and upgrades, which NSIS and Inno already solve.
- Size: unmeasured. The payload is already ~146 MB, so a few extra MB matter little; a prototype should measure a minimal GPUI binary. GPUI needs a working GPU path on the target machine, which a plain Win32 installer does not.
- Signing: one more exe to sign, through the same `signCommand` Tauri uses ([Tauri Windows signing](https://v2.tauri.app/distribute/sign/windows/)).
- Effort: the highest of the realistic options; F1 is moderate, F2 large.

### G. Velopack (for completeness)

A one-click `Setup.exe` with no wizard; the only visual is a splash image (PNG, JPEG or animated GIF) via `--splashImage`, installs per-user to `%LocalAppData%` ([Velopack installer](https://docs.velopack.io/packaging/installer)), and it brings its own Rust update crate ([Velopack Rust](https://docs.velopack.io/getting-started/rust)). Calm and minimal, but it replaces both installer and updater, which is out of proportion to a visual question.

## Comparison

| | Visual freedom | Dark theme | Brand fonts | Silent for updater | zh-CN | Signing | Effort |
| --- | --- | --- | --- | --- | --- | --- | --- |
| A. NSIS, config (today) | Bitmaps only | No | No | Yes (`/P /R /UPDATE`) | Yes | Tauri `signCommand` | Done |
| B. NSIS, own template | Layout, colours, own pages; buttons stay system | Title bar and backgrounds only | No (system fonts) | Yes, unchanged | Yes | Same as A, uninstaller via `!uninstfinalize` in template | Low-moderate |
| C. NSIS + skin plug-in | Full | Yes | Possible | Yes | Yes | Same | Blocked (no licence) |
| D. Inno Setup 6.6+/7 | Styles, VCL skins, backgrounds | Yes, dynamic | No (system fonts) | Yes, new switches | Official | `SignTool` | Moderate (leave Tauri bundler, migrate installs) |
| E. WiX Burn custom BA | Full | Yes | Yes | Yes | Via .wxl | Detach/reattach | High |
| F1. GPUI front end + NSIS | Full, matches app | Yes | Yes | Yes, unchanged | Our i18n | One more exe | Moderate-high |
| G. Velopack | Splash only | n/a | n/a | Own model | Unclear | Yes | High (replaces updater) |

## Recommendation for the owner to decide

1. **B now, for 1.0**: a custom NSIS template copied from Tauri's, flat light pages with our own layout, Klein-blue accents, Segoe UI / Microsoft YaHei UI, 2x bitmaps, dark title bar where Windows supports it. Keeps the updater contract and the build untouched apart from one template file. Accepts: grey system buttons, no full dark theme, no Inter/MiSans.
2. **F1 later, if the installer must look like the app**: a GPUI front end on top of the same NSIS installer. It is the only path to identical tokens, fonts, dark theme and motion, and it leaves the updater alone. Prototype first and measure size and progress reporting.
3. **D instead of B if a skinned dark installer matters more than staying on the Tauri bundler**: Inno gives dark/dynamic mode and skinned buttons off the shelf, at the cost of a new packaging step, new updater switches and a migration for existing installs.

Not recommended: C (no licence), E (cost without extra freedom over F), G (replaces the updater).

## Open points

- Size and startup time of a minimal GPUI setup exe (decides F1).
- Whether the installer should be dark at all: it runs before the app's theme setting exists, so "follow Windows" is the only sensible rule for any dark variant.
