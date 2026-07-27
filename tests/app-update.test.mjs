import test from 'node:test';
import assert from 'node:assert/strict';
import {
  createUpdateUiState,
  parseUpdateError,
  reduceUpdateUi,
  updateControlVisibility
} from '../app-update.mjs';

test('update ui moves through check available confirm and cancel without failing', () => {
  let state = createUpdateUiState();
  state = reduceUpdateUi(state, { type: 'check_start' });
  assert.equal(state.status, 'checking');
  state = reduceUpdateUi(state, {
    type: 'check_result',
    available: true,
    currentVersion: '1.2.0',
    version: '1.3.0',
    notes: 'notes'
  });
  assert.equal(state.status, 'available');
  state = reduceUpdateUi(state, { type: 'confirm_open' });
  assert.equal(state.status, 'confirming');
  state = reduceUpdateUi(state, { type: 'confirm_cancel' });
  assert.equal(state.status, 'available');
  assert.equal(state.errorKind, null);
});

test('background-style check failure lands in failed with kind', () => {
  let state = createUpdateUiState();
  state = reduceUpdateUi(state, { type: 'check_start' });
  state = reduceUpdateUi(state, { type: 'check_result', errorKind: 'network' });
  assert.equal(state.status, 'failed');
  assert.equal(state.errorKind, 'network');
  assert.equal(state.busy, false);
});

test('install progress and failure update state', () => {
  let state = createUpdateUiState();
  state = reduceUpdateUi(state, { type: 'check_start' });
  state = reduceUpdateUi(state, {
    type: 'check_result',
    available: true,
    version: '1.3.0',
    currentVersion: '1.2.0'
  });
  state = reduceUpdateUi(state, { type: 'confirm_open' });
  state = reduceUpdateUi(state, { type: 'install_start' });
  assert.equal(state.status, 'downloading');
  state = reduceUpdateUi(state, { type: 'progress', percent: 40 });
  assert.equal(state.percent, 40);
  state = reduceUpdateUi(state, { type: 'install_failed', errorKind: 'signature' });
  assert.equal(state.status, 'failed');
  assert.equal(state.errorKind, 'signature');
});

test('parseUpdateError reads rust kind prefixes', () => {
  assert.equal(parseUpdateError('signature:bad sig'), 'signature');
  assert.equal(parseUpdateError('network:offline'), 'network');
  assert.equal(parseUpdateError('update_busy'), 'update_busy');
});

test('ignores duplicate check while busy', () => {
  let state = createUpdateUiState();
  state = reduceUpdateUi(state, { type: 'check_start' });
  const again = reduceUpdateUi(state, { type: 'check_start' });
  assert.equal(again.status, 'checking');
  assert.equal(again.busy, true);
});

test('update control visibility hides confirm when up to date', () => {
  assert.deepEqual(updateControlVisibility('upToDate'), {
    checkDisabled: false,
    installHidden: true,
    confirmHidden: true,
    notesHidden: true
  });
  assert.deepEqual(updateControlVisibility('available'), {
    checkDisabled: false,
    installHidden: false,
    confirmHidden: true,
    notesHidden: false
  });
  assert.deepEqual(updateControlVisibility('confirming'), {
    checkDisabled: false,
    installHidden: true,
    confirmHidden: false,
    notesHidden: false
  });
  assert.deepEqual(updateControlVisibility('downloading'), {
    checkDisabled: true,
    installHidden: true,
    confirmHidden: true,
    notesHidden: true
  });
});
