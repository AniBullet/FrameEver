# Cap-aniBullet Contributor Guide

## Introduction

### What is Cap-aniBullet?

Cap-aniBullet is a local-first, open source screen recording application forked from [Cap](https://github.com/CapSoftware/Cap). It focuses on unlimited local recording, editing, and export without cloud dependencies.

### How can I contribute?

- [Report a bug](https://github.com/AniBullet/Cap-aniBullet/issues/new)
- [Start a discussion](https://github.com/AniBullet/Cap-aniBullet/discussions)
- Submit a PR

## Running Cap-aniBullet

### Development Requirements

- Node 20+
- Rust 1.88.0+
- pnpm 10+

### General Setup

Run `pnpm install`, then run the setup scripts in `scripts/` to install native dependencies (FFmpeg, LLVM, etc.).

On Windows, see `BUILD.md` for detailed instructions.

### `@cap/desktop` (desktop app)

To start development:

```bash
pnpm dev
```

Or use the PowerShell scripts:

```powershell
.\scripts\1-install.ps1   # Install dependencies
.\scripts\2-dev.ps1       # Start dev server
.\scripts\3-build.ps1     # Build production
```

When running on Windows, you may need to grant permissions (screen recording, microphone, etc.) depending on your Windows settings.

#### Where are my recordings stored?

You can find your recordings at `%APPDATA%/so.cap.desktop.anibullet/recordings` on Windows.
