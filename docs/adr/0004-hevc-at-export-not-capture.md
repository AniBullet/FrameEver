---
status: accepted
---

# HEVC belongs to export, not capture

Before the 0.6.1 merge we recorded in HEVC to shrink files. We now capture in H.264 (upstream's hardware-first pipeline) and offer HEVC (and later AV1) as export codecs instead. A Studio recording is an intermediate that the editor decodes and scrubs repeatedly, and HEVC decode is heavier there; the size win matters for the file the user keeps, which is the export.
