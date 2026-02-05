# GitHub Workflows 说明

本目录包含 GitHub Actions 工作流配置文件。

## 🚀 可用工作流

### ✅ `release.yml` - 本地优先版发布构建
**用途**：自动化跨平台打包（Windows、macOS、Linux）

**特点**：
- ✅ 完全免费 - 无需云服务或付费订阅
- ✅ 无需密钥 - 跳过代码签名和公证
- ✅ 本地优先 - 专为离线使用设计

**触发方式**：
- 推送 Git tag: `git tag v0.1.0 && git push origin v0.1.0`
- 手动触发: Actions → Release Build → Run workflow

**输出**：
- 自动创建 GitHub Release 草稿
- 附带所有平台安装包（带平台标识）

---

### ✅ `ci.yml` - 持续集成检查
**用途**：代码质量检查（格式、类型、Clippy）

**特点**：
- 自动在 PR 和 main 分支推送时运行
- 检查 TypeScript 类型错误
- 检查 Rust Clippy 警告
- 检查代码格式（Biome + rustfmt）

**适用场景**：
- Pull Request 代码审查
- main 分支代码质量保证

---

### ⚠️ `publish.yml` - 原版发布流程（已弃用）
**状态**：保留但不推荐使用

**原因**：
- ❌ 依赖 CrabNebula Cloud（付费服务）
- ❌ 需要配置多个密钥（Apple 证书、签名密钥等）
- ❌ 包含 Sentry、Discord 等云端集成

**替代方案**：
使用 `release.yml` 进行本地优先的发布构建

---

### ⚠️ `opencode.yml` - OpenCode 集成（可选）
**状态**：可选功能

**用途**：
- 通过 Issue 评论触发 AI 代码审查
- 需要配置 `OPENCODE_API_KEY`

**建议**：
- 如不需要可删除此文件
- 或修改为其他 AI 代码审查工具

---

## 📝 建议调整

基于 aniBullet 的本地优先理念，建议：

1. **✅ 使用 `release.yml`**
   - 完全免费，无需云服务
   - 适合本地优先的项目

2. **✅ 保留 `ci.yml`**
   - 代码质量检查很重要
   - 不依赖任何付费服务

3. **⚠️ 禁用或删除 `publish.yml`**
   - 依赖付费云服务（CrabNebula）
   - 与本地优先理念不符
   - 建议重命名为 `publish.yml.disabled`

4. **⚠️ 可选删除 `opencode.yml`**
   - 需要 API 密钥
   - 如不使用可直接删除

---

## 🎯 推荐配置

对于 aniBullet 本地优先版本，推荐保留：
- ✅ `release.yml` - 自动化打包
- ✅ `ci.yml` - 代码质量检查

可以移除：
- ⚠️ `publish.yml` - 原版发布流程（含付费服务）
- ⚠️ `opencode.yml` - AI 代码审查（需 API 密钥）
