---
status: accepted
---

# Upstream crates are the baseline; our old fixes are re-judged, not replayed

When merging upstream, `crates/*` (capture, encoding, muxing, rendering, editing, export) takes upstream's code wholesale. Our earlier local fixes are not replayed: for each one we identify the problem it solved, check whether upstream has since solved it, and re-implement only what is still missing — in whatever form is best now, not necessarily our old patch. Upstream reworked these pipelines heavily (e.g. hardware-first encoder selection, warmup-boundary anchoring), and grafting old patches onto rewritten code risks subtle timing and quality regressions.

The per-fix verdicts for each merge live in `docs/upstream-sync/`.
