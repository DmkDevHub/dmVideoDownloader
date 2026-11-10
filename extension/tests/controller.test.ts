import { expect, test, vi } from 'vitest';
import { startBackground } from '../src/controller';
import { chromeFixture, fakePort } from './helpers';
async function setup() {
  const fixture = chromeFixture(), settled = startBackground(fixture.chrome);
  await settled();
  fixture.events.committed.emit({ tabId: 1, frameId: 0, documentId: 'doc-1', url: 'https://example.org/watch' });
  await settled();
  const response = { tabId: 1, frameId: 0, documentId: 'doc-1', url: 'https://cdn.example.org/movie.mp4', method: 'GET', statusCode: 200, type: 'media', responseHeaders: [{ name: 'Content-Type', value: 'video/mp4' }] };
  fixture.events.response.emit(response); await settled();
  return { ...fixture, settled, response };
}
test('observation and popup reads never start native messaging; only explicit current candidate selection does', async () => {
  const fixture = await setup();
  const state = await fixture.request({ type: 'POPUP_STATE' });
  expect(state.candidates).toHaveLength(1);
  expect(fixture.api.action.setBadgeText).toHaveBeenLastCalledWith({ tabId: 1, text: '1' });
  expect(fixture.api.runtime.connectNative).not.toHaveBeenCalled();
  fixture.events.onMessage.emit({ type: 'DOWNLOAD_SELECTED', tabId: 1, candidateId: state.candidates[0].candidateId },
    { id: fixture.sender.id, url: 'https://example.org/watch', tab: { id: 1 }, frameId: 0, documentId: 'doc-1' }, () => undefined);
  expect(fixture.api.runtime.connectNative).not.toHaveBeenCalled();
  expect(await fixture.request({ type: 'DOWNLOAD_SELECTED', tabId: 1, candidateId: 'stale' })).toMatchObject({ ok: false });
  expect(fixture.api.runtime.connectNative).not.toHaveBeenCalled();
  expect(await fixture.request({ type: 'DOWNLOAD_SELECTED', tabId: 1, candidateId: state.candidates[0].candidateId })).toMatchObject({ ok: false, code: 'HOST_START_FAILED' });
  expect(fixture.api.runtime.connectNative).toHaveBeenCalledTimes(1);
});
test('last confirmed off clears rows/badge and suppresses new observations without starting the app', async () => {
  const fixture = await setup();
  fixture.events.changed.emit({ lastRuntimeSettings: { newValue: { browserAutoDetect: false } } }, 'local');
  await fixture.settled(); fixture.events.response.emit(fixture.response); await fixture.settled();
  expect(await fixture.request({ type: 'POPUP_STATE' })).toMatchObject({ autoDetect: false, settingsSource: 'last-confirmed', candidates: [] });
  expect(fixture.api.action.setBadgeText).toHaveBeenLastCalledWith({ tabId: 1, text: '' });
  expect(fixture.api.runtime.connectNative).not.toHaveBeenCalled();
  expect(fixture.api.storage.local.set).not.toHaveBeenCalled();
  expect(await fixture.request({ type: 'DETAIL_SCAN' })).toEqual({ code: 'disabled' });
  expect(fixture.api.tabs.sendMessage).not.toHaveBeenCalledWith(1, { type: 'SCAN_OPEN_SHADOW' }, expect.anything());
});
test('detail scan only accepts the popup, binds the active document, and filters candidate data', async () => {
  const fixture = await setup();
  fixture.events.onMessage.emit({ type: 'DETAIL_SCAN' },
    { id: fixture.sender.id, url: 'https://example.org/watch', tab: { id: 1 } }, () => undefined);
  expect(fixture.api.tabs.sendMessage).not.toHaveBeenCalled();
  fixture.api.tabs.sendMessage.mockResolvedValue([
    { mediaUrl: 'https://cdn.example.org/open.mp4' },
    { mediaUrl: 'https://cdn.example.org/private.mp4?token=fixture' },
    { mediaUrl: 'blob:https://example.org/mock' },
  ]);
  expect(await fixture.request({ type: 'DETAIL_SCAN', tabId: 7 })).toEqual({ code: 'ok', added: 1 });
  expect(fixture.api.tabs.sendMessage).toHaveBeenCalledWith(1, { type: 'SCAN_OPEN_SHADOW' }, { documentId: 'doc-1' });
  expect(await fixture.request({ type: 'DETAIL_SCAN' })).toEqual({ code: 'ok', added: 0 });
  expect((await fixture.request({ type: 'POPUP_STATE' })).candidates).toHaveLength(2);
  expect(fixture.api.runtime.connectNative).not.toHaveBeenCalled();
});
test('detail scan discards replies after navigation and refuses overlapping requests', async () => {
  const fixture = await setup();
  let complete!: (reply: unknown) => void;
  fixture.api.tabs.sendMessage.mockImplementationOnce(() => new Promise(resolve => { complete = resolve; }));
  const scanning = fixture.request({ type: 'DETAIL_SCAN' });
  await vi.waitFor(() => expect(complete).toBeTypeOf('function'));
  expect(await fixture.request({ type: 'DETAIL_SCAN' })).toEqual({ code: 'busy' });
  fixture.api.webNavigation.getFrame.mockResolvedValue({ documentId: 'doc-1', url: 'https://example.org/next', documentLifecycle: 'active' });
  fixture.events.history.emit({ tabId: 1, frameId: 0, documentId: 'doc-1', url: 'https://example.org/next' });
  complete([{ mediaUrl: 'https://cdn.example.org/stale.mp4' }]);
  expect(await scanning).toEqual({ code: 'unavailable' });
  expect((await fixture.request({ type: 'POPUP_STATE' })).candidates).toEqual([]);
});
test('detail scan does not commit a reply when detection was disabled during the scan', async () => {
  const fixture = await setup();
  let complete!: (reply: unknown) => void;
  fixture.api.tabs.sendMessage.mockImplementationOnce(() => new Promise(resolve => { complete = resolve; }));
  const scanning = fixture.request({ type: 'DETAIL_SCAN' });
  await vi.waitFor(() => expect(complete).toBeTypeOf('function'));
  fixture.events.changed.emit({ lastRuntimeSettings: { newValue: { browserAutoDetect: false } } }, 'local');
  complete([{ mediaUrl: 'https://cdn.example.org/open.mp4' }]);
  expect(await scanning).toEqual({ code: 'disabled' });
  expect((await fixture.request({ type: 'POPUP_STATE' })).candidates).toEqual([]);
});
test('previous document events and stale selections after navigation are rejected', async () => {
  const fixture = await setup(), state = await fixture.request({ type: 'POPUP_STATE' });
  fixture.events.committed.emit({ tabId: 1, frameId: 0, documentId: 'doc-2', url: 'https://example.org/next' });
  fixture.events.response.emit(fixture.response); await fixture.settled();
  expect((await fixture.request({ type: 'POPUP_STATE' })).candidates).toEqual([]);
  expect(await fixture.request({ type: 'DOWNLOAD_SELECTED', tabId: 1, candidateId: state.candidates[0].candidateId })).toMatchObject({ ok: false });
  expect(fixture.api.runtime.connectNative).not.toHaveBeenCalled();
});
test('duplicate selection is rejected while ACK is pending, and cache errors are visible', async () => {
  const fixture = await setup(), state = await fixture.request({ type: 'POPUP_STATE' });
  const port = fakePort(); fixture.api.runtime.connectNative.mockImplementation(() => port as never);
  const command = { type: 'DOWNLOAD_SELECTED', tabId: 1, candidateId: state.candidates[0].candidateId };
  const first = fixture.request(command);
  expect(await fixture.request(command)).toMatchObject({ ok: false, code: 'CANDIDATE_BUSY' });
  await Promise.resolve(); await Promise.resolve(); await Promise.resolve();
  const sent = port.postMessage.mock.calls[0][0];
  port.onMessage.emit({ version: 1, type: 'ACK', requestId: sent.requestId, payload: { accepted: true, candidateId: command.candidateId } });
  expect(await first).toMatchObject({ ok: true, code: 'ACK' });
  expect(fixture.api.runtime.connectNative).toHaveBeenCalledTimes(1);
  fixture.api.storage.session.set.mockRejectedValueOnce(new Error('test storage failure'));
  fixture.events.response.emit(fixture.response); await fixture.settled();
  expect(await fixture.request({ type: 'POPUP_STATE' })).toMatchObject({ cacheAvailable: false });
});
test('only popup starts diagnosis and a completed test reads the app detection setting on a separate port', async () => {
  const fixture = await setup(), port = fakePort(), settingsPort = fakePort();
  fixture.api.runtime.connectNative.mockImplementationOnce(() => port as never).mockImplementationOnce(() => settingsPort as never);
  fixture.events.onMessage.emit({ type: 'CONNECTION_TEST' }, { id: fixture.sender.id, url: 'https://example.org/watch', tab: { id: 1 } }, () => undefined);
  expect(fixture.api.runtime.connectNative).not.toHaveBeenCalled();
  const result = fixture.request({ type: 'CONNECTION_TEST' });
  expect(await fixture.request({ type: 'CONNECTION_TEST' })).toMatchObject({ code: 'CONNECTION_TEST_BUSY' });
  await Promise.resolve(); await Promise.resolve();
  const ping = port.postMessage.mock.calls[0][0];
  port.onMessage.emit({ version: 1, type: 'PONG', requestId: ping.requestId, payload: { protocolVersion: 1, appReachable: true } });
  expect(fixture.session.lastSuccessfulTest).toBeUndefined();
  port.onMessage.emit({ version: 1, type: 'ACK', requestId: ping.requestId, payload: { accepted: true } });
  expect(await result).toMatchObject({ ok: true });
  const settingsRequest = settingsPort.postMessage.mock.calls[0][0];
  expect(settingsRequest.type).toBe('GET_RUNTIME_STATUS');
  expect(port.disconnect).toHaveBeenCalledOnce();
  settingsPort.onMessage.emit({ version: 1, type: 'RUNTIME_STATUS', requestId: settingsRequest.requestId,
    payload: { protocolVersion: 1, appReachable: true, browserAutoDetect: false } });
  await Promise.resolve(); await Promise.resolve();
  expect(fixture.api.storage.local.set).toHaveBeenCalledWith({ lastRuntimeSettings: { browserAutoDetect: false } });
  const state = await fixture.request({ type: 'POPUP_STATE' });
  expect(state.lastVerifiedAt).toBe(fixture.session.lastSuccessfulTest);
  expect(state.connected).toBeUndefined();
  expect(port.postMessage.mock.calls.some(([message]) => message.type === 'VIDEO_CANDIDATE')).toBe(false);
});
