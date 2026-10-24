---
status: accepted
---

# Product name: FrameEver · 帧刻, by aniBullet

The product ships as **FrameEver** (Chinese **帧刻**), published by aniBullet, with the tagline 一刻，凝成永恒 ("a moment, crystallised into forever"). Upstream's AGPL-3.0 licence lets us modify and redistribute the code but grants no rights to the "Cap" name or logo (AGPL §7(e)), so the fork must not ship under "Cap"; the earlier name "Cap - aniBullet Edition" is retired for the same reason. We keep upstream's copyright notice and licence, publish our source, and credit "based on Cap" in the README and About page.

## Considered Options

Rejected after web checks on 2026-10-21: Kairo (a band, a game, a film and a Japanese word already), Imago (an existing Windows screen recorder), AniBlast (an anime app), RecShot (close to "Record Shot", a popular iOS recorder), Vidi (close to Vidma Recorder), and the "amber" family (owner preference). "Everframe" was dropped because it is already a pet-portrait brand (shopeverframe.com) and "Everframes" a Canadian picture-frame company. FrameEver had no exact match; near names are generic ones such as Frame Recorder and FrameOS. It reads as "forever", echoing the tagline,. The Chinese name is not a translation: 帧刻 joins 帧 (frame) with 刻, which reads as "a moment" (the tagline), "recording" (刻录) and "engraving" (铭刻, permanence). Rejected Chinese names: 永帧 and 恒刻 (owner found them tacky), 须臾 (too common), 驻景 (homophone of 猪精). A formal trademark search (CNIPA, USPTO) for both names, and an app-store check for 帧刻, is still due before public release.

## Consequences

Brand strings, icons, window titles, installer names and the app-data identifier are centralised during phase 3 so a rename touches one place.

Applied on 2026-10-22, with the constants in `crates/utils/src/brand.rs` (shared by the GPUI app and the CLI):

- App identifier `io.github.anibullet.frameever`: the app-data folder, logs, update staging and the installer identity. It is only a unique ID in reverse-domain form; no website is involved. It differs from official Cap (`so.cap.desktop`) and from Cap-aniBullet 2.x (`so.cap.desktop.anibullet`), so the apps never share settings or recordings.
- Link scheme `frameever://`; `cap-desktop://` and `cap://` are no longer accepted.
- Executable `FrameEver.exe`, installer `FrameEver_<version>_x64-setup.exe` (per-user install, English and Simplified Chinese).
- CLI command `frameever` (installed to `~/.frameever/bin`); agent skills install as `frameever` and `frameever-demo`, and the MCP server is registered as `frameever`.
- First launch imports Cap-aniBullet 2.x once: the settings store is copied (excluded-window rules titled "Cap …" are renamed), the old recordings folder is added to the known recording folders without moving files, and old screenshots are copied.
- Logo: a record button above two timeline clips, chosen by the owner on 2026-10-22; it reads as "record" first and "edit" second. On 2026-10-24 the owner set the core brand colour to Klein blue `#1F3FD8`: the plate is Klein blue, the clips cream `#FFFDED` and sky blue `#87BFFF`, and the record button follows the iOS control (white ring, navy gap, red `#FF3B30` dot). The original `#111111` plate disappeared on dark taskbars. Palette, method and file list: `docs/brand/app-icon.md`. The wordmark is "FrameEver" in Geist Bold beside the icon.
- The `.cap` project extension stays, so projects remain compatible with upstream tooling. Superseded by ADR-0006: projects use `.ever`, and `.cap` projects come in through the converter.
- Rule for "Cap" names (2026-10-23): everything a user can see is FrameEver; internal names stay. Renamed: the bundled programs `frameever-cli.exe` and `frameever-muxer.exe` (the installer deletes the old `cap-cli.exe` and `cap-muxer.exe`), the log file `frameever.log`, every CLI message, help text and JSON name, the MCP server and skill names (`frameever`, `frameever-demo`), and environment variables (`FRAMEEVER_NO_MODIFY_PATH`; automations get `FRAMEEVER_PROJECT_PATH`/`_IMAGE_PATH`/`_OUTPUT_PATH`, with the `CAP_*` names still set so 2.x rules keep working). Kept: Rust crate and package names (`cap-*`, the `cap` CLI package), code identifiers and the `.cap` extension, so upstream merges stay cheap (ADR-0003). "Based on Cap" stays in the About area as the attribution AGPL requires. Revised 2026-10-24 by ADR-0007: internal names are renamed too (crates to `frameever-*`, environment variables to `FRAMEEVER_*`), and the `CAP_*` automation variables are no longer set; FrameEver Converter rewrites old rules.
