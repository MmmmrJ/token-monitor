import '@phosphor-icons/web/regular';
import {
  beginProviderRequest,
  completeProviderRequest,
  createProviderState,
  finishProviderRequest,
  nextResetWindow,
  normalizeProvider,
  shouldRenderProviderResponse
} from './app-core.mjs';

const nativeInvoke = window.__TAURI__?.core?.invoke;
const nativeWindow = window.__TAURI__?.window?.getCurrentWindow?.();
const nativeListen = window.__TAURI__?.event?.listen;
const nativeResizeDirection = window.__TAURI__?.window?.ResizeDirection;
const previewMode = new URLSearchParams(location.search).get('preview') === '1';
const storageKey = 'codex-usage-monitor:widget:v2';

const copy = {
  zh: {
    localAccount: '本机账户', fiveRemaining: '5 小时剩余', weekRemaining: '7 天剩余',
    firstPartyRemaining: '订阅额度剩余', apiRemaining: 'API 额度剩余',
    nextReset: '下次重置', dualAria: '双环额度视图', focusAria: '聚焦额度视图',
    loading: '正在读取', unavailable: '当前账户未提供', resets: '重置', justNow: '刚刚更新', cached: '离线缓存',
    live: '实时', partial: '部分可用', unavailableStatus: '暂不可用',
    authMissingStatus: '未登录', authUnreadableStatus: '登录态不可读', authInvalidStatus: '登录态损坏',
    reauthStatus: '登录过期', unsupportedStatus: '登录模式不支持', networkStatus: '网络异常',
    serviceStatus: '服务异常', invalidStatus: '数据格式变化',
    settingsKicker: '小组件设置', settingsTitle: '显示与启动', viewStyle: '展示样式', dualView: '双环', focusView: '聚焦',
    providerSource: '额度来源', providerSwitch: '额度来源',
    sectionDisplay: '显示与窗口', sectionAlerts: '额度提醒',
    alertsEnabled: '启用额度提醒', alertsEnabledHelp: '低于阈值时发送系统通知',
    alertThreshold10: '10% 阈值', alertThreshold10Help: '默认启用',
    alertThreshold20: '20% 阈值', alertThreshold20Help: '可选',
    alertThreshold5: '5% 阈值', alertThreshold5Help: '可选',
    alertOnReset: '重置恢复提醒', alertOnResetHelp: '额度周期重置后提醒一次',
    alertsDenied: '系统通知权限被拒绝，请在系统设置中允许后再开启。',
    alertsSaveFailed: '无法保存提醒设置，已回滚。',
    updateAvailable: '发现新版本', updateInstall: '下载并安装', updateLater: '稍后',
    updateFailed: '检查或安装更新失败',
    switchCodex: '切换到 Codex 额度', switchCursor: '切换到 Cursor 额度',
    brand: 'Token Monitor',
    language: '界面语言', languageHelp: '中文 / English', alwaysTop: '始终置顶', alwaysTopHelp: '保持小组件浮在其他窗口上方',
    startLogin: '登录时启动', startLoginHelp: '进入系统后自动运行', privacy: '登录令牌只在 Rust 进程内读取，不会发送到前端、日志或导出文件。',
    startLoginQueryFailed: '无法读取系统启动项状态，请稍后重试。',
    startLoginEnableFailed: '无法创建 Token Monitor 启动项；已保留原有启动项。',
    startLoginCleanupFailed: '新启动项已生效，但旧启动项清理失败。',
    startLoginDisableFailed: '无法关闭系统启动项，请检查系统权限后重试。',
    switchView: '切换视图', refresh: '刷新额度', settings: '打开设置', close: '关闭设置',
    nativeOnly: '请运行桌面应用以读取本机登录态。',
    missingBoth: '已连接，但当前账户没有下发完整额度窗口。',
    missingFive: '已连接；当前账户暂未下发主额度窗口。',
    missingWeek: '已连接；当前账户暂未下发次额度窗口。',
    authMissing: '未找到本机 Codex 登录态，请先运行 codex login。',
    authMissingCursor: '未找到本机 Cursor 登录态，请先在 Cursor 中登录。',
    authUnreadable: '本机 Codex 登录文件无法读取，请检查文件权限。',
    authUnreadableCursor: '本机 Cursor 登录数据库无法读取，请检查文件权限。',
    authInvalid: '本机 Codex 登录文件格式损坏，请重新运行 codex login。',
    authInvalidCursor: '本机 Cursor 登录数据格式无效，请在 Cursor 中重新登录。',
    reauth: 'Codex 登录态已过期，请重新运行 codex login。',
    reauthCursor: 'Cursor 登录态已过期，请在 Cursor 中重新登录。',
    unsupportedAuth: '当前为 API Key 模式；请使用 ChatGPT 登录以读取订阅额度。',
    networkError: '暂时无法连接额度服务，请检查网络。',
    serviceError: '额度服务暂时不可用，请稍后重试。',
    invalidResponse: '额度数据格式已变化，当前无法解析。',
    staleNotice: '刷新失败，正在显示上次成功数据。',
    preview: '设计预览数据', noReset: '无重置时间',
    unknownAccount: '本机 Codex 账户', unknownAccountCursor: '本机 Cursor 账户'
  },
  en: {
    localAccount: 'Local account', fiveRemaining: '5-hour remaining', weekRemaining: '7-day remaining',
    firstPartyRemaining: 'First-party remaining', apiRemaining: 'API remaining',
    nextReset: 'Next reset', dualAria: 'Dual-ring quota view', focusAria: 'Focused quota view',
    loading: 'Loading', unavailable: 'Not provided for this account', resets: 'reset', justNow: 'Updated just now', cached: 'Offline cache',
    live: 'Live', partial: 'Partial', unavailableStatus: 'Unavailable',
    authMissingStatus: 'Signed out', authUnreadableStatus: 'Sign-in unreadable', authInvalidStatus: 'Sign-in damaged',
    reauthStatus: 'Sign-in expired', unsupportedStatus: 'Unsupported sign-in', networkStatus: 'Network issue',
    serviceStatus: 'Service issue', invalidStatus: 'Data format changed',
    settingsKicker: 'Widget settings', settingsTitle: 'Display & startup', viewStyle: 'View style', dualView: 'Dual rings', focusView: 'Focus',
    sectionDisplay: 'Display & window', sectionAlerts: 'Quota alerts',
    providerSource: 'Usage source', providerSwitch: 'Usage source',
    switchCodex: 'Switch to Codex usage', switchCursor: 'Switch to Cursor usage',
    brand: 'Token Monitor',
    language: 'Language', languageHelp: '中文 / English', alwaysTop: 'Always on top', alwaysTopHelp: 'Keep the widget above other windows',
    startLogin: 'Start at login', startLoginHelp: 'Launch after signing in to the computer', privacy: 'Sign-in tokens are read only inside the Rust process and never sent to the frontend, logs, or exports.',
    startLoginQueryFailed: 'Could not read the system login item. Try again later.',
    startLoginEnableFailed: 'Could not create the Token Monitor login item. The previous item was kept.',
    startLoginCleanupFailed: 'The new login item is active, but the old item could not be removed.',
    startLoginDisableFailed: 'Could not disable the system login item. Check permissions and try again.',
    alertsEnabled: 'Enable quota alerts', alertsEnabledHelp: 'Send a system notification below thresholds',
    alertThreshold10: '10% threshold', alertThreshold10Help: 'Enabled by default',
    alertThreshold20: '20% threshold', alertThreshold20Help: 'Optional',
    alertThreshold5: '5% threshold', alertThreshold5Help: 'Optional',
    alertOnReset: 'Notify on reset', alertOnResetHelp: 'Notify once when a quota cycle resets',
    alertsDenied: 'Notification permission was denied. Alerts were turned off. Allow access in system settings, then try again.',
    alertsSaveFailed: 'Could not save alert settings. Try again later.',
    updateAvailable: 'Update available', updateInstall: 'Download & install', updateLater: 'Later',
    updateFailed: 'Update check or install failed',
    switchView: 'Switch view', refresh: 'Refresh usage', settings: 'Open settings', close: 'Close settings',
    nativeOnly: 'Run the desktop app to read the local sign-in session.',
    missingBoth: 'Connected, but this account did not return complete quota windows.',
    missingFive: 'Connected; this account did not return the primary quota window.',
    missingWeek: 'Connected; this account did not return the secondary quota window.',
    authMissing: 'No local Codex sign-in was found. Run codex login first.',
    authMissingCursor: 'No local Cursor sign-in was found. Sign in to Cursor first.',
    authUnreadable: 'The local Codex sign-in file could not be read. Check its permissions.',
    authUnreadableCursor: 'The local Cursor session database could not be read. Check its permissions.',
    authInvalid: 'The local Codex sign-in file is damaged. Run codex login again.',
    authInvalidCursor: 'The local Cursor session data is invalid. Sign in to Cursor again.',
    reauth: 'The Codex sign-in has expired. Run codex login again.',
    reauthCursor: 'The Cursor sign-in has expired. Sign in to Cursor again.',
    unsupportedAuth: 'Codex is using API-key mode. Sign in with ChatGPT to read subscription limits.',
    networkError: 'The quota service could not be reached. Check the network.',
    serviceError: 'The quota service is temporarily unavailable.',
    invalidResponse: 'The quota response format changed and could not be parsed.',
    staleNotice: 'Refresh failed. Showing the last successful data.',
    preview: 'Design preview data', noReset: 'No reset time',
    unknownAccount: 'Local Codex account', unknownAccountCursor: 'Local Cursor account'
  }
};

const defaults = {
  language: navigator.language.toLowerCase().startsWith('zh') ? 'zh' : 'en',
  view: 'dual',
  provider: 'codex',
  alwaysOnTop: false,
  startAtLogin: false,
  reduceMotion: matchMedia('(prefers-reduced-motion: reduce)').matches
};

function loadPreferences() {
  try {
    const loaded = { ...defaults, ...JSON.parse(localStorage.getItem(storageKey) || '{}') };
    loaded.provider = loaded.provider === 'cursor' ? 'cursor' : 'codex';
    return loaded;
  } catch {
    return { ...defaults };
  }
}

const preferences = loadPreferences();
const state = {
  ...createProviderState(),
  statusKey: '',
  statusTimer: null
};
const $ = (selector) => document.querySelector(selector);
const $$ = (selector) => [...document.querySelectorAll(selector)];
const t = (key) => copy[preferences.language][key] || key;
const isCursor = (provider = preferences.provider) => normalizeProvider(provider) === 'cursor';
const currentSnapshot = () => state.snapshots[preferences.provider];

function savePreferences() {
  localStorage.setItem(storageKey, JSON.stringify(preferences));
}

async function invoke(command, payload) {
  if (!nativeInvoke) return null;
  try { return await nativeInvoke(command, payload); }
  catch (error) {
    console.warn(`Native command ${command} failed`, error);
    return null;
  }
}

const autostartErrorKeys = {
  query_failed: 'startLoginQueryFailed',
  enable_failed: 'startLoginEnableFailed',
  cleanup_failed: 'startLoginCleanupFailed',
  disable_failed: 'startLoginDisableFailed'
};

let startAtLoginErrorKind = null;

function setStartAtLoginBusy(busy) {
  $('#start-at-login').disabled = busy;
}

function renderStartAtLoginError() {
  const el = $('#start-at-login-error');
  const key = startAtLoginErrorKind ? autostartErrorKeys[startAtLoginErrorKind] : null;
  el.textContent = key ? t(key) : '';
  el.hidden = !key;
}

function applyAutostartStatus(status) {
  if (!status || typeof status.enabled !== 'boolean') return;
  preferences.startAtLogin = status.enabled;
  savePreferences();
  $('#start-at-login').checked = status.enabled;
  startAtLoginErrorKind = status.errorKind || null;
  renderStartAtLoginError();
}

async function syncStartAtLogin(enabled) {
  if (!nativeInvoke || previewMode) {
    preferences.startAtLogin = enabled;
    savePreferences();
    $('#start-at-login').checked = enabled;
    return;
  }
  setStartAtLoginBusy(true);
  const status = await invoke('set_start_at_login', { enabled });
  setStartAtLoginBusy(false);
  if (status) applyAutostartStatus(status);
  else {
    $('#start-at-login').checked = preferences.startAtLogin;
    startAtLoginErrorKind = 'query_failed';
    renderStartAtLoginError();
  }
}

async function initializeStartAtLogin() {
  if (!nativeInvoke || previewMode) return;
  setStartAtLoginBusy(true);
  const status = await invoke('initialize_start_at_login', {
    preferredEnabled: preferences.startAtLogin
  });
  setStartAtLoginBusy(false);
  if (status) applyAutostartStatus(status);
}

function syncUiPreferences(extra = {}) {
  if (!nativeInvoke || previewMode) return;
  invoke('sync_ui_preferences', {
    payload: {
      provider: preferences.provider,
      language: preferences.language,
      view: preferences.view,
      alwaysOnTop: preferences.alwaysOnTop,
      ...extra
    }
  });
}

function defaultAlertPreferences() {
  return {
    enabled: false,
    notificationDenied: false,
    codex: {
      fiveHour: { enabled: true, thresholdsRemaining: [10], notifyOnReset: true },
      sevenDay: { enabled: true, thresholdsRemaining: [10], notifyOnReset: true }
    },
    cursor: {
      fiveHour: { enabled: true, thresholdsRemaining: [10], notifyOnReset: true },
      sevenDay: { enabled: true, thresholdsRemaining: [10], notifyOnReset: true }
    }
  };
}

let monitoringPreferences = defaultAlertPreferences();
let alertsErrorKey = null;

function setAlertsBusy(busy) {
  ['#alerts-enabled', '#alert-threshold-20', '#alert-threshold-5', '#alert-on-reset']
    .forEach((selector) => { const el = $(selector); if (el) el.disabled = busy || selector === '#alert-threshold-10'; });
  $('#alert-threshold-10').disabled = true;
}

function renderAlertsError() {
  const el = $('#alerts-error');
  if (!el) return;
  el.textContent = alertsErrorKey ? t(alertsErrorKey) : '';
  el.hidden = !alertsErrorKey;
}

function applyMonitoringPreferences(prefs) {
  if (!prefs) return;
  monitoringPreferences = prefs;
  $('#alerts-enabled').checked = Boolean(prefs.enabled);
  const thresholds = new Set([
    ...(prefs.codex?.fiveHour?.thresholdsRemaining || []),
    ...(prefs.codex?.sevenDay?.thresholdsRemaining || [])
  ]);
  $('#alert-threshold-10').checked = true;
  $('#alert-threshold-20').checked = thresholds.has(20);
  $('#alert-threshold-5').checked = thresholds.has(5);
  $('#alert-on-reset').checked = prefs.codex?.fiveHour?.notifyOnReset !== false;
  alertsErrorKey = prefs.notificationDenied ? 'alertsDenied' : null;
  renderAlertsError();
}

function buildMonitoringPreferencesFromForm() {
  const thresholds = [10];
  if ($('#alert-threshold-20').checked) thresholds.push(20);
  if ($('#alert-threshold-5').checked) thresholds.push(5);
  thresholds.sort((a, b) => b - a);
  const rule = {
    enabled: true,
    thresholdsRemaining: thresholds,
    notifyOnReset: $('#alert-on-reset').checked
  };
  return {
    enabled: $('#alerts-enabled').checked,
    notificationDenied: false,
    codex: { fiveHour: { ...rule }, sevenDay: { ...rule } },
    cursor: { fiveHour: { ...rule }, sevenDay: { ...rule } }
  };
}

async function saveMonitoringPreferences() {
  if (!nativeInvoke || previewMode) return;
  setAlertsBusy(true);
  const next = buildMonitoringPreferencesFromForm();
  const saved = await invoke('set_monitor_preferences', { preferences: next });
  setAlertsBusy(false);
  if (!saved) {
    alertsErrorKey = 'alertsSaveFailed';
    applyMonitoringPreferences(monitoringPreferences);
    renderAlertsError();
    return;
  }
  applyMonitoringPreferences(saved);
}

async function initializeMonitoringPreferences() {
  if (!nativeInvoke || previewMode) return;
  const prefs = await invoke('get_monitor_preferences');
  applyMonitoringPreferences(prefs || defaultAlertPreferences());
}

function ingestSnapshot(snapshot) {
  if (!snapshot?.provider?.kind) return;
  const provider = normalizeProvider(snapshot.provider.kind);
  state.snapshots[provider] = snapshot;
  if (provider === preferences.provider) renderSnapshot();
}

async function checkForAppUpdate() {
  if (!nativeInvoke || previewMode) return;
  const result = await invoke('check_app_update');
  if (!result?.available || !result.version) return;
  const banner = $('#status-banner');
  clearTimeout(state.statusTimer);
  state.statusKey = `update:${result.version}`;
  banner.hidden = false;
  banner.classList.remove('is-error');
  $('#status-copy').textContent = `${t('updateAvailable')} ${result.version}`;
  let actions = $('#update-actions');
  if (!actions) {
    actions = document.createElement('span');
    actions.id = 'update-actions';
    actions.style.marginLeft = '8px';
    banner.appendChild(actions);
  }
  actions.innerHTML = '';
  const install = document.createElement('button');
  install.type = 'button';
  install.className = 'text-button';
  install.textContent = t('updateInstall');
  install.addEventListener('click', async () => {
    install.disabled = true;
    const ok = await invoke('install_app_update');
    if (ok === null) showStatus(t('updateFailed'), true);
  });
  const later = document.createElement('button');
  later.type = 'button';
  later.className = 'text-button';
  later.textContent = t('updateLater');
  later.addEventListener('click', () => {
    banner.hidden = true;
    actions.innerHTML = '';
  });
  actions.append(install, later);
}

function scheduleUpdateChecks() {
  if (!nativeInvoke || previewMode) return;
  setTimeout(() => { checkForAppUpdate(); }, 15_000);
  setInterval(() => { checkForAppUpdate(); }, 24 * 60 * 60 * 1000);
}

function defaultAuthLabel(provider = preferences.provider) {
  return isCursor(provider)
    ? (navigator.platform.toLowerCase().includes('win')
      ? '%APPDATA%\\Cursor\\User\\globalStorage\\state.vscdb'
      : '~/Library/Application Support/Cursor/User/globalStorage/state.vscdb')
    : '~/.codex/auth.json';
}

function previewSnapshot(provider = preferences.provider) {
  const now = Date.now();
  const quota = (remainingPercent, durationSeconds, resetMs, extras = {}) => ({
    remainingPercent, usedPercent: 100 - remainingPercent, durationSeconds,
    resetsAt: new Date(now + resetMs).toISOString(), resetAfterSeconds: Math.floor(resetMs / 1000),
    ...extras
  });
  if (isCursor(provider)) {
    return {
      account: { displayName: 'bianchi@example.com', plan: 'pro' },
      provider: {
        availability: 'live', errorKind: null, kind: 'cursor',
        source: 'preview', authPathLabel: defaultAuthLabel(provider)
      },
      windows: {
        fiveHour: quota(68, 2_592_000, 12 * 86400_000),
        sevenDay: quota(78, 2_592_000, 12 * 86400_000)
      },
      refreshedAt: new Date().toISOString(), checkedAt: new Date().toISOString(), cached: false
    };
  }
  return {
    account: { displayName: 'bianchi@example.com', plan: 'plus' },
    provider: {
      availability: 'live', errorKind: null, kind: 'codex',
      source: 'preview', authPathLabel: '~/.codex/auth.json'
    },
    windows: { fiveHour: quota(72, 18_000, 3 * 3600_000 + 12 * 60_000), sevenDay: quota(68, 604_800, 4 * 86400_000) },
    refreshedAt: new Date().toISOString(), checkedAt: new Date().toISOString(), cached: false
  };
}

function syncProviderControls() {
  $$('#provider-switch .provider-button, .provider-picker button').forEach((button) => {
    const active = button.dataset.provider === preferences.provider;
    button.classList.toggle('is-active', active);
    button.classList.toggle('active', active);
    button.setAttribute('aria-pressed', String(active));
  });
  $('#brand-title').textContent = t('brand');
  document.title = t('brand');
  $$('[data-i18n="fiveRemaining"]').forEach((el) => {
    el.textContent = isCursor() ? t('firstPartyRemaining') : t('fiveRemaining');
  });
  $$('[data-i18n="weekRemaining"]').forEach((el) => {
    el.textContent = isCursor() ? t('apiRemaining') : t('weekRemaining');
  });
}

function setProvider(provider, { persist = true, refreshData = true } = {}) {
  const next = normalizeProvider(provider);
  if (preferences.provider === next) {
    syncProviderControls();
    return;
  }
  preferences.provider = next;
  syncProviderControls();
  if (persist) {
    savePreferences();
    syncUiPreferences({ provider: next });
  }
  if (refreshData) {
    if (currentSnapshot()) renderSnapshot();
    else renderLoadingState();
    refresh({ provider: next, bypassDebounce: true });
  }
}

function setView(view, persist = true) {
  preferences.view = view === 'focus' ? 'focus' : 'dual';
  $('#dual-view').hidden = preferences.view !== 'dual';
  $('#focus-view').hidden = preferences.view !== 'focus';
  $$('.view-picker button').forEach((button) => button.classList.toggle('active', button.dataset.view === preferences.view));
  $('#view-button i').className = preferences.view === 'dual' ? 'ph ph-gauge' : 'ph ph-circles-three-plus';
  if (persist) {
    savePreferences();
    syncUiPreferences({ view: preferences.view });
    renderSnapshot();
  }
}

function setLanguage(language) {
  preferences.language = language === 'en' ? 'en' : 'zh';
  document.documentElement.lang = preferences.language === 'zh' ? 'zh-CN' : 'en';
  $$('[data-i18n]').forEach((element) => {
    if (element.dataset.i18n === 'fiveRemaining' || element.dataset.i18n === 'weekRemaining') return;
    element.textContent = t(element.dataset.i18n);
  });
  $('#account-pill-copy').textContent = t('localAccount');
  $('#language-button').textContent = preferences.language === 'zh' ? 'English' : '中文';
  $('#view-button').ariaLabel = t('switchView'); $('#view-button').title = t('switchView');
  $('#refresh-button').ariaLabel = t('refresh'); $('#refresh-button').title = t('refresh');
  $('#settings-button').ariaLabel = t('settings'); $('#settings-button').title = t('settings');
  $('#settings-close').ariaLabel = t('close');
  $('#dual-view').ariaLabel = t('dualAria');
  $('#focus-view').ariaLabel = t('focusAria');
  $('#provider-switch').ariaLabel = t('providerSwitch');
  $('#provider-codex').ariaLabel = t('switchCodex');
  $('#provider-codex').title = 'Codex';
  $('#provider-cursor').ariaLabel = t('switchCursor');
  $('#provider-cursor').title = 'Cursor';
  syncProviderControls();
  savePreferences();
  syncUiPreferences({ language: preferences.language });
  renderSnapshot();
  renderStartAtLoginError();
  renderAlertsError();
}

function formatPercent(windowData) {
  return windowData ? `${Math.round(windowData.remainingPercent)}%` : '—';
}

function formatAmountHint(windowData) {
  if (!windowData?.unit || windowData.usedAmount == null || windowData.limitAmount == null) return '';
  if (windowData.unit === 'usd_cents') {
    const used = (windowData.usedAmount / 100).toFixed(2);
    const limit = (windowData.limitAmount / 100).toFixed(2);
    return `$${used} / $${limit}`;
  }
  if (windowData.unit === 'requests') {
    return `${Math.round(windowData.usedAmount)} / ${Math.round(windowData.limitAmount)}`;
  }
  return '';
}

function formatReset(windowData) {
  if (!windowData?.resetsAt) return t('noReset');
  const date = new Date(windowData.resetsAt);
  const now = new Date();
  const sameDay = date.toDateString() === now.toDateString();
  const time = new Intl.DateTimeFormat(preferences.language === 'zh' ? 'zh-CN' : 'en', { hour: '2-digit', minute: '2-digit' }).format(date);
  const amount = formatAmountHint(windowData);
  const resetText = sameDay
    ? (preferences.language === 'zh' ? `今天 ${time} ${t('resets')}` : `Today ${time} ${t('resets')}`)
    : `${new Intl.DateTimeFormat(preferences.language === 'zh' ? 'zh-CN' : 'en', { month: 'short', day: 'numeric' }).format(date)} ${time} ${t('resets')}`;
  return amount ? `${resetText} · ${amount}` : resetText;
}

function countdown(windowData) {
  if (!windowData?.resetsAt) return '—';
  const total = Math.max(0, Math.floor((Date.parse(windowData.resetsAt) - Date.now()) / 1000));
  if (total < 60) return preferences.language === 'zh' ? '不足1分钟' : '<1 min';
  const days = Math.floor(total / 86_400);
  const hours = Math.floor(total % 86_400 / 3_600);
  const minutes = Math.floor(total % 3_600 / 60);
  const parts = [];
  if (days > 0) parts.push(preferences.language === 'zh' ? `${days}天` : `${days}d`);
  if (hours > 0) parts.push(preferences.language === 'zh' ? `${hours}小时` : `${hours}h`);
  if (minutes > 0) parts.push(preferences.language === 'zh' ? `${minutes}分` : `${minutes}m`);
  return parts.join(' ');
}

function setRing(selector, windowData) {
  $(selector).style.setProperty('--progress', windowData ? Math.max(0, Math.min(100, windowData.remainingPercent)) : 0);
}

function renderWindow(prefix, windowData) {
  $(`#${prefix}-percent`).textContent = formatPercent(windowData);
  $(`#${prefix}-state`).textContent = windowData ? '' : t('unavailable');
  if ($(`#${prefix}-reset`)) $(`#${prefix}-reset`).textContent = windowData ? formatReset(windowData) : '—';
}

function renderLoadingState() {
  $('#monitor').classList.add('is-loading');
  $('#monitor').ariaBusy = 'true';
  setRing('#five-ring', null); setRing('#week-ring', null); setRing('#focus-ring', null);
  renderWindow('five', null); renderWindow('week', null);
  $('[data-window="fiveHour"]').classList.remove('is-unavailable');
  $('[data-window="sevenDay"]').classList.remove('is-unavailable');
  $('#five-state').textContent = t('loading');
  $('#week-state').textContent = t('loading');
  $('#focus-five-percent').textContent = '—';
  $('#focus-week-percent').textContent = '—';
  $('#focus-five-state').textContent = t('loading');
  $('#focus-week-state').textContent = t('loading');
  $('#focus-week-rail').style.width = '0%';
  $('#next-countdown').textContent = '—';
  $('#next-reset-time').textContent = '—';
  const pill = $('#account-pill');
  pill.classList.remove('is-error', 'is-stale', 'is-warning');
  $('#account-pill-copy').textContent = t('loading');
  $('#source-account').textContent = isCursor() ? t('unknownAccountCursor') : t('unknownAccount');
  $('#source-plan').textContent = '—';
  $('#source-detail').textContent = defaultAuthLabel();
  $('#source-status').textContent = t('loading');
  showStatus('');
}

function showStatus(message, error = false) {
  const banner = $('#status-banner');
  if (!message) {
    banner.hidden = true;
    state.statusKey = '';
    clearTimeout(state.statusTimer);
    return;
  }
  const key = `${error}:${message}`;
  if (state.statusKey === key) return;
  state.statusKey = key;
  clearTimeout(state.statusTimer);
  banner.hidden = false;
  banner.classList.toggle('is-error', error);
  $('#status-copy').textContent = message;
  state.statusTimer = setTimeout(() => { banner.hidden = true; }, 3_000);
}

function providerErrorCopyKey(errorKind, cursor, short = false) {
  const shortKeys = {
    auth_missing: 'authMissingStatus', auth_unreadable: 'authUnreadableStatus',
    auth_invalid: 'authInvalidStatus', reauth_required: 'reauthStatus',
    unsupported_auth: 'unsupportedStatus', network_error: 'networkStatus',
    service_error: 'serviceStatus', invalid_response: 'invalidStatus',
    desktop_required: 'unavailableStatus'
  };
  const detailKeys = {
    auth_missing: cursor ? 'authMissingCursor' : 'authMissing',
    auth_unreadable: cursor ? 'authUnreadableCursor' : 'authUnreadable',
    auth_invalid: cursor ? 'authInvalidCursor' : 'authInvalid',
    reauth_required: cursor ? 'reauthCursor' : 'reauth',
    unsupported_auth: 'unsupportedAuth', network_error: 'networkError',
    service_error: 'serviceError', invalid_response: 'invalidResponse',
    desktop_required: 'nativeOnly'
  };
  return (short ? shortKeys : detailKeys)[errorKind];
}

function providerHealthLabel(snapshot) {
  const provider = snapshot.provider || {};
  if (snapshot.cached) return t('cached');
  const errorKey = providerErrorCopyKey(provider.errorKind, provider.kind === 'cursor', true);
  if (errorKey) return t(errorKey);
  if (provider.availability === 'live') return t('live');
  if (provider.availability === 'partial') return t('partial');
  return t('unavailableStatus');
}

function providerHealthDetail(snapshot) {
  const label = providerHealthLabel(snapshot);
  const provider = snapshot.provider || {};
  if (!snapshot.cached || !provider.errorKind) return label;
  const errorKey = providerErrorCopyKey(provider.errorKind, provider.kind === 'cursor', true);
  return errorKey ? `${label} · ${t(errorKey)}` : label;
}

function localizedProviderMessage(snapshot, five, week) {
  const provider = snapshot.provider || {};
  if (previewMode) return t('preview');
  const cursor = provider.kind === 'cursor' || isCursor();
  const errorKey = providerErrorCopyKey(provider.errorKind, cursor);
  if (snapshot.cached) {
    return errorKey ? `${t('staleNotice')} ${t(errorKey)}` : t('staleNotice');
  }
  if (errorKey) return t(errorKey);
  if (provider.availability === 'live') return '';
  if (provider.availability === 'partial') {
    if (!five && week) return t('missingFive');
    if (five && !week) return t('missingWeek');
    return t('missingBoth');
  }
  return t('missingBoth');
}

function renderSnapshot() {
  const snapshot = currentSnapshot();
  if (!snapshot) {
    renderLoadingState();
    return;
  }
  const five = snapshot.windows?.fiveHour || null;
  const week = snapshot.windows?.sevenDay || null;
  const provider = snapshot.provider || {};

  $('#monitor').classList.remove('is-loading');
  $('#monitor').ariaBusy = 'false';
  const pill = $('#account-pill');
  const hasError = Boolean(provider.errorKind);
  pill.classList.toggle('is-error', hasError && !snapshot.cached);
  pill.classList.toggle('is-stale', snapshot.cached);
  pill.classList.toggle('is-warning', !hasError && provider.availability !== 'live');
  $('#account-pill-copy').textContent = providerHealthLabel(snapshot);

  setRing('#five-ring', five); setRing('#week-ring', week); setRing('#focus-ring', five);
  renderWindow('five', five); renderWindow('week', week);
  $('#focus-five-percent').textContent = formatPercent(five);
  $('#focus-five-state').textContent = five ? formatReset(five) : t('unavailable');
  $('#focus-week-percent').textContent = formatPercent(week);
  $('#focus-week-state').textContent = week ? formatReset(week) : t('unavailable');
  $('#focus-week-rail').style.width = `${week ? Math.max(0, Math.min(100, week.remainingPercent)) : 0}%`;

  $('[data-window="fiveHour"]').classList.toggle('is-unavailable', !five);
  $('[data-window="sevenDay"]').classList.toggle('is-unavailable', !week);
  const next = nextResetWindow(snapshot.windows);
  $('#next-countdown').textContent = countdown(next);
  $('#next-reset-time').textContent = next ? formatReset(next) : '—';

  const unknown = isCursor() ? t('unknownAccountCursor') : t('unknownAccount');
  $('#source-account').textContent = snapshot.account?.displayName || unknown;
  $('#source-plan').textContent = snapshot.account?.plan || '—';
  $('#source-detail').textContent = provider.authPathLabel || defaultAuthLabel();
  $('#source-status').textContent = providerHealthDetail(snapshot);

  const message = localizedProviderMessage(snapshot, five, week);
  pill.title = message || providerHealthDetail(snapshot);
  showStatus(message, hasError && !snapshot.cached && !previewMode);
}

function unavailableSnapshot(provider, errorKind) {
  const now = new Date().toISOString();
  return {
    account: { displayName: isCursor(provider) ? t('unknownAccountCursor') : t('unknownAccount'), plan: '—' },
    provider: {
      availability: 'unavailable', errorKind, kind: provider,
      source: provider === 'cursor' ? 'local_cursor_session' : 'local_codex_oauth',
      authPathLabel: defaultAuthLabel(provider)
    },
    windows: { fiveHour: null, sevenDay: null },
    refreshedAt: null,
    checkedAt: now,
    cached: false
  };
}

function syncRefreshIndicator() {
  $('#refresh-button').classList.toggle('is-spinning', state.requests[preferences.provider].inFlight);
}

async function refresh({ manual = false, provider = preferences.provider, bypassDebounce = false } = {}) {
  const requestedProvider = normalizeProvider(provider);
  const requestId = beginProviderRequest(state, requestedProvider, { manual, bypassDebounce });
  if (requestId == null) return;
  syncRefreshIndicator();
  try {
    let snapshot;
    if (previewMode) snapshot = previewSnapshot(requestedProvider);
    else if (nativeInvoke) {
      try {
        snapshot = await nativeInvoke('refresh_monitor_data', { provider: requestedProvider });
      } catch {
        const cached = state.snapshots[requestedProvider];
        snapshot = cached
          ? {
              ...cached,
              provider: { ...cached.provider, errorKind: 'service_error' },
              checkedAt: new Date().toISOString(),
              cached: true
            }
          : unavailableSnapshot(requestedProvider, 'service_error');
      }
    } else {
      snapshot = unavailableSnapshot(requestedProvider, 'desktop_required');
    }

    completeProviderRequest(state, requestedProvider, requestId, snapshot);
    if (shouldRenderProviderResponse(state, requestedProvider, requestId, preferences.provider)) {
      renderSnapshot();
    }
  } finally {
    finishProviderRequest(state, requestedProvider, requestId);
    syncRefreshIndicator();
  }
}

function openSettings(open) {
  $('#settings-layer').hidden = !open;
  $('#settings-layer').setAttribute('aria-hidden', String(!open));
  $('#settings-button').setAttribute('aria-expanded', String(open));
  if (open) $('#settings-close').focus();
}

document.documentElement.classList.toggle('tauri-host', Boolean(nativeWindow));
document.body.classList.toggle('tauri-host', Boolean(nativeWindow));
document.body.classList.toggle('reduce-motion', preferences.reduceMotion);
$('#always-on-top').checked = preferences.alwaysOnTop;
$('#start-at-login').checked = preferences.startAtLogin;
setView(preferences.view, false);
setLanguage(preferences.language);
syncProviderControls();

$('#drag-handle').addEventListener('pointerdown', (event) => {
  if (event.button !== 0 || event.target.closest('button')) return;
  if (nativeInvoke) invoke('start_window_drag');
});
$$('.resize-handle').forEach((handle) => handle.addEventListener('pointerdown', (event) => {
  if (!nativeWindow || event.button !== 0) return;
  event.preventDefault();
  const key = handle.dataset.direction;
  const enumKey = key[0].toUpperCase() + key.slice(1);
  nativeWindow.startResizeDragging(nativeResizeDirection?.[enumKey] ?? enumKey).catch((error) => console.warn('Native resize failed', error));
}));
$('#view-button').addEventListener('click', () => setView(preferences.view === 'dual' ? 'focus' : 'dual'));
$$('.view-picker button').forEach((button) => button.addEventListener('click', () => setView(button.dataset.view)));
$$('#provider-switch .provider-button, .provider-picker button').forEach((button) => {
  button.addEventListener('click', () => setProvider(button.dataset.provider));
});
$('#refresh-button').addEventListener('click', () => refresh({ manual: true }));
$('#settings-button').addEventListener('click', () => openSettings($('#settings-layer').hidden));
$('#settings-close').addEventListener('click', () => openSettings(false));
$('#settings-layer').addEventListener('pointerdown', (event) => { if (event.target === $('#settings-layer')) openSettings(false); });
$('#language-button').addEventListener('click', () => setLanguage(preferences.language === 'zh' ? 'en' : 'zh'));
$('#always-on-top').addEventListener('change', (event) => {
  preferences.alwaysOnTop = event.target.checked;
  savePreferences();
  invoke('set_always_on_top', { enabled: preferences.alwaysOnTop });
  syncUiPreferences({ alwaysOnTop: preferences.alwaysOnTop });
});
$('#start-at-login').addEventListener('change', (event) => {
  syncStartAtLogin(event.target.checked);
});
['#alerts-enabled', '#alert-threshold-20', '#alert-threshold-5', '#alert-on-reset'].forEach((selector) => {
  $(selector)?.addEventListener('change', () => { saveMonitoringPreferences(); });
});
window.addEventListener('keydown', (event) => { if (event.key === 'Escape') openSettings(false); });

if (nativeInvoke && !previewMode) {
  invoke('set_always_on_top', { enabled: preferences.alwaysOnTop });
  syncUiPreferences();
  initializeStartAtLogin();
  initializeMonitoringPreferences();
  scheduleUpdateChecks();
  nativeListen?.('monitor:refresh', () => refresh({ manual: true }));
  nativeListen?.('monitor:snapshot', (event) => ingestSnapshot(event.payload));
  nativeListen?.('monitor:open-settings', () => openSettings(true));
  nativeListen?.('monitor:set-provider', (event) => setProvider(event.payload, { refreshData: false }));
  nativeListen?.('monitor:set-view', (event) => setView(event.payload));
  nativeListen?.('monitor:set-always-on-top', (event) => {
    preferences.alwaysOnTop = Boolean(event.payload);
    $('#always-on-top').checked = preferences.alwaysOnTop;
    savePreferences();
  });
}
refresh();
setInterval(() => {
  if (currentSnapshot()) renderSnapshot();
}, 30_000);
if (!nativeInvoke || previewMode) {
  setInterval(() => { if (nativeInvoke) refresh(); }, 60_000);
}
