# CLAUDE.md

This file provides comprehensive guidance to Claude Code when working with code in this repository.

## Project Overview

Cap - aniBullet Edition is a Windows desktop screen recording application built with Tauri v2 (Rust + SolidStart). This is a local-first fork that removes all cloud dependencies from the original Cap project and focuses exclusively on Windows platform support.

### Product Context
- **Core Purpose**: Local screen recording with professional editing capabilities
- **Target Users**: Content creators, developers, educators who prioritize privacy
- **Key Features**: Unlimited recording, local storage, offline AI captions (Whisper.cpp), professional editing
- **Business Model**: Completely free and open source

## File Location Patterns & Key Directories

### Core Application
- `apps/desktop/` — Tauri v2 desktop app (recording, editing)
  - `src/` — SolidJS frontend code
  - `src-tauri/` — Rust backend code
  - `src-tauri/src/` — Core Rust modules

### Shared Packages
- `packages/ui-solid/` — SolidJS components for desktop
- `packages/utils/` — Shared utilities, types, constants

### Rust Crates
- `crates/media*/` — Video/audio processing pipeline
- `crates/recording/` — Core recording functionality
- `crates/rendering/` — Video rendering and effects
- `crates/camera*/` — Cross-platform camera handling
- `crates/scap-*/` — Screen capture implementations

### Important File Patterns
- `**/tauri.ts` — Auto-generated IPC bindings (DO NOT EDIT)
- `**/queries.ts` — Auto-generated query bindings (DO NOT EDIT)
- `apps/desktop/src-tauri/gen/` — Generated files (DO NOT EDIT)
- `*.config.*` — Configuration files (Tailwind, Vite, etc.)

## Key Commands

### Development
```bash
pnpm dev                 # Start desktop dev server
pnpm dev:desktop         # Start desktop dev server (alias)
pnpm build               # Build all packages via Turbo
pnpm lint                # Lint with Biome
pnpm format              # Format with Biome
pnpm typecheck           # TypeScript check
pnpm tauri:build         # Build desktop app (release)
```

### Desktop-Specific Commands
```bash
cd apps/desktop && pnpm dev              # Start SolidStart + Tauri dev
cd apps/desktop && pnpm build:tauri      # Build development version
cd apps/desktop && pnpm build:tauri --config src-tauri/tauri.prod.conf.json  # Build production version
```

## Development Environment Guidelines

### Server Management
- aniBullet is a pure desktop application with no server dependencies
- Run `pnpm dev` or `pnpm dev:desktop` to start the development server
- **Database**: SQLite (local file-based, no server required)
- **Storage**: Local filesystem (videos saved to user's local drive)

### Auto-generated Bindings (Desktop)
- **NEVER EDIT**: `tauri.ts`, `queries.ts` (auto-generated on app load)
- **NEVER EDIT**: Files under `apps/desktop/src-tauri/gen/`
- **Icons**: Auto-imported in desktop app; do not import manually
- **Regeneration**: These files update automatically when Rust types change

### Common Development Pain Points
- **Node Version**: Must use Node 20 (specified in package.json engines)
- **PNPM Version**: Locked to 10.5.2 for consistency
- **Rust Version**: Requires Rust 1.88+ for compilation
- **Turbo Cache**: May need clearing if builds behave unexpectedly (`rm -rf .turbo`)
- **Desktop Icons**: Use `unplugin-icons` auto-import instead of manual imports
- **FFmpeg**: Required for video processing (must be installed on system)

## Architecture Overview

### Monorepo Structure
- `apps/desktop` — Tauri v2 desktop app (primary application)
- `packages/ui-solid` — SolidJS UI components
- `packages/utils` — Shared utilities and types
- `crates/*` — Rust crates for media, rendering, recording, camera, etc.

### Technology Stack
- **Package Manager**: pnpm (`pnpm@10.5.2`)
- **Build System**: Turborepo
- **Desktop Framework**: Tauri v2 + Rust 1.88+
- **Frontend**: SolidJS + SolidStart
- **Styling**: Tailwind CSS
- **State Management**: `@tanstack/solid-query` for async state
- **Database**: SQLite (local, file-based)
- **AI Integration**: Whisper.cpp (local, offline transcription)
- **Video Processing**: FFmpeg (local)

### Critical Architectural Decisions
1. **Local-First**: All data stays on the user's machine - no cloud, no servers
2. **SQLite Database**: Lightweight local database for metadata and settings
3. **Desktop IPC**: Use `tauri_specta` for strongly typed commands/events; do not modify generated bindings
4. **Local AI**: Whisper.cpp for offline transcription - no API keys needed

#### Desktop event pattern
Rust (emit):
```rust
use specta::Type;
use tauri_specta::Event;

#[derive(Serialize, Type, tauri_specta::Event, Debug, Clone)]
pub struct UploadProgress {
    progress: f64,
    message: String,
}

UploadProgress { progress: 0.0, message: "Starting upload...".to_string() }
    .emit(&app)
    .ok();
```

Frontend (listen; generated bindings):
```ts
import { events } from "./tauri"; // auto-generated
await events.uploadProgress.listen((event) => {
  // update UI with event.payload
});
```

## Development Workflow & Best Practices

### Code Organization Principles
1. **Follow Local Patterns**: Study neighboring files and shared packages first
2. **Strict Typing**: Use existing types from Rust and TypeScript
3. **Component Consistency**: Use `@cap/ui-solid` for SolidJS components
4. **No Manual Edits**: Never touch auto-generated bindings (`tauri.ts`, `queries.ts`)

### Key Implementation Patterns

#### Desktop IPC Commands
```rust
// Rust side - emit events
UploadProgress { progress: 0.5, message: "Uploading...".to_string() }
  .emit(&app)
  .ok();
```

```typescript
// Frontend side - listen to events (auto-generated)
import { events, commands } from "./tauri";

// Call commands
await commands.startRecording({ ... });

// Listen to events
await events.uploadProgress.listen((event) => {
  setProgress(event.payload.progress);
});
```

#### SolidJS Query Patterns (Desktop)
```typescript
import { createQuery } from "@tanstack/solid-query";
import { commands } from "./tauri";

function VideoList() {
  const videos = createQuery(() => ({
    queryKey: ["videos"],
    queryFn: () => commands.getVideos(),
  }));

  return (
    <Show when={!videos.isLoading} fallback={<Skeleton />}>
      <For each={videos.data}>
        {(video) => <VideoCard video={video} />}
      </For>
    </Show>
  );
}
```

## Environment Variables

aniBullet is a pure desktop application with minimal configuration needs:

- **No API keys required** - all processing is local
- **No database connection strings** - SQLite is file-based
- **No cloud storage** - files saved to local filesystem
- Build configuration is in `tauri.conf.json` and `tauri.prod.conf.json`

## Testing & Build Optimization

### Testing Strategy
- **Desktop App**: Vitest for SolidJS components
- **Rust**: Standard Cargo test framework (`cargo test`)
- **Integration Tests**: Test recording, encoding, export workflows

### Build Performance
- **Turborepo Caching**: Aggressive caching across packages
- **Rust Incremental Builds**: Recompiles only changed modules
- **Development**: Fast refresh with Vite + Tauri dev mode
- **Release Builds**: Optimized with `--release` flag

### Performance Monitoring
- **Desktop Memory**: Rust handles video processing efficiently
- **File I/O**: Monitor local storage operations
- **FFmpeg Performance**: Track encoding/export times

## Troubleshooting Common Issues

### Build Failures
- **"Cannot find module"**: Check workspace dependencies in package.json
- **TypeScript errors**: Run `pnpm typecheck` to see project-wide issues
- **Turbo cache issues**: Clear with `rm -rf .turbo`
- **Node version mismatch**: Ensure Node 20 is active
- **Rust compile errors**: Check Cargo.toml dependencies, ensure Rust 1.88+

### Desktop App Issues
- **IPC binding errors**: Restart dev server to regenerate `tauri.ts`
- **Permission issues**: Windows may require screen recording permissions
- **Recording failures**: 
  - Verify screen capture permissions in system settings
  - Check microphone/camera permissions if using those features
- **FFmpeg errors**: Ensure FFmpeg is installed on system PATH
- **SQLite errors**: Check file permissions in user's app data directory

## SolidJS Coding Standards (Desktop App)

### Data Fetching & State Management
- Use `@tanstack/solid-query` for async state management
- Call Tauri commands through auto-generated bindings from `./tauri`
- Use `createSignal`, `createMemo`, `createResource` for local state

Basic pattern:
```tsx
import { createQuery } from "@tanstack/solid-query";
import { commands } from "./tauri";
import { Show, For } from "solid-js";

function VideoList() {
  const videos = createQuery(() => ({
    queryKey: ["videos"],
    queryFn: () => commands.getVideos(),
  }));

  return (
    <Show when={!videos.isLoading} fallback={<Skeleton />}>
      <For each={videos.data}>
        {(video) => <VideoCard video={video} />}
      </For>
    </Show>
  );
}
```

### UI/UX Guidelines
- Styling: Tailwind CSS only; stay consistent with existing design tokens
- Loading: Use `Show` with fallback for loading states
- Performance: SolidJS is fine-grained reactive; avoid unnecessary `createMemo`
- Use `@cap/ui-solid` components for consistency

## Desktop (SolidJS + Tauri) Patterns

### Data Fetching
- Use `@tanstack/solid-query` for async state management
- Never call Tauri commands directly in render; wrap them in queries/mutations

### IPC Communication
- Always use auto-generated `commands` and `events` from `./tauri`
- Listen to events with strongly-typed bindings
- Keep UI logic separate from IPC logic

### File System Operations
- All file operations go through Tauri commands (security)
- Use native file dialogs for user file selection
- Videos saved to user-chosen directories

### Permissions
- Screen recording, microphone, camera permissions handled by Tauri
- Check permissions before starting recording
- Guide users to system settings if permissions denied

## Conventions
- **CRITICAL: NO CODE COMMENTS**: Never add any form of comments to code. This includes:
  - Single-line comments: `//` (JavaScript/TypeScript/Rust), `#` (Python/Shell)
  - Multi-line comments: `/* */` (JavaScript/TypeScript), `/* */` (Rust)
  - Documentation comments: `///`, `//!` (Rust), `/** */` (JSDoc)
  - Any other comment syntax in any language
  - Code must be self-explanatory through naming, types, and structure. Use docs/READMEs for explanations when necessary.
- Directory naming: lower-case-dashed
- Components: PascalCase; hooks: camelCase starting with `use`
- Strict TypeScript; avoid `any`; leverage shared types
- Use Biome for linting/formatting; match existing formatting

## Rust Clippy Rules (Workspace Lints)
All Rust code must respect these workspace-level lints defined in `Cargo.toml`. Violating any of these will fail CI:

**Rust compiler lints:**
- `unused_must_use = "deny"` — Always handle `Result`/`Option` or types marked `#[must_use]`; never ignore them.

**Clippy lints (all denied — code MUST NOT contain these patterns):**
- `dbg_macro` — Never use `dbg!()` in code; use proper logging (`tracing::debug!`, etc.) instead.
- `let_underscore_future` — Never write `let _ = async_fn()` which silently drops futures; await or explicitly handle them.
- `unchecked_duration_subtraction` — Use `duration.saturating_sub(other)` instead of `duration - other` to avoid panics on underflow.
- `collapsible_if` — Merge nested `if` statements: write `if a && b { }` instead of `if a { if b { } }`.
- `clone_on_copy` — Don't call `.clone()` on `Copy` types (integers, bools, etc.); just copy them directly.
- `redundant_closure` — Use function references directly: `iter.map(foo)` instead of `iter.map(|x| foo(x))`.
- `ptr_arg` — Accept `&[T]` or `&str` instead of `&Vec<T>` or `&String` in function parameters for flexibility.
- `len_zero` — Use `.is_empty()` instead of `.len() == 0` or `.len() > 0` / `.len() != 0`.
- `let_unit_value` — Don't assign `()` to a variable: write `foo();` instead of `let _ = foo();` or `let x = foo();` when return is unit.
- `unnecessary_lazy_evaluations` — Use `.unwrap_or(val)` instead of `.unwrap_or_else(|| val)` when the default is a simple/cheap value.
- `needless_range_loop` — Use `for item in &collection` or `for (i, item) in collection.iter().enumerate()` instead of `for i in 0..collection.len()`.
- `manual_clamp` — Use `value.clamp(min, max)` instead of manual `if` chains or `.min(max).max(min)` patterns.

**Examples of violations to avoid:**

```rust
dbg!(value);
let _ = some_async_function();
let duration = duration_a - duration_b;
if condition {
    if other_condition {
        do_something();
    }
}
let x = 5.clone();
vec.iter().map(|x| process(x))
fn example(v: &Vec<i32>) { }
if vec.len() == 0 { }
let _ = returns_unit();
option.unwrap_or_else(|| 42)
for i in 0..vec.len() { println!("{}", vec[i]); }
value.min(max).max(min)
```

**Correct alternatives:**

```rust
tracing::debug!(?value);
some_async_function().await;
let duration = duration_a.saturating_sub(duration_b);
if condition && other_condition {
    do_something();
}
let x = 5;
vec.iter().map(process)
fn example(v: &[i32]) { }
if vec.is_empty() { }
returns_unit();
option.unwrap_or(42)
for item in &vec { println!("{}", item); }
value.clamp(min, max)
```

## Security & Privacy

### Data Handling
- **Video Storage**: Local filesystem only - no cloud uploads
- **Database**: SQLite local file - no remote connections
- **Privacy**: All data stays on user's machine
- **No Analytics**: No tracking, no telemetry

### Recording Permissions
- **Windows**: No special permissions required for screen recording
- **Microphone/Camera**: Standard Windows permissions when using these features

## AI & Processing Pipeline

### Local AI (Whisper.cpp)
- **Transcription**: Offline, local Whisper models
- **No API Keys**: All processing happens on-device
- **Privacy**: Audio never leaves the user's machine
- **Models**: Downloaded once, stored locally

### Media Processing Flow
```
Screen Capture → Local Memory Buffer → 
FFmpeg Encoding → Local File →
Optional: Whisper.cpp Transcription →
Local SQLite Database
```

## References & Documentation

### Core Technologies
- **Tauri v2**: https://github.com/tauri-apps/tauri
- **tauri_specta**: https://github.com/oscartbeaumont/tauri-specta
- **SolidJS**: https://solidjs.com/
- **TanStack Solid Query**: https://tanstack.com/query/latest
- **FFmpeg**: https://ffmpeg.org/
- **Whisper.cpp**: https://github.com/ggerganov/whisper.cpp

### aniBullet-Specific
- **README**: Development setup and build instructions
- **AGENTS.md**: Code style and conventions
- **Rust Crates**: See individual crate documentation in `crates/*/`

## Code Formatting

Always format code before completing work:
- **TypeScript/JavaScript**: Run `pnpm format` to format all code with Biome
- **Rust**: Run `cargo fmt` to format all Rust code with rustfmt

These commands should be run regularly during development and always at the end of a coding session to ensure consistent formatting across the codebase.
