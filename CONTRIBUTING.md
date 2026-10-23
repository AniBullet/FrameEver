# FrameEver 贡献指南

## 项目简介

FrameEver（帧刻）是基于 [Cap](https://github.com/CapSoftware/Cap) 的本地优先屏幕录制应用分支，专注于：

- 无时长 / 数量限制的本地录制
- 完全离线运行（无云端依赖）
- Windows x64 平台
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
| Rust | rustup（1.88 与 stable，`pnpm fe setup` 会补装） |
| Visual Studio 2022 生成工具 | C++ 桌面开发、LLVM、CMake |
| Windows | x64 |

> **注意**：本项目仅支持 Windows 平台，macOS / Linux 不在维护范围。

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
├── apps/desktop/           # Tauri v2 桌面应用
│   ├── src/                # SolidJS 前端（录制 UI、编辑器、设置）
│   └── src-tauri/          # Rust 后端（窗口管理、录制控制）
├── packages/
│   ├── ui-solid/           # SolidJS 共享组件库
│   └── utils/              # 共享工具 / 类型 / 常量
├── crates/
│   ├── recording/          # 核心录制管线
│   ├── rendering/          # 视频渲染 & 特效
│   ├── export/             # 导出 & 编码
│   ├── camera*/            # 摄像头跨平台抽象
│   └── media*/             # 音视频处理
└── scripts/                # 开发工具脚本
```

### 常用命令

```bash
pnpm dev              # 启动开发服务器
pnpm build            # 构建所有包（Turbo）
pnpm lint             # Biome 代码检查
pnpm format           # Biome 格式化
pnpm typecheck        # TypeScript 类型检查
cargo fmt             # Rust 格式化
cargo test -p <crate> # 运行指定 crate 测试
```

---

## 录制文件位置

```
%APPDATA%\so.cap.desktop.anibullet\recordings\
```

---

## 代码规范

完整规范请参阅 [AGENTS.md](AGENTS.md)，核心要点：

### TypeScript / SolidJS
- 2 空格缩进，Biome 格式化（`pnpm format`）
- 文件名 kebab-case，组件名 PascalCase
- **不写注释** — 代码通过命名和结构自说明

### Rust
- `rustfmt` + workspace clippy lints（`cargo fmt`）
- 模块名 snake_case，crate 名 kebab-case
- 禁止 `dbg!()`，正确处理 `Result` / `Option`

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
sync:     上游同步
```

示例：
```
feat: 新增 HEVC 视频压缩功能
fix: 修复区域录制工具条跑到其他屏幕的问题
chore: v2.2.0 - 清理上游残留云端/认证代码
```

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
- ❌ 编辑自动生成文件（`**/tauri.ts`、`**/queries.ts`、`apps/desktop/src-tauri/gen/**`）
- ❌ 在代码中添加注释（`//`、`/* */`、`///` 等）
- ❌ 使用 `dbg!()` 宏

---

## 开源协议

贡献的代码将遵循本项目的 **AGPL-3.0** 协议开源。
