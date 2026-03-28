# 上游同步计划：Cap v0.4.5 → v0.4.81

> **For agentic workers:** REQUIRED: Use superpowers:subagent-driven-development (if subagents available) or superpowers:executing-plans to implement this plan. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 将上游 Cap v0.4.7 ~ v0.4.81 的性能优化、Bug 修复和新功能引入 aniBullet 本地版 fork，保持 Windows-only + 本地优先的设计原则。

**Architecture:** 采用分阶段 cherry-pick + 手动移植策略。先添加上游 remote，对比差异后按 4 个阶段逐步引入：Bug 修复 → 性能优化 → 新功能 → 润色。每阶段完成后构建验证。

**Tech Stack:** Rust 1.88+, Tauri v2, SolidJS, wgpu, MediaFoundation, FFmpeg, Whisper.cpp

**Base version:** aniBullet fork based on Cap v0.4.5 (upstream tag `cap-v0.4.5` or closest commit)

---

## 前置准备

### Task 0: 设置上游 Remote 并分析差异

**Files:**
- N/A (git operations only)

- [ ] **Step 1: 添加上游 remote**

```powershell
git remote add upstream https://github.com/CapSoftware/Cap.git
git fetch upstream --tags
```

- [ ] **Step 2: 确认 fork 基点**

```powershell
git log --oneline --all | Select-String "v0.4.5"
git log --oneline upstream/main --since="2026-01-08" --until="2026-03-28" -- crates/ apps/desktop/src-tauri/ apps/desktop/src/routes/editor/
```

- [ ] **Step 3: 创建同步工作分支**

```powershell
git checkout -b sync/upstream-v0481
```

- [ ] **Step 4: 生成差异报告**

```powershell
git diff HEAD...upstream/main --stat -- crates/ apps/desktop/src-tauri/src/ apps/desktop/src/routes/editor/ > docs/superpowers/plans/diff-stat.txt
```

---

## Phase 1: Bug 修复（低风险，高确定性）

### Task 1.1: 多段录制恢复数据丢失修复

**Source:** [PR #1514](https://github.com/CapSoftware/Cap/pull/1514)
**Files:**
- Modify: `crates/recording/src/recovery.rs`

- [ ] **Step 1: 查看上游 PR 变更**

```powershell
git log upstream/main --oneline --all -- crates/recording/src/recovery.rs | Select-Object -First 10
git diff cap-v0.4.5..upstream/main -- crates/recording/src/recovery.rs
```

- [ ] **Step 2: Cherry-pick 或手动移植变更**

关键变更：多段 M4S 恢复时，segment manifest 解析需正确处理所有片段，避免跳过中间片段导致数据丢失。

- [ ] **Step 3: 编译验证**

```powershell
cargo build -p cap-recording
cargo test -p cap-recording
```

- [ ] **Step 4: Commit**

```powershell
git add crates/recording/src/recovery.rs
git commit -m "fix: multi-segment recording recovery data loss (upstream #1514)"
```

---

### Task 1.2: 音频声道 panic 修复

**Source:** [PR #1536](https://github.com/CapSoftware/Cap/pull/1536)
**Files:**
- Modify: `crates/audio/src/audio_data.rs` (or wherever `AudioInfo::channel_layout()` is defined)

- [ ] **Step 1: 定位变更**

```powershell
git diff cap-v0.4.5..upstream/main -- crates/audio/
```

- [ ] **Step 2: 移植修复**

修复 `channel_layout()` 对不支持声道数的处理，返回合理的默认值而非 panic。

- [ ] **Step 3: 编译验证**

```powershell
cargo build -p cap-audio
cargo test -p cap-audio
```

- [ ] **Step 4: Commit**

```powershell
git add crates/audio/
git commit -m "fix: prevent panic in AudioInfo::channel_layout for unsupported channels (upstream #1536)"
```

---

### Task 1.3: 预热叠加层阻塞点击修复

**Source:** [PR #1523](https://github.com/CapSoftware/Cap/pull/1523)
**Files:**
- Modify: 相关 overlay/window 组件文件

- [ ] **Step 1: 查看变更范围**

```powershell
git log upstream/main --oneline -- "*prewarm*" "*overlay*" | Select-Object -First 10
```

- [ ] **Step 2: 移植修复**

预热的 overlay 窗口需设置 `cursor_events: false`（ignore pointer events），避免阻塞用户点击。

- [ ] **Step 3: 编译验证并 Commit**

---

### Task 1.4: Windows 编码器编译错误修复

**Source:** [PR #1578](https://github.com/CapSoftware/Cap/pull/1578)
**Files:**
- Modify: `crates/enc-mediafoundation/src/` (closure 中 `continue` 的编译错误)

- [ ] **Step 1: 对比差异**

```powershell
git diff cap-v0.4.5..upstream/main -- crates/enc-mediafoundation/
```

- [ ] **Step 2: 修复并验证**

```powershell
cargo build -p cap-enc-mediafoundation
```

- [ ] **Step 3: Commit**

```powershell
git commit -m "fix: continue-in-closure compile errors in Windows encoder (upstream #1578)"
```

---

### Task 1.5: 裁剪/边距渲染修复

**Source:** v0.4.7 + v0.4.81 crop/padding fixes
**Files:**
- Modify: `crates/rendering/src/` (composite_frame.rs, scene.rs 等)

- [ ] **Step 1: 对比渲染模块差异**

```powershell
git diff cap-v0.4.5..upstream/main -- crates/rendering/src/composite_frame.rs crates/rendering/src/scene.rs
```

- [ ] **Step 2: 移植裁剪区域对 padding/background 的正确处理**

- [ ] **Step 3: 编译并视觉验证**

```powershell
cargo build -p cap-rendering
```

- [ ] **Step 4: Commit**

```powershell
git commit -m "fix: crop and padding rendering respect active crop region"
```

---

### Task 1.6: GPU 上下文清理 + 退出守卫

**Source:** [PR #1661](https://github.com/CapSoftware/Cap/pull/1661)
**Files:**
- Modify: `apps/desktop/src-tauri/src/` (main.rs, lib.rs 等生命周期处理)
- Modify: `crates/rendering/src/` (GPU 资源清理)

- [ ] **Step 1: 对比 shutdown/cleanup 相关变更**

```powershell
git diff cap-v0.4.5..upstream/main -- apps/desktop/src-tauri/src/lib.rs apps/desktop/src-tauri/src/main.rs
```

- [ ] **Step 2: 移植 graceful shutdown 逻辑**

关键：应用退出时完整释放 GPU 资源、WebSocket 连接、相机进程。

- [ ] **Step 3: 编译验证**

```powershell
cargo build -p cap-desktop
```

- [ ] **Step 4: Commit**

```powershell
git commit -m "fix: graceful app exit with GPU context cleanup and exit guards (upstream #1661)"
```

---

### Phase 1 验证检查点

- [ ] **全量编译**

```powershell
cargo build -p cap-desktop
```

- [ ] **运行应用，验证基本功能正常**

```powershell
pnpm dev
```

- [ ] **Phase 1 完成 Commit**

```powershell
git commit --allow-empty -m "chore: phase 1 complete - bug fixes from upstream v0.4.7-v0.4.81"
```

---

## Phase 2: Windows 性能优化（中风险，体感提升最大）

### Task 2.1: MediaFoundation 解码器升级 — 零拷贝 + 批处理

**Source:** [PR #1613](https://github.com/CapSoftware/Cap/pull/1613), [#1614](https://github.com/CapSoftware/Cap/pull/1614), [#1616](https://github.com/CapSoftware/Cap/pull/1616)
**Files:**
- Modify: `crates/video-decode/src/media_foundation.rs`
- Modify: `crates/rendering/src/decoder/media_foundation.rs`
- Modify: `crates/rendering/src/decoder/mod.rs`

- [ ] **Step 1: 对比 video-decode 模块完整差异**

```powershell
git diff cap-v0.4.5..upstream/main -- crates/video-decode/src/media_foundation.rs
```

- [ ] **Step 2: 移植关键改进**

核心变更:
- MF 解码器批量帧请求（减少逐帧 syscall 开销）
- 零拷贝帧传递：D3D11 纹理直接传给 wgpu，不走 CPU 回读
- DX12 适配器优先选择
- FFmpeg 自动回退（当硬件解码不可用时）

- [ ] **Step 3: 对比 rendering decoder 适配**

```powershell
git diff cap-v0.4.5..upstream/main -- crates/rendering/src/decoder/
```

- [ ] **Step 4: 编译验证**

```powershell
cargo build -p cap-video-decode
cargo build -p cap-rendering
```

- [ ] **Step 5: 运行应用测试播放性能**

- [ ] **Step 6: Commit**

```powershell
git commit -m "perf: MediaFoundation decoder zero-copy batching and DX12 preference (upstream #1613/#1616)"
```

---

### Task 2.2: GPU 颜色转换优化

**Source:** v0.4.7 GPU color conversion
**Files:**
- Modify: `crates/rendering/src/yuv_converter.rs`
- Modify: `crates/rendering/src/cpu_yuv.rs`
- Possibly modify: `crates/rendering/src/shaders/nv12_to_rgba.wgsl`

- [ ] **Step 1: 对比 YUV 转换模块**

```powershell
git diff cap-v0.4.5..upstream/main -- crates/rendering/src/yuv_converter.rs crates/rendering/src/cpu_yuv.rs crates/rendering/src/shaders/
```

- [ ] **Step 2: 移植 GPU 端颜色转换优化**

核心：颜色转换从 CPU 移至 GPU compute shader，减少 62% 数据传输。

- [ ] **Step 3: 编译 + 视觉验证**

```powershell
cargo build -p cap-rendering
```

- [ ] **Step 4: Commit**

```powershell
git commit -m "perf: GPU-accelerated color conversion reducing 62% data transfer"
```

---

### Task 2.3: 导出管线重构

**Source:** [PR #1601](https://github.com/CapSoftware/Cap/pull/1601), [#1675](https://github.com/CapSoftware/Cap/pull/1675)
**Files:**
- Modify: `crates/export/src/mp4.rs`
- Modify: `crates/export/src/lib.rs`
- Modify: `crates/rendering/src/` (渲染-解码流水线)
- Modify: `apps/desktop/src-tauri/src/export.rs`

- [ ] **Step 1: 对比导出模块完整差异**

```powershell
git diff cap-v0.4.5..upstream/main -- crates/export/ apps/desktop/src-tauri/src/export.rs
```

- [ ] **Step 2: 移植关键优化**

核心变更:
- 光标轨迹预计算（避免逐帧计算光标位置）
- 编码器参数调优（bitrate/quality 平衡）
- 解码-渲染流水线化（decode 和 render 不再串行等待）
- GPU NV12 编码路径（可用时使用，自动 CPU 回退）
- 移除不必要的导出前延迟
- 导出估算基于真实数据校准

- [ ] **Step 3: 编译 + 导出测试**

```powershell
cargo build -p cap-export
```

- [ ] **Step 4: Commit**

```powershell
git commit -m "perf: export pipeline restructure with cursor precomputation and encoder tuning (upstream #1601/#1675)"
```

---

### Task 2.4: 编辑器播放优化

**Source:** [PR #1598](https://github.com/CapSoftware/Cap/pull/1598), v0.4.81 playback improvements
**Files:**
- Modify: `crates/rendering/src/frame_pipeline.rs`
- Modify: `crates/rendering/src/decoder/mod.rs`
- Modify: `apps/desktop/src/routes/editor/Player.tsx`

- [ ] **Step 1: 对比帧管线差异**

```powershell
git diff cap-v0.4.5..upstream/main -- crates/rendering/src/frame_pipeline.rs
```

- [ ] **Step 2: 移植播放优化**

核心变更:
- 帧缓存查找优化（O(1) lookup）
- 自适应预读（根据播放速度动态调整预取量）
- 分辨率感知预取（预览质量与预取策略联动）
- 逐帧开销降低

- [ ] **Step 3: 对比前端播放器变更**

```powershell
git diff cap-v0.4.5..upstream/main -- apps/desktop/src/routes/editor/Player.tsx
```

- [ ] **Step 4: 编译 + 播放测试**

- [ ] **Step 5: Commit**

```powershell
git commit -m "perf: editor playback with adaptive readahead and resolution-aware prefetch (upstream #1598)"
```

---

### Task 2.5: 音视频同步增强

**Source:** [PR #1674](https://github.com/CapSoftware/Cap/pull/1674), v0.4.81 sync improvements
**Files:**
- Modify: `crates/audio/src/sync_analysis.rs`
- Modify: `crates/audio/src/latency.rs`
- Modify: `crates/recording/src/sync_calibration.rs` (if exists)
- Modify: `crates/recording/src/output_pipeline/` (时间戳处理)

- [ ] **Step 1: 对比音频同步模块**

```powershell
git diff cap-v0.4.5..upstream/main -- crates/audio/src/ crates/recording/src/sync_calibration*
```

- [ ] **Step 2: 移植同步改进**

核心变更:
- 音频漂移限制（音频不能跑在视频前面太多）
- 时间戳偏移延迟处理（等视频就绪后再应用偏移）
- 系统音频起始时间对齐

- [ ] **Step 3: 编译验证**

```powershell
cargo build -p cap-audio
cargo build -p cap-recording
```

- [ ] **Step 4: Commit**

```powershell
git commit -m "perf: tighter audio-video sync with drift limiting and timestamp alignment"
```

---

### Phase 2 验证检查点

- [ ] **全量编译**

```powershell
cargo build -p cap-desktop
pnpm build
```

- [ ] **功能验证清单**
  - 编辑器播放长录制（>5 分钟）流畅度
  - 导出速度对比（录制 1 分钟视频，对比导出时间）
  - 音视频同步（录制含音频内容，播放验证唇形同步）
  - 退出应用无崩溃

- [ ] **Phase 2 完成 Commit**

```powershell
git commit --allow-empty -m "chore: phase 2 complete - Windows performance optimizations"
```

---

## Phase 3: 新功能引入（中高风险，功能差异化）

### Task 3.1: 键盘按键叠加 + 字幕时间线轨道

**Source:** [PR #1615](https://github.com/CapSoftware/Cap/pull/1615) — 这是一个大型 PR，包含两个核心功能
**Files (估计):**
- Create/Modify: `crates/recording/src/` (键盘捕获模块)
- Create/Modify: `apps/desktop/src/routes/editor/Timeline/` (新轨道类型: keyboard, captions)
- Modify: `apps/desktop/src/routes/editor/context.ts` (TimelineTrackType 扩展)
- Create/Modify: `crates/rendering/src/layers/` (键盘叠加渲染层)
- Modify: `apps/desktop/src-tauri/src/` (Tauri 命令接口)
- Modify: `crates/rendering/src/` (字幕渲染适配)

- [ ] **Step 1: 深入分析 PR #1615 完整变更**

```powershell
# 在浏览器中查看: https://github.com/CapSoftware/Cap/pull/1615/files
# 或者
git log upstream/main --oneline --all -- "*keyboard*" "*caption*track*"
git diff cap-v0.4.5..upstream/main -- apps/desktop/src/routes/editor/Timeline/ apps/desktop/src/routes/editor/context*
```

- [ ] **Step 2: 键盘捕获 — Rust 层**

需要实现:
- Windows 全局键盘钩子 (Win32 `SetWindowsHookExW` + `WH_KEYBOARD_LL`)
- 录制期间捕获按键事件，附带时间戳
- 按键数据序列化为时间线可用的片段格式

- [ ] **Step 3: 键盘叠加 — 渲染层**

需要实现:
- `crates/rendering/src/layers/keyboard.rs` 键盘叠加渲染
- 按键事件 → 时间线片段的映射逻辑

- [ ] **Step 4: TimelineTrackType 扩展**

在 `apps/desktop/src/routes/editor/context.ts` 中添加:
- `keyboard` 轨道类型
- `captions` 轨道类型

- [ ] **Step 5: 时间线 UI 组件**

在 `apps/desktop/src/routes/editor/Timeline/` 中添加:
- `KeyboardTrack.tsx` 键盘轨道组件
- `CaptionsTrack.tsx` 字幕轨道组件
- 更新 `index.tsx` 注册新轨道

- [ ] **Step 6: 字幕时间线集成**

将现有的 CaptionsTab 侧边栏编辑与时间线轨道打通:
- 字幕片段可拖拽/裁剪/分割
- 逐段样式覆盖
- 与渲染层同步

- [ ] **Step 7: Tauri 命令接口**

确保 `commands` 和 `events` 正确暴露键盘捕获和字幕轨道 API。

- [ ] **Step 8: 全量编译 + 功能测试**

```powershell
cargo build -p cap-desktop
pnpm dev
```

测试清单:
- 录制时按键是否被捕获
- 编辑器中是否显示键盘轨道
- 字幕是否在时间线中可编辑
- 导出视频是否包含键盘叠加

- [ ] **Step 9: i18n 更新**

更新 `apps/desktop/src/i18n/locales/` 所有语言文件，添加键盘和字幕轨道相关翻译。

- [ ] **Step 10: Commit**

```powershell
git commit -m "feat: keyboard press overlay and caption timeline tracks (upstream #1615)"
```

---

### Task 3.2: GPU 管线预热

**Source:** v0.4.7 prewarmed GPU pipeline
**Files:**
- Modify: `crates/rendering/src/lib.rs` 或 `frame_pipeline.rs`
- Modify: `apps/desktop/src-tauri/src/lib.rs` (启动时触发预热)

- [ ] **Step 1: 对比上游预热实现**

```powershell
git diff cap-v0.4.5..upstream/main -- crates/rendering/src/ | rg -i "prewarm|preheat|warmup|warm_up"
```

- [ ] **Step 2: 移植预热逻辑**

在应用启动时后台初始化 wgpu 设备、shader 编译、管线布局，避免打开编辑器时的冷启动延迟。

- [ ] **Step 3: 编译 + 编辑器打开速度测试**

- [ ] **Step 4: Commit**

```powershell
git commit -m "feat: prewarm GPU pipeline at startup for instant editor readiness"
```

---

### Phase 3 验证检查点

- [ ] **全量编译**

```powershell
cargo build -p cap-desktop
pnpm build
```

- [ ] **功能验证清单**
  - 录制时按键捕获正常工作
  - 编辑器中键盘轨道显示、可编辑
  - 字幕时间线轨道拖拽/分割正常
  - 导出包含键盘叠加效果
  - 编辑器打开速度对比

- [ ] **Phase 3 完成 Commit**

```powershell
git commit --allow-empty -m "chore: phase 3 complete - new features (keyboard overlay, caption tracks, GPU prewarm)"
```

---

## Phase 4: 润色与集成（低风险）

### Task 4.1: 编辑器多轨道 + 渐变噪点 + 权限修复

**Source:** [PR #1669](https://github.com/CapSoftware/Cap/pull/1669)
**Files:**
- Modify: `apps/desktop/src/routes/editor/` (多处)
- Modify: `crates/rendering/src/` (渐变噪点效果)

- [ ] **Step 1: 评估 PR #1669 中适用的变更**

此 PR 较大（15 个文件），选择性引入与本地录制相关的部分。

- [ ] **Step 2: 移植并 Commit**

---

### Task 4.2: 渲染引擎微调 + 录制韧性

**Source:** [PR #1674](https://github.com/CapSoftware/Cap/pull/1674)
**Files:**
- Modify: `crates/rendering/src/`
- Modify: `crates/recording/src/`

- [ ] **Step 1: 选择性移植渲染和录制韧性改进**

排除 analytics tracking 部分（云端功能）。

- [ ] **Step 2: 编译 + Commit**

---

### Task 4.3: 光标和缩放润色 + 预取修复

**Source:** [PR #1686](https://github.com/CapSoftware/Cap/pull/1686)
**Files:**
- Modify: 编辑器光标渲染、缩放行为、预取逻辑

- [ ] **Step 1: 移植光标/缩放/预取相关改进**

- [ ] **Step 2: 编译 + Commit**

---

### Task 4.4: GIF 导出帧处理优化

**Source:** [PR #1682](https://github.com/CapSoftware/Cap/pull/1682)
**Files:**
- Modify: `crates/export/src/gif.rs`

- [ ] **Step 1: 对比 GIF 导出变更**

- [ ] **Step 2: 移植优化并 Commit**

---

### Task 4.5: 代码格式化 + 最终验证

- [ ] **Step 1: Rust 格式化**

```powershell
cargo fmt
```

- [ ] **Step 2: TypeScript 格式化**

```powershell
pnpm format
```

- [ ] **Step 3: Clippy 检查**

```powershell
cargo clippy -p cap-desktop -- -D warnings
```

- [ ] **Step 4: TypeScript 类型检查**

```powershell
pnpm typecheck
```

- [ ] **Step 5: 全量构建**

```powershell
pnpm tauri:build
```

- [ ] **Step 6: 最终 Commit**

```powershell
git commit -m "chore: code formatting and final validation after upstream sync"
```

---

## 排除清单（不引入的上游变更）

| PR/功能 | 排除原因 |
|---------|---------|
| [#1640](https://github.com/CapSoftware/Cap/pull/1640) Pro 座位管理 | 云端订阅 |
| [#1639](https://github.com/CapSoftware/Cap/pull/1639) 开发者平台/SDK | 云端 API |
| [#1643](https://github.com/CapSoftware/Cap/pull/1643) 匿名观看通知 | 云端分享 |
| [#1383](https://github.com/CapSoftware/Cap/pull/1383) Cap Analytics | 遥测/分析 |
| [#1681](https://github.com/CapSoftware/Cap/pull/1681) 新引导流程 | 需完全重做适配本地化 UX，可单独评估 |
| 所有 `apps/web/` 变更 | Web 应用不在范围内 |
| 所有 macOS-only 修复 | Windows-only fork |
| 所有上传/分享相关变更 | 本地优先原则 |

---

## 执行进度（更新于 2026-03-28）

### 已完成（分支 sync/upstream-v0481，3 个 commits，54 文件，+9892/-2202 行）

| 模块 | 状态 | 策略 | 包含功能 |
|------|------|------|---------|
| `crates/project/` | ✅ 完成 | 取上游 + 重应用 fork 改动 | 键盘类型、字幕段落、渐变噪点、动画速度 |
| `crates/rendering/` | ✅ 完成 | 直接取上游（fork 未修改） | GPU 颜色转换、MF 零拷贝、键盘叠加层、NV12 shader、编辑器播放优化 |
| `crates/video-decode/` | ✅ 完成 | 直接取上游 | MF 解码器改进 |
| `crates/gpu-converters/` | ✅ 完成 | 直接取上游 | GPU 转换优化 |
| `crates/editor/` | ✅ 完成 | 取上游 + 重应用安全修复 | 共享 GPU 设备、键盘事件、播放优化 |
| `crates/enc-gif/` | ✅ 完成 | 直接取上游 | GIF 帧处理优化 |
| `crates/media-info/` | ✅ 完成 | 直接取上游 | 媒体信息改进 |
| `crates/export/` | ✅ 完成 | 取上游 + 删除 export_priority | 导出管线重构、光标预计算、编码器调优 |
| `crates/recording/src/recovery.rs` | ✅ 完成 | 手动移植 bug fix | M4S 恢复修复 |

### 待完成

| 模块 | 冲突程度 | 详情 |
|------|---------|------|
| `crates/recording/` (除 recovery.rs) | 🔴 高 | 7 个文件双方修改：capture_pipeline, instant_recording, win.rs, win_fragmented_m4s, screen_capture/windows, studio_recording, lib.rs |
| `crates/enc-ffmpeg/` | 🟡 中 | fork 改了 HEVC 支持相关文件 |
| `crates/enc-mediafoundation/` | 🟡 中 | fork 改了 HEVC 相关文件 |
| `apps/desktop/src-tauri/src/` | 🔴 高 | 双方都大量修改（lib.rs +534行上游, fork 也重构了） |
| `apps/desktop/src/` | 🔴 高 | fork 大量本地化改动 + 上游新功能 |
| `crates/cap-test/` | 🟢 低 | 可直接取上游 |

### 关键发现

1. **v0.4.5 已包含的修复无需引入**: audio panic (#1536), encoder compile (#1578)
2. **fork 没有预热叠加层功能**: #1523 不适用
3. **逐 PR cherry-pick 不可行**: 上游改动高度互相依赖，21K+ 行
4. **模块级同步最有效**: 先同步无冲突模块，再处理冲突模块

---

## 风险评估

| 阶段 | 风险 | 缓解措施 |
|------|------|---------|
| Phase 1 (Bug fixes) | 低 — 独立修复，影响面小 | 每个修复单独 commit，易回滚 |
| Phase 2 (Performance) | 中 — 涉及核心管线改动 | 每个优化独立 commit + 性能基准对比 |
| Phase 3 (New features) | 中高 — #1615 是大型 PR | 拆分为独立步骤，键盘和字幕分开验证 |
| Phase 4 (Polish) | 低 — 选择性引入 | 只取确定有益的部分 |

## 时间估算

| 阶段 | 预计工时 |
|------|---------|
| Task 0 (前置准备) | 0.5h |
| Phase 1 (Bug fixes) | 2-4h |
| Phase 2 (Performance) | 4-8h |
| Phase 3 (New features) | 8-16h |
| Phase 4 (Polish) | 2-4h |
| **总计** | **16-32h** |
