# Token Monitor 开发路线图

> 状态：`v1.1.2` 已完成本地实现与 macOS 验收，待 Windows CI；后续版本待实施
>
> 已发布基线：`v1.1.1`；当前工作区：`v1.1.2`
>
> 规划周期：未来 4 个版本，约 10–13 周
>
> 平台策略：Windows / macOS 同等支持与验收
>
> 最后更新：2026-07-16

## 1. 产品方向

Token Monitor 继续定位为一个轻量、低噪声、隐私优先的 Codex / Cursor 桌面额度悬浮组件。

未来四个版本不追求竞品的“大而全”，而是依次解决：

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
- [第三方 Provider 的登录方式缺少明确引导](https://github.com/Javis603/token-monitor/issues/154)。该反馈来自 Claude 场景，仅作为“登录引导必须清晰”的通用信号，不代表本项目计划支持 Claude。

由此确定 Token Monitor 的差异化：**更小、更准、更可信，而不是功能最多。**

## 3. 版本总览

| 版本 | 主题 | 目标周期 | 发布门槛 |
| --- | --- | --- | --- |
| `v1.1.2` | 安全与数据可信度热修 | 1–2 周 | URL、Cursor 映射、竞态、重置时间、状态模型和 CI 完成 |
| `v1.1.3` | 原生窗口与生命周期修复 | 1–2 周 | 0px 贴边画布、关闭到托盘、单实例和 autostart 迁移完成 |
| `v1.2.0` | 后台监控、托盘、提醒与更新引导 | 3–5 周 | 隐藏刷新、通知去重、Updater bootstrap 和性能达标 |
| `v1.3.0` | 更新体验闭环与安全诊断 | 3–4 周 | v1.2.0 应用内升级、Release 完整且签名状态明确披露 |

已确认的产品默认值：窗口内容 `0px` 贴合原生画布；Cursor 中文 UI 使用“订阅额度 / API 额度”；提醒默认仅启用 10%；当前阶段允许发布未经过 Apple 公证或 Windows 商业代码签名的 Stable 安装包，但必须明确提示风险。

### 3.1 当前实施进度（2026-07-16）

`v1.1.2` 已完成代码实现和 macOS 本地验收，本次提交将创建对应 `v1.1.2` Tag；Windows 编译和双平台安装资产以 Tag 触发的 GitHub Actions 结果为最终发布门槛。

| 工作项 | 状态 | 已落地内容 |
| --- | --- | --- |
| Codex 请求安全 | 已完成 | 精确 HTTPS 域名/端口/路径校验，禁止 301/302/307/308 跳转，恶意基址回退默认地址 |
| Cursor 数据真实性 | 已完成 | 仅映射 `autoPercentUsed` / `apiPercentUsed`，删除金额、总百分比和 legacy 请求桶补数 |
| 重置时间 | 已完成 | `resetsAt` 支持 `null`，Codex/Cursor 缺失重置时间时不再生成占位值 |
| Provider 状态模型 | 已完成 | availability、errorKind、cached 正交表达，失败请求不覆盖最近成功时间 |
| 前端竞态与缓存 | 已完成 | Provider 独立 Snapshot、request ID、刷新防抖和 stale 缓存分桶，20 次快切测试通过 |
| 双语状态 UI | 已完成 | 中文“订阅额度 / API 额度”，完整错误、部分可用、离线缓存和无重置时间文案 |
| 工程质量 | 已完成 | 前端纯函数测试、Rust 测试、Clippy、版本一致性脚本和双平台 Quality workflow |
| macOS 本地验收 | 已完成 | Vite 构建、Tauri `.app` 打包、真实 Codex/Cursor 切换和 3 秒状态短条验证通过 |
| Windows 与 Release | 待 CI | `v1.1.2` Tag 推送后构建 Windows MSI/NSIS 与 macOS Universal DMG，并检查 Release 资产 |

当前自动化基线：前端测试 4 项、Rust 测试 16 项，`cargo clippy --all-targets -- -D warnings` 零警告。`v1.1.3`、`v1.2.0`、`v1.3.0` 尚未开始实现。

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
- Codex HTTP Client 使用 `reqwest::redirect::Policy::none()`；任何 3xx 都作为 `service_error` 返回，禁止携带 Bearer Token 或 `ChatGPT-Account-Id` 跟随重定向。

必须覆盖以下恶意样例：

```text
https://chatgpt.com.evil.example
https://chatgpt.com@evil.example
http://chatgpt.com
https://chatgpt.com:444
https://chat.openai.com.evil.example/backend-api
```

除恶意基址外，测试还必须覆盖 301、302、307、308，确认没有发出第二次请求。

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

### 4.3 Cursor 严格额度映射

Cursor 双槽只允许使用当前 Dashboard 响应中的目标字段：

- `fiveHour` ← `planUsage.autoPercentUsed`，中文 UI 显示“订阅额度”，English 显示 First-party。
- `sevenDay` ← `planUsage.apiPercentUsed`，中文 UI 显示“API 额度”，English 显示 API。
- 任一字段缺失时对应窗口为 `null`；只返回一侧时状态为 partial。
- 两个字段都缺失时返回 connected-but-unavailable，不使用 `includedSpend`、`remaining`、`limit` 或 `totalPercentUsed` 填数。
- 删除 `/auth/usage` 模型请求桶到双槽的 legacy 回退；Dashboard 端点不可用时显示明确服务/响应错误。
- `spendLimitUsage` 可以继续反序列化以兼容响应结构，但不得映射到主界面额度。

必须增加“双字段完整、单侧缺失、双侧缺失、legacy 仅有请求桶、字段类型变化”测试。

### 4.4 Cursor 重置时间真实性

调整统一快照模型：

```text
QuotaWindow.resetsAt: string | null
```

规则：

- Codex 有合法 `reset_at` 时序列化 RFC3339 字符串。
- Cursor 缺少 `billingCycleEnd` 时返回 `null`，UI 显示“无重置时间”。
- `null` 窗口不得参加“最近重置”排序。
- Cursor 主界面只接受 Dashboard 返回的 `billingCycleEnd`；已删除的 legacy 请求桶不得再通过 `startOfMonth + 30 天` 构造主界面重置时间。
- 不得使用 `Utc::now()` 作为缺失重置时间的占位值。

### 4.5 来源健康状态

现有单一 `ProviderStatus.state` 无法同时表达“缓存可用”和“本次失败原因”，统一调整为正交状态：

```text
ProviderAvailability = live | partial | unavailable
ProviderErrorKind =
  auth_missing | auth_unreadable | auth_invalid | reauth_required |
  unsupported_auth | network_error | service_error | invalid_response | null

ProviderStatus
  kind: codex | cursor
  source: local_codex_oauth | local_cursor_session
  authPathLabel: string            # 只能是通用标签
  availability: ProviderAvailability
  errorKind: ProviderErrorKind

MonitorSnapshot.cached: boolean    # fresh / cached
MonitorSnapshot.refreshedAt        # 最近成功时间；失败时不得覆盖
MonitorSnapshot.checkedAt          # 最近尝试时间
```

不再用 `state = stale` 覆盖原始失败原因；缓存快照保留 `availability`，同时设置 `cached = true` 和本次 `errorKind`。来源卡据此持续显示：

- 实时
- 部分可用
- 离线缓存，并显示最近成功时间
- 未登录
- 登录过期
- 网络失败
- 服务异常
- 响应格式变化

所有状态必须提供中文、English、Title 和无障碍名称。健康状态放在来源卡持续展示；顶部短条只用于短期操作反馈，不使用 Toast。

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
- Cargo.lock 检查必须按根包 `codex-usage-monitor` 定位，不能把依赖中相同版本号误认为应用版本。
- Release workflow 在打包前执行同样的版本和测试门禁。

### 4.7 v1.1.2 完成标准

- 恶意 Codex 基址不会发出携带 Bearer 的请求。
- Codex 3xx 响应不会跟随跳转或泄露账户 ID。
- 快速切换 Provider 20 次不串屏、不跨来源使用 stale 数据。
- Cursor 只使用 `autoPercentUsed` / `apiPercentUsed`，目标字段缺失时不使用金额或 legacy 请求桶补数。
- Cursor 缺少周期字段时不生成虚假时间。
- fresh/cached 与失败原因可以同时表达，最近成功时间不会被失败请求覆盖。
- 所有质量检查在 GitHub Actions 通过。

## 5. v1.1.3 — 原生窗口与生命周期修复

### 5.1 贴边画布与缩放热区

- `.widget-shell` 使用 `100% × 100%`、`0px` 外边距完全贴合透明原生画布，不保留外围透明阴影空间。
- 八个 `.resize-handle` 贴合可见组件四边和四角，命中区位于窗口内部，不依赖圆角外的透明区域。
- `.widget-shell` 保持 28px 连续圆角和内容裁切；层次感使用内描边、内部高光与渐变，不使用会超出窗口边界的外投影，也不启用 `windowEffects` 或系统阴影。
- 默认窗口仍为 `620×360`，最小尺寸仍为 `480×300`。

### 5.2 关闭到托盘与单实例

- `CloseRequested` 调用 `prevent_close()` 后隐藏窗口；托盘“退出”使用显式退出标记并调用 `app.exit(0)`。
- macOS Cmd+W、Windows 关闭按钮和 Alt+F4 均隐藏；macOS Cmd+Q 与托盘“退出”真正退出。
- 引入 `tauri-plugin-single-instance`，并作为 Builder 注册的第一个插件。
- 第二实例回调对已有窗口依次执行 show、unminimize、set_focus，不产生第二个后台协调器或托盘。

### 5.3 autostart 兼容迁移

- 用户可见及新注册名称统一为 `Token Monitor`，保留 `com.codexusagemonitor.desktop` identifier、内部二进制名和 localStorage 键。
- 迁移时分别检测旧 `Codex Usage Monitor` 与新 `Token Monitor` 启动项；只要旧项或已保存偏好为启用，就先成功注册新项，再删除旧项。
- 迁移必须幂等；新项注册失败时保留旧项，设置控件回滚并显示内联错误。
- 使用应用配置中的一次性迁移标记记录完成状态，不依赖修改 Codex/Cursor 或 Shell 配置。

### 5.4 v1.1.3 完成标准

- Windows/macOS 均通过拖动、八向缩放、480×300 最小尺寸、四边无透明留白和 28px 圆角验收。
- Cmd+W/Alt+F4/关闭按钮隐藏到托盘，Cmd+Q/托盘退出真正结束进程。
- 连续启动 10 次只有一个实例，已有窗口被显示并聚焦。
- 从 v1.1.1 的旧 autostart 项升级后不重复启动，系统启动项显示 Token Monitor。

## 6. v1.2.0 — 后台监控、托盘、额度提醒与更新引导

### 6.1 Rust 后台刷新协调器

新增 `MonitorCoordinator`：

- 活动 Provider 默认每 60 秒刷新。
- 为每个 Provider 保证最多一个进行中的请求。
- 连续网络/服务失败按 60、120、300 秒退避，最大 5 分钟。
- 手动刷新遇到同 Provider 已有请求时复用该请求结果，不启动第二个请求；手动刷新成功后重置退避。
- Coordinator 保存单调时钟 tick；相邻 tick 间隔超过 90 秒视为发生睡眠/唤醒，并立即调度活动 Provider。
- 不宣称依赖 Tauri 桌面 `Suspended/Resumed` 事件。网络恢复由下一次退避探测成功确认，成功后立即恢复 60 秒周期；设置页与托盘仍允许用户主动重试。
- 通知规则启用的非活动 Provider 也可在后台刷新。
- 成功快照写入 Provider 独立的内存缓存。
- 通过 `monitor:snapshot` 事件把非敏感 Snapshot 推给 WebView。

WebView 继续负责展示，但不再依赖页面 `setInterval` 维持后台监控。

### 6.2 托盘效率

托盘菜单增加：

- Codex 摘要：5 小时、7 天、最近重置倒计时。
- Cursor 摘要：中文使用“订阅额度、API 额度”，English 使用“First-party、API”，并显示最近重置倒计时。
- 切换 Provider。
- 切换双环 / 聚焦。
- 手动刷新。
- 始终置顶。
- 显示窗口 / 设置。
- 退出。

Windows 单击托盘显示窗口；macOS 保持菜单栏标准行为。托盘文案跟随中文 / English。

### 6.3 原生额度提醒

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

AlertRuntimeState[provider][window]
  cycleKey: string | null
  notifiedThresholds: number[]
  resetNotified: boolean
  lastRemainingPercent: number | null
```

默认行为：

- 总开关关闭，不主动申请通知权限。
- 用户开启时才申请系统权限。
- 默认仅启用 10% 阈值；用户可另外选择 20% 和 5%。
- Reset 恢复提醒默认开启。
- `MonitoringPreferences` 与 `AlertRuntimeState` 都以原子写入方式保存在应用配置目录；后者不包含账户或 Token，并跨应用重启保留去重状态。
- 同一 Provider、窗口、重置周期、阈值仅提醒一次；`resetsAt` 变化时建立新周期并清空该窗口的阈值记录。只有先观察过旧周期、随后收到新周期的 fresh Snapshot，才发送一次 Reset 恢复提醒。
- 只对 fresh、非 cached、真实存在的窗口触发。
- cached、离线和不可用窗口不触发。
- 无 `resetsAt` 时，额度回升到已触发阈值以上至少 5 个百分点才重新布防该阈值。
- 无 `resetsAt` 时不发送 Reset 恢复提醒，因为无法可靠确认周期已经重置。
- 通知仅包含 Provider、窗口、剩余百分比和重置倒计时。
- 用户拒绝权限后记录拒绝状态、回滚总开关并显示系统设置引导；只有用户再次主动操作开关时才重新检查权限，不在后台重复请求。

### 6.4 设置与原生状态同步

设置面板分为：

1. 显示与窗口
2. 额度提醒

新增命令：

```text
get_monitor_preferences
set_monitor_preferences
sync_ui_preferences
```

显示偏好可继续保留 localStorage；通知与后台刷新偏好存入应用自身的非敏感 JSON 配置。写入采用临时文件 + rename，读取失败回退安全默认值并保留损坏文件供用户手动诊断。原生命令失败时，控件必须回滚并显示内联状态。

### 6.5 Updater bootstrap 与 Release 聚合

为保证 v1.2.0 能在应用内升级到后续版本，本版本必须先交付可工作的更新客户端：

- 引入 Tauri updater/process 插件、公钥和 Stable endpoint；设置 `createUpdaterArtifacts: true`。
- 启动 15 秒后检查一次，此后每 24 小时检查；发现更新只显示顶部短条，下载、安装和重启都需要用户确认。
- 更新 artifact 使用独立 Tauri 私钥签名；私钥与密码只保存在 GitHub Secrets，公钥写入应用配置。
- Windows/macOS 构建 job 只上传 Actions artifact，不分别创建或修改 GitHub Release。
- 独立 release job 等待两个平台完成，统一创建 Release、聚合平台更新信息并只生成一个 `latest.json`。
- Release 后置完整性检查至少验证 DMG、MSI、NSIS EXE、平台 updater artifact、签名文件和 `latest.json` 均存在且下载响应有效。

v1.1.2/v1.1.3 没有更新客户端，只承诺通过手动安装包升级并保留设置；从 v1.2.0 开始承诺应用内升级。

### 6.6 v1.2.0 性能与完成标准

- 窗口隐藏后托盘和通知继续刷新。
- 每个 Provider 同时最多一个请求。
- 同一阈值/周期只通知一次。
- 用户拒绝通知权限后总开关回滚，且不重复弹权限请求。
- Release 构建在 Windows 11 x64 与 macOS 14+ Apple Silicon 上启动后预热 60 秒，再以 1 秒间隔采样 10 分钟。
- 综合内存按应用主进程及其 WebView 子进程 RSS 之和统计，p95 目标 `<150MB`；综合 CPU p95 目标 `<1%`。
- 使用前端 Long Task/交互时间记录验证单次刷新不阻塞 WebView 主线程超过 100ms。
- 睡眠超过 90 秒后 10 秒内发起一次刷新；网络失败按退避探测，首次成功后恢复 60 秒周期。
- v1.2.0 Release 包含可验证签名的 updater artifact 和唯一 `latest.json`。

## 7. v1.3.0 — 更新体验闭环与安全诊断

### 7.1 更新体验与可选平台签名

在 v1.2.0 已具备 updater client、签名验证和单一 `latest.json` 的基础上，v1.3.0 完成用户可见的更新闭环：

- 设置页提供手动“检查更新”，并显示当前版本、目标版本、Release notes 和下载进度。
- 仅提供 Stable 通道；Beta 通道延后。
- 不静默安装；下载、安装和重启都必须由用户确认。
- Tauri updater 签名不匹配、断网、下载中断、磁盘空间不足和安装失败必须终止更新，并通过顶部短条或控件自身显示可恢复的错误，不使用 Toast。
- Tauri updater artifact 签名只负责应用内更新包的完整性，不能替代操作系统安装包签名。
- Tauri updater artifact 签名是应用内更新的强制门槛；私钥和密码只存放在 GitHub Secrets，公钥写入应用配置。
- macOS Developer ID/notarization 与 Windows Authenticode 当前为可选增强项；存在对应 Secrets 时执行签名和验证，不存在时仍允许发布 Stable。
- 未签名 Stable 必须在 Release 标题附近、安装说明和应用“关于”区域明确标注平台签名状态，并提供 SHA-256 校验值和系统安全提示，不得暗示已通过 Apple 或 Microsoft 验证。
- 代码签名证书、notarization 凭据和 updater 私钥必须相互独立；任何已配置凭据都不得输出到构建日志。

平台代码签名缺失不阻止 Stable；但 updater 签名缺失、签名验证失败或签名状态披露缺失时，不得创建 Stable Release 或更新 `latest.json`。

### 7.2 关于与安全诊断

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
  version
  os
  arch
  releaseUrl
  issueUrl

SafeDiagnostics
  version
  os
  arch
  provider
  availability
  errorKind
  cached
  checkedAt
  refreshedAt
  hasFiveHour
  hasSevenDay
  sourceLabel

get_app_info
get_safe_diagnostics
```

`sourceLabel` 只能使用 `Codex auth.json`、`Cursor local session` 等固定通用标签，不包含用户名或完整路径。诊断摘要禁止包含账户显示名、套餐身份、余额具体数值、Token、账户 ID、请求 URL、自定义基址和原始响应；序列化后还必须经过敏感键名与 Bearer/JWT 模式的二次拒绝检查。

### 7.3 Release 完整性

Release workflow 完成后必须检查：

- Windows NSIS setup EXE 与 MSI 存在；启用 Authenticode 时必须验证成功，未启用时 Release 明确标记“未签名”。
- macOS Universal DMG 存在；启用 Developer ID/notarization 时验证签名、hardened runtime、notarization ticket 和 Gatekeeper，未启用时 Release 明确标记“未公证”。
- 两个平台 updater artifact 及其 Tauri 签名文件存在且签名可验证。
- Release 中只有聚合 job 生成的一个 `latest.json`，其中版本、URL、平台、架构和签名与真实资产一致。
- 所有必需资产均可匿名下载，SHA-256 记录与工作流产物一致。

平台构建 job 仍只上传 Actions artifact；只有聚合 release job 可以创建或修改 GitHub Release。缺少任何必需资产或验证失败时，工作流失败，不把该标签视为成功发布。

### 7.4 v1.3.0 完成标准

- `v1.1.2` / `v1.1.3` 通过手动安装 v1.3.0 保留设置；`v1.2.0` 通过应用内 updater 升级成功。
- 升级保留窗口位置、语言、Provider、视图、始终置顶和提醒配置。
- Tauri updater 签名不匹配、断网和下载中断均停止安装并显示明确错误。
- 诊断文本经过敏感字段断言测试。
- 双平台 Release 资产、SHA-256、updater 签名和平台签名状态说明全部完整后才发布 Stable；平台代码签名本身不是当前版本的强制条件。

## 8. 测试矩阵

### 8.1 Provider fixture

使用手工构造的脱敏 JSON / Connect RPC fixtures，禁止提交生产原始响应。

覆盖：

- Codex 完整双窗口
- Codex 只返回一个窗口
- Codex 未知窗口时长
- Cursor First-party / API 完整与单侧缺失
- Cursor 双目标字段缺失时不回退金额、总百分比或 legacy 请求桶
- 缺少 reset 时间
- 401/403
- 429
- Codex 301/302/307/308 不跟随
- 网络失败与超时
- 上游字段新增、缺失或类型变化
- 同 Provider stale 缓存
- Provider 之间不得复用缓存

### 8.2 前端

- 请求先后顺序反转。
- Provider 快速切换。
- per-provider 手动刷新防抖。
- availability、errorKind 与 cached 组合文案。
- null reset 的格式化和最近重置排序。
- 中文/English、Title 和 aria-label。
- 设置写入失败后的控件回滚。
- updater 检查、确认、下载进度和失败状态。

### 8.3 Rust 协调器与状态机

- availability、errorKind、cached 三者正交，失败请求不覆盖 refreshedAt。
- 同 Provider 并发刷新只产生一个上游请求；手动刷新复用进行中的结果。
- 60/120/300 秒退避、成功恢复和超过 90 秒 tick gap 的唤醒调度。
- 提醒状态跨重启去重、按 resetsAt 换周期、无 resetsAt 时回升 5 个百分点重新布防。
- Provider 缓存和提醒状态严格分桶。
- 配置原子写入、损坏配置回退和权限拒绝状态恢复。
- SafeDiagnostics 的字段白名单、敏感键名与 Bearer/JWT 拒绝检查。

### 8.4 原生平台

- 标题栏拖动。
- 八向缩放和最小尺寸。
- 四边 `0px` 透明留白、无外投影和 28px 圆角。
- 关闭到托盘、托盘退出、单实例。
- 始终置顶、窗口位置恢复和旧/新 autostart 迁移。
- 系统通知权限允许/拒绝。
- 睡眠唤醒和网络恢复。
- v1.1.2/v1.1.3 手动覆盖安装保留设置，v1.2.0 应用内升级保留设置。
- updater 签名强制验证；macOS/Windows 平台签名启用时验证成功，未启用时验证风险说明和 SHA-256 是否完整。

## 9. macOS 后续开发交接

### 9.1 环境准备

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

### 9.2 开发起点

```bash
git checkout main
git pull --ff-only
git checkout -b codex/v1.1.2-trust-hotfix
```

第一批提交建议拆分为：

1. `fix: harden Codex usage URL allowlist`
2. `fix: enforce strict Cursor quota mapping`
3. `fix: isolate provider refresh requests`
4. `fix: preserve missing Cursor reset times`
5. `refactor: separate provider availability and errors`
6. `ci: add cross-platform quality gates`

v1.1.3 从 `main` 新建 `codex/v1.1.3-native-lifecycle`，独立提交透明画布、关闭到托盘、单实例和 autostart 迁移，避免把原生生命周期风险混入安全热修。

### 9.3 macOS 本地验证

```bash
npm run build
cargo test --manifest-path src-tauri/Cargo.toml
cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets -- -D warnings
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

## 10. 明确暂缓

以下内容不进入未来四个版本：

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

## 11. 发布原则

- 一个功能版本配合必要补丁，不采用日更发布。
- Windows/macOS 同时通过才发布 Stable。
- 每次 Release 明确推荐安装包、已知风险和升级说明。
- 发布后必须确认 GitHub Releases 中存在真实安装资产，而不只是 Tags 源码包。
- Tauri updater 签名、macOS Developer ID/notarization 与 Windows Authenticode 是三套独立验证；Release 必须分别记录“通过 / 未配置 / 失败”，其中 updater 签名失败会阻止发布，平台签名未配置暂不阻止发布。
- 任何安全、额度正确性或凭据边界问题优先于新功能。

下一步在 Windows CI 验证 `v1.1.2`，通过后开始 `v1.1.3` 的原生窗口与生命周期修复。
