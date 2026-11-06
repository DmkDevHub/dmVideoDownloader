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
  const type = mime.split(';')[0].trim().toLowerCase();
  if (type === 'video/mp4') return 'mp4';
  if (type === 'video/webm') return 'webm';
  if (type && !type.startsWith('video/') && type !== 'application/octet-stream') return null;
  const path = new URL(url).pathname.toLowerCase();
  if (path.endsWith('.mp4')) return 'mp4';
  if (path.endsWith('.webm')) return 'webm';
  return type.startsWith('video/') ? 'unknown' : null;
}

export interface Observation { tabId: number; mediaUrl: string; sourceType: SourceType }
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
  private tabs = new Map<number, Candidate[]>();
  add(observation: Observation, page: { url?: string; title?: string }): Candidate | null {
    const pageUrl = publicUrl(page.url);
    if (!pageUrl) return null;
    const rows = this.tabs.get(observation.tabId) ?? [];
    const candidate: Candidate = {
      candidateId: crypto.randomUUID(), tabId: observation.tabId, pageUrl,
      pageTitle: page.title?.replace(/[\u0000-\u001f\u007f]/gu, '').slice(0, 200),
      mediaUrl: observation.mediaUrl, sourceType: observation.sourceType,
      detectedAt: new Date().toISOString(), status: 'detected',
    };
    rows.push(candidate); this.tabs.set(observation.tabId, rows);
    return candidate;
  }
  list(tabId: number): Candidate[] { return this.tabs.get(tabId) ?? []; }
}
