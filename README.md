# Codex Usage Monitor

一个面向 Windows / macOS 的紧凑型 Tauri 桌面悬浮组件，用来查看本机 **Codex**（5 小时 / 7 天）或 **Cursor**（Included / On-demand）额度。

## 当前能力

- 标题栏 Codex / Cursor 商标图标快切，设置面板同源切换；选择会持久化。
- **Codex**：自动读取 `$CODEX_HOME/auth.json`，未设置时读取 `~/.codex/auth.json`；按 `limit_window_seconds` 识别 5 小时与 7 天窗口。
- **Cursor**：只读本机 `state.vscdb` 登录态，查询周期 Included 与 On-demand 额度；无需粘贴 Cookie 或 API Key。
- 同时提供「双环」与「聚焦」两套 UI，右上角按钮或设置面板可即时切换并记住选择。
- 显示各窗口剩余百分比、重置时间（Cursor 可附带美元/请求用量提示）和最近窗口倒计时。
- 某个窗口未下发时明确显示“当前账户未提供”，不会填充 Demo 或推算数据。
- 60 秒自动刷新，手动刷新有 10 秒防抖；失败时按 Provider 保留本次运行中的最后成功快照。
- 无边框透明窗口、28px 圆角、标题栏拖动、四边/四角原生缩放、始终置顶、登录时启动、托盘摘要和窗口位置恢复。
- 中文 / English 完整切换。

## 运行真实数据

要求：Node.js 18+、npm、Rust stable，以及当前平台的 Tauri 构建依赖。

1. 先确认 Codex CLI 已用 ChatGPT 账户登录：

   ```bash
   codex login
   ```

2. 安装依赖并启动桌面应用：

   ```bash
   npm install
   npm run tauri:dev
   ```

应用启动后会自动读取本机登录态。无需复制 Token，也不要把 `auth.json` 内容粘贴到设置页或 Issue 中。

仅验证前端布局时可以运行：

```bash
npm run start
```

浏览器中的普通地址不会读取本机登录文件；`?preview=1` 只用于本地设计验收，并会明确标记“设计预览数据”。真实数据只在 Tauri 桌面进程中启用。

## 如何切换 Provider 与 UI

- 标题栏布局按钮左侧的 Codex / Cursor 商标图标：一点即切换额度来源并刷新。
- 或点击齿轮，在“额度来源”中选择；与标题栏状态同步。
- 布局按钮在「双环」和「聚焦」之间切换；选择与 Provider 一并保存在本机 Local Storage，重启后自动恢复。

## 数据与安全边界

本应用读取本机登录态并调用各客户端正在使用的额度端点。这些端点**不是**承诺长期稳定的公开第三方 API，提供方可能随时调整响应或鉴权。

### Codex

```text
GET https://chatgpt.com/backend-api/wham/usage
Authorization: Bearer <local Codex access token>
ChatGPT-Account-Id: <local account id>
```

- Token 仅由 Rust 从 `auth.json` 临时读取；不写回、不刷新该文件。
- 仅信任 `https://chatgpt.com` / `https://chat.openai.com`。
- 401/403 时提示重新执行 `codex login`。

### Cursor

```text
POST https://api2.cursor.sh/aiserver.v1.DashboardService/GetCurrentPeriodUsage
Authorization: Bearer <local Cursor access token>
```

- Token 仅由 Rust 从 Cursor `state.vscdb` **只读**读取；前端、日志、托盘均拿不到 Token。
- Access token 过期时，可在内存内用 refresh token 调用 `/oauth/token`，**不会**写回 SQLite。
- 仅信任 `https://api2.cursor.sh`。
- 401/403 或 refresh 失败时提示在 Cursor 中重新登录。

### 共性

- 不读取浏览器 Cookie，不抓取网页，不保存或复制 access/refresh token。
- 不切换账户，不修改全局 Codex / Cursor / Shell 配置。
- Codex / Cursor 商标仅用于来源识别，归属各自权利人。

## 随 Codex CLI 启动

包装器会优先唤起已安装的原生 Codex Usage Monitor；开发环境找不到已安装应用时，会回退到 `tauri:dev`，然后原样启动 Codex CLI：

```bash
npm run codex --
npm run codex -- --monitor-only
```

设置里的“登录时启动”用于让 Monitor 常驻。应用不会注入或监听 Codex Desktop 私有进程。

## 构建发布包

```bash
npm run tauri:build
```

产物位于 `src-tauri/target/release/bundle/`。正式分发仍需配置 macOS 签名/公证和 Windows 代码签名。

## 下载桌面安装包

版本标签推送到 GitHub 后，发布工作流会自动构建并上传以下安装包到 [GitHub Releases](https://github.com/MmmmrJ/token-monitor/releases)：

- macOS：通用架构 `.dmg`，同时支持 Apple Silicon 与 Intel Mac。
- Windows：x64 `.msi` 和 NSIS `-setup.exe`。

当前公开构建未配置 Apple Developer ID 公证或 Windows 商业代码签名。macOS 首次打开时可能需要在“隐私与安全性”中确认允许；Windows 可能显示 SmartScreen 提示。正式对外分发前应配置两端签名证书。

创建新版本时，先将 `package.json`、`src-tauri/tauri.conf.json` 和 `src-tauri/Cargo.toml` 的版本保持一致，再推送对应标签。例如：

```bash
git tag -a v1.0 -m "Codex Usage Monitor v1.0"
git push origin v1.0
```

## 验证

```bash
npm run build
cargo test --manifest-path src-tauri/Cargo.toml
```

原生验收还应覆盖：真实账户响应、登录过期、单窗口缺失、窗口拖动/八方向缩放、多显示器位置恢复、始终置顶、托盘刷新、开机启动和两种语言。
