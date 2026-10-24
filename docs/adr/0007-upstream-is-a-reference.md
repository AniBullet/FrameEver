---
status: accepted
---

# Upstream is a reference, not a merge baseline

FrameEver is an independent product that shares Cap's architecture and origin. We no longer merge upstream on a schedule or keep our code shaped for cheap merges: internal names, folder structure, data formats and features are ours to change. When upstream ships a fix or feature worth having, we read it and port it into our own structure, judging first whether the problem exists for us. The `upstream` remote stays for reading only, and the AGPL attribution ("based on Cap") stays because the licence requires it.

Supersedes ADR-0003. Its rule that old fixes are re-judged rather than replayed carries over to upstream changes: nothing is taken wholesale.

## Considered Options

- **Keep merging upstream (ADR-0003)**: upstream fixes arrive almost for free, but every merge-friendly constraint (upstream names, upstream formats, compatibility with upstream tools) is weight carried forever, and upstream has already moved 933 commits past the 0.6.1 merge.

## Consequences

- Upstream bug fixes reach us only when someone ports them; `docs/upstream-sync/` keeps a watch list of upstream changes worth porting.
- Version numbers start over: the first FrameEver release is 1.0.0, not a continuation of Cap-aniBullet 2.x (whose updater was off, so no 2.x install is offered it).
- Rules that existed only to keep merges cheap (keeping `cap-*` names, upstream's project format, accepting upstream tools' output) no longer bind.
