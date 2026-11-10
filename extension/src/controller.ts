import { CandidateStore, DETAIL_SCAN_LIMIT, domObservation, networkObservation, publicUrl, type DetailScanResult } from './media';
import { failure, readRuntimeSettings, sendCandidate, testConnection, type NativeResult } from './native';
export function startBackground(api: typeof chrome): () => Promise<unknown> {
  const store = new CandidateStore();
  let autoDetect = true;
  let settingsSource: 'default' | 'last-confirmed' = 'default';
  let cacheAvailable = true;
  const pendingSelections = new Set<string>();
  let connectionTestInFlight: Promise<NativeResult> | undefined;
  let detailScanInFlight = false;
  let lastVerifiedAt: string | undefined;
  const readSettings = (value: unknown) => {
    autoDetect = true; settingsSource = 'default';
    if (value && typeof value === 'object' && typeof (value as Record<string, unknown>).browserAutoDetect === 'boolean') {
      autoDetect = (value as { browserAutoDetect: boolean }).browserAutoDetect;
      settingsSource = 'last-confirmed';
    }
    if (!autoDetect) store.clearCandidates();
  };
  const badges = async () => {
    const tabs = await api.tabs.query({});
    await Promise.all(tabs.filter(tab => tab.id !== undefined).map(tab => api.action.setBadgeText({ tabId: tab.id!, text: autoDetect && store.list(tab.id!).length ? String(store.list(tab.id!).length) : '' }).catch(() => undefined)));
  };
  const hydrate = async () => {
    await api.storage.local.setAccessLevel({ accessLevel: 'TRUSTED_CONTEXTS' });
    const settings = await api.storage.local.get('lastRuntimeSettings');
    readSettings(settings.lastRuntimeSettings);
    await api.storage.session.setAccessLevel({ accessLevel: 'TRUSTED_CONTEXTS' });
    const saved = await api.storage.session.get(['candidateSession', 'lastSuccessfulTest']);
    if (typeof saved.lastSuccessfulTest === 'string' && /^\d{4}-\d{2}-\d{2}T[\d:.]+Z$/u.test(saved.lastSuccessfulTest) && Number.isFinite(Date.parse(saved.lastSuccessfulTest))) lastVerifiedAt = saved.lastSuccessfulTest;
    store.restore(saved.candidateSession);
    readSettings(settings.lastRuntimeSettings);
    for (const tabId of store.tabIds()) {
      try {
        const frames = await api.webNavigation.getAllFrames({ tabId });
        const root = frames?.find(frame => frame.frameId === 0);
        if (!root || root.documentId !== store.rootDocument(tabId) || publicUrl(root.url) !== store.pageUrl(tabId)) { store.remove(tabId); continue; }
        for (const frame of frames ?? []) if (frame.frameId !== 0) store.navigate(tabId, {}, frame.documentId, frame.frameId);
      } catch { store.remove(tabId); }
    }
    await badges();
  };
  let queue: Promise<unknown> = hydrate().catch(() => { cacheAvailable = false; });
  const enqueue = (task: () => void | Promise<unknown>) => {
    queue = queue.then(task).then(async () => {
      await badges();
      try { await api.storage.session.set({ candidateSession: store.snapshot() }); cacheAvailable = true; }
      catch { cacheAvailable = false; }
      void api.runtime.sendMessage({ type: 'CANDIDATES_CHANGED' }).catch(() => undefined);
    }).catch(() => { cacheAvailable = false; });
    return queue;
  };
  const activeDocument = async () => {
    const [tab] = await api.tabs.query({ active: true, currentWindow: true });
    if (tab?.id === undefined) return;
    const tabId = tab.id;
    const root = await api.webNavigation.getFrame({ tabId, frameId: 0 });
    const pageUrl = publicUrl(root?.url);
    if (!root || root.documentLifecycle !== 'active' || !pageUrl ||
      !store.accepts(tabId, 0, root.documentId) || store.pageUrl(tabId) !== pageUrl) return;
    return { tabId, documentId: root.documentId, pageUrl };
  };
  const detailScan = async (): Promise<DetailScanResult> => {
    await queue;
    if (!autoDetect) return { code: 'disabled' };
    const target = await activeDocument();
    if (!target) return { code: 'unavailable' };
    if (!autoDetect) return { code: 'disabled' };
    let timer: ReturnType<typeof setTimeout> | undefined;
    const reply: unknown = await Promise.race([
      api.tabs.sendMessage(target.tabId, { type: 'SCAN_OPEN_SHADOW' }, { documentId: target.documentId }),
      new Promise<undefined>(resolve => { timer = setTimeout(() => resolve(undefined), 1500); }),
    ]).finally(() => clearTimeout(timer));
    if (!Array.isArray(reply)) return { code: 'unavailable' };
    let result: DetailScanResult = { code: 'unavailable' };
    await enqueue(async () => {
      if (!autoDetect) { result = { code: 'disabled' }; return; }
      const current = await activeDocument();
      if (!current || current.tabId !== target.tabId || current.documentId !== target.documentId ||
        current.pageUrl !== target.pageUrl) return;
      const before = store.list(target.tabId).length;
      for (const item of reply.slice(0, DETAIL_SCAN_LIMIT)) {
        const observation = domObservation(target.tabId, item);
        if (observation) store.add(observation);
      }
      result = { code: 'ok', added: store.list(target.tabId).length - before };
    });
    return result;
  };
  api.storage.onChanged.addListener((changes, area) => {
    if (area === 'local' && changes.lastRuntimeSettings) void enqueue(async () => {
      readSettings(changes.lastRuntimeSettings.newValue);
      const tabs = await api.tabs.query({});
      await Promise.all(tabs.filter(tab => tab.id !== undefined).map(tab => api.tabs.sendMessage(tab.id!, { type: 'DETECTION_POLICY', enabled: autoDetect }).catch(() => undefined)));
    });
  });
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
    const { tabId, frameId, documentId } = details;
    void enqueue(() => { if (autoDetect && store.accepts(tabId, frameId, documentId)) store.add(observation); });
  }, { urls: ['http://*/*', 'https://*/*'] }, ['responseHeaders']);
  api.runtime.onMessage.addListener((message, sender, respond) => {
    if (sender.id !== api.runtime.id) return;
    if (message?.type === 'CONTENT_POLICY' && sender.tab?.id !== undefined) {
      void queue.then(() => respond({ enabled: !!publicUrl(sender.url) && autoDetect })).catch(() => respond({ enabled: false }));
      return true;
    }
    if (message?.type === 'DOM_MEDIA' && sender.tab?.id !== undefined && publicUrl(sender.url) &&
      (!sender.documentLifecycle || sender.documentLifecycle === 'active')) {
      const observation = domObservation(sender.tab.id, message);
      if (!observation) return;
      const tabId = sender.tab.id, frameId = sender.frameId ?? 0, documentId = sender.documentId;
      void enqueue(() => { if (autoDetect && store.accepts(tabId, frameId, documentId)) store.add(observation); });
    }
    if (message?.type === 'DETAIL_SCAN' && sender.url === api.runtime.getURL('popup.html')) {
      if (detailScanInFlight) { respond({ code: 'busy' }); return; }
      detailScanInFlight = true;
      void detailScan().catch(() => ({ code: 'unavailable' })).then(result => {
        detailScanInFlight = false;
        respond(result);
      });
      return true;
    }
    if (message?.type === 'CONNECTION_TEST' && sender.url === api.runtime.getURL('popup.html')) {
      if (connectionTestInFlight) { respond(failure('CONNECTION_TEST_BUSY')); return; }
      connectionTestInFlight = queue.then(() => testConnection(api.runtime));
      void connectionTestInFlight.then(async result => {
        if (result.ok && result.verifiedAt) {
          lastVerifiedAt = result.verifiedAt;
          try { await api.storage.session.set({ lastSuccessfulTest: lastVerifiedAt }); } catch { cacheAvailable = false; }
        }
        respond(result);
        if (result.ok) {
          const browserAutoDetect = await readRuntimeSettings(api.runtime);
          if (browserAutoDetect !== undefined) {
            await api.storage.local.set({ lastRuntimeSettings: { browserAutoDetect } }).catch(() => undefined);
          }
        }
      }).catch(() => respond(failure('HOST_START_FAILED'))).finally(() => { connectionTestInFlight = undefined; });
      return true;
    }
    if (message?.type === 'DOWNLOAD_SELECTED' && sender.url === api.runtime.getURL('popup.html')) {
      const id: unknown = message.candidateId;
      if (typeof id !== 'string' || id.length > 64 || !Number.isSafeInteger(message.tabId)) { respond(failure('INVALID_PAYLOAD')); return; }
      if (pendingSelections.has(id)) { respond(failure('CANDIDATE_BUSY')); return; }
      pendingSelections.add(id);
      void queue.then(async () => {
        const candidate = store.list(message.tabId).find(row => row.candidateId === id);
        const frames = await api.webNavigation.getAllFrames({ tabId: message.tabId });
        const root = frames?.find(frame => frame.frameId === 0);
        if (!candidate || !root || root.documentId !== store.rootDocument(message.tabId) || publicUrl(root.url) !== candidate.pageUrl) return failure('INVALID_PAYLOAD');
        return sendCandidate(api.runtime, candidate);
      }).then(respond).catch(() => respond(failure('INVALID_PAYLOAD'))).finally(() => pendingSelections.delete(id));
      return true;
    }
    if (message?.type === 'POPUP_STATE' && sender.url === api.runtime.getURL('popup.html')) {
      const selected = Number.isSafeInteger(message.tabId) && message.tabId >= 0
        ? api.tabs.get(message.tabId).then(tab => [tab]) : api.tabs.query({ active: true, currentWindow: true });
      void queue.then(() => selected).then(([tab]) => respond({ ready: true, tabId: tab?.id,
        candidates: tab?.id === undefined ? [] : store.list(tab.id), autoDetect, settingsSource, cacheAvailable, lastVerifiedAt }))
        .catch(() => respond({ ready: true, candidates: [], autoDetect, settingsSource, cacheAvailable, lastVerifiedAt }));
      return true;
    }
  });
  return () => queue;
}
