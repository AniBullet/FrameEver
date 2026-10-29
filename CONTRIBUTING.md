# FrameEver 贡献指南

## 项目简介

FrameEver · 帧刻是 aniBullet 的纯本地屏幕录制和编辑应用，最初从 [Cap](https://github.com/CapSoftware/Cap) 分出，现在独立开发。它专注于：

- 无时长 / 数量限制的本地录制
- 录制、编辑、导出都只在本机完成，不上传任何内容
- 先发布 Windows x64 版，之后再做 macOS
- 完全免费、开源

---

## 如何参与

- [报告 Bug](https://github.com/AniBullet/FrameEver/issues/new)
- [发起讨论](https://github.com/AniBullet/FrameEver/discussions)
- 提交 Pull Request

---

## 开发环境

### 系统要求

| 依赖 | 版本 |
|------|------|
| Node.js | 20+ |
| pnpm | 10+ |
| Rust | rustup（stable，`pnpm fe setup` 会补装） |
| Visual Studio 2022 生成工具 | C++ 桌面开发、LLVM、CMake |
| Windows | x64 |

> **注意**：目前只构建和发布 Windows 版。macOS 代码保留，Windows 1.0.0 之后再做；不支持 Linux。

### 快速启动

```powershell
pnpm fe setup     # 准备环境（首次）
pnpm fe dev       # 用独立的测试数据运行应用（--watch 自动重编译）
pnpm fe lint      # 提交前检查：格式、Clippy、翻译
pnpm fe package   # 打包安装程序
```

详细步骤请参阅 [BUILD.md](BUILD.md)。

---

## 项目结构

```
FrameEver/
├── apps/
│   ├── desktop-gpui/       # GPUI 桌面应用（录制、编辑器、资源库、设置）
│   │   └── converter/      # FrameEver Converter 向导（单独下载）
│   └── cli/                # 命令行工具 frameever
├── crates/
│   ├── recording/          # 核心录制管线
│   ├── rendering/          # 视频渲染 & 特效
│   ├── editor/             # 编辑器播放
│   ├── export/             # 导出 & 编码
│   ├── project/            # 项目格式
│   ├── converter/          # FrameEver Converter 转换核心
│   ├── utils/              # 品牌常量、存储文件夹
│   ├── camera*/            # 摄像头
│   └── media*/             # 音视频处理
├── docs/                   # 决策记录（adr/）、agent 指南、交接文档
└── scripts/                # pnpm fe 和其他开发脚本
```

术语以 [GLOSSARY.md](GLOSSARY.md) 为准。

### 常用命令

```bash
pnpm fe dev          # 编译并运行应用
pnpm fe check        # 编译检查应用、FrameEver Converter 和 CLI
pnpm fe test --all   # 运行全部测试
pnpm fe lint         # 格式检查、clippy 和翻译检查
pnpm fe fmt          # 格式化
pnpm fe converter    # 用假的 Cap-aniBullet 2.x 数据试运行 FrameEver Converter
```

---

## 数据位置

| 内容 | 位置 |
|------|------|
| 项目、截图、导出（存储文件夹） | 默认 `%USERPROFILE%\Videos\FrameEver\`（`Recordings`、`Screenshots`、`Exports`），可在设置里更改 |
| 设置 | `%APPDATA%\io.github.anibullet.frameever\` |
| 日志 | `%LOCALAPPDATA%\io.github.anibullet.frameever\logs\` |
| `pnpm fe dev` 的测试数据 | `apps/desktop-gpui/target/testdata/` |

---

## 代码规范

完整规范请参阅 [AGENTS.md](AGENTS.md)，核心要点：

### 通用
- **不写注释** — 代码通过命名和结构自说明
- 界面文字只有英文和简体中文，每条新文字都要有中文翻译；专有名词在中文界面保持英文（见 [docs/i18n.md](docs/i18n.md)）
- 纯扁平设计，不用渐变和毛玻璃

### Rust
- `rustfmt` + workspace clippy lints（`pnpm fe lint`）
- 模块名 snake_case，crate 名 kebab-case 并以 `frameever-` 开头
- 禁止 `dbg!()`，正确处理 `Result` / `Option`

### 脚本
- `scripts/*.mjs`：Tab 缩进，Biome 格式化（`pnpm format`），文件名 kebab-case

### 本地优先原则
- 不引入任何云端 SDK / API
- 不添加需要联网的功能
- 不引入账号 / 认证 / 付费相关逻辑

---

## 提交规范

使用 Conventional Commits 格式：

```
feat:     新功能
fix:      Bug 修复
chore:    构建、依赖、清理
improve:  功能改进（非新功能）
refactor: 重构（不影响行为）
docs:     文档变更
```

提交说明用中文，有对应 Issue 时写上编号。示例：
```
feat: 新增 HEVC 视频压缩功能（#18）
fix: 修复区域录制工具条跑到其他屏幕的问题
```

`git push` 前会自动运行隐私检查（`pnpm fe privacy`），不要绕过。

---

## Pull Request 要求

1. **描述清晰** — 说明改动内容和原因
2. **关联 Issue** — 如有对应 Issue，请在描述中 `Closes #xxx`
3. **截图 / GIF** — UI 变更必须附图
4. **保持范围** — 一个 PR 专注一件事
5. **文档同步** — 行为变更时同步更新 README / CHANGELOG

---

## 禁止事项

- ❌ 引入云端依赖或需要联网的功能
- ❌ 添加用户账号 / 认证 / 付费相关代码
- ❌ 在代码中添加注释（`//`、`/* */`、`///` 等）
- ❌ 使用 `dbg!()` 宏

---

## 开源协议

贡献的代码将遵循本项目的 **AGPL-3.0** 协议开源。
