# Token Monitor

> 一款面向 Windows 与 macOS 的轻量桌面悬浮额度监视器，支持 Codex 和 Cursor。

**Token Monitor** is a compact, privacy-first Tauri desktop widget for monitoring local Codex and Cursor quota windows. It reads the existing local sign-in session and never asks users to paste tokens into the UI.

[![Latest release](https://img.shields.io/github/v/release/MmmmrJ/token-monitor?display_name=tag)](https://github.com/MmmmrJ/token-monitor/releases)
[![Release desktop installers](https://github.com/MmmmrJ/token-monitor/actions/workflows/release.yml/badge.svg)](https://github.com/MmmmrJ/token-monitor/actions/workflows/release.yml)
![Platforms](https://img.shields.io/badge/platform-Windows%20%7C%20macOS-5f8cff)
![Tauri](https://img.shields.io/badge/Tauri-v2-35d9ff)

## 项目背景

Codex 与 Cursor 都会根据账户方案提供不同的用量窗口，但开发过程中频繁切换客户端或网页查看剩余额度会打断工作流。Token Monitor 将这些信息收拢到一个低视觉噪声的桌面悬浮窗口中，并直接复用当前计算机已经存在的登录状态。

项目坚持以下原则：

- **本机优先**：只读取本机 Codex 或 Cursor 的登录状态。
- **最小权限**：不读取浏览器 Cookie，不要求粘贴 Token，不切换账户。
- **数据透明**：服务端缺少某个窗口时显示不可用，不复制其他窗口或生成演示数据。
- **低打扰**：紧凑窗口、系统托盘、双视图和克制动效，不占用主要工作空间。

## 核心功能

- 标题栏快速切换 Codex / Cursor Provider，并与设置面板保持同步。
- Codex 显示 5 小时和 7 天额度窗口。
- Cursor 中文界面显示订阅额度（First-party）和 API 额度（API）用量。
- 提供 `.dual-view` 双环视图和 `.focus-view` 聚焦视图，选择会在重启后恢复。
- 显示剩余百分比、重置时间、最近窗口倒计时，以及实时、部分可用、离线缓存和具体失败原因。
- 渐变圆环和横向进度条平滑更新，并遵循系统“减少动态效果”设置。
- 60 秒自动刷新、手动刷新防抖、Provider 独立的本次运行缓存。
- 支持中文 / English、始终置顶、登录时启动、系统托盘和窗口位置恢复。
- 无边框透明窗口、28px 连续圆角、标题栏拖动和八方向原生缩放。

## 数据来源

| Provider | 本机登录来源 | 展示窗口 |
| --- | --- | --- |
| Codex | `$CODEX_HOME/auth.json` 或 `~/.codex/auth.json` | 5 小时、7 天 |
| Cursor | `Cursor/User/globalStorage/state.vscdb` | 订阅额度（First-party）、API 额度（API） |

应用只会展示服务端真实返回的可识别窗口。账户没有下发某个窗口时，该位置会显示“当前账户未提供”。

## 下载与安装

请从项目的 [GitHub Releases](https://github.com/MmmmrJ/token-monitor/releases) 页面下载安装包，**不要从 Tags 页面下载 Source code 压缩包**。Tags 中的 ZIP/TAR.GZ 只是源代码，不是桌面应用。

### Windows

支持 Windows x64：

- `*-setup.exe`：NSIS 安装程序，普通用户推荐使用。
- `*.msi`：Windows Installer 安装包，适合企业部署或统一安装。

下载后运行安装程序即可。当前公开构建未配置商业代码签名，Windows SmartScreen 可能显示“Windows 已保护你的电脑”；请确认文件来自本仓库 Release 后再选择继续运行。

### macOS

下载 Universal `.dmg`，同时支持 Apple Silicon 和 Intel Mac。将应用拖入 Applications 后启动。

当前构建未配置 Apple Developer ID 公证。首次打开如果被 Gatekeeper 阻止，可在“系统设置 → 隐私与安全性”中确认允许来自本项目的应用。

### 登录要求

使用 Codex 前，确认 Codex CLI 已通过 ChatGPT 账户登录：

```bash
codex login
```

使用 Cursor 前，先在 Cursor 桌面客户端中正常登录。Token Monitor 不提供账户登录、切换或 Token 输入界面。

## 使用说明

- 点击标题栏中的 Codex / Cursor 图标切换额度来源；切换会刷新当前 Provider 数据。
- 点击布局按钮在双环和聚焦视图之间切换。
- 点击刷新按钮手动请求最新额度；短时间连续点击会被防抖。
- 点击齿轮设置语言、额度来源、始终置顶、登录时启动和额度提醒；登录时启动与提醒状态以原生返回为准，失败时在设置项内显示错误。
- 窗口隐藏到托盘后仍由后台协调器按约 60 秒刷新；前端不再依赖页面定时器维持监控。
- 关闭主窗口（含 Windows 关闭按钮、Alt+F4、macOS Cmd+W）会隐藏到系统托盘，不会结束进程；托盘“退出”或 macOS Cmd+Q 才会真正退出。
- 重复启动会复用已有进程并恢复主窗口，不会产生第二个实例。
- 关闭主窗口后可通过系统托盘查看摘要、切换来源/视图、刷新、置顶、打开设置或退出。
- 发现更新时顶部短条提示；下载、安装与重启需用户确认。

### 从旧版升级

- 若本机仍保留旧名称 `Codex Usage Monitor` 的登录启动项，启动时会先创建 `Token Monitor` 启动项并验证成功，再删除旧项。
- 从 v1.1.x 升级到 v1.2.0 请先手动安装；自 v1.2.0 起可通过应用内更新升级到后续版本（需 Release 提供已签名 updater 资产与 `latest.json`）。

### 维护者：Updater 签名 Secrets

应用内更新需要 GitHub Secrets（私钥勿提交仓库）：

- `TAURI_SIGNING_PRIVATE_KEY`：私钥文件内容（本机生成于 `~/.tauri/token-monitor.key`）
- `TAURI_SIGNING_PRIVATE_KEY_PASSWORD`：生成密钥时使用的密码

公钥已写入 `src-tauri/tauri.conf.json` 的 `plugins.updater.pubkey`。未配置私钥 Secrets 时，Release workflow 会拒绝发布 updater 资产。

## 数据与安全边界

Token Monitor 使用的额度接口属于对应客户端正在使用的内部端点，**不是承诺长期稳定的公开第三方 API**。Codex、Cursor 或其服务提供方可能随时调整响应结构、鉴权方式或可用性。

### Codex

```text
GET https://chatgpt.com/backend-api/wham/usage
Authorization: Bearer <local Codex access token>
ChatGPT-Account-Id: <local account id>
```

- Token 仅由 Rust 从本机 `auth.json` 临时读取，不写回或刷新该文件。
- HTTPS 基址只允许 `chatgpt.com` 与 `chat.openai.com`。
- 自定义基址必须精确匹配允许域名、默认 HTTPS 端口和 `/backend-api` 路径；请求禁止跟随重定向。
- 401/403 时提示重新执行 `codex login`。

### Cursor

```text
POST https://api2.cursor.sh/aiserver.v1.DashboardService/GetCurrentPeriodUsage
Authorization: Bearer <local Cursor access token>
```

- Cursor SQLite 以只读方式打开；不会修改 `state.vscdb`。
- Access token 过期时，只允许在内存中使用 refresh token 调用 `/oauth/token`，不会写回数据库。
- 网络请求只发送到 `https://api2.cursor.sh`。
- 主界面只映射 `planUsage.autoPercentUsed` 与 `planUsage.apiPercentUsed`；字段缺失时显示不可用，不使用金额、总百分比或 legacy 请求桶补数。
- `billingCycleEnd` 缺失时显示“无重置时间”，不会生成当前时间或固定 30 天后的占位值。
- 401/403 或刷新失败时提示在 Cursor 中重新登录。

### 共性约束

- Bearer Token、refresh token、ID token、账户 ID 和原始响应不会发送到 WebView、日志、导出或仓库。
- 不读取浏览器 Cookie，不抓取网页，不修改 Codex、Cursor 或 Shell 全局配置。
- 网络失败时只会显示该 Provider 在本次运行中的最后成功快照，不跨 Provider 复用缓存。
- `?preview=1` 仅用于本地设计验收，并会明确标记预览数据。

## 本地开发

项目使用静态 HTML/CSS/JavaScript 前端与 Tauri v2 Rust 后端，不依赖 React。

### 通用依赖

- Node.js 18 或更高版本
- npm
- Rust stable
- 当前平台的 Tauri v2 系统依赖

完整平台依赖说明可参考 [Tauri v2 Prerequisites](https://v2.tauri.app/start/prerequisites/)。

### Windows 开发环境

安装 Rust：

```powershell
winget install --source winget --id Rustlang.Rustup --exact
```

安装 Visual Studio C++ Build Tools：

```powershell
winget install --source winget --id Microsoft.VisualStudio.2022.BuildTools --exact --override "--wait --passive --add Microsoft.VisualStudio.Workload.VCTools --includeRecommended"
```

Windows 11 通常已包含 WebView2 Runtime；其他系统可以从 [Microsoft WebView2](https://developer.microsoft.com/microsoft-edge/webview2/) 安装 Evergreen Runtime。安装工具链后重新打开终端，并确认：

```powershell
cargo --version
rustc --version
```

### macOS 开发环境

```bash
xcode-select --install
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
```

### 启动项目

```bash
git clone https://github.com/MmmmrJ/token-monitor.git
cd token-monitor
npm ci
npm run tauri:dev
```

仅验证前端布局时运行：

```bash
npm run start
```

普通浏览器模式不会读取真实本机登录文件。设计验收可访问 Vite 地址并添加 `?preview=1`。

### 随 Codex CLI 启动

包装器会优先唤起已安装的 Token Monitor，并兼容旧版应用名称；开发环境找不到已安装应用时，会回退到 `tauri:dev`，然后原样启动 Codex CLI：

```bash
npm run codex --
npm run codex -- --monitor-only
```

## 构建安装包

Windows：

```powershell
npm run tauri:build -- --bundles nsis,msi
```

macOS：

```bash
npm run tauri:build -- --bundles dmg
```

构建产物位于：

```text
src-tauri/target/release/bundle/
```

常规验证命令：

```bash
npm test
npm run check:versions
npm run build
cargo test --manifest-path src-tauri/Cargo.toml
cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets -- -D warnings
```

## 自动发布到 GitHub

`.github/workflows/release.yml` 会在推送 `v*` 标签后，自动构建 Windows x64 MSI/NSIS 和 macOS Universal DMG，并将产物上传至 GitHub Releases。

发布新版本前必须同步以下版本号：

- `package.json`
- `package-lock.json`
- `src-tauri/tauri.conf.json`
- `src-tauri/Cargo.toml`
- `src-tauri/Cargo.lock`

然后提交并推送对应标签：

```bash
git tag -a vX.Y.Z -m "Token Monitor vX.Y.Z"
git push origin main
git push origin vX.Y.Z
```

如果 Releases 页面暂时只有源代码压缩包，通常表示工作流仍在运行或构建失败。请前往 [Actions](https://github.com/MmmmrJ/token-monitor/actions) 查看状态；安装包只会在发布任务成功后出现。

## 常见问题

### GitHub 上为什么只有 Source code？

你可能打开了 Tags 页面，或者 Release workflow 尚未完成。请从 [Releases](https://github.com/MmmmrJ/token-monitor/releases) 下载，并检查 Actions 状态。

### 为什么额度显示不可用？

- Codex：执行 `codex login` 后重新打开或刷新应用。
- Cursor：确认 Cursor 桌面客户端已经登录。
- 某些账户本身不会返回全部窗口，缺失窗口不会被推算或补齐。

### 为什么 Windows 构建提示找不到 cargo？

Rust 尚未安装或 `%USERPROFILE%\.cargo\bin` 没有进入当前终端的 PATH。安装 Rustup 后关闭并重新打开终端。

### 为什么系统提示应用来源未知？

当前公开构建没有配置 Apple 公证或 Windows 商业代码签名。请只从本仓库 Releases 下载，并自行核对来源。

## 参与贡献

欢迎通过 Issues 报告问题或提出建议，也欢迎提交 Pull Request。提交问题时请提供操作系统、应用版本、Provider 和可复现步骤；**不要上传 `auth.json`、`state.vscdb`、Token、账户 ID 或原始接口响应**。

Pull Request 建议至少完成：

```bash
npm test
npm run check:versions
npm run build
cargo test --manifest-path src-tauri/Cargo.toml
cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets -- -D warnings
```

## 声明

本项目是独立的开源工具，与 OpenAI、Codex、Cursor 或 Anysphere 没有官方隶属或背书关系。相关名称与商标归各自权利人所有，仅用于说明兼容的数据来源。
