<div align="center">

<img src="apps/desktop-gpui/icons/128x128@2x.png" alt="FrameEver" width="112" />

# FrameEver · 帧刻

**一刻，凝成永恒**

纯本地的 Windows 录屏与剪辑工具：不限时长、不用账号、不上传任何内容、完全免费。

[![Release](https://img.shields.io/github/v/release/AniBullet/FrameEver?include_prereleases&label=%E7%89%88%E6%9C%AC&color=1F3FD8)](https://github.com/AniBullet/FrameEver/releases)
[![Platform](https://img.shields.io/badge/Windows-10%20%2F%2011-1F3FD8)](https://github.com/AniBullet/FrameEver/releases)
[![License](https://img.shields.io/badge/license-AGPL--3.0-1F3FD8)](LICENSE)

[**下载**](https://github.com/AniBullet/FrameEver/releases/latest) · [更新日志](CHANGELOG.md) · [反馈问题](https://github.com/AniBullet/FrameEver/issues/new)

</div>

---

## 为什么用 FrameEver

- **只在你的电脑上**：录制、剪辑、字幕识别、压缩全部在本机完成，内容不会离开你的电脑。
- **没有限制**：没有时长上限、没有数量上限、没有付费功能、没有水印。
- **原生、轻快**：基于 [Cap](https://github.com/CapSoftware/Cap) 0.6.1 的原生 GPUI 界面，用 Rust 编写，直接用 GPU 绘制。
- **文件小**：导出支持 HEVC 和 AV1，同样画质下文件约为 H.264 的 1/3 到 1/4。
- **中文友好**：完整的简体中文界面，中文用 HarmonyOS Sans 字体。

## 功能

**录制**
- 三种模式：Studio（录完进编辑器精修）、即时录制（录完直接得到视频）、截图
- 录制全屏、窗口或任意区域，可叠加摄像头画面，支持摄像头背景虚化
- 同时录制麦克风和系统声音
- 提词器、全局快捷键、录制前倒计时

**编辑**
- 时间轴剪辑：裁剪、分段、删除片段
- 自动缩放跟随光标，光标美化
- 背景、圆角、阴影、画面边框
- 按键显示轨道，适合教程录制
- 本地 AI 字幕：Whisper 或 Parakeet 模型，离线识别，可在时间轴上逐句调整

**导出与管理**
- MP4（H.264 / HEVC / AV1）、GIF、MOV
- 即时录制可自动压缩为 HEVC，原版与压缩版随时切换或删除
- 截图编辑器：标注、裁剪、美化

**其他**
- 命令行工具 `frameever`，可供脚本、自动化规则和 AI 智能体调用（[说明](apps/cli/README.md)）
- 从 Cap-aniBullet 2.x 或 Cap 升级：用单独下载的 FrameEver Converter 把旧的设置、录制、截图和规则转换进 FrameEver 的存储文件夹

## 安装

从 [Releases](https://github.com/AniBullet/FrameEver/releases/latest) 下载 `FrameEver_<版本>_x64-setup.exe` 并运行。支持 Windows 10 / 11（64 位）。安装后应用会自动检查更新。

## 隐私

FrameEver 只在以下情况联网，且不会发送你录制的任何内容：

- 检查和下载更新（来自本仓库的 GitHub Releases）
- 首次使用 AI 字幕时下载识别模型
- 你主动点击的链接，在浏览器里打开

## 和 Cap 原版的区别

| | Cap 原版 | FrameEver |
| --- | --- | --- |
| 录制时长与数量 | 免费版有限制 | 无限制 |
| 账号 | 需要注册 | 不需要 |
| 分享与存储 | 上传到云端 | 只保存在本机 |
| AI 字幕 | 云端服务 | 本地模型，离线 |
| 导出编码 | H.264 | H.264、HEVC、AV1 |
| 界面语言 | 英文 | 简体中文、英文 |
| 平台 | macOS、Windows | Windows（macOS 版在 Windows 1.0 之后） |

## 开发

```powershell
pnpm fe setup     # 准备开发环境（首次）
pnpm fe dev       # 用独立的测试数据运行应用
pnpm fe test      # 运行测试
pnpm fe package   # 打包安装程序到 dist/
```

完整说明见 [BUILD.md](BUILD.md)，贡献前请阅读 [CONTRIBUTING.md](CONTRIBUTING.md)。技术栈：Rust、GPUI、FFmpeg、Whisper / Parakeet。

## 协议与致谢

FrameEver 基于 [Cap](https://github.com/CapSoftware/Cap) 改造，遵循 **AGPL-3.0** 协议：保留原始版权声明、公开全部源代码。部分模块（`frameever-camera*`、`scap-*`）为 MIT 协议，见各模块的 LICENSE。FrameEver 是独立项目，与 Cap 官方无关，不使用 Cap 的名称和标志。

感谢 Cap 团队和所有贡献者。界面字体使用 [Inter](https://rsms.me/inter/)（SIL OFL 1.1）和华为 [HarmonyOS Sans](https://developer.huawei.com/consumer/cn/design/resource/)。

---

<div align="center">

如果 FrameEver 对你有帮助，欢迎点个 ⭐ Star

<details>
<summary>☕ 请作者喝杯咖啡</summary>
<br>
<img src=".github/images/wechat.png" width="180" alt="微信赞赏码" />
<br>
<sub>微信 WeChat</sub>
</details>

</div>
