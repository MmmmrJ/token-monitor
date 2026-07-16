export const PROVIDER_NAMES = ['codex', 'cursor'];

export function normalizeProvider(value) {
  return value === 'cursor' ? 'cursor' : 'codex';
}

export function createProviderState() {
  return {
    snapshots: Object.fromEntries(PROVIDER_NAMES.map((provider) => [provider, null])),
    requests: Object.fromEntries(PROVIDER_NAMES.map((provider) => [provider, {
      inFlight: false,
      latestRequestId: 0,
      lastManualRefresh: 0
    }]))
  };
}

export function beginProviderRequest(state, provider, options = {}) {
  const normalized = normalizeProvider(provider);
  const bucket = state.requests[normalized];
  const now = options.now ?? Date.now();
  const manual = options.manual === true;
  const bypassDebounce = options.bypassDebounce === true;
  const debounceMs = options.debounceMs ?? 10_000;

  if (bucket.inFlight) return null;
  if (manual && !bypassDebounce && now - bucket.lastManualRefresh < debounceMs) return null;
  if (manual) bucket.lastManualRefresh = now;

  bucket.inFlight = true;
  bucket.latestRequestId += 1;
  return bucket.latestRequestId;
}

export function completeProviderRequest(state, provider, requestId, snapshot) {
  const normalized = normalizeProvider(provider);
  const bucket = state.requests[normalized];
  if (requestId !== bucket.latestRequestId) return false;
  bucket.inFlight = false;
  state.snapshots[normalized] = snapshot;
  return true;
}

export function finishProviderRequest(state, provider, requestId) {
  const normalized = normalizeProvider(provider);
  const bucket = state.requests[normalized];
  if (requestId !== bucket.latestRequestId) return false;
  bucket.inFlight = false;
  return true;
}

export function shouldRenderProviderResponse(state, provider, requestId, currentProvider) {
  const normalized = normalizeProvider(provider);
  return normalized === normalizeProvider(currentProvider)
    && state.requests[normalized].latestRequestId === requestId;
}

export function nextResetWindow(windows) {
  return [windows?.fiveHour, windows?.sevenDay]
    .filter((windowData) => {
      if (!windowData?.resetsAt) return false;
      return Number.isFinite(Date.parse(windowData.resetsAt));
    })
    .sort((left, right) => Date.parse(left.resetsAt) - Date.parse(right.resetsAt))[0] ?? null;
}
