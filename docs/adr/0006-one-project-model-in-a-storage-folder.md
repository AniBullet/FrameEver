---
status: accepted
---

# One project model in a visible storage folder; older data comes in through a separate converter

Everything FrameEver owns is a project, kept in one visible storage folder (default `Videos\FrameEver`, with `Recordings`, `Screenshots` and `Exports` subfolders whose names stay English in every UI language so paths, the CLI and automation rules do not change with the language). Projects use our own extension, `.ever`, and carry a FrameEver format version so the format can evolve. The app contains no code for older data: it does not import Cap-aniBullet 2.x settings, scan 2.x folders, detect 2.x installs or treat `.cap` folders as projects. Older data (2.x and official Cap settings, `.cap` projects, 2.x loose MP4 exports and Instant recordings with their `_compressed` copies, screenshots, automation rules) comes in through FrameEver Converter, a separate small program the user downloads from our Releases page and runs when they want; it moves everything into the storage folder as `.ever` projects, using the same shared storage rules as the app. All knowledge of old layouts lives in the converter. The library scans folders with no database, and an export's path is recorded in a FrameEver-owned file inside the project.

## Considered Options

- **Keep scanning 2.x folders and accept `.cap` everywhere**: no files move, but every listing, delete guard and file picker carries the old layouts forever. Rejected for the converter (see ADR-0007: no legacy weight).
- **Teach the library a second kind of item for loose 2.x videos**: doubles every library action (compress, switch version, delete). Rejected; the converter turns each into a project.
- **Keep `.cap`**: shows the upstream name in Explorer. Only about ten places create or check the extension, so the rename is cheap.
- **`.frameever`**: unambiguous but long. `.fe`, `.fev` and `.frm` are taken by other software; no use of `.ever` was found (web check, 2026-10-24).
- **Default export folder that the library scans** (2.x style): loses track of exports saved elsewhere. Rejected for recording the path per export; the save dialog opens in the exports folder and then remembers the last one.

## Consequences

- Until the user runs the converter, their old data stays where it was and the app does not show it; 2.x users learn about the converter from the 3.0 release notes, the README and the Releases page.
- Moves within a drive are instant renames; across drives the converter copies, verifies, then deletes the original.
- Supersedes the "`.cap` project extension stays" line of ADR-0005.
