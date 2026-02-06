# Windows 打包指南

本文档说明如何在 Windows 平台上构建 Cap aniBullet。

> **注意**：Cap aniBullet 仅支持 Windows 平台。我们不再维护 macOS 和 Linux 版本的构建。

## 📋 目录

- [本地打包](#本地打包)
- [常见问题](#常见问题)

---

## 🖥️ 本地打包

### 前置要求

- Node.js 20+
- Rust 1.88+ (MSVC toolchain)
- Visual Studio 2022 Build Tools
- CMake (Kitware 官方版本)
- FFmpeg
- vcpkg

### 构建步骤

#### 1. 一键安装依赖

```powershell
.\scripts\1-install.ps1
```

安装完成后**重启终端**。

#### 2. 构建应用

```powershell
.\scripts\3-build.ps1
```

选择构建类型：
- `1` - Development（开发版，快速构建）
- `2` - Production（生产版，推荐发布）

#### 3. 输出位置

```
apps/desktop/src-tauri/target/release/bundle/
├── nsis/
│   └── Cap_aniBullet_x.x.x_x64-setup.exe  ← 推荐
└── msi/
    └── cap-anibullet_x.x.x_x64_en-US.msi
```

---

## ❓ 常见问题

### 构建相关

**Q: 构建失败 "cargo not found"**  
A: Rust 安装后需重启终端才能生效。

**Q: FFmpeg 链接错误**  
A: 确保运行了 `.\scripts\1-install.ps1` 并且 `~/.ffmpeg-dev` 目录存在。

**Q: vcpkg 相关错误**  
A: 检查 `VCPKG_ROOT` 环境变量是否正确设置。

**Q: 构建速度慢**  
A: 首次构建会下载大量依赖，后续构建会利用 Cargo 缓存。


**Q: 为什么不支持其他平台？**  
A: Cap aniBullet 专注于 Windows 平台以简化维护。如果您需要 macOS 或 Linux 支持，建议使用原版 [Cap](https://github.com/CapSoftware/Cap)。

---

## 📚 参考资源

- [Tauri 官方文档](https://v2.tauri.app/)
- [Rust 安装指南](https://www.rust-lang.org/tools/install)
- [FFmpeg 官网](https://ffmpeg.org/)

---

## 🤝 贡献

如果你在打包过程中遇到问题或有改进建议，欢迎提交 Issue 或 Pull Request！
