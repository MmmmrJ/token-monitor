# Token Monitor 开发路线图

> 状态：已确认，待实施  
> 基线版本：`v1.1.1`  
> 规划周期：未来 3 个版本，约 12 周  
> 平台策略：Windows / macOS 同等支持与验收  
> 最后更新：2026-07-15

## 1. 产品方向

Token Monitor 继续定位为一个轻量、低噪声、隐私优先的 Codex / Cursor 桌面额度悬浮组件。

未来三个版本不追求竞品的“大而全”，而是依次解决：

1. **可信**：额度来源安全、窗口映射准确、Provider 不串屏、错误状态可理解。
2. **常驻**：窗口隐藏后仍可靠刷新，通过托盘和克制的原生通知提供价值。
3. **可维护**：用户可以稳定升级，维护者可以获得不含敏感信息的诊断摘要。

必须继续遵守以下边界：

- 前端保持静态 HTML/CSS/JavaScript，不迁移 React。
- Rust 负责登录态、网络、缓存、后台调度、托盘、通知和原生窗口。
- 主界面只保留 `.dual-view` 与 `.focus-view`。
- 不读取浏览器 Cookie，不要求用户粘贴 Token。
- 不导入、切换或改写 Codex/Cursor 账户。
- 不把 Token、账户 ID、完整本机路径或原始接口响应发送到 WebView、日志、诊断或仓库。
- Codex/Cursor 接口均视为非公开稳定接口；上游变化必须表现为明确错误或 partial/stale 状态，不能造数。

## 2. 竞品研究结论

参考项目：[Javis603/token-monitor](https://github.com/Javis603/token-monitor)。

### 2.1 值得借鉴

- 应用内版本与更新入口。
- 托盘快捷切换、摘要和重置倒计时。
- 可选额度提醒。
- 清晰分组的设置与来源健康状态。
- 跨平台 CI、Release 资产完整性校验和自动更新元数据。

### 2.2 不应照搬

- 20+ Provider 扩张。
- Token/费用扫描、会话解析和大型历史 Dashboard。
- 多设备 Hub、Cloudflare Worker 和 iOS Widget。
- 浏览器 Session Token 粘贴或普通 JSON 凭据存储。
- Codex 多账户切换及 `auth.json` 重写。
- 高频、接近日更的发布节奏。

### 2.3 公开反馈信号

竞品的功能广度同时扩大了性能与发布风险：

- [M4 Max 上出现卡顿与约 2.2GB 内存占用](https://github.com/Javis603/token-monitor/issues/155)。
- [Windows 安装新版本后仍启动旧版本](https://github.com/Javis603/token-monitor/issues/115)。
- [用户要求 Stable / Beta 双通道以降低高频更新干扰](https://github.com/Javis603/token-monitor/issues/149)。
- [托盘百分比缺少重置倒计时，难以判断是否需要等待](https://github.com/Javis603/token-monitor/issues/133)。
- [登录状态缺失但没有明确登录入口](https://github.com/Javis603/token-monitor/issues/154)。

由此确定 Token Monitor 的差异化：**更小、更准、更可信，而不是功能最多。**

## 3. 版本总览

| 版本 | 主题 | 目标周期 | 发布门槛 |
| --- | --- | --- | --- |
| `v1.1.2` | 安全与可信度热修 | 1–2 周 | 安全、竞态、重置时间、窗口生命周期和 CI 全部完成 |
| `v1.2.0` | 后台监控、托盘与额度提醒 | 3–5 周 | 隐藏窗口持续刷新、通知不重复、双平台性能达标 |
| `v1.3.0` | 更新闭环与安全诊断 | 3–4 周 | 从前两版升级成功，Release 资产与更新签名完整 |

## 4. v1.1.2 — 安全与可信度热修

### 4.1 Codex URL 安全校验

当前 `configured_usage_url` 使用字符串前缀判断域名，必须优先修复，避免伪装域名获得 Bearer Token。

实施要求：

- 使用 URL 解析器，不再使用 `starts_with` 作为安全边界。
- 仅允许 `https`。
- Host 必须精确等于 `chatgpt.com` 或 `chat.openai.com`，大小写不敏感。
- 仅允许默认端口或显式 `443`。
- 禁止 username、password、自定义端口、query 和 fragment。
- 只接受空路径、`/backend-api` 及其尾部 `/` 形式；其他路径回退默认地址。
- 任何解析失败都回退 `https://chatgpt.com/backend-api/wham/usage`。

必须覆盖以下恶意样例：

```text
https://chatgpt.com.evil.example
https://chatgpt.com@evil.example
http://chatgpt.com
https://chatgpt.com:444
https://chat.openai.com.evil.example/backend-api
```

### 4.2 Provider 切换竞态

当前 Provider 切换会强行修改全局刷新状态，旧请求可能在新请求之后返回并覆盖界面。

实施要求：

- 前端为每个 Provider 保存独立 Snapshot。
- 每次请求生成递增 request ID，并捕获发起时的 Provider。
- 只有“最新 request ID + 当前 Provider 匹配”的响应可以进入当前 UI。
- 旧响应仍可写入对应 Provider 缓存，但不得改变当前页面。
- 切换 Provider 后立即显示该 Provider 自有缓存；没有缓存时显示 loading，不保留上一 Provider 数字。
- 手动刷新防抖按 Provider 分桶。
- Provider 切换不受刷新按钮的 10 秒防抖限制。

建议抽离一个无 DOM 的 `app-core.mjs`，承载 request gate、Snapshot 分桶和倒计时纯函数，并使用 Node 内置测试运行器验证。

### 4.3 Cursor 重置时间真实性

调整统一快照模型：

```text
QuotaWindow.resetsAt: string | null
```

规则：

- Codex 有合法 `reset_at` 时序列化 RFC3339 字符串。
- Cursor 缺少 `billingCycleEnd` 时返回 `null`，UI 显示“无重置时间”。
- `null` 窗口不得参加“最近重置”排序。
- Cursor legacy `startOfMonth` 使用日历自然月计算下一个周期，不再固定增加 30 天。
- 不得使用 `Utc::now()` 作为缺失重置时间的占位值。

### 4.4 来源健康状态

复用现有 `ProviderStatus`、`refreshedAt`、`checkedAt` 和 `cached`，在来源卡显示：

- 实时
- 部分可用
- 离线缓存，并显示最近成功时间
- 未登录
- 登录过期
- 网络失败
- 服务异常
- 响应格式变化

所有新增状态必须提供中文、English、Title 和无障碍名称。普通状态不使用 Toast。

### 4.5 原生窗口与托盘生命周期

- `.widget-shell` 相对透明原生画布四周保留 12px。
- 八个 `.resize-handle` 移至 shell 外的窗口画布层，继续覆盖四边和四角。
- `.widget-shell` 继续保持 28px 圆角、内容裁切与透明外部区域。
- 拦截普通关闭请求并隐藏到托盘。
- 托盘“退出”才真正结束进程。
- 引入单实例保护；重复启动时显示并聚焦已有窗口。
- autostart 应用名称统一为 `Token Monitor`。
- 保留 `com.codexusagemonitor.desktop` identifier，避免破坏已有安装升级链路。

### 4.6 工程质量

新增 `.github/workflows/quality.yml`：

- PR 和 `main` push 触发。
- 前端任务：`npm ci`、纯函数测试、`npm run build`。
- Rust 双平台矩阵：Windows、macOS 执行 fmt check、clippy 和 test。
- 增加脚本检查以下五处版本完全一致：
  - `package.json`
  - `package-lock.json`
  - `src-tauri/tauri.conf.json`
  - `src-tauri/Cargo.toml`
  - `src-tauri/Cargo.lock`
- Release workflow 在打包前执行同样的版本和测试门禁。

### 4.7 v1.1.2 完成标准

- 恶意 Codex 基址不会发出携带 Bearer 的请求。
- 快速切换 Provider 20 次不串屏、不跨来源使用 stale 数据。
- Cursor 缺少周期字段时不生成虚假时间。
- Alt+F4/关闭隐藏到托盘，托盘退出真正结束。
- 重复启动不产生第二个实例。
- Windows/macOS 均通过拖动、八向缩放、480×300 最小尺寸和透明圆角验收。
- 所有质量检查在 GitHub Actions 通过。

## 5. v1.2.0 — 后台监控、托盘与额度提醒

### 5.1 Rust 后台刷新协调器

新增 `MonitorCoordinator`：

- 活动 Provider 默认每 60 秒刷新。
- 为每个 Provider 保证最多一个进行中的请求。
- 连续网络/服务失败按 60、120、300 秒退避，最大 5 分钟。
- 手动刷新、网络恢复和睡眠唤醒立即重试并重置退避。
- 通知规则启用的非活动 Provider 也可在后台刷新。
- 成功快照写入 Provider 独立的内存缓存。
- 通过 `monitor:snapshot` 事件把非敏感 Snapshot 推给 WebView。

WebView 继续负责展示，但不再依赖页面 `setInterval` 维持后台监控。

### 5.2 托盘效率

托盘菜单增加：

- Codex 摘要：5 小时、7 天、最近重置倒计时。
- Cursor 摘要：First-party、API、最近重置倒计时。
- 切换 Provider。
- 切换双环 / 聚焦。
- 手动刷新。
- 始终置顶。
- 显示窗口 / 设置。
- 退出。

Windows 单击托盘显示窗口；macOS 保持菜单栏标准行为。托盘文案跟随中文 / English。

### 5.3 原生额度提醒

新增非敏感配置：

```text
MonitoringPreferences
  enabled: boolean                 # 默认 false
  providerRules: Map<Provider, ProviderAlertRules>

ProviderAlertRules
  fiveHour: WindowAlertRule
  sevenDay: WindowAlertRule

WindowAlertRule
  enabled: boolean
  thresholdsRemaining: [20, 10, 5]
  notifyOnReset: boolean
```

默认行为：

- 总开关关闭，不主动申请通知权限。
- 用户开启时才申请系统权限。
- 默认仅启用 10% 阈值，用户可选 20%、10%、5%。
- Reset 恢复提醒默认开启。
- 同一 Provider、窗口、重置周期、阈值仅提醒一次。
- 只对 fresh、非 cached、真实存在的窗口触发。
- stale、离线和不可用窗口不触发。
- 无 `resetsAt` 时，只有额度明显恢复后才重新布防。
- 通知仅包含 Provider、窗口、剩余百分比和重置倒计时。

### 5.4 设置与原生状态同步

设置面板分为：

1. 显示与窗口
2. 额度提醒

新增命令：

```text
get_monitor_preferences
set_monitor_preferences
sync_ui_preferences
```

显示偏好可继续保留 localStorage；通知与后台刷新偏好存入应用自身的非敏感配置。原生命令失败时，控件必须回滚并显示内联状态。

### 5.5 v1.2.0 性能与完成标准

- 窗口隐藏后托盘和通知继续刷新。
- 每个 Provider 同时最多一个请求。
- 同一阈值/周期只通知一次。
- 用户拒绝通知权限后总开关回滚，且不重复弹权限请求。
- 空闲 10 分钟综合内存目标 `<150MB`。
- 空闲 CPU p95 目标 `<1%`。
- 单次刷新不得阻塞 WebView 主线程超过 100ms。
- Windows/macOS 的睡眠、唤醒、网络断开与恢复行为一致。

## 6. v1.3.0 — 更新闭环与安全诊断

### 6.1 Tauri updater

- 使用 Tauri updater 和 GitHub Release `latest.json`。
- 更新 artifact 使用独立 Tauri 私钥签名。
- 私钥与密码只保存在 GitHub Secrets，公钥写入应用配置。
- 仅提供 Stable 通道；Beta 通道延后。
- 启动 15 秒后检查一次，此后每 24 小时检查。
- 设置页提供手动“检查更新”。
- 不静默安装；下载、安装和重启必须由用户确认。
- 更新状态通过顶部短条或控件自身呈现，不使用 Toast。

### 6.2 关于与诊断

设置新增第三个分区：

3. 关于与诊断

包含：

- 当前版本
- OS / 架构
- GitHub Releases
- 问题反馈
- 检查更新
- 复制诊断摘要

新增接口：

```text
AppInfo
SafeDiagnostics
get_app_info
get_safe_diagnostics
```

`SafeDiagnostics` 只允许包含：

- 应用版本
- OS / 架构
- 当前 Provider
- Provider state
- cached 标记
- checkedAt / refreshedAt
- fiveHour / sevenDay 是否存在
- 通用来源标签，例如 `~/.codex/auth.json`

禁止包含账户显示名、套餐身份、完整用户路径、Token、账户 ID 和原始响应。

### 6.3 Release 完整性

Release workflow 完成后必须检查：

- Windows NSIS setup EXE
- Windows MSI
- macOS Universal DMG
- updater artifact
- `latest.json`

缺少任何必需资产时，工作流失败，不把该标签视为成功发布。

### 6.4 v1.3.0 完成标准

- `v1.1.2` 和 `v1.2.0` 均能升级到 `v1.3.0`。
- 升级保留窗口位置、语言、Provider、视图、始终置顶和提醒配置。
- 签名不匹配、断网和下载中断均停止安装并显示明确错误。
- 诊断文本经过敏感字段断言测试。
- Windows/macOS Release 资产完整后才发布成功。

## 7. 测试矩阵

### 7.1 Provider fixture

使用手工构造的脱敏 JSON / Connect RPC fixtures，禁止提交生产原始响应。

覆盖：

- Codex 完整双窗口
- Codex 只返回一个窗口
- Codex 未知窗口时长
- Cursor First-party / API 完整与单侧缺失
- 缺少 reset 时间
- 401/403
- 429
- 网络失败与超时
- 上游字段新增、缺失或类型变化
- 同 Provider stale 缓存
- Provider 之间不得复用缓存

### 7.2 前端

- 请求先后顺序反转。
- Provider 快速切换。
- per-provider 手动刷新防抖。
- stale/partial/auth/offline 文案。
- null reset 的格式化和最近重置排序。
- 通知阈值去重与重新布防。
- 中文/English、Title 和 aria-label。

### 7.3 原生平台

- 标题栏拖动。
- 八向缩放和最小尺寸。
- 12px 透明阴影空间和 28px 圆角。
- 关闭到托盘、托盘退出、单实例。
- 始终置顶、登录启动、窗口位置恢复。
- 系统通知权限允许/拒绝。
- 睡眠唤醒和网络恢复。
- 从最近两个稳定版升级。

## 8. macOS 后续开发交接

### 8.1 环境准备

```bash
xcode-select --install
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
rustup target add aarch64-apple-darwin x86_64-apple-darwin
```

安装 Node.js 22 后：

```bash
git clone https://github.com/MmmmrJ/token-monitor.git
cd token-monitor
npm ci
```

### 8.2 开发起点

```bash
git checkout main
git pull --ff-only
git checkout -b codex/v1.1.2-trust-hotfix
```

第一批提交建议拆分为：

1. `fix: harden Codex usage URL allowlist`
2. `fix: isolate provider refresh requests`
3. `fix: preserve missing Cursor reset times`
4. `fix: restore transparent canvas and tray lifecycle`
5. `ci: add cross-platform quality gates`

### 8.3 macOS 本地验证

```bash
npm run build
cargo test --manifest-path src-tauri/Cargo.toml
npm run tauri:dev
```

生成 Universal DMG：

```bash
npm run tauri:build -- --target universal-apple-darwin --bundles dmg
```

重点手工检查：

- 菜单栏图标在浅色/深色模式可见。
- Cmd+W 隐藏窗口但应用继续运行。
- 托盘“退出”结束进程。
- 多显示器与不同缩放比例的位置恢复。
- 透明边缘无白色角块。
- 登录启动名称显示为 Token Monitor。
- Universal 构建同时包含 arm64 与 x86_64。

## 9. 明确暂缓

以下内容不进入未来三个版本：

- 第三个 Provider
- 多账户和账户切换
- `auth.json` 导入或手填 Token
- 浏览器 Cookie
- Token/费用历史
- 大型趋势 Dashboard
- 多设备同步
- Linux 正式发行
- 极简气泡模式
- 主题系统
- Stable/Beta 双通道
- 预测额度耗尽时间

这些能力只有在 v1.3.0 稳定发布后，并获得明确用户需求与性能预算时再评估。

## 10. 发布原则

- 一个功能版本配合必要补丁，不采用日更发布。
- Windows/macOS 同时通过才发布 Stable。
- 每次 Release 明确推荐安装包、已知风险和升级说明。
- 发布后必须确认 GitHub Releases 中存在真实安装资产，而不只是 Tags 源码包。
- 任何安全、额度正确性或凭据边界问题优先于新功能。

下一步从 `v1.1.2` 的 Codex URL 安全校验开始实施。
