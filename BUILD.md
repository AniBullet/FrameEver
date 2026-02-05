# Windows 打包指南

本文档说明如何在 Windows 平台上构建 Cap aniBullet。

> **注意**：Cap aniBullet 仅支持 Windows 平台。我们不再维护 macOS 和 Linux 版本的构建。

## 📋 目录

- [本地打包](#本地打包)
- [GitHub Actions 自动打包](#github-actions-自动打包)
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

## 🤖 GitHub Actions 自动打包

### 使用方法

#### 方式一：推送 Git Tag 触发

1. 更新版本号（在 `apps/desktop/src-tauri/Cargo.toml`）：
```toml
[package]
version = "0.2.0"
```

2. 创建并推送 tag：
```bash
git add apps/desktop/src-tauri/Cargo.toml
git commit -m "chore: bump version to 0.2.0"
git push

git tag v0.2.0
git push origin v0.2.0
```

3. GitHub Actions 自动触发构建。

#### 方式二：手动触发

1. 前往仓库 GitHub 页面
2. 点击 `Actions` 标签
3. 选择 `Release Build` 工作流
4. 点击 `Run workflow` 按钮
5. 选择分支（通常是 `main`）
6. 点击 `Run workflow` 确认

### 构建流程

1. **读取版本号**：从 `Cargo.toml` 自动读取
2. **Windows 构建**：在 Windows runner 上构建 x64 版本
3. **上传 Artifacts**：构建产物保留 30 天
4. **创建草稿 Release**：自动附带安装包

### 下载构建产物

#### 从 Actions Artifacts 下载

1. 点击工作流运行记录
2. 滚动到底部 "Artifacts" 区域
3. 下载 Windows 压缩包

#### 从 Release 下载

1. 前往 `Releases` 页面
2. 找到对应版本的草稿 Release
3. 点击 "Edit" 编辑描述后发布
4. 用户可直接下载

### 构建时间

- **首次构建**：20-40 分钟（需下载依赖）
- **后续构建**：5-15 分钟（有 Rust 缓存）

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

### GitHub Actions 相关

**Q: 构建失败怎么办？**  
A: 
1. 查看 Actions 日志中的具体错误信息
2. 检查 `Cargo.toml` 版本号格式是否正确
3. 确保没有修改 `.github/workflows/release.yml` 中的关键配置

**Q: 为什么不支持其他平台？**  
A: Cap aniBullet 专注于 Windows 平台以简化维护。如果您需要 macOS 或 Linux 支持，建议使用原版 [Cap](https://github.com/CapSoftware/Cap)。

---

## 📚 参考资源

- [Tauri 官方文档](https://v2.tauri.app/)
- [Rust 安装指南](https://www.rust-lang.org/tools/install)
- [GitHub Actions 文档](https://docs.github.com/en/actions)
- [FFmpeg 官网](https://ffmpeg.org/)

---

## 🤝 贡献

如果你在打包过程中遇到问题或有改进建议，欢迎提交 Issue 或 Pull Request！
