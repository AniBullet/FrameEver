# 跨平台打包指南

本文档详细说明如何在 Windows、macOS、Linux 三个平台上构建 Cap aniBullet。

## 📋 目录

- [本地打包](#本地打包)
  - [Windows](#windows-打包)
  - [macOS](#macos-打包)
  - [Linux](#linux-打包)
- [GitHub Actions 自动打包](#github-actions-自动打包)
- [常见问题](#常见问题)

---

## 🖥️ 本地打包

### Windows 打包

#### 前置要求
- Node.js 20+
- Rust 1.88+ (MSVC toolchain)
- Visual Studio 2022 Build Tools
- CMake (Kitware 官方版本)
- FFmpeg
- vcpkg

#### 构建步骤

1. **一键安装依赖**：
```powershell
.\1-install.ps1
```
安装完成后**重启终端**。

2. **构建应用**：
```bash
./3-build.sh
```
或直接调用：
```powershell
.\scripts\build-windows.ps1
```

选择构建类型：
- `1` - Development（开发版，快速构建）
- `2` - Production（生产版，推荐发布）

3. **输出位置**：
```
apps/desktop/src-tauri/target/release/bundle/
├── nsis/
│   └── Cap_aniBullet_x.x.x_x64-setup.exe  ← 推荐
└── msi/
    └── cap-anibullet_x.x.x_x64_en-US.msi
```

---

### macOS 打包

#### 前置要求
```bash
brew install ffmpeg cmake rust node@20 pnpm
```

#### 构建步骤

1. **安装项目依赖**：
```bash
pnpm install
```

2. **构建应用**：
```bash
./3-build.sh
```
或直接调用：
```bash
chmod +x scripts/build-macos.sh
./scripts/build-macos.sh
```

3. **输出位置**（自动检测架构）：
```
apps/desktop/src-tauri/target/[aarch64|x86_64]-apple-darwin/release/bundle/
└── dmg/
    └── Cap_aniBullet_x.x.x_[aarch64|x86_64].dmg
```

#### 注意事项
- **未签名版本**：需右键点击 "打开"，或执行：
```bash
xattr -cr "/Applications/Cap aniBullet.app"
```
- **架构**：脚本自动检测 Intel / Apple Silicon

---

### Linux 打包

#### 前置要求（Ubuntu/Debian）
```bash
sudo apt update
sudo apt install -y \
  libwebkit2gtk-4.1-dev build-essential curl wget file \
  libssl-dev libgtk-3-dev libayatana-appindicator3-dev \
  librsvg2-dev libavutil-dev libavcodec-dev libavformat-dev \
  libavfilter-dev libavdevice-dev libswscale-dev \
  libswresample-dev ffmpeg

curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
curl -fsSL https://deb.nodesource.com/setup_20.x | sudo -E bash -
sudo apt install -y nodejs
npm install -g pnpm@10.5.2
```

#### 构建步骤

1. **安装项目依赖**：
```bash
pnpm install
```

2. **构建应用**：
```bash
./3-build.sh
```
或直接调用：
```bash
chmod +x scripts/build-linux.sh
./scripts/build-linux.sh
```

3. **输出位置**：
```
apps/desktop/src-tauri/target/release/bundle/
├── deb/
│   └── cap-anibullet_x.x.x_amd64.deb        ← 推荐（Debian/Ubuntu）
└── appimage/
    └── cap-anibullet_x.x.x_amd64.AppImage   ← 通用格式
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
2. **并行构建**：同时在 4 个平台上构建
   - Windows x64
   - macOS Intel
   - macOS Apple Silicon
   - Linux x64
3. **上传 Artifacts**：构建产物保留 30 天
4. **创建草稿 Release**：自动附带所有安装包

### 下载构建产物

#### 从 Actions Artifacts 下载
1. 点击工作流运行记录
2. 滚动到底部 "Artifacts" 区域
3. 下载对应平台的压缩包

#### 从 Release 下载
1. 前往 `Releases` 页面
2. 找到对应版本的草稿 Release
3. 点击 "Edit" 编辑描述后发布
4. 用户可直接下载

### 构建时间

- **首次构建**：30-60 分钟（需下载依赖）
- **后续构建**：10-20 分钟（有 Rust 缓存）

---

## ❓ 常见问题

### Windows 相关

**Q: 构建失败 "cargo not found"**  
A: Rust 安装后需重启终端才能生效。

**Q: FFmpeg 链接错误**  
A: 确保运行了 `.\1-install.ps1` 并且 `~/.ffmpeg-dev` 目录存在。

**Q: vcpkg 相关错误**  
A: 检查 `VCPKG_ROOT` 环境变量是否正确设置。

### macOS 相关

**Q: "App is damaged and can't be opened"**  
A: 执行 `xattr -cr "/path/to/Cap aniBullet.app"` 移除隔离属性。

**Q: 构建速度慢**  
A: 首次构建会下载大量依赖，后续构建会利用 Cargo 缓存。

**Q: 如何构建 Universal Binary？**  
A: 当前不支持，需分别构建 Intel 和 Apple Silicon 版本。

### Linux 相关

**Q: "webkit2gtk not found"**  
A: 安装 `libwebkit2gtk-4.1-dev`（Ubuntu 22.04+）或 `libwebkit2gtk-4.0-dev`（旧版本）。

**Q: AppImage 无法运行**  
A: 添加可执行权限：`chmod +x cap-anibullet_*.AppImage`

**Q: Fedora/RHEL 依赖**  
A: 使用 `dnf` 安装对应的 `-devel` 包。

### GitHub Actions 相关

**Q: 构建失败怎么办？**  
A: 
1. 查看 Actions 日志中的具体错误信息
2. 检查 `Cargo.toml` 版本号格式是否正确
3. 确保没有修改 `.github/workflows/release.yml` 中的关键配置

**Q: macOS 构建失败 "security framework error"**  
A: 这是正常的，因为我们没有配置代码签名。安装包仍会生成，只是未签名。

**Q: 如何添加代码签名？**  
A: 需要 Apple Developer 账号和证书，参考原仓库 `.github/workflows/publish.yml` 中的签名配置（需付费）。

---

## 📚 参考资源

- [Tauri 官方文档](https://v2.tauri.app/)
- [Rust 安装指南](https://www.rust-lang.org/tools/install)
- [GitHub Actions 文档](https://docs.github.com/en/actions)
- [FFmpeg 官网](https://ffmpeg.org/)

---

## 🤝 贡献

如果你在打包过程中遇到问题或有改进建议，欢迎提交 Issue 或 Pull Request！
