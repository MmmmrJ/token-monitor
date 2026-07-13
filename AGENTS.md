# Codex Usage Monitor — Agent Guide

## 项目定位

这是一个 Tauri v2 桌面悬浮组件。主任务是以最少视觉噪声显示当前本机 Codex 账户的 5 小时额度、7 天额度和重置时间。主界面只保留两种可切换视图：`.dual-view` 与 `.focus-view`。

## 技术约束

- 前端保持静态 HTML/CSS/JavaScript；Rust 负责登录态读取、网络请求、托盘和原生窗口。
- 小型修改不得迁移到 React；图标统一使用现有 Phosphor Icons，产品标记使用 `src-tauri/icons/icon.svg`。
- 使用 `apply_patch` 编辑仓库源文件；不要删除、重置或覆盖用户无关改动。
- `dist/` 和 `src-tauri/target/` 是构建产物，不是设计源文件。

## 核心交互不变量

- `#drag-handle` 是唯一拖动区，标题栏按钮必须保持 `no-drag`。
- `.resize-handle` 覆盖四边和四角，并调用 Tauri 原生 `startResizeDragging`。
- 原生窗口最小尺寸为 `480 × 300`；透明画布与 `.widget-shell` 之间保留 12px 阴影空间。
- `.widget-shell` 必须保持 28px 连续圆角、内容裁切和透明外部区域，禁止重新引入 `windowEffects` 造成白色角块。
- `.dual-view` 与 `.focus-view` 共享同一份 Snapshot；切换视图不得触发伪刷新或改写数据。
- 中文 / English 必须覆盖新增文案、错误状态、Title 和无障碍名称。
- 不使用 Toast；状态变化使用顶部短条或控件自身状态，并可自动消失/隐藏。

## 数据与安全

- 默认 Provider 是只读 `LocalCodexOAuth`：读取 `$CODEX_HOME/auth.json`，否则读取 `~/.codex/auth.json`。
- 允许访问 Codex 使用的 `https://chatgpt.com/backend-api/wham/usage`；这是非公开稳定接口，代码和文档必须明确这一风险。
- Bearer Token、refresh token、ID token、账户 ID 和原始响应不得发送到 WebView、日志、导出或仓库。
- 仅允许 `chatgpt.com` / `chat.openai.com` 的 HTTPS 基址；不得把登录 Token 发送到任意配置主机。
- 不读取浏览器 Cookie，不抓取网页，不刷新或写回 `auth.json`，不切换账户，不修改全局 Codex/Shell 配置。
- 5 小时和 7 天窗口必须按 `limit_window_seconds` 识别；窗口缺失时显示不可用，不得复制另一窗口或填充 Demo。
- 401/403 显示重新执行 `codex login`；网络失败可显示本次运行中的最后成功快照。
- 浏览器 `?preview=1` 仅用于设计 QA，必须清晰标为预览；普通浏览器模式不得伪装真实数据。

## 本地开发与验证

```bash
npm run start
npm run build
cargo test --manifest-path src-tauri/Cargo.toml
npm run tauri:dev
```

变更后至少验证：

1. 双环/聚焦切换及重启恢复。
2. 中文/英文、加载、未登录、过期、离线缓存、只返回单个窗口。
3. 标题栏拖动、四边/四角缩放、最小尺寸、圆角透明边缘。
4. 手动刷新防抖、60 秒刷新、托盘刷新与摘要。
5. macOS/Windows 的位置恢复、始终置顶和登录时启动。
