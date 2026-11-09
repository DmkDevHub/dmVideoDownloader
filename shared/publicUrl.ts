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
