import { contentMessage, DETAIL_SCAN_LIMIT } from './media';

export function observeVideos(doc: Document, send: (source: { mediaUrl: string; mime?: string }) => void): { setEnabled: (enabled: boolean) => void } {
  let timer: ReturnType<typeof setTimeout> | undefined;
  let paused = false;
  let enabled = false;
  let seen = new WeakMap<Element, string>();
  function emit(element: HTMLVideoElement | HTMLSourceElement, mediaUrl: string, mime?: string) {
    if (!mediaUrl || !/^https?:\/\//iu.test(mediaUrl)) return;
    const key = `${mediaUrl}\n${mime ?? ''}`;
    if (seen.get(element) === key) return;
    seen.set(element, key);
    send({ mediaUrl, ...(mime ? { mime } : {}) });
  }
  function scan() {
    timer = undefined;
    if (paused || !enabled) return;
    // Bound each sweep; repeated mutations cannot create unbounded pending messages.
    for (const video of Array.from(doc.querySelectorAll('video')).slice(0, 100)) {
      emit(video, video.currentSrc || video.src);
      for (const source of Array.from(video.querySelectorAll('source')).slice(0, 20)) emit(source, source.src, source.type);
    }
  }
  function schedule() { if (!paused && enabled && timer === undefined) timer = setTimeout(scan, 100); }
  const observer = new MutationObserver(schedule);
  const watch = () => observer.observe(doc, { subtree: true, childList: true, attributes: true, attributeFilter: ['src', 'type'] });
  watch(); schedule();
  for (const event of ['loadedmetadata', 'loadstart', 'emptied', 'playing']) doc.addEventListener(event, schedule, true);
  doc.defaultView?.addEventListener('pagehide', () => { paused = true; observer.disconnect(); clearTimeout(timer); timer = undefined; });
  doc.defaultView?.addEventListener('pageshow', () => { paused = false; watch(); schedule(); });
  return { setEnabled(value) {
    enabled = value; seen = new WeakMap();
    clearTimeout(timer); timer = undefined;
    if (enabled) schedule();
  } };
}
export function scanOpenShadowVideos(doc: Document): NonNullable<ReturnType<typeof contentMessage>>[] {
  const results = new Map<string, NonNullable<ReturnType<typeof contentMessage>>>();
  const walkers = [doc.createTreeWalker(doc, NodeFilter.SHOW_ELEMENT)];
  for (let visited = 0; walkers.length && visited < 5_000 && results.size < DETAIL_SCAN_LIMIT;) {
    const walker = walkers[walkers.length - 1];
    const element = walker.nextNode() as Element | null;
    if (!element) { walkers.pop(); continue; }
    visited++;
    if (element.shadowRoot) walkers.push(doc.createTreeWalker(element.shadowRoot, NodeFilter.SHOW_ELEMENT));
    if (walker.root === doc) continue;
    const source = element instanceof HTMLVideoElement
      ? { mediaUrl: element.currentSrc || element.src }
      : element instanceof HTMLSourceElement && element.parentElement instanceof HTMLVideoElement
        ? { mediaUrl: element.src, mime: element.type } : null;
    const message = source && contentMessage(source);
    if (message) results.set(message.mediaUrl, message);
  }
  return [...results.values()];
}
