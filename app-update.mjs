/**
 * Pure helpers for the settings update UI state machine.
 * States: idle | checking | upToDate | available | confirming | downloading | installing | failed
 */

export function createUpdateUiState() {
  return {
    status: 'idle',
    currentVersion: '',
    version: null,
    notes: null,
    errorKind: null,
    percent: null,
    busy: false
  };
}

export function reduceUpdateUi(state, event) {
  const next = { ...state };
  switch (event.type) {
    case 'check_start':
      if (state.busy) return state;
      return {
        ...next,
        status: 'checking',
        busy: true,
        errorKind: null,
        percent: null
      };
    case 'check_result':
      if (event.errorKind) {
        return {
          ...next,
          status: 'failed',
          busy: false,
          errorKind: event.errorKind,
          version: null,
          notes: null
        };
      }
      if (event.available && event.version) {
        return {
          ...next,
          status: 'available',
          busy: false,
          currentVersion: event.currentVersion || state.currentVersion,
          version: event.version,
          notes: event.notes || null,
          errorKind: null
        };
      }
      return {
        ...next,
        status: 'upToDate',
        busy: false,
        currentVersion: event.currentVersion || state.currentVersion,
        version: null,
        notes: null,
        errorKind: null
      };
    case 'confirm_open':
      if (state.status !== 'available' || state.busy) return state;
      return { ...next, status: 'confirming' };
    case 'confirm_cancel':
      if (state.status !== 'confirming') return state;
      return { ...next, status: 'available', errorKind: null };
    case 'install_start':
      if (state.status !== 'confirming' && state.status !== 'available') return state;
      if (state.busy) return state;
      return {
        ...next,
        status: 'downloading',
        busy: true,
        errorKind: null,
        percent: null
      };
    case 'progress':
      if (!state.busy) return state;
      return {
        ...next,
        status: event.installing ? 'installing' : 'downloading',
        percent: event.percent ?? state.percent
      };
    case 'install_failed':
      return {
        ...next,
        status: 'failed',
        busy: false,
        errorKind: event.errorKind || 'unknown',
        percent: null
      };
    case 'reset':
      return createUpdateUiState();
    default:
      return state;
  }
}

export function parseUpdateError(error) {
  if (!error) return 'unknown';
  const text = String(error);
  const match = text.match(/^(signature|network|disk|install|unknown|no_update|update_busy):/i);
  if (match) return match[1].toLowerCase();
  const lower = text.toLowerCase();
  if (lower.includes('update_busy') || lower.includes('busy')) return 'update_busy';
  if (lower.includes('signature')) return 'signature';
  if (lower.includes('network') || lower.includes('fetch')) return 'network';
  if (lower.includes('disk') || lower.includes('space')) return 'disk';
  if (lower.includes('install')) return 'install';
  return 'unknown';
}

/** Which update controls should be hidden/disabled for a given status. */
export function updateControlVisibility(status) {
  const installing = status === 'downloading' || status === 'installing';
  return {
    checkDisabled: status === 'checking' || installing,
    installHidden: status !== 'available',
    confirmHidden: status !== 'confirming',
    notesHidden: !(status === 'available' || status === 'confirming')
  };
}
