# Token Monitor — Agent Guide

## 项目定位

这是一个 Tauri v2 桌面悬浮组件。主任务是以最少视觉噪声显示当前本机账户额度：Codex 的 5 小时 / 7 天窗口，或 Cursor 的 First-party / API。主界面只保留两种可切换视图：`.dual-view` 与 `.focus-view`；标题栏提供 Codex / Cursor Provider 快切。

## 技术约束

- 前端保持静态 HTML/CSS/JavaScript；Rust 负责登录态读取、网络请求、托盘和原生窗口。
- 小型修改不得迁移到 React；UI 图标统一使用现有 Phosphor Icons；产品标记使用 `src-tauri/icons/icon.svg`；Provider 商标使用 `public/assets/brands/*.svg`（构建时由 Vite 复制到 `dist`）。
- 使用 `apply_patch` 编辑仓库源文件；不要删除、重置或覆盖用户无关改动。
- `dist/` 和 `src-tauri/target/` 是构建产物，不是设计源文件。

## 核心交互不变量

- `#drag-handle` 是唯一拖动区，标题栏按钮（含 `#provider-switch`）必须保持 `no-drag`。
- `.resize-handle` 覆盖四边和四角，并调用 Tauri 原生 `startResizeDragging`。
- 原生窗口最小尺寸为 `480 × 300`；透明画布与 `.widget-shell` 之间保留 12px 阴影空间。
- `.widget-shell` 必须保持 28px 连续圆角、内容裁切和透明外部区域，禁止重新引入 `windowEffects` 造成白色角块。
- `.dual-view` 与 `.focus-view` 共享同一份 Snapshot；切换视图不得触发伪刷新或改写数据。
- Provider 切换会刷新当前来源数据；缓存按 provider 分桶，不得串用对方 stale 快照。
- 中文 / English 必须覆盖新增文案、错误状态、Title 和无障碍名称。
- 不使用 Toast；状态变化使用顶部短条或控件自身状态，并可自动消失/隐藏。

## 数据与安全

### Codex（`LocalCodexOAuth`）

- 读取 `$CODEX_HOME/auth.json`，否则读取 `~/.codex/auth.json`。
- 允许访问 `https://chatgpt.com/backend-api/wham/usage`；这是非公开稳定接口，代码和文档必须明确这一风险。
- 仅允许 `chatgpt.com` / `chat.openai.com` 的 HTTPS 基址。
- 不刷新或写回 `auth.json`；401/403 提示重新执行 `codex login`。
- 5 小时和 7 天窗口必须按 `limit_window_seconds` 识别；窗口缺失时显示不可用。

### Cursor（`LocalCursorSession`）

- 只读本机 Cursor SQLite：`%APPDATA%\Cursor\User\globalStorage\state.vscdb`（macOS/Linux 对应路径）。
- 允许访问 `https://api2.cursor.sh` 的用量/鉴权端点（含 Connect RPC `GetCurrentPeriodUsage`）；同属非公开稳定接口。
- Access token 过期时允许**仅在内存中**用 refresh token 调 `/oauth/token`；禁止写回 `state.vscdb`。
- 双槽映射：`fiveHour` ← First-party（`planUsage.autoPercentUsed`）；`sevenDay` ← API（`planUsage.apiPercentUsed`）；缺失则不可用，不得造数。

### 共性

- Bearer Token、refresh token、ID token、账户 ID 和原始响应不得发送到 WebView、日志、导出或仓库。
- 不读取浏览器 Cookie，不抓取网页，不切换账户，不修改全局 Codex/Cursor/Shell 配置。
- 网络失败可显示该 Provider 本次运行中的最后成功快照。
- 浏览器 `?preview=1` 仅用于设计 QA，必须清晰标为预览；普通浏览器模式不得伪装真实数据。

## 本地开发与验证

```bash
npm run start
npm run build
cargo test --manifest-path src-tauri/Cargo.toml
npm run tauri:dev
```

变更后至少验证：

1. 双环/聚焦切换及重启恢复；标题栏 Codex ↔ Cursor 快切与设置同源同步。
2. 中文/英文、加载、未登录、过期、离线缓存、只返回单个窗口（含 Cursor 仅一侧百分比）。
3. 标题栏拖动、四边/四角缩放、最小尺寸、圆角透明边缘。
4. 手动刷新防抖、60 秒刷新、托盘刷新与摘要。
5. macOS/Windows 的位置恢复、始终置顶和登录时启动。
