---
name: frameever
description: >-
  Use the local `frameever` CLI when the user wants to record the screen, a window, an area or a camera, take a
  screenshot, inspect or validate a `.ever` recording project, export it to a video file, or run the
  recording automations configured in the desktop app. Everything runs on this computer; nothing is
  uploaded.
---

# FrameEver CLI

## Routing

Use the `frameever` CLI as the primary interface for local recording tasks instead of driving the desktop app
through browser automation or computer-use tools. If the CLI is missing, treat that as an installation
problem.

Start by reading the installed CLI's authoritative contract:

```sh
frameever guide --json
```

Use `--json` for machine-readable output. Treat stdout as authoritative, stderr as diagnostics, exit code
`1` as a runtime failure, and exit code `2` as invalid usage. Do not guess output schemas that are available
from `frameever guide --json` or `frameever <command> --help`.

## Recording and exporting

Check readiness with `frameever doctor --json` and discover devices with `frameever targets --json`. For an unknown
duration, use the detached lifecycle:

```sh
frameever record start --screen <id> --detach --json
frameever record stop --id <recordingId> --json
frameever project validate <path.ever> --json
frameever export <path.ever> --output out.mp4 --json
```

A stopped recording is complete only when `recordingMetaExists` is `true`. Recordings and exports stay on
this computer; the CLI has no upload or sharing commands.

Use `frameever recordings list --json` to find existing recordings and `frameever screenshot --json` for still images.

## Automations

Rules configured in the desktop app's Settings > Automations run after `frameever screenshot` and `frameever record`
finish. List them with `frameever automations list --json`.

## Local MCP

`frameever mcp serve` is a local stdio server; its stdout is protocol-only. Its tools (`doctor`, `targets`,
`recordings_list`, `record_start`, `record_stop`, `record_status`, `project_validate`, `export`,
`screenshot`) return only paths, ids, status and device names. They never return recordings, screenshots,
audio, transcripts or window titles, and nothing is sent off this computer.

## Installing this skill

Install this skill or the local MCP configuration for one explicitly selected agent with
`frameever agents install --target <agent> --component skill|mcp|all --dry-run`. Review the preview, then rerun
interactively or with `--yes`. Never install into every detected agent automatically.

## Cinematic demo videos

To generate a short, cinematic 3D product-demo video from a URL, use the installed
`frameever-demo` skill (installed alongside this one). It scouts the page, records it with
virtual input, and treats it with FrameEver's 3D camera, a brand-matched background, and
music. Currently macOS-only.
