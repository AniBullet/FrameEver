# Windows 构建指南

本文档说明如何在 Windows 平台上构建和开发 Cap aniBullet。

> **注意**：Cap aniBullet 仅支持 Windows 平台。如需 macOS / Linux 支持，请使用原版 [Cap](https://github.com/CapSoftware/Cap)。

## 📋 目录

- [前置要求](#前置要求)
- [开发模式](#开发模式)
- [生产构建](#生产构建)
- [常见问题](#常见问题)
- [参考资源](#参考资源)

---

## 前置要求

| 依赖 | 版本 | 说明 |
|------|------|------|
| Node.js | 20+ | [下载](https://nodejs.org/) |
| pnpm | 10+ | `npm install -g pnpm` |
| Rust | 1.88+ MSVC | [下载](https://www.rust-lang.org/tools/install)，选择 `x86_64-pc-windows-msvc` |
| Visual Studio 2022 Build Tools | — | 含 "C++ 桌面开发" 工作负载 |
| CMake | — | [Kitware 官方版本](https://cmake.org/download/)（非 VS 附带版） |
| LLVM / Clang | — | [下载](https://github.com/llvm/llvm-project/releases)，设置 `LIBCLANG_PATH` 环境变量 |
| FFmpeg | — | 由安装脚本自动下载 |
| vcpkg | — | 由安装脚本自动配置 |

---

## 开发模式

### 第一步：安装依赖（仅首次）

```powershell
.\scripts\1-install.ps1
```

脚本将自动：
- 下载并配置 FFmpeg（存放于 `~/.ffmpeg-dev`）
- 配置 vcpkg 和 native 依赖
- 设置必要的环境变量

> 安装完成后必须**重启终端**，环境变量才能生效。

### 第二步：启动开发服务器

```powershell
.\scripts\2-dev.ps1
```

等价命令（需在 `apps/desktop` 目录下执行）：

```bash
pnpm dev
```

开发服务器启动后会自动打开 Tauri 窗口，前端热更新生效，Rust 代码变更需重启。

### 常用开发命令

```bash
pnpm lint          # Biome 代码检查
pnpm format        # Biome 自动格式化
pnpm typecheck     # TypeScript 类型检查
cargo fmt          # Rust 代码格式化
cargo test -p <crate>  # 运行指定 crate 的测试
```

---

## 生产构建

```powershell
.\scripts\3-build.ps1
```

选择构建类型：
- `1` — **Development**（开发版，快速构建，含调试符号）
- `2` — **Production**（生产版，体积更小，推荐发布使用）

> **重要**：发布或测试安装包时，始终选择 Production 模式（使用 `tauri.prod.conf.json`）。

### 输出位置

```
apps/desktop/src-tauri/target/release/bundle/
├── nsis/
│   └── Cap_aniBullet_x.x.x_x64-setup.exe   ← 推荐（支持静默升级）
└── msi/
    └── cap-anibullet_x.x.x_x64_en-US.msi
```

安装包支持**静默覆盖升级**（`allowDowngrades`），无需先卸载旧版本。

---

## 常见问题

**Q: `cargo not found`**  
A: Rust 安装后需重启终端。检查 `rustup show` 是否输出工具链信息。

**Q: FFmpeg 链接错误**  
A: 确认已运行 `.\scripts\1-install.ps1` 并重启终端。检查 `~/.ffmpeg-dev` 目录是否存在。FFmpeg 应从 `target/native-deps` 自动查找，无需手动设置 PATH。

**Q: `LIBCLANG_PATH` 相关错误**  
A: 安装 LLVM 并设置环境变量：
```powershell
$env:LIBCLANG_PATH = "C:\Program Files\LLVM\bin"
```
或将其添加到系统环境变量中。

**Q: vcpkg 相关错误**  
A: 检查 `VCPKG_ROOT` 环境变量是否正确设置，重新运行安装脚本。

**Q: pnpm 找不到**  
A: 运行 `npm install -g pnpm`，然后重启终端。

**Q: 构建速度慢**  
A: 首次构建需下载并编译大量 Rust crate，耗时 10–30 分钟属正常。后续构建将利用 Cargo 缓存大幅提速。

**Q: 开发版和生产版有何区别？**  
A: Development 版包含调试符号、Tauri 开发工具，体积较大；Production 版使用 `tauri.prod.conf.json`，启用完整优化，为正式发布版本。

---

## 参考资源

- [Tauri v2 官方文档](https://v2.tauri.app/)
- [Rust 安装指南](https://www.rust-lang.org/tools/install)
- [LLVM 下载](https://github.com/llvm/llvm-project/releases)
- [CMake 官网](https://cmake.org/download/)

---

遇到文档中未覆盖的问题？欢迎[提交 Issue](https://github.com/AniBullet/Cap-aniBullet/issues/new)。
