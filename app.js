import '@phosphor-icons/web/regular';

const nativeInvoke = window.__TAURI__?.core?.invoke;
const nativeWindow = window.__TAURI__?.window?.getCurrentWindow?.();
const nativeListen = window.__TAURI__?.event?.listen;
const nativeResizeDirection = window.__TAURI__?.window?.ResizeDirection;
const previewMode = new URLSearchParams(location.search).get('preview') === '1';
const storageKey = 'codex-usage-monitor:widget:v2';

const copy = {
  zh: {
    localAccount: '本机账户', fiveRemaining: '5 小时剩余', weekRemaining: '7 天剩余',
    firstPartyRemaining: 'First-party 剩余', apiRemaining: 'API 剩余',
    nextReset: '下次重置', dualAria: '双环额度视图', focusAria: '聚焦额度视图',
    loading: '正在读取', unavailable: '当前账户未提供', resets: '重置', justNow: '刚刚更新', cached: '缓存数据',
    settingsKicker: '小组件设置', settingsTitle: '显示与启动', viewStyle: '展示样式', dualView: '双环', focusView: '聚焦',
    providerSource: '额度来源', providerSwitch: '额度来源',
    switchCodex: '切换到 Codex 额度', switchCursor: '切换到 Cursor 额度',
    brand: 'Token Monitor',
    language: '界面语言', languageHelp: '中文 / English', alwaysTop: '始终置顶', alwaysTopHelp: '保持小组件浮在其他窗口上方',
    startLogin: '登录时启动', startLoginHelp: '进入系统后自动运行', privacy: '登录令牌只在 Rust 进程内读取，不会发送到前端、日志或导出文件。',
    switchView: '切换视图', refresh: '刷新额度', settings: '打开设置', close: '关闭设置',
    nativeOnly: '请运行桌面应用以读取本机登录态。',
    missingBoth: '已连接，但当前账户没有下发完整额度窗口。',
    missingFive: '已连接；当前账户暂未下发主额度窗口。',
    missingWeek: '已连接；当前账户暂未下发次额度窗口。',
    authMissing: '未找到本机 Codex 登录态，请先运行 codex login。',
    authMissingCursor: '未找到本机 Cursor 登录态，请先在 Cursor 中登录。',
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
    loading: 'Loading', unavailable: 'Not provided for this account', resets: 'reset', justNow: 'Updated just now', cached: 'Cached data',
    settingsKicker: 'Widget settings', settingsTitle: 'Display & startup', viewStyle: 'View style', dualView: 'Dual rings', focusView: 'Focus',
    providerSource: 'Usage source', providerSwitch: 'Usage source',
    switchCodex: 'Switch to Codex usage', switchCursor: 'Switch to Cursor usage',
    brand: 'Token Monitor',
    language: 'Language', languageHelp: '中文 / English', alwaysTop: 'Always on top', alwaysTopHelp: 'Keep the widget above other windows',
    startLogin: 'Start at login', startLoginHelp: 'Launch after signing in to the computer', privacy: 'Sign-in tokens are read only inside the Rust process and never sent to the frontend, logs, or exports.',
    switchView: 'Switch view', refresh: 'Refresh usage', settings: 'Open settings', close: 'Close settings',
    nativeOnly: 'Run the desktop app to read the local sign-in session.',
    missingBoth: 'Connected, but this account did not return complete quota windows.',
    missingFive: 'Connected; this account did not return the primary quota window.',
    missingWeek: 'Connected; this account did not return the secondary quota window.',
    authMissing: 'No local Codex sign-in was found. Run codex login first.',
    authMissingCursor: 'No local Cursor sign-in was found. Sign in to Cursor first.',
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
const state = { snapshot: null, refreshing: false, lastManualRefresh: 0, statusKey: '', statusTimer: null };
const $ = (selector) => document.querySelector(selector);
const $$ = (selector) => [...document.querySelectorAll(selector)];
const t = (key) => copy[preferences.language][key] || key;
const isCursor = () => preferences.provider === 'cursor';

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

function defaultAuthLabel() {
  return isCursor()
    ? (navigator.platform.toLowerCase().includes('win')
      ? '%APPDATA%\\Cursor\\User\\globalStorage\\state.vscdb'
      : '~/Library/Application Support/Cursor/User/globalStorage/state.vscdb')
    : '~/.codex/auth.json';
}

function previewSnapshot() {
  const now = Date.now();
  const quota = (remainingPercent, durationSeconds, resetMs, extras = {}) => ({
    remainingPercent, usedPercent: 100 - remainingPercent, durationSeconds,
    resetsAt: new Date(now + resetMs).toISOString(), resetAfterSeconds: Math.floor(resetMs / 1000),
    ...extras
  });
  if (isCursor()) {
    return {
      account: { displayName: 'bianchi@example.com', plan: 'pro' },
      provider: {
        connected: true, state: 'preview', message: t('preview'), kind: 'cursor',
        source: 'preview', authPathLabel: defaultAuthLabel()
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
      connected: true, state: 'preview', message: t('preview'), kind: 'codex',
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
  const next = provider === 'cursor' ? 'cursor' : 'codex';
  if (preferences.provider === next) {
    syncProviderControls();
    return;
  }
  preferences.provider = next;
  syncProviderControls();
  if (persist) savePreferences();
  if (refreshData) {
    state.refreshing = false;
    state.lastManualRefresh = 0;
    refresh({ manual: true });
  }
}

function setView(view, persist = true) {
  preferences.view = view === 'focus' ? 'focus' : 'dual';
  $('#dual-view').hidden = preferences.view !== 'dual';
  $('#focus-view').hidden = preferences.view !== 'focus';
  $$('.view-picker button').forEach((button) => button.classList.toggle('active', button.dataset.view === preferences.view));
  $('#view-button i').className = preferences.view === 'dual' ? 'ph ph-gauge' : 'ph ph-circles-three-plus';
  if (persist) { savePreferences(); renderSnapshot(); }
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
  renderSnapshot();
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

function localizedProviderMessage(provider, five, week) {
  if (previewMode) return t('preview');
  if (provider.state === 'connected') return '';
  if (provider.state === 'partial') {
    if (!five && week) return t('missingFive');
    if (five && !week) return t('missingWeek');
    return t('missingBoth');
  }
  const cursor = provider.kind === 'cursor' || isCursor();
  const mapped = {
    auth_missing: cursor ? 'authMissingCursor' : 'authMissing',
    auth_unreadable: cursor ? 'authMissingCursor' : 'authMissing',
    auth_invalid: cursor ? 'authMissingCursor' : 'authMissing',
    reauth_required: cursor ? 'reauthCursor' : 'reauth',
    unsupported_auth: 'unsupportedAuth',
    network_error: 'networkError',
    service_error: 'serviceError',
    invalid_response: 'invalidResponse',
    stale: 'staleNotice',
    desktop_required: 'nativeOnly'
  }[provider.state];
  return mapped ? t(mapped) : (provider.message || '');
}

function renderSnapshot() {
  const snapshot = state.snapshot;
  if (!snapshot) return;
  const five = snapshot.windows?.fiveHour || null;
  const week = snapshot.windows?.sevenDay || null;
  const provider = snapshot.provider || {};

  $('#monitor').classList.remove('is-loading');
  $('#monitor').ariaBusy = 'false';
  const pill = $('#account-pill');
  pill.classList.toggle('is-error', !provider.connected && provider.state !== 'stale');
  pill.classList.toggle('is-stale', provider.state === 'stale');
  $('#account-pill-copy').textContent = provider.state === 'stale' ? t('cached') : t('localAccount');

  setRing('#five-ring', five); setRing('#week-ring', week); setRing('#focus-ring', five);
  renderWindow('five', five); renderWindow('week', week);
  $('#focus-five-percent').textContent = formatPercent(five);
  $('#focus-five-state').textContent = five ? formatReset(five) : t('unavailable');
  $('#focus-week-percent').textContent = formatPercent(week);
  $('#focus-week-state').textContent = week ? formatReset(week) : t('unavailable');
  $('#focus-week-rail').style.width = `${week ? Math.max(0, Math.min(100, week.remainingPercent)) : 0}%`;

  $('[data-window="fiveHour"]').classList.toggle('is-unavailable', !five);
  $('[data-window="sevenDay"]').classList.toggle('is-unavailable', !week);
  const next = [five, week].filter(Boolean).sort((a, b) => Date.parse(a.resetsAt) - Date.parse(b.resetsAt))[0] || null;
  $('#next-countdown').textContent = countdown(next);
  $('#next-reset-time').textContent = formatReset(next);

  const unknown = isCursor() ? t('unknownAccountCursor') : t('unknownAccount');
  $('#source-account').textContent = snapshot.account?.displayName || unknown;
  $('#source-plan').textContent = snapshot.account?.plan || '—';
  $('#source-detail').textContent = provider.authPathLabel || defaultAuthLabel();

  const message = localizedProviderMessage(provider, five, week);
  showStatus(message, !provider.connected && provider.state !== 'stale' && !previewMode);
}

async function refresh({ manual = false } = {}) {
  if (state.refreshing) return;
  if (manual && Date.now() - state.lastManualRefresh < 10_000) return;
  if (manual) state.lastManualRefresh = Date.now();
  state.refreshing = true;
  $('#refresh-button').classList.add('is-spinning');
  try {
    if (previewMode) state.snapshot = previewSnapshot();
    else if (nativeInvoke) {
      state.snapshot = await invoke('refresh_monitor_data', { provider: preferences.provider });
    } else {
      state.snapshot = {
        account: { displayName: isCursor() ? t('unknownAccountCursor') : t('unknownAccount'), plan: '—' },
        provider: {
          connected: false, state: 'desktop_required', message: t('nativeOnly'),
          kind: preferences.provider, authPathLabel: defaultAuthLabel()
        },
        windows: { fiveHour: null, sevenDay: null }
      };
    }
    renderSnapshot();
  } finally {
    state.refreshing = false;
    $('#refresh-button').classList.remove('is-spinning');
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
  preferences.alwaysOnTop = event.target.checked; savePreferences(); invoke('set_always_on_top', { enabled: preferences.alwaysOnTop });
});
$('#start-at-login').addEventListener('change', (event) => {
  preferences.startAtLogin = event.target.checked; savePreferences(); invoke('set_start_at_login', { enabled: preferences.startAtLogin });
});
window.addEventListener('keydown', (event) => { if (event.key === 'Escape') openSettings(false); });

if (nativeInvoke) {
  invoke('set_always_on_top', { enabled: preferences.alwaysOnTop });
  if (preferences.startAtLogin) invoke('set_start_at_login', { enabled: true });
  nativeListen?.('monitor:refresh', () => refresh({ manual: true }));
}
refresh();
setInterval(() => {
  if (state.snapshot) renderSnapshot();
}, 30_000);
setInterval(() => { if (nativeInvoke) refresh(); }, 60_000);
