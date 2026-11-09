import { publicUrl } from './publicUrl';
export const HOST_NAME = 'com.browser_video_downloader.host';
export const MAX_BYTES = 65_536;
export type Envelope = { version: 1; type: string; requestId: string; payload: Record<string, unknown> };
export type ValidationError = 'INVALID_PAYLOAD' | 'UNSUPPORTED_VERSION' | 'URL_NOT_ALLOWED';

const id = (value: unknown): boolean => typeof value === 'string' && /^[A-Za-z0-9_-]{1,64}$/u.test(value);
const record = (value: unknown): value is Record<string, unknown> => !!value && typeof value === 'object' && !Array.isArray(value);
const fields = (value: Record<string, unknown>, allowed: string[]) => Object.keys(value).every(key => allowed.includes(key));
const string = (value: unknown, max: number) => typeof value === 'string' && Array.from(value).length <= max && !/[\u0000-\u001f\u007f-\u009f]/u.test(value);
const uint32 = (value: unknown): value is number => typeof value === 'number' && Number.isInteger(value) && value >= 0 && value <= 0xffffffff;
const wireUrl = (value: unknown) => typeof value === 'string' && !value.includes('#') && !!publicUrl(value);
const errorCodes = new Set([
  'INVALID_PAYLOAD', 'UNSUPPORTED_VERSION', 'URL_NOT_ALLOWED', 'HOST_NOT_REGISTERED', 'HOST_START_FAILED',
  'HOST_PATH_MISMATCH', 'EXTENSION_ID_MISMATCH', 'APP_NOT_RUNNING', 'APP_LAUNCH_FAILED', 'IPC_TIMEOUT',
  'CONNECTION_TEST_TIMEOUT', 'CONNECTION_TEST_NOT_WAITING', 'DOWNLOAD_ENGINE_UNAVAILABLE', 'REGISTRATION_FAILED',
]);
function timestamp(value: unknown): boolean {
  if (typeof value !== 'string' || !/^\d{4}-\d\d-\d\dT\d\d:\d\d:\d\d(?:\.\d+)?(?:Z|[+-]\d\d:\d\d)$/iu.test(value)) return false;
  const day = new Date(value.slice(0, 10) + 'T00:00:00Z');
  return Number.isFinite(Date.parse(value)) && Number.isFinite(day.valueOf()) && day.toISOString().slice(0, 10) === value.slice(0, 10);
}

export function validateEnvelope(value: unknown): ValidationError | null {
  if (!record(value) || !fields(value, ['version', 'type', 'requestId', 'payload']) ||
    !uint32(value.version) || !id(value.requestId) || !record(value.payload)) return 'INVALID_PAYLOAD';
  if (value.version !== 1) return 'UNSUPPORTED_VERSION';
  if (new TextEncoder().encode(JSON.stringify(value)).length > MAX_BYTES) return 'INVALID_PAYLOAD';
  const p = value.payload;
  let valid: boolean;
  switch (value.type) {
    case 'VIDEO_CANDIDATE':
      valid = fields(p, ['candidateId', 'tabId', 'pageUrl', 'pageTitle', 'mediaUrl', 'sourceType', 'detectedAt']) &&
        id(p.candidateId) && typeof p.tabId === 'number' && Number.isInteger(p.tabId) && p.tabId >= 0 && p.tabId <= 0x7fffffff &&
        (p.pageTitle == null || string(p.pageTitle, 200)) && timestamp(p.detectedAt) &&
        typeof p.sourceType === 'string' && ['mp4', 'webm', 'hls', 'dash', 'unknown'].includes(p.sourceType);
      if (valid && (!wireUrl(p.pageUrl) || (p.mediaUrl != null && !wireUrl(p.mediaUrl)))) return 'URL_NOT_ALLOWED';
      break;
    case 'PING': valid = fields(p, ['purpose']) && p.purpose === 'connection_test'; break;
    case 'PONG':
    case 'RUNTIME_STATUS':
      if (p.protocolVersion !== 1 && uint32(p.protocolVersion)) return 'UNSUPPORTED_VERSION';
      valid = p.protocolVersion === 1 && typeof p.appReachable === 'boolean' &&
        (value.type === 'PONG' ? fields(p, ['protocolVersion', 'appReachable']) :
          fields(p, ['protocolVersion', 'appReachable', 'browserAutoDetect']) && typeof p.browserAutoDetect === 'boolean');
      break;
    case 'CONNECTION_TEST_RESULT': valid = fields(p, ['success']) && p.success === true; break;
    case 'ACK': valid = fields(p, ['accepted', 'candidateId']) && typeof p.accepted === 'boolean' && (p.candidateId == null || id(p.candidateId)); break;
    case 'GET_RUNTIME_STATUS': valid = Object.keys(p).length === 0; break;
    case 'ERROR': valid = fields(p, ['code', 'safeMessage']) && typeof p.code === 'string' && errorCodes.has(p.code) && string(p.safeMessage, 256); break;
    default: valid = false;
  }
  return valid ? null : 'INVALID_PAYLOAD';
}

// Chrome has already parsed native replies. Raw fixture/transport consumers can reject duplicate keys before object validation.
export function validateWire(wire: string): ValidationError | null {
  if (new TextEncoder().encode(wire).length > MAX_BYTES) return 'INVALID_PAYLOAD';
  try {
    const value: unknown = JSON.parse(wire);
    const stack: ({ keys: Set<string>; nextKey: boolean } | null)[] = [];
    for (let i = 0; i < wire.length; i++) {
      const char = wire[i];
      const frame = stack.at(-1);
      if (char === '{') stack.push({ keys: new Set(), nextKey: true });
      else if (char === '[') stack.push(null);
      else if (char === '}' || char === ']') stack.pop();
      else if (char === ',' && frame) frame.nextKey = true;
      else if (char === '"') {
        const start = i++;
        for (; wire[i] !== '"'; i++) if (wire[i] === '\\') i++;
        if (frame?.nextKey) {
          const key: string = JSON.parse(wire.slice(start, i + 1));
          if (frame.keys.has(key)) return 'INVALID_PAYLOAD';
          frame.keys.add(key);
          frame.nextKey = false;
        }
      }
      if (stack.length > 32) return 'INVALID_PAYLOAD';
    }
    return validateEnvelope(value);
  } catch { return 'INVALID_PAYLOAD'; }
}
