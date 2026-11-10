import { vi } from 'vitest';
export class FakeEvent {
  listeners: ((...args: any[]) => any)[] = [];
  addListener = (fn: (...args: any[]) => any) => { this.listeners.push(fn); };
  removeListener = (fn: (...args: any[]) => any) => { this.listeners = this.listeners.filter(item => item !== fn); };
  emit(...args: any[]): any[] { return this.listeners.map(fn => fn(...args)); }
}
export function fakePort() {
  const onMessage = new FakeEvent(), onDisconnect = new FakeEvent();
  const port = { onMessage, onDisconnect, postMessage: vi.fn(), disconnect: vi.fn(() => onDisconnect.emit()) };
  return port;
}
export function chromeFixture() {
  const onMessage = new FakeEvent(), committed = new FakeEvent(), history = new FakeEvent(), fragment = new FakeEvent();
  const response = new FakeEvent(), changed = new FakeEvent(), updated = new FakeEvent(), removed = new FakeEvent();
  const session: Record<string, unknown> = {}, local: Record<string, unknown> = {};
  const runtime = { id: 'a'.repeat(32), getURL: (path: string) => `chrome-extension://${'a'.repeat(32)}/${path}`,
    onMessage, sendMessage: vi.fn(async () => undefined), connectNative: vi.fn(() => { throw new Error('do not expose raw browser details'); }), lastError: undefined };
  const storage = (data: Record<string, unknown>) => ({ get: vi.fn(async () => ({ ...data })), setAccessLevel: vi.fn(async () => undefined), set: vi.fn(async (value: Record<string, unknown>) => { Object.assign(data, value); }) });
  const api = {
    runtime, storage: { session: storage(session), local: storage(local), onChanged: changed },
    tabs: { get: vi.fn(async (id: number) => ({ id, url: 'https://example.org/watch', title: 'Public video' })),
      query: vi.fn(async () => [{ id: 1, url: 'https://example.org/watch' }]), sendMessage: vi.fn(async (): Promise<unknown> => undefined), onUpdated: updated, onRemoved: removed },
    webNavigation: { onCommitted: committed, onHistoryStateUpdated: history, onReferenceFragmentUpdated: fragment,
      getAllFrames: vi.fn(async () => [{ frameId: 0, documentId: 'doc-1', url: 'https://example.org/watch' }]),
      getFrame: vi.fn(async () => ({ documentId: 'doc-1', url: 'https://example.org/watch', documentLifecycle: 'active' })) },
    webRequest: { onResponseStarted: response }, action: { setBadgeText: vi.fn(async () => undefined) },
  };
  const sender = { id: runtime.id, url: runtime.getURL('popup.html') };
  const request = (message: unknown) => new Promise<any>(resolve => onMessage.emit(message, sender, resolve));
  return { api, chrome: api as unknown as typeof chrome, request, sender, events: { onMessage, committed, history, fragment, response, changed, updated, removed }, session, local };
}
