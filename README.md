<div align="center">

<img src="apps/desktop-gpui/icons/128x128@2x.png" alt="FrameEver" width="120" />

# FrameEver · 帧刻

[![License](https://img.shields.io/badge/license-AGPL--3.0-blue.svg)](LICENSE)
[![Version](https://img.shields.io/github/v/release/AniBullet/FrameEver?include_prereleases)](https://github.com/AniBullet/FrameEver/releases)
[![Platform](https://img.shields.io/badge/platform-Windows-blue.svg)](https://github.com/AniBullet/FrameEver/releases)
[![Rust](https://img.shields.io/badge/rust-1.88%2B-orange.svg)](https://www.rust-lang.org)
[![Node](https://img.shields.io/badge/node-20%2B-brightgreen.svg)](https://nodejs.org)

**一刻，凝成永恒 — 纯本地的录屏与剪辑：无时长限制、无账号、无云端、完全免费**

由 aniBullet 制作，基于开源项目 [Cap](https://github.com/CapSoftware/Cap)。原名 Cap - aniBullet Edition。

[📥 下载](https://github.com/AniBullet/FrameEver/releases) • [快速开始](#-快速开始) • [更新日志](#-更新日志) • [贡献](#-贡献)

![FrameEver 主界面](.github/images/screenshot-main.png)

</div>

---

> 正在从 2.x（Tauri 界面）迁移到新的原生界面（GPUI）。新版首次启动时会自动导入 2.x 的设置，并继续显示 2.x 的录制文件（原文件不移动）。命令行工具改名为 `frameever`。

## 和原版的区别

| 特性 | Cap 原版 | FrameEver |
|------|---------|----------------|
| 录制时长 | 免费版限 5 分钟 | ✅ 无限制 |
| 视频数量 | 免费版限 25 个 | ✅ 无限制 |
| 数据存储 | 云端 | 🏠 本地 SQLite |
| 用户账号 | 必须注册 | 🏠 不需要 |
| 付费订阅 | Pro 功能需付费 | ✅ 全功能免费 |
| AI 字幕 | 云端 API（付费） | ✅ 本地 Whisper.cpp |
| 视频压缩 | 云端处理 | ✅ 本地 HEVC |
| 键盘 Overlay | — | ✅ 实时按键显示 |
| 离线使用 | 需联网 | ✅ 完全离线 |
| 界面语言 | 英语 | ✅ 简体中文 / 英语 |

---

## 功能

- **多种录制模式** — 全屏、窗口、区域（工具条锁定在录制区旁）、仅摄像头
- **Studio 工作室** — 时间轴编辑、裁剪、渐变编辑器
- **键盘 Overlay** — 实时显示按键，教学录制专属
- **字幕时间轴** — 可视化字幕编辑，精准对齐视频
- **本地 AI 字幕** — Whisper.cpp，支持 99+ 语言，完全离线
- **HEVC 压缩** — 库内右键一键压缩，原版 / 压缩版随时切换
- **高质量导出** — MP4 (H.264/H.265)、GIF、智能 CRF 模式
- **截图编辑器** — 注释、裁剪、美化

---

## 🚀 快速开始

```powershell
pnpm fe setup     # 准备环境（首次）
pnpm fe dev       # 用独立的测试数据运行应用
pnpm fe package   # 打包安装程序到 dist/
```

详细步骤见 [BUILD.md](BUILD.md)。

---

## 📋 更新日志

> 完整历史见 [CHANGELOG.md](CHANGELOG.md)

**v2.2.0** — 删除 OAuth / 水印 Shader / 商业授权 / 付费限时残留代码，移除 mysql2 依赖。

**v2.1.0** — HEVC 压缩、原版 / 压缩版管理、智能 CRF 导出、快速录制自动压缩。

**v2.0.x** — Studio GPU 编码（消除马赛克）、区域录制工具条修复、上游同步 v0.4.81。

---

## 🛠️ 技术栈

SolidJS + TypeScript + TailwindCSS • Rust + Tauri 2.0 • SQLite • FFmpeg + MediaFoundation • Whisper.cpp • Windows x64

---

## 贡献

欢迎 Issue 和 PR，提交前请阅读 [CONTRIBUTING.md](CONTRIBUTING.md)。

---

## 协议

**AGPL-3.0**，基于 [Cap](https://github.com/CapSoftware/Cap) v0.6.1。FrameEver 是独立的分支，与 Cap 官方无关，不使用 Cap 的名称与标志。

<details>
<summary>详情</summary>

本项目完全遵守 AGPL 3.0 协议：保留原始协议、公开源代码、明确标注修改内容。

部分 crates（`cap-camera*`、`scap-*`）采用 MIT 协议，详见各模块 LICENSE 文件。

</details>

---

## 致谢

基于 [Cap](https://github.com/CapSoftware/Cap) 改造，感谢 Cap 团队和所有贡献者。

---

<div align="center">

[⭐ Star](https://github.com/AniBullet/FrameEver) • [📥 下载](https://github.com/AniBullet/FrameEver/releases) • [🐛 反馈](https://github.com/AniBullet/FrameEver/issues)

<details>
<summary>☕ 支持作者</summary>

<br>

如果有帮助，可以请作者喝杯咖啡 ☕ 或者给个 Star ⭐

<div align="center">
  <img src=".github/images/wechat.png" width="180" alt="微信" />
  <br>
  <sub>微信 WeChat</sub>
</div>

</details>

</div>
