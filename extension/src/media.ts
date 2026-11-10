import { publicUrl } from '../../shared/publicUrl';
export { publicUrl };
export type SourceType = 'mp4' | 'webm' | 'hls' | 'dash' | 'unknown';
export const DETAIL_SCAN_LIMIT = 30;
export type DetailScanResult = { code: 'ok'; added: number } | { code: 'disabled' | 'unavailable' | 'busy' };
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
  if (/\.(?:ts|m4s|cmfv|cmfa|m4a|aac|mp3|vtt|key)$/iu.test(parsed.pathname) || mime.split(';')[0].trim().toLowerCase() === 'video/mp2t') return true;
  const parts = parsed.pathname.split('/');
  const file = parts[parts.length - 1];
  const parent = parts[parts.length - 2];
  if (file && (/^(?:v-\d{3,4}p-|a-[a-z]{2,3}-\d+k-).+-s\d+\.mp4$/iu.test(file)
    || /^playlist_a-[a-z]{2,3}-\d+k-.+\.m3u8$/iu.test(file)
    || (parent && file.match(/^(.+)_\d+\.(?:m4v|m4a|mp4)$/iu)?.[1] === parent))) return true;
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
function streamGroup(value: string, type: SourceType): { key: string; rank: number } | null {
  if (type !== 'hls' && type !== 'dash') return null;
  const url = new URL(mediaIdentity(value));
  let queryVariant = false;
  for (const key of Array.from(url.searchParams.keys())) {
    const selections = url.searchParams.getAll(key);
    if (/^(?:quality|height|resolution|bitrate)$/iu.test(key) && selections.length
      && selections.every(selection => /^(?:\d{2,7}p?|auto)$/iu.test(selection))) {
      url.searchParams.delete(key);
      queryVariant = true;
    }
  }
  const split = url.pathname.match(/^(.*\/)([^/]+)\.(?:m3u8|mpd)$/iu);
  if (!split) return { key: JSON.stringify([type, url.origin, url.pathname, url.search]), rank: queryVariant ? 1 : 2 };
  const [, directory, name] = split;
  const root = /^(?:master|manifest|index|hls|dash|playlist)$/iu.test(name);
  const child = /^(?:\d{3,4}p|(?:variant|rendition|stream|chunklist)[_-]\d[\w-]*|playlist_v-\d{3,4}p-.+)$/iu.test(name);
  const asset = name.replace(/[-_.]\d{3,4}p$/iu, '');
  const stem = root || child ? null : asset;
  return {
    key: JSON.stringify([type, url.origin, directory, stem, url.search]),
    rank: queryVariant || child || asset !== name ? 1 : root ? 3 : 2,
  };
}
export function domObservation(tabId: number, message: unknown): Observation | null {
  if (!Number.isSafeInteger(tabId) || tabId < 0 || !message || typeof message !== 'object') return null;
  const input = message as Record<string, unknown>;
  const mediaUrl = publicUrl(input.mediaUrl);
  if (!mediaUrl || (input.mime !== undefined && (typeof input.mime !== 'string' || input.mime.length > 200))) return null;
  const sourceType = classifyMedia(mediaUrl, typeof input.mime === 'string' ? input.mime : '');
  return sourceType ? { tabId, mediaUrl, sourceType } : null;
}
export function contentMessage(source: unknown): { type: 'DOM_MEDIA'; mediaUrl: string; mime: string } | null {
  const observation = domObservation(0, source);
  if (!observation) return null;
  const mime: Record<SourceType, string> = { mp4: 'video/mp4', webm: 'video/webm',
    hls: 'application/vnd.apple.mpegurl', dash: 'application/dash+xml', unknown: 'video/unknown' };
  return { type: 'DOM_MEDIA', mediaUrl: observation.mediaUrl, mime: mime[observation.sourceType] };
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
    const group = streamGroup(mediaUrl, observation.sourceType);
    const exact = rows.find(row => mediaIdentity(row.mediaUrl) === identity);
    const related = exact || !group ? [] : rows.filter(row => {
      const previous = streamGroup(row.mediaUrl, row.sourceType);
      return previous?.key === group.key && (previous.rank === 1 || group.rank === 1);
    });
    const existing = exact ?? (related.length === 1 ? related[0] : undefined);
    if (existing) {
      if (existing.sourceType === 'unknown') existing.sourceType = observation.sourceType;
      const previous = streamGroup(existing.mediaUrl, existing.sourceType);
      if (!exact && group && previous && group.rank > previous.rank) existing.mediaUrl = mediaUrl;
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
        const rows = this.tabs.get(tabId)?.rows;
        const before = rows?.length;
        const candidate = this.add({ tabId, mediaUrl: item.mediaUrl, sourceType: item.sourceType as SourceType });
        if (candidate && rows?.length !== before) { candidate.candidateId = item.candidateId; candidate.detectedAt = item.detectedAt; }
      }
    }
  }
}
function cleanTitle(value?: string): string | undefined {
  return typeof value === 'string' ? value.replace(/[\u0000-\u001f\u007f]/gu, '').replace(/https?:\/\/[^\s]+/giu, input => {
    try { return new URL(input).hostname; } catch { return '영상 주소'; }
  }).slice(0, 200) : undefined;
}
