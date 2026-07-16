import assert from 'node:assert/strict';
import test from 'node:test';
import {
  beginProviderRequest,
  completeProviderRequest,
  createProviderState,
  nextResetWindow,
  shouldRenderProviderResponse
} from '../app-core.mjs';

test('provider responses remain in their own buckets when completion order reverses', () => {
  const state = createProviderState();
  const codexRequest = beginProviderRequest(state, 'codex');
  const cursorRequest = beginProviderRequest(state, 'cursor');

  completeProviderRequest(state, 'cursor', cursorRequest, { provider: 'cursor' });
  assert.equal(shouldRenderProviderResponse(state, 'cursor', cursorRequest, 'cursor'), true);

  completeProviderRequest(state, 'codex', codexRequest, { provider: 'codex' });
  assert.equal(shouldRenderProviderResponse(state, 'codex', codexRequest, 'cursor'), false);
  assert.deepEqual(state.snapshots.cursor, { provider: 'cursor' });
  assert.deepEqual(state.snapshots.codex, { provider: 'codex' });
});

test('twenty provider switches never render the other provider response', () => {
  const state = createProviderState();
  for (let index = 0; index < 20; index += 1) {
    const provider = index % 2 === 0 ? 'codex' : 'cursor';
    const other = provider === 'codex' ? 'cursor' : 'codex';
    const requestId = beginProviderRequest(state, provider);
    completeProviderRequest(state, provider, requestId, { provider, index });
    assert.equal(shouldRenderProviderResponse(state, provider, requestId, provider), true);
    assert.equal(shouldRenderProviderResponse(state, provider, requestId, other), false);
  }
});

test('manual refresh debounce is isolated per provider and can be bypassed on switch', () => {
  const state = createProviderState();
  const codexRequest = beginProviderRequest(state, 'codex', { manual: true, now: 20_000 });
  completeProviderRequest(state, 'codex', codexRequest, {});

  assert.equal(beginProviderRequest(state, 'codex', { manual: true, now: 25_000 }), null);
  assert.notEqual(beginProviderRequest(state, 'cursor', { manual: true, now: 25_000 }), null);
  completeProviderRequest(state, 'cursor', 1, {});
  assert.notEqual(beginProviderRequest(state, 'codex', {
    manual: true,
    bypassDebounce: true,
    now: 25_000
  }), null);
});

test('next reset ignores windows without a valid reset timestamp', () => {
  const next = nextResetWindow({
    fiveHour: { resetsAt: null },
    sevenDay: { resetsAt: '2026-07-16T00:00:00Z' }
  });
  assert.equal(next.resetsAt, '2026-07-16T00:00:00Z');
  assert.equal(nextResetWindow({ fiveHour: { resetsAt: null } }), null);
});
