# FrameEver 贡献指南

FrameEver · 帧刻是纯本地的录屏、截图与剪辑工具，从 [Cap](https://github.com/CapSoftware/Cap) 分出，现在独立开发。欢迎报告问题、提出想法和提交代码。

## 参与方式

- **报告问题或提建议**：[新建 Issue](https://github.com/AniBullet/FrameEver/issues/new/choose)。请写上版本、系统和复现步骤，有日志更好（位置见[数据位置](#数据位置)）。
- **提交代码**：先在 Issue 里说明要改什么，确认方向后再开 Pull Request。还没处理的工作都在 [Issues](https://github.com/AniBullet/FrameEver/issues) 里。

## 三条底线

这三条决定一个改动能不能合并：

1. **内容不出本机。** 不引入云端 SDK、账号、认证、付费和遥测。允许的联网只有三种：从本仓库的 GitHub Releases 检查和下载更新、下载字幕识别模型、用户自己点开的链接（[ADR-0002](docs/adr/0002-local-only-no-cloud.md)）。
2. **不丢功能。** 除了云端和账号，Cap 有的功能和交互都要保留。
3. **界面扁平。** 没有渐变和毛玻璃，颜色取自设计令牌，样子以 [`docs/design`](docs/design) 的设计页为准。

## 开发环境

| 依赖 | 版本 |
| --- | --- |
| Node.js | 20+ |
| pnpm | 10+ |
| Rust | stable（`pnpm fe setup` 会补装） |
| Windows | 10 / 11 x64，Visual Studio 2022 生成工具（C++ 桌面开发、LLVM、CMake） |
| macOS | Apple 芯片；目前只在 CI 上编译和测试，还没在真机上验证 |

不支持 Linux。

```powershell
pnpm fe setup     # 准备环境（首次）
pnpm fe doctor    # 检查缺什么、怎么补
pnpm fe dev       # 用独立的测试数据运行应用（--watch 自动重编译，--lang zh-CN 中文界面）
```

所有构建、测试和打包都走 `pnpm fe <命令>`，不要用别的构建命令。完整说明见 [BUILD.md](BUILD.md)。

## 项目结构

```
FrameEver/
├── apps/
│   ├── desktop-gpui/       GPUI 桌面应用：录制、编辑器、资源库、设置
│   │   └── converter/      FrameEver Converter 向导（单独下载）
│   └── cli/                命令行工具 frameever
├── crates/
│   ├── recording/ rendering/ editor/ export/ project/    录制与编辑管线
│   ├── enc-*/ media*/ video-decode/ muxer*/              编码、解码、封装
│   ├── scap-*/ camera*/ audio/ cursor-*/                 屏幕、摄像头、音频、光标采集
│   ├── converter/          FrameEver Converter 转换核心
│   └── utils/              品牌常量、存储文件夹
├── docs/                   决策记录（adr/）、设计页（design/）、文案和翻译规则
└── scripts/                pnpm fe 和其他开发脚本
```

术语以 [GLOSSARY.md](GLOSSARY.md) 为准：项目、存储文件夹、导出、工作室录制、即时录制、资源库。

## 数据位置

| 内容 | 位置 |
| --- | --- |
| 项目、截图、导出（存储文件夹） | 默认 `%USERPROFILE%\Videos\FrameEver\`（`Recordings`、`Screenshots`、`Exports`），可在设置里更改 |
| 设置 | `%APPDATA%\io.github.anibullet.frameever\` |
| 日志 | `%LOCALAPPDATA%\io.github.anibullet.frameever\logs\` |
| `pnpm fe dev` 的测试数据 | `apps/desktop-gpui/target/testdata/` |

## 写代码

完整规范在 [AGENTS.md](AGENTS.md)，要点如下。

**通用**

- 不写任何注释，代码靠命名、类型和结构说明自己，解释写进文档
- 先写测试：Rust 测试放在代码旁（`#[cfg(test)]`）或 crate 的 `tests/`，脚本测试是 `scripts/*.test.mjs`
- 修 Bug 先复现，再用测试或实测数据证明修好了

**Rust**

- `rustfmt` 加工作区的 clippy 规则
- 模块名 snake_case；crate 名 kebab-case，以 `frameever-` 开头；环境变量以 `FRAMEEVER_` 开头
- 不用 `dbg!()`，处理每一个 `Result` 和 `Option`

**界面**

- 文字只有英文和简体中文，每条 `t!` 文字都要有中文翻译；专有名词（FrameEver、HEVC、Whisper）在中文界面保持英文（[docs/i18n.md](docs/i18n.md)）
- 文案的语气和写法见 [docs/copy-style.md](docs/copy-style.md)
- 图标用 Lucide，Lucide 没有的才用 Tabler
- 改了样子，就在同一个 PR 里改对应的设计页

**脚本**

- `scripts/*.mjs`：Tab 缩进，Biome 格式化（`pnpm format`），文件名 kebab-case

## 检查

| 时机 | 命令 |
| --- | --- |
| 每次改动后 | `pnpm fe fmt`，再跑 `pnpm fe test <过滤词>`（根工作区的 crate 用 `--crate <包名>`） |
| 提交 PR 前 | `pnpm fe test --all` 和 `pnpm fe lint` 各一次 |

## 提交

使用 Conventional Commits 前缀，说明用中文，有对应 Issue 时写上编号：

```
feat:     新功能
fix:      修复
improve:  改进已有功能
refactor: 重构，不改变行为
chore:    构建、依赖、清理
docs:     文档
```

```
feat: 资源库加 Space 预览（#96）
fix: 修复区域录制控制条跑到其他屏幕的问题
```

`git push` 前会自动运行隐私检查（`pnpm fe privacy`），不要绕过。

## Pull Request

- 一个 PR 只做一件事，说明改了什么、为什么
- 关联 Issue：`Closes #编号`
- 界面改动附截图或 GIF
- 行为变了，同步更新 [CHANGELOG.md](CHANGELOG.md) 里未发布的那一节，必要时更新 README
- 上游 Cap 只作参考：修复手工移植，不合并，也不要向 `CapSoftware/Cap` 推送或提 Issue

## 协议

贡献的代码以本项目的 **AGPL-3.0** 协议开源（[LICENSE](LICENSE)）；`frameever-camera*` 和 `scap-*` 模块为 MIT 协议（[licenses/LICENSE-MIT](licenses/LICENSE-MIT)）。提交即表示你有权按这些协议授权你的贡献。
