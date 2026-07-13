# Codex Usage Monitor

一个面向 Windows / macOS 的紧凑型 Tauri 桌面悬浮组件，用来查看**当前本机 Codex 登录账户**返回的 5 小时额度、7 天额度和重置时间。

## 当前能力

- 自动读取 `$CODEX_HOME/auth.json`，未设置时读取 `~/.codex/auth.json`。
- 使用本机 Codex OAuth 登录态查询额度；无需再填写 Admin API Key。
- 按 `limit_window_seconds` 识别 5 小时（18,000 秒）和 7 天（604,800 秒）窗口，不依赖 API 字段顺序。
- 同时提供「双环」与「5 小时聚焦」两套 UI，右上角按钮或设置面板可即时切换并记住选择。
- 显示各窗口剩余百分比、准确重置时间和最近窗口的倒计时。
- 某个窗口未下发时明确显示“当前账户未提供”，不会填充 Demo 或推算数据。
- 60 秒自动刷新，手动刷新有 10 秒防抖；失败时保留本次运行中的最后成功快照。
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

## 如何切换两种 UI

- 点击标题栏右上角第一个布局按钮，在「双环」和「聚焦」之间快速切换。
- 或点击齿轮，在“展示样式”中直接选择。
- 选择保存在本机 Local Storage 中，重启后自动恢复。

## 数据与安全边界

本版本应用户要求，不再局限于公开的组织 Usage API。它采用 Codex 客户端正在使用的本机 OAuth 登录态和额度端点：

```text
GET https://chatgpt.com/backend-api/wham/usage
Authorization: Bearer <local Codex access token>
ChatGPT-Account-Id: <local account id>
```

这是 Codex 客户端内部使用的数据面，不是承诺长期稳定的公开第三方 API，OpenAI 可能随时调整响应或鉴权。项目采取以下限制：

- Token 仅由 Rust 进程从 `auth.json` 临时读取；前端、日志、托盘和导出内容均拿不到 Token。
- 不读取浏览器 Cookie，不抓取网页，不保存或复制 access/refresh token。
- 首版不会刷新或改写 `auth.json`；遇到 401/403 时提示用户重新执行 `codex login`。
- 只信任 `https://chatgpt.com` / `https://chat.openai.com` 的 HTTPS 地址，避免把 Token 发送给任意自定义主机。
- 不切换账户，不替换全局 Codex 会话，不修改 Shell 配置。

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
