export type SourceType = 'mp4' | 'webm' | 'hls' | 'dash' | 'unknown';
export interface Candidate {
  candidateId: string;
  tabId: number;
  pageUrl: string;
  pageTitle?: string;
  mediaUrl: string;
  sourceType: SourceType;
  detectedAt: string;
  status: 'detected';
}

// Candidates never fetch URLs. DNS/redirect/subrequest enforcement belongs to R4's engine.
// Reject literal internal destinations and recognizable embedded credentials before caching.
export function publicUrl(value: unknown): string | null {
  if (typeof value !== 'string' || value.length > 8192 || /[\u0000-\u0020\u007f]/u.test(value)) return null;
  let url: URL;
  try { url = new URL(value); } catch { return null; }
  if (!['http:', 'https:'].includes(url.protocol) || url.username || url.password) return null;
  const host = url.hostname.toLowerCase().replace(/\.$/, '');
  if (!host.includes('.') && !host.startsWith('[')) return null;
  if (/(^|\.)(localhost|local|internal|lan|home|invalid)$/u.test(host)) return null;
  if (/^\d+\.\d+\.\d+\.\d+$/u.test(host)) {
    const [a, b] = host.split('.').map(Number);
    if (a === 0 || a === 10 || a === 127 || a >= 224 ||
      (a === 100 && b >= 64 && b <= 127) || (a === 169 && b === 254) ||
      (a === 172 && b >= 16 && b <= 31) || (a === 192 && (b === 168 || b === 0)) ||
      (a === 198 && (b === 18 || b === 19))) return null;
  }
  if (host.startsWith('[') && (!/^\[[23][\da-f]{3}:/u.test(host) || host.startsWith('[2001:db8:'))) return null;
  for (const [key, entry] of url.searchParams) {
    const normalized = key.toLowerCase().replace(/[^a-z0-9]/gu, '');
    if (/token|signature|password|passwd|secret|credential|authorization|session|jwt|xamz|xgoog/iu.test(normalized) ||
      ['auth', 'sig', 'key', 'apikey', 'policy', 'hdnea', 'hdnts'].includes(normalized) ||
      /^bearer\s/iu.test(entry) || /^[\w-]+\.[\w-]+\.[\w-]+$/u.test(entry)) return null;
  }
  if (/(?:^|\/)(?:token|auth|session|bearer)(?:\/|$)/iu.test(url.pathname)) return null;
  url.hash = '';
  return url.href;
}

export function classifyMedia(url: string, mime = ''): SourceType | null {
  if (isSegment(url, mime)) return null;
  const type = mime.split(';')[0].trim().toLowerCase();
  if (['application/vnd.apple.mpegurl', 'application/x-mpegurl', 'audio/mpegurl', 'audio/x-mpegurl'].includes(type)) return 'hls';
  if (type === 'application/dash+xml') return 'dash';
  if (type === 'video/mp4') return 'mp4';
  if (type === 'video/webm') return 'webm';
  if (type && !type.startsWith('video/') && type !== 'application/octet-stream') return null;
  const path = new URL(url).pathname.toLowerCase();
  if (path.endsWith('.mp4')) return 'mp4';
  if (path.endsWith('.webm')) return 'webm';
  if (path.endsWith('.m3u8')) return 'hls';
  if (path.endsWith('.mpd')) return 'dash';
  return type.startsWith('video/') ? 'unknown' : null;
}

export interface Observation { tabId: number; mediaUrl: string; sourceType: SourceType }
export function isSegment(url: string, mime = ''): boolean {
  const parsed = new URL(url);
  if (/\.(?:ts|m4s|m4a|aac|mp3|vtt|key)$/iu.test(parsed.pathname) || mime.split(';')[0].trim().toLowerCase() === 'video/mp2t') return true;
  if (/(?:^|[\/_.-])(?:seg(?:ment)?s?|chunks?|frag(?:ment)?s?|init)(?:[\/_.-]?\d|[\/_.-]|$)/iu.test(parsed.pathname)) return true;
  return Array.from(parsed.searchParams.keys()).some(key => /^(?:range|bytestart|byteend|sq|seg(?:ment)?|fragment|chunk)$/iu.test(key));
}
function mediaIdentity(value: string): string {
  const url = new URL(value);
  for (const key of Array.from(url.searchParams.keys())) {
    if (/^(?:utm_.+|fbclid|gclid)$/iu.test(key)) url.searchParams.delete(key);
  }
  url.searchParams.sort(); return url.href;
}
export function domObservation(tabId: number, message: unknown): Observation | null {
  if (!Number.isSafeInteger(tabId) || tabId < 0 || !message || typeof message !== 'object') return null;
  const input = message as Record<string, unknown>;
  const mediaUrl = publicUrl(input.mediaUrl);
  if (!mediaUrl || (input.mime !== undefined && (typeof input.mime !== 'string' || input.mime.length > 200))) return null;
  const sourceType = classifyMedia(mediaUrl, typeof input.mime === 'string' ? input.mime : '');
  return sourceType ? { tabId, mediaUrl, sourceType } : null;
}
export function networkObservation(details: {
  tabId: number; url: string; method: string; statusCode: number; type: string;
  responseHeaders?: { name: string; value?: string }[];
}): Observation | null {
  if (!Number.isSafeInteger(details.tabId) || details.tabId < 0 || details.method !== 'GET' ||
    details.statusCode < 200 || details.statusCode >= 300 ||
    !['media', 'xmlhttprequest', 'other', 'main_frame'].includes(details.type)) return null;
  const mediaUrl = publicUrl(details.url);
  if (!mediaUrl) return null;
  // Do not copy, retain, log or forward response headers. Read only Content-Type.
  const mime = details.responseHeaders?.find(h => h.name.toLowerCase() === 'content-type')?.value ?? '';
  const sourceType = classifyMedia(mediaUrl, mime);
  return sourceType ? { tabId: details.tabId, mediaUrl, sourceType } : null;
}

export class CandidateStore {
  private tabs = new Map<number, { pageUrl: string; pageTitle?: string; documentId: string; frames: Record<string, string>; rows: Candidate[] }>();
  static readonly perTabLimit = 30;
  static readonly totalLimit = 200;
  static readonly tabLimit = 20;
  navigate(tabId: number, page: { url?: string; title?: string }, documentId: string, frameId = 0): void {
    const pageUrl = publicUrl(page.url);
    if (!Number.isSafeInteger(tabId) || tabId < 0 || typeof documentId !== 'string' || !documentId || documentId.length > 128) return;
    if (frameId !== 0) {
      const tab = this.tabs.get(tabId);
      if (tab && Number.isSafeInteger(frameId) && frameId > 0 && Object.keys(tab.frames).length < 100) tab.frames[frameId] = documentId;
      return;
    }
    this.remove(tabId);
    if (!pageUrl) return;
    this.tabs.set(tabId, { pageUrl, pageTitle: cleanTitle(page.title), documentId, frames: { 0: documentId }, rows: [] });
    while (this.tabs.size > CandidateStore.tabLimit) this.tabs.delete(this.tabs.keys().next().value!);
  }
  accepts(tabId: number, frameId: number, documentId?: string): boolean {
    return !!documentId && this.tabs.get(tabId)?.frames[frameId] === documentId;
  }
  rootDocument(tabId: number): string | undefined { return this.tabs.get(tabId)?.documentId; }
  pageUrl(tabId: number): string | undefined { return this.tabs.get(tabId)?.pageUrl; }
  tabIds(): number[] { return Array.from(this.tabs.keys()); }
  remove(tabId: number): void { this.tabs.delete(tabId); }
  clearCandidates(): void { for (const tab of this.tabs.values()) tab.rows = []; }
  add(observation: Observation): Candidate | null {
    const tab = this.tabs.get(observation.tabId);
    const mediaUrl = publicUrl(observation.mediaUrl);
    if (!tab || !mediaUrl || isSegment(mediaUrl)) return null;
    const rows = tab.rows;
    const identity = mediaIdentity(mediaUrl);
    const existing = rows.find(row => mediaIdentity(row.mediaUrl) === identity);
    if (existing) {
      if (existing.sourceType === 'unknown') existing.sourceType = observation.sourceType;
      return existing;
    }
    if (rows.length >= CandidateStore.perTabLimit || Array.from(this.tabs.values()).reduce((n, entry) => n + entry.rows.length, 0) >= CandidateStore.totalLimit) return null;
    const candidate: Candidate = {
      candidateId: crypto.randomUUID(), tabId: observation.tabId, pageUrl: tab.pageUrl,
      pageTitle: tab.pageTitle,
      mediaUrl, sourceType: observation.sourceType,
      detectedAt: new Date().toISOString(), status: 'detected',
    };
    rows.push(candidate);
    return candidate;
  }
  list(tabId: number): Candidate[] { return (this.tabs.get(tabId)?.rows ?? []).map(row => ({ ...row })); }
  snapshot(): unknown {
    return { schemaVersion: 1, tabs: Array.from(this.tabs, ([tabId, state]) => ({ tabId, ...state })) };
  }
  restore(snapshot: unknown): void {
    if (!snapshot || typeof snapshot !== 'object') return;
    const data = snapshot as Record<string, unknown>;
    if (data.schemaVersion !== 1 || !Array.isArray(data.tabs)) return;
    for (const entry of data.tabs.slice(0, CandidateStore.tabLimit)) {
      if (!entry || typeof entry !== 'object') continue;
      const state = entry as Record<string, unknown>;
      if (!Number.isSafeInteger(state.tabId) || typeof state.documentId !== 'string' || !Array.isArray(state.rows)) continue;
      const tabId = state.tabId as number;
      this.navigate(tabId, { url: typeof state.pageUrl === 'string' ? state.pageUrl : '', title: typeof state.pageTitle === 'string' ? state.pageTitle : undefined }, state.documentId);
      // Frame document IDs are re-read from Chrome during hydration; only root survives here.
      for (const row of state.rows.slice(0, CandidateStore.perTabLimit)) {
        if (!row || typeof row !== 'object') continue;
        const item = row as Record<string, unknown>;
        if (typeof item.candidateId !== 'string' || !/^[\da-f]{8}(?:-[\da-f]{4}){3}-[\da-f]{12}$/iu.test(item.candidateId) ||
          typeof item.mediaUrl !== 'string' || typeof item.sourceType !== 'string' ||
          !['mp4', 'webm', 'hls', 'dash', 'unknown'].includes(item.sourceType) ||
          typeof item.detectedAt !== 'string' || !Number.isFinite(Date.parse(item.detectedAt)) ||
          item.status !== 'detected' || item.tabId !== tabId || item.pageUrl !== state.pageUrl) continue;
        const candidate = this.add({ tabId, mediaUrl: item.mediaUrl, sourceType: item.sourceType as SourceType });
        if (candidate) { candidate.candidateId = item.candidateId; candidate.detectedAt = item.detectedAt; }
      }
    }
  }
}
function cleanTitle(value?: string): string | undefined {
  return typeof value === 'string' ? value.replace(/[\u0000-\u001f\u007f]/gu, '').replace(/https?:\/\/[^\s]+/giu, input => {
    try { return new URL(input).hostname; } catch { return '영상 주소'; }
  }).slice(0, 200) : undefined;
}
