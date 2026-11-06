import { CandidateStore, domObservation, networkObservation, publicUrl } from './media';
export function startBackground(api: typeof chrome): void {
  const store = new CandidateStore();
  api.webRequest.onResponseStarted.addListener(details => {
    const observation = networkObservation(details);
    if (!observation) return;
    void api.tabs.get(observation.tabId).then(tab => store.add(observation, tab)).catch(() => undefined);
  }, { urls: ['http://*/*', 'https://*/*'] }, ['responseHeaders']);
  api.runtime.onMessage.addListener((message, sender, respond) => {
    if (sender.id !== api.runtime.id) return;
    if (message?.type === 'DOM_MEDIA' && sender.tab?.id !== undefined && publicUrl(sender.url) &&
      (!sender.documentLifecycle || sender.documentLifecycle === 'active')) {
      const observation = domObservation(sender.tab.id, message);
      if (!observation) return;
      void api.tabs.get(sender.tab.id).then(tab => store.add(observation, tab)).catch(() => undefined);
    }
    if (message?.type === 'POPUP_STATE' && sender.url === api.runtime.getURL('popup.html')) {
      const selected = Number.isSafeInteger(message.tabId) && message.tabId >= 0
        ? api.tabs.get(message.tabId).then(tab => [tab]) : api.tabs.query({ active: true, currentWindow: true });
      void selected.then(([tab]) => respond({ ready: true, tabId: tab?.id,
        candidates: tab?.id === undefined ? [] : store.list(tab.id) })).catch(() => respond({ ready: true, candidates: [] }));
      return true;
    }
  });
}
