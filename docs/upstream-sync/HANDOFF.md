# Handoff: FrameEver

The state of the work on one page. Keep it that short: when something here stops being true, replace it.

- What is left: the open issues on `AniBullet/FrameEver` (`docs/agents/issue-tracker.md`).
- What a ticket did: its closing comment (commits, per-commit verdicts for upstream ports, what was seen failing, measurements, what was not checked). Do not copy it here.
- Why something is the way it is: `docs/adr/`, `GLOSSARY.md`, `docs/design/README.md`.
- Everything before 2026-11-20: `handoff-until-2026-11-20.md` (the old handoff, by ticket) and `0.6.1.md` (the upstream merge log).

## Where things stand (2026-11-20)

- FrameEver 1.0.0 was released on 2026-11-19 (`v1.0.0`, Windows installer, FrameEver Converter, macOS disk image, `latest.json`). macOS is built and tested in CI only; nothing has run on a real Mac.
- `main` is the only branch that matters. Work is one ticket at a time in the main checkout.
- The open work is the upstream comparison #192 (`5a8927de5..beb537fe6`). #193 and #194 are done. The next comparison starts at `beb537fe6`; do not start it until these tickets are closed.

Suggested order, from the blocking links and from which tickets touch the same files:

1. Fixes: #207 to #210 are done. #195 and #196 are done; the three largest ports of #195 became #216 (clip media clean-up), #217 (frozen-instance recovery) and #218 (area ratio snapping), which can follow in any order.
2. #205, #206 and #202 are done. Studio switches camera to camera in place, on Windows too (`CameraFrameConformer`); the review findings left on #202 are listed in its closing comment.
3. #197 (design) is done; #198 to #203, #229 and #230 build against its pages (the closing comment of #197 lists which page holds what). #198 to #201, #221 and #222 are done. Audio-only recording was split from #202 into #229 (recording, MP3 and WAV export; upstream has neither Instant nor CLI for it, so both are ours to build) and #230 (editor and import), which #229 blocks. #203 is next. Drawing on screen while recording is already in the app (`screen_ink.rs`), so read #203 against the code as well.
4. #204 waits for the owner: whether the app gets the in-app automations runner. So does #220: whether chapter marks of a late-audio project move with the migration from #210.

## Waiting for the owner in the running app

Agents never screenshot the owner's screen, so nothing visible is confirmed until the owner has looked. The closing comments of #180 to #194 each end with a "Not checked" line, and the old handoff has 51 such notes going back to October. They have not been gathered into one list. What the owner finds becomes a new issue; it does not reopen the ticket.

## Things that cost time before

- `pnpm fe test --all` does not run the library tests of `frameever-recording`, `frameever-project` or `frameever-rendering`: run `pnpm fe test --crate <package>` for the ones a change touches. No `pnpm fe` command runs a root-workspace crate's `tests/` folder, so a ported integration test goes into the library as a `#[cfg(test)]` module.
- `pnpm fe fmt` and the format check in `pnpm fe lint` cover the GPUI workspace and six root crates only. They skip `frameever-rendering`, `frameever-editor`, `frameever-recording`, `frameever-project` and `frameever-verify`: after editing those, run `cargo fmt -p <package>` and restore the files you did not touch.
- `crates/verify/tests/recording.rs` runs with `FRAMEEVER_VERIFY_CAPTURE=off cargo test -p frameever-verify --test recording` (FFmpeg's `bin` on `PATH`); without that variable its `real_capture` test opens a window and records it.
- A single ticket never gets its own worktree: each one compiles the GPUI app from scratch (10 to 20 minutes). A worktree that is needed anyway wants the git-ignored setup copied from the main checkout first: `.cargo/config.toml`, `target/native-deps`, `target/ffmpeg` and `apps/desktop-gpui/assets/fonts/harmonyos-sans`.
- After the checkout moves or is renamed, a stale `ffmpeg-sys-next` build cache points at the old folder and linking fails with `avcodec.lib` missing: `cargo clean -p ffmpeg-sys-next` in both workspaces.
- Package from the main checkout. MSVC's linker and NSIS fail past 260 characters even with Windows long paths on.
- In a new clone, set the `upstream` remote and `gh repo set-default AniBullet/FrameEver` as `docs/agents/issue-tracker.md` says.
- gpui: a native modal dialog is opened from a spawned task's async body, never inside a listener, `update` or `render` (it panics with "RefCell already borrowed"); `window.request_animation_frame()` is called only while drawing.
- Upstream ports: read upstream's own commits (`git log --no-merges <merge>^1..<merge>`), not the merge diff. Try `git apply --3way` with the crate names rewritten first; when it conflicts in more than a few places, port by hand from the diff. Bring upstream's test over and watch it fail before the fix.
