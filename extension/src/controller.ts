import { CandidateStore, domObservation, networkObservation, publicUrl } from './media';
export function startBackground(api: typeof chrome): void {
  const store = new CandidateStore();
  const hydrate = async () => {
    await api.storage.session.setAccessLevel({ accessLevel: 'TRUSTED_CONTEXTS' });
    const saved = await api.storage.session.get('candidateSession');
    store.restore(saved.candidateSession);
    for (const tabId of store.tabIds()) {
      try {
        const frames = await api.webNavigation.getAllFrames({ tabId });
        const root = frames?.find(frame => frame.frameId === 0);
        if (!root || root.documentId !== store.rootDocument(tabId) || publicUrl(root.url) !== store.pageUrl(tabId)) { store.remove(tabId); continue; }
        for (const frame of frames ?? []) if (frame.frameId !== 0) store.navigate(tabId, {}, frame.documentId, frame.frameId);
      } catch { store.remove(tabId); }
    }
  };
  let queue: Promise<unknown> = hydrate().catch(() => undefined);
  const enqueue = (task: () => void | Promise<unknown>) => {
    queue = queue.then(task).then(() => api.storage.session.set({ candidateSession: store.snapshot() })).catch(() => undefined);
    return queue;
  };
  api.webNavigation.onCommitted.addListener(details => {
    void enqueue(async () => {
      const tab = await api.tabs.get(details.tabId).catch(() => undefined);
      if (tab) store.navigate(details.tabId, { url: details.url, title: tab.title }, details.documentId, details.frameId);
    });
  });
  const sameDocument = (details: chrome.webNavigation.WebNavigationTransitionCallbackDetails) => {
    if (details.frameId !== 0) return;
    void enqueue(() => store.navigate(details.tabId, { url: details.url }, details.documentId));
  };
  api.webNavigation.onHistoryStateUpdated.addListener(sameDocument);
  api.webNavigation.onReferenceFragmentUpdated.addListener(sameDocument);
  api.tabs.onUpdated.addListener((tabId, change) => {
    if (change.status === 'loading') void enqueue(() => store.remove(tabId));
  });
  api.tabs.onRemoved.addListener(tabId => { void enqueue(() => store.remove(tabId)); });
  api.webRequest.onResponseStarted.addListener(details => {
    const observation = networkObservation(details);
    if (!observation) return;
    void enqueue(() => { if (store.accepts(details.tabId, details.frameId, details.documentId)) store.add(observation); });
  }, { urls: ['http://*/*', 'https://*/*'] }, ['responseHeaders']);
  api.runtime.onMessage.addListener((message, sender, respond) => {
    if (sender.id !== api.runtime.id) return;
    if (message?.type === 'DOM_MEDIA' && sender.tab?.id !== undefined && publicUrl(sender.url) &&
      (!sender.documentLifecycle || sender.documentLifecycle === 'active')) {
      const observation = domObservation(sender.tab.id, message);
      if (!observation) return;
      void enqueue(() => { if (store.accepts(sender.tab!.id!, sender.frameId ?? 0, sender.documentId)) store.add(observation); });
    }
    if (message?.type === 'POPUP_STATE' && sender.url === api.runtime.getURL('popup.html')) {
      const selected = Number.isSafeInteger(message.tabId) && message.tabId >= 0
        ? api.tabs.get(message.tabId).then(tab => [tab]) : api.tabs.query({ active: true, currentWindow: true });
      void queue.then(() => selected).then(([tab]) => respond({ ready: true, tabId: tab?.id,
        candidates: tab?.id === undefined ? [] : store.list(tab.id) })).catch(() => respond({ ready: true, candidates: [] }));
      return true;
    }
  });
}
