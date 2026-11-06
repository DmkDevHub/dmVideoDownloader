import { afterEach, expect, test, vi } from 'vitest';
import { HOST_NAME, sendCandidate } from '../src/native';
import { CandidateStore } from '../src/media';
import { fakePort } from './helpers';
function candidate() {
  const store = new CandidateStore(); store.navigate(1, { url: 'https://example.org/watch' }, 'doc');
  return store.add({ tabId: 1, mediaUrl: 'https://cdn.example.org/movie.mp4', sourceType: 'mp4' })!;
}
afterEach(() => vi.useRealTimers());
test('selection sends only protocol v1 candidate fields, and only matching app ACK is accepted', async () => {
  const port = fakePort(), connectNative = vi.fn(() => port as unknown as chrome.runtime.Port);
  const row = candidate(), result = sendCandidate({ connectNative }, row);
  expect(connectNative).toHaveBeenCalledWith(HOST_NAME);
  const sent = port.postMessage.mock.calls[0][0];
  expect(sent).toMatchObject({ version: 1, type: 'VIDEO_CANDIDATE', payload: { candidateId: row.candidateId } });
  expect(Object.keys(sent.payload).sort()).toEqual(['candidateId', 'detectedAt', 'mediaUrl', 'pageUrl', 'sourceType', 'tabId']);
  port.onMessage.emit({ version: 1, type: 'ACK', requestId: sent.requestId, payload: { accepted: true, candidateId: row.candidateId } });
  expect(await result).toMatchObject({ ok: true, code: 'ACK' });
  expect(port.disconnect).toHaveBeenCalledTimes(1);
});
test('wrong ID, rejected candidate, unknown version, oversized and raw errors never report success', async () => {
  for (const mode of ['id', 'candidate', 'version', 'large', 'error']) {
    const port = fakePort();
    const result = sendCandidate({ connectNative: () => port as unknown as chrome.runtime.Port }, candidate());
    const sent = port.postMessage.mock.calls[0][0];
    const reply = { version: mode === 'version' ? 99 : 1, type: mode === 'error' ? 'ERROR' : 'ACK',
      requestId: mode === 'id' ? 'wrong' : sent.requestId,
      payload: { accepted: true, candidateId: mode === 'candidate' ? 'wrong' : sent.payload.candidateId,
        code: 'APP_NOT_RUNNING', safeMessage: 'DO_NOT_EXPOSE_SECRET', ...(mode === 'large' ? { junk: 'x'.repeat(70_000) } : {}) } };
    port.onMessage.emit(reply);
    expect(await result).toMatchObject({ ok: false });
    expect(JSON.stringify(await result)).not.toContain('DO_NOT_EXPOSE_SECRET');
  }
});
test('disconnect and timeout are bounded, and credential URLs never invoke a native host', async () => {
  vi.useFakeTimers();
  const port = fakePort(), connectNative = vi.fn(() => port as unknown as chrome.runtime.Port);
  const result = sendCandidate({ connectNative }, candidate());
  await vi.advanceTimersByTimeAsync(15_000);
  expect(await result).toMatchObject({ ok: false, code: 'IPC_TIMEOUT' });
  const second = fakePort(), disconnected = sendCandidate({ connectNative: () => second as unknown as chrome.runtime.Port }, candidate());
  second.onDisconnect.emit(); expect(await disconnected).toMatchObject({ ok: false, code: 'HOST_START_FAILED' });
  const forbidden = { ...candidate(), mediaUrl: 'https://example.org/a.mp4?token=secret' };
  connectNative.mockClear(); expect(await sendCandidate({ connectNative }, forbidden)).toMatchObject({ code: 'URL_NOT_ALLOWED' });
  expect(connectNative).not.toHaveBeenCalled();
});
