# Changelog

本文档记录 Cap - aniBullet Edition 各版本的主要变更。

> 格式遵循 [Keep a Changelog](https://keepachangelog.com/zh-CN/1.1.0/)，版本号遵循 [Semantic Versioning](https://semver.org/lang/zh-CN/)。

---

## [2.2.0] — 2026-06-01

### 移除
- 删除 `callback.template.ts`（OAuth 认证回调模板）
- 删除 `logging.rs`（云端日志上传空模块）
- 删除 `watermark.wgsl`（Pro 水印 Shader，无任何引用）
- 移除 `CommercialLicense` 结构体及 `commercial_license` 字段
- 移除 `createLicenseQuery`、`organizationId` 等云账号残留逻辑
- 移除付费限时录制死代码（`MAX_RECORDING_FOR_FREE`）
- 移除 `mysql2` 云数据库依赖（`package.json`）

### 清理
- 四语言 i18n（en / zh-CN / ja / ko）删除 `signingIn`、`editor.video.organization` 等云端死字符串
- 清理测试 / 示例中的 `sharing` / `upload` 残留字段

### 文档
- 重写 `CONTRIBUTING.md` 为本地版开发指南

### 修复
- 修复 `windows.rs` 中 macOS 专用方法在 Windows 构建时的 `dead_code` 警告

---

## [2.1.0] — 2026-05-xx

### 新增
- **HEVC 视频压缩**：库内右键菜单和详情面板均可触发本地压缩
- 详情面板支持原版 / 压缩版切换、复制文件、分别删除
- 导出页面新增**智能压缩** CRF 模式（HEVC CRF 28），大幅缩小文件体积
- 快速录制支持**自动压缩**开关（位于设置页）
- Grid / List / Compact 视图：无缩略图时自动降级为 video 预览 + 相对时间显示

### 改进
- 录制质量 BPP 参数上调，画质更清晰
- MediaFoundation 编码器增加峰值码率 / 缓冲区控制（`maxrate` / `bufsize`）
- NSIS 安装包支持静默卸载旧版后原地升级（`allowDowngrades`）
- `tauri:build` 默认使用生产配置（`tauri.prod.conf.json`）

### 国际化
- 四语言（en / zh-CN / ja / ko）补全压缩 / 导出相关 i18n 条目

---

## [2.0.2] — 2026-04-xx

### 改进
- **区域录制工具条**锁定在录制区域旁边，不再跑到其他屏幕
- `MirrorTexturePool`（3 纹理轮转）替代单一 GPU 纹理，消除全屏 / 裁剪路径撕裂
- `D3D11 Flush()` 确保 GPU 复制完成后再交给编码器
- Studio 模式读取用户质量设置（之前硬编码 `0.055 bpp`）
- 分段 Studio 路径同样传入用户码率参数
- H.264 / HEVC 编码器添加 VBV buffer（`maxrate=1.5x`，`bufsize=1x`），防冷启动马赛克
- Studio 强制使用 H.264 BPP 表（Studio 仅编码 H.264）
- `RecordingAreaState` 在所有录制退出路径统一清理

### 修复
- 窗口 / 区域模式添加远程桌面提示；移除"禁用内容保护"选项
- `tauri.ts` `JsonValue` 类型冲突
- Muxer 中强制单调递增 DTS
- 修复 `mediafoundation-ffmpeg` 预存 clippy 警告

---

## [2.0.1] — 2026-04-xx

### 修复
- Studio 模式切换为 **MediaFoundation GPU 编码**，彻底消除马赛克和编码超时
- WGC 捕获时复制纹理，消除灰帧
- D3D11 多线程保护，修复并发灰帧问题
- 键盘 Tab 与设置面板切换逻辑修复

---

## [2.0.0] — 2026-03-xx

### 新增（上游同步 v0.4.81）
- **键盘 Overlay**：实时显示按键操作，教学录制专属功能
- **字幕时间轴**：可视化字幕编辑面板，精准对齐视频
- **渐变编辑器**：Studio 背景渐变精细调节
- 录制帧率优化（时钟驱动帧节拍 + Buffer 复用）
- FFmpeg 依赖路径修正（从 `target/native-deps` 自动查找）

### 改进
- 上游导出管线优化（`crates/export`）
- 渲染 / 项目 / 编辑器 / 视频解码 / GPU 转换器同步至 v0.4.81

### 清理
- 移除全部上传、分享、云端相关代码（`fa11118a2`）

---

## [1.x] — 初始本地版

### 核心改造
- 基于 Cap v0.4.5 构建首个本地版本
- 移除时长 / 数量限制，打通完整离线录制流程
- 本地 SQLite 替代云端数据库
- 本地 Whisper.cpp AI 字幕
- 多语言界面（中 / 英 / 日 / 韩）
- 新增 HEVC 录制支持（`crates/enc-ffmpeg`）
- 重构构建脚本（PowerShell `1-install / 2-dev / 3-build`）
- 修复启动 / 库 / 编辑 / 打包 / 终端 / 右键 / 播放等多项初始问题

---

[2.2.0]: https://github.com/aniBullet/Cap-aniBullet/releases/tag/v2.2.0
[2.1.0]: https://github.com/aniBullet/Cap-aniBullet/releases/tag/v2.1.0
[2.0.2]: https://github.com/aniBullet/Cap-aniBullet/releases/tag/v2.0.2
[2.0.1]: https://github.com/aniBullet/Cap-aniBullet/releases/tag/v2.0.1
[2.0.0]: https://github.com/aniBullet/Cap-aniBullet/releases/tag/v2.0.0
