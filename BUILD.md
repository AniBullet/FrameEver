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
| Rust（rustup） | stable；`pnpm fe setup` 会补装 |
| Visual Studio 2022 生成工具 | 含“使用 C++ 的桌面开发”、LLVM（clang）组件和 CMake |
| Git for Windows | 提供脚本要用的 Git Bash |

## 第一次：准备环境

```powershell
pnpm fe setup
```

它会依次：检查工具链；安装 JS 依赖；下载 FFmpeg、ONNX Runtime、DXC 到 `target/`；生成 `.cargo/config.toml`；把打过补丁的 gpui 准备到仓库同级的 `../zed-cap`；从华为官网下载中文界面字体 HarmonyOS Sans（约 22 MB 的官方压缩包，校验 SHA1 后只取简体中文可变字体和许可文本，放进 `apps/desktop-gpui/assets/fonts/harmonyos-sans/`）。它的许可不允许单独再分发字体文件，所以不进仓库，只嵌入安装包，许可文本随安装包放在 `licenses/fonts/`；准备 Tauri 打包用的 NSIS（下载地址和 SHA1 与 Tauri 2.11.2 完全一致，放在 `%LOCALAPPDATA%\tauri\NSIS`），并给它的编译器 `Bin\makensis.exe` 打开大地址支持（见下方常见问题）。重复运行是安全的，已下载的内容会直接复用。

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

先在 [CHANGELOG.md](CHANGELOG.md) 最上面写这个版本的一节，这一节会原样成为 GitHub Release 的说明、应用内的更新提示和“更新日志”页的内容：

```markdown
## [3.1.0] — 未发布

一句话概括这个版本（可选）。

### 新增
- 用户看得到的新功能，一行一条

### 改进
- 变得更好用的地方

### 修复
- 修好的问题，写现象而不是代码

### 移除
- 去掉的功能
```

标题里的日期先写“未发布”，发布时由 `pnpm fe release` 自动改成当天日期。没有内容的小节直接省略。写给用户看，不写文件名和代码细节。用 `pnpm fe notes 3.1.0` 可以预览截取出来的说明。然后：

```powershell
pnpm fe release 3.1.0
```

先检查工作区是否干净、标签是否已存在、`CHANGELOG.md` 里有没有这个版本，再跑全部测试，显示将要发布的说明；你确认后，把更新记录的日期改成当天并提交，再推送当前分支和 `v3.1.0` 标签。CI 在开始编译前也会检查 `CHANGELOG.md`，缺少这一节会直接失败。GitHub Actions（`.github/workflows/release.yml`）会用同样的 `pnpm fe setup` 和 `pnpm fe package` 打包、签名，并发布到 GitHub Releases，已安装的用户会收到更新。带 `-` 的版本（如 `3.1.0-beta.1`）发布为预览版。

更新签名密钥只需配置一次：`scripts/setup-updater-signing.sh`（用 Git Bash 运行）。

## 更新依赖

依赖不是完全锁死的。两个 `Cargo.lock` 记录每个依赖实际用的版本，保证本地和 CI 编出来一模一样；`Cargo.toml` 里写的是版本范围，范围内可以随时升级。

```powershell
pnpm fe deps          # 看能升什么：范围内的升级，以及需要手动升的新版本
pnpm fe deps --apply  # 应用范围内的升级（两个工作区都会更新）
```

建议每次发版前做一次，然后跑 `pnpm fe test --all`，用 `pnpm fe dev` 启动应用，再用 `pnpm fe export hevc` 导出一次，都正常再提交两个 `Cargo.lock`。

`pnpm fe deps` 列出的“held back”是范围外的新版本，需要逐个看更新说明再手动改 `Cargo.toml`，不要一次全升。故意锁定的依赖（预发布版 `ort`、`specta`，gpui 的提交号，Cap 团队 fork 的 cpal / rust-ffmpeg / nokhwa，`vendor/` 下的补丁）和原因记在 `docs/upstream-sync/0.6.1.md` 路线图的 Phase 4。预发布版（版本号带 `-rc`）在 `Cargo.toml` 里用 `=` 精确锁定，因为两个 rc 之间常有不兼容改动。

## 推送前的隐私检查

`pnpm fe setup` 会把 Git 钩子目录设为 `.githooks`，之后每次 `git push` 都会先扫描这次要推送的提交，发现疑似隐私就中止推送。也可以手动运行：

```powershell
pnpm fe privacy           # 扫描还没推送的提交：作者邮箱、提交说明、新增的每一行、文件名
pnpm fe privacy --staged  # 只扫描已暂存的改动
pnpm fe privacy --all     # 扫描仓库里所有已跟踪的文件
```

它会查：非 noreply 的邮箱、本机用户名和计算机名（会出现在 `C:\Users\…` 这类路径里）、私钥和签名密钥、GitHub / AWS / OpenAI / Anthropic 等 token、写在代码里的密码、手机号和身份证号，以及 `.env`、`*.pem`、`*.log`、`settings.local.json` 这类文件。命中的内容在输出里只显示开头几个字符。真的是公开内容时（比如第三方作者的邮箱），把它加进 `scripts/privacy-allow.json`；已经提交的隐私要改写提交后再推送。

## 常见问题

**`bash` 报 WSL 错误**：PowerShell 里的 `bash` 是 WSL 的。`pnpm fe` 会自动找到 Git Bash；需要手动运行 `.sh` 脚本时，用 `& "C:\Program Files\Git\bin\bash.exe" <脚本>`。

**启动时报找不到 DLL（0xc0000135）**：用 `pnpm fe dev` 启动，它会把 FFmpeg 加进 PATH；不要直接双击 `target/debug` 里的 exe。

**编译报 `cold_path` 或 gpui 相关错误**：GPUI 应用需要 Rust stable（见 `apps/desktop-gpui/rust-toolchain.toml`），运行 `pnpm fe setup`。

**`cargo build` 报 “Access is denied”**：应用还开着，先关掉再编译。

**打包报 `Internal compiler error #12345: deflateInit() failed`**：NSIS 的编译器 `makensis` 是 32 位程序，默认只能用 2 GB 地址空间。安装脚本和上游一样用 128 MB 的 LZMA 字典，压缩器要一次申请约 1.3 GB 连续内存，内存碎片多时就会失败，所以时好时坏。`pnpm fe setup` 和 `pnpm fe package` 会给 `%LOCALAPPDATA%\tauri\NSIS\Bin\makensis.exe` 打开 PE 文件头里的大地址标志（效果同 `editbin /LARGEADDRESSAWARE`），在 64 位 Windows 上可用 4 GB。Tauri 只检查这个文件是否存在、不校验哈希，所以标志会一直保留；如果 Tauri 因为工具不完整重新解压了 NSIS，再运行一次 `pnpm fe package` 就会重新打上。

**libclang / bindgen 报错**：`.cargo/config.toml` 缺少 `LIBCLANG_PATH`，运行 `pnpm fe setup`。

更多见 [docs/agents/debugging.md](docs/agents/debugging.md)。遇到文档没覆盖的问题，欢迎[提交 Issue](https://github.com/AniBullet/FrameEver/issues/new)。
