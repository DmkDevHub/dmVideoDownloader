export function observeVideos(doc: Document, send: (source: { mediaUrl: string; mime?: string }) => void): void {
  let timer: ReturnType<typeof setTimeout> | undefined;
  let paused = false;
  const seen = new WeakMap<Element, string>();
  function emit(element: HTMLVideoElement | HTMLSourceElement, mediaUrl: string, mime?: string) {
    if (!mediaUrl || !/^https?:\/\//iu.test(mediaUrl)) return;
    const key = `${mediaUrl}\n${mime ?? ''}`;
    if (seen.get(element) === key) return;
    seen.set(element, key);
    send({ mediaUrl, ...(mime ? { mime } : {}) });
  }
  function scan() {
    timer = undefined;
    if (paused) return;
    // Bound each sweep; repeated mutations cannot create unbounded pending messages.
    for (const video of Array.from(doc.querySelectorAll('video')).slice(0, 100)) {
      emit(video, video.currentSrc || video.src);
      for (const source of Array.from(video.querySelectorAll('source')).slice(0, 20)) emit(source, source.src, source.type);
    }
  }
  function schedule() { if (!paused && timer === undefined) timer = setTimeout(scan, 100); }
  const observer = new MutationObserver(schedule);
  const watch = () => observer.observe(doc, { subtree: true, childList: true, attributes: true, attributeFilter: ['src', 'type'] });
  watch(); schedule();
  for (const event of ['loadedmetadata', 'loadstart', 'emptied', 'playing']) doc.addEventListener(event, schedule, true);
  doc.defaultView?.addEventListener('pagehide', () => { paused = true; observer.disconnect(); clearTimeout(timer); timer = undefined; });
  doc.defaultView?.addEventListener('pageshow', () => { paused = false; watch(); schedule(); });
}
