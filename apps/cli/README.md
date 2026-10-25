# FrameEver CLI

FrameEver screen recording from the command line. The `frameever` command is built for scripts, local
automation and AI agents: every command can print JSON, and nothing it records leaves your computer.

## Install

In FrameEver: Settings → CLI → Install CLI. This puts a `frameever` command on your PATH that runs the
CLI bundled with the app (`frameever-cli.exe` next to `FrameEver.exe`). Open a new terminal afterwards.

## For agents

Install the FrameEver skill and the local MCP server for one agent:

```sh
frameever agents install --target <codex|claude|cursor> --component all --dry-run --json
frameever agents install --target <codex|claude|cursor> --component all --yes --json
```

The skill installs as `frameever` (plus the `frameever-demo` companion skill), and the MCP server is
registered as `frameever` running `frameever mcp serve`. Unrelated agent configuration is left alone.
Restart the agent afterwards. The full machine-readable contract is always `frameever guide --json`.

## Output conventions

- Pass `--json` for JSON on stdout; stderr stays human-readable.
- On failure the process exits non-zero, and in JSON mode the last object carries an `error` field.
- `record` and `export` stream newline-delimited JSON events.

## Environment variables

| Variable                   | Used by               | Notes                                                    |
| -------------------------- | --------------------- | -------------------------------------------------------- |
| `FRAMEEVER_NO_MODIFY_PATH` | `desktop install-cli` | Set to skip editing shell profiles / the user PATH.      |

Run Command automations receive `FRAMEEVER_PROJECT_PATH`, `FRAMEEVER_IMAGE_PATH` and
`FRAMEEVER_OUTPUT_PATH` when the trigger provides them. The older `CAP_*` names are no longer set;
FrameEver Converter rewrites rules from Cap-aniBullet 2.x.

## Typical workflow

```sh
frameever doctor --json                               # permissions and capture readiness
frameever targets --json                              # screens, windows, cameras, microphones
frameever record start --screen <id> --json --detach  # -> {"type":"started","recordingId","pid","path"}
frameever record stop --id <recordingId> --json       # finalize the .ever recording
frameever project validate <path.ever> --json          # confirm it is complete
frameever export <path.ever> --output out.mp4 --json   # H.264 by default; --codec hevc|av1
```

## Commands

- `record start` / `stop` / `status`: record in the foreground or with `--detach`, and manage sessions.
- `export`: render a `.ever` project to a video file.
- `screenshot`: capture a still of a screen or window.
- `targets`: list screens, windows, cameras and microphones.
- `project inspect` / `validate` / `config get|set`: inspect and edit `.ever` projects.
- `recordings list`: list `.ever` recordings in the FrameEver library.
- `update`: ask FrameEver to check GitHub Releases for a signed update and install it.
- `doctor` / `version` / `guide`: diagnostics, version, and the agent capability manifest.
- `automations list`: list the rules from Settings → Automations that the CLI runs.
- `desktop status|install-cli|uninstall-cli`: manage the `frameever` command on PATH.
- `mcp serve`: the local stdio MCP server; tools return only paths, ids, status and device names.
- `completions <shell>`: shell completion scripts.

## Automations

Rules made in Settings → Automations are shared with the CLI, which runs them after `screenshot` and
when a recording finishes. Save, export, reveal, open, run command, apply preset and delete run here;
clipboard, OCR, notification and open-editor actions need the desktop app and are skipped. Uploads and
webhooks were removed in FrameEver. See the active rules with `frameever automations list --json`.

Run `frameever --help` or `frameever <command> --help` for every flag.
