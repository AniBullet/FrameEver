---
status: accepted
---

# Local only: user content never leaves the machine

Recordings, screenshots, exports, transcripts, project metadata, file paths, window titles and logs must never be sent off the user's machine — this is the product's hard line, because FrameEver must be safe to use inside companies where a leak would be unacceptable. Network access is allowed only for requests that fetch and send nothing about the user's content: checking for and downloading app updates from our own GitHub Releases, fetching our release notes, and downloading local AI models (e.g. Whisper). Opening a URL in the user's browser for them to act on (e.g. a pre-filled bug report they review and submit themselves) is allowed. Where an upstream feature only needs the network to send content, we rewrite it to work locally (or against our own fetch-only endpoints) rather than delete it, and delete only what has no local value.

On every upstream merge we strip, rather than disable behind flags, every path that could send content or metadata out: accounts and sign-in, uploads and share links, subscriptions and licence keys, log upload, crash reporting and telemetry (Sentry, PostHog), and outbound webhooks. AI-agent surfaces (the local MCP server, the CLI's JSON output when driven by an agent, installed agent skills) are a leak path too, because whatever a tool returns reaches the agent's model provider. They may return only control metadata — file paths and names of the user's own recordings, ids, status, durations, device and target names (but not the titles of other open windows) — and never media, screenshots, audio, transcripts or other recording content, and they offer no tool that reads file contents or sends anything off the machine.

Diagnosability stays local and user-driven: logs are kept on disk and the app offers exporting them to a file or opening their folder, so the user can inspect them and decide whether to share them by hand; the app itself never transmits them.

Upstream's web, mobile, browser-extension, deployment and cloud-backend directories are never brought in.
