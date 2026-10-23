# 开发与打包指南

本文说明如何在 Windows 上开发、测试和打包 FrameEver · 帧刻（`apps/desktop-gpui`，原生 GPUI 应用）。所有操作都通过一个入口完成：

```powershell
pnpm fe            # 列出全部命令
```

本地开发、CI 发布、Claude 桌面版里的“FrameEver 开发”面板都调用同一套命令，所以本地能跑通，CI 就能跑通。给 agent 的调试说明见 [docs/agents/debugging.md](docs/agents/debugging.md)。

## 前置要求

| 依赖 | 说明 |
|---|---|
| Windows 10/11 x64 | 只支持 Windows |
| Node.js 20+ 与 pnpm 10 | `npm install -g pnpm@10.5.2` |
| Rust（rustup） | 根工作区用 1.88，GPUI 应用用 stable；`pnpm fe setup` 会补装 |
| Visual Studio 2022 生成工具 | 含“使用 C++ 的桌面开发”、LLVM（clang）组件和 CMake |
| Git for Windows | 提供脚本要用的 Git Bash |

## 第一次：准备环境

```powershell
pnpm fe setup
```

它会依次：检查工具链；安装 JS 依赖；下载 FFmpeg、ONNX Runtime、DXC 到 `target/`；生成 `.cargo/config.toml`；把打过补丁的 gpui 准备到仓库同级的 `../zed-cap`；从小米官网下载中文界面字体 MiSans（约 230 MB 的官方压缩包，只取 4 个字重放进 `apps/desktop-gpui/assets/fonts/misans/`）。MiSans 的许可不允许单独再分发字体文件，所以它不进仓库，只嵌入安装包。重复运行是安全的，已下载的内容会直接复用。

之后随时可以用 `pnpm fe doctor` 检查环境，缺什么会直接说怎么修。

## 日常开发

| 命令 | 作用 |
|---|---|
| `pnpm fe dev` | 编译并运行应用。默认使用独立的测试数据（`apps/desktop-gpui/target/testdata/`），不影响你自己的设置和录制 |
| `pnpm fe dev --watch` | 保存代码后自动重新编译并重启，编译失败时保留正在运行的旧版本 |
| `pnpm fe dev --lang zh-CN` | 用中文界面启动测试数据（`--lang en` 切回英文） |
| `pnpm fe dev --real` | 使用你真实的数据运行（谨慎） |
| `pnpm fe logs` | 查看最近一次运行的日志，`--grep <关键词>` 过滤，`--lines 500` 看更多 |
| `pnpm fe check` | 编译检查（GPUI 应用和命令行，含测试代码） |
| `pnpm fe test [过滤词]` | 运行 GPUI 测试；`--all` 再加上编码库、导出、命令行和脚本测试 |
| `pnpm fe lint` | 格式检查、Clippy、翻译完整性检查，提交前跑一次 |
| `pnpm fe fmt` | 格式化代码 |
| `pnpm fe export h264\|hevc\|av1` | 用最新的测试录制导出视频到 `target/testdata/exports/` |

## 打包安装程序

```powershell
pnpm fe package
```

依次编译附带程序（命令行、muxer）、正式版应用，再生成 NSIS 安装包，复制到 `dist/`。第一次要完整编译，需要十几到几十分钟。

- 没有设置 `TAURI_SIGNING_PRIVATE_KEY` 时，会生成不带自动更新签名的安装包，自己测试用足够了。
- `--version 3.0.0` 会把版本号写进应用再打包（CI 发版时使用）。
- `--skip-sidecars` 跳过附带程序的编译，只重新打应用和安装包。

## 发布新版本

```powershell
pnpm fe release 3.0.0
```

先检查工作区是否干净、标签是否已存在，再跑全部测试，然后请你确认，确认后才推送 `v3.0.0` 标签。GitHub Actions（`.github/workflows/release.yml`）会用同样的 `pnpm fe setup` 和 `pnpm fe package` 打包、签名，并发布到 GitHub Releases，已安装的用户会收到更新。带 `-` 的版本（如 `3.1.0-beta.1`）发布为预览版。

更新签名密钥只需配置一次：`scripts/setup-updater-signing.sh`（用 Git Bash 运行）。

## 常见问题

**`bash` 报 WSL 错误**：PowerShell 里的 `bash` 是 WSL 的。`pnpm fe` 会自动找到 Git Bash；需要手动运行 `.sh` 脚本时，用 `& "C:\Program Files\Git\bin\bash.exe" <脚本>`。

**启动时报找不到 DLL（0xc0000135）**：用 `pnpm fe dev` 启动，它会把 FFmpeg 加进 PATH；不要直接双击 `target/debug` 里的 exe。

**编译报 `cold_path` 或 gpui 相关错误**：GPUI 应用需要 Rust stable（见 `apps/desktop-gpui/rust-toolchain.toml`），运行 `pnpm fe setup`。

**`cargo build` 报 “Access is denied”**：应用还开着，先关掉再编译。

**libclang / bindgen 报错**：`.cargo/config.toml` 缺少 `LIBCLANG_PATH`，运行 `pnpm fe setup`。

更多见 [docs/agents/debugging.md](docs/agents/debugging.md)。遇到文档没覆盖的问题，欢迎[提交 Issue](https://github.com/AniBullet/FrameEver/issues/new)。
