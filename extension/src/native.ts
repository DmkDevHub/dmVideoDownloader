import { publicUrl, type Candidate } from './media';
export const HOST_NAME = 'com.browser_video_downloader.host';
export interface NativeResult { ok: boolean; code: string; safeMessage: string; verifiedAt?: string }
const errors: Record<string, string> = {
  HOST_NOT_REGISTERED: '호스트 등록을 확인하세요.',
  HOST_START_FAILED: '연결을 시작하지 못했습니다. 앱 설정에서 호스트 등록·확장 ID·경로와 Chrome 정책을 확인하세요.',
  EXTENSION_ID_MISMATCH: '앱 설정의 확장 ID를 확인하고 다시 등록하세요.',
  HOST_PATH_MISMATCH: '이동한 앱 경로로 호스트를 다시 등록하세요.',
  APP_NOT_RUNNING: '앱을 실행한 뒤 다시 시도하세요.',
  APP_LAUNCH_FAILED: '앱을 시작할 수 없습니다. 포터블 폴더를 확인하세요.',
  IPC_TIMEOUT: '앱의 응답이 없습니다. 앱 상태를 확인하고 다시 시도하세요.',
  INVALID_PAYLOAD: '올바른 응답이나 현재 페이지 후보를 확인할 수 없습니다.',
  UNSUPPORTED_VERSION: '앱·확장·호스트의 프로토콜 버전을 확인하세요.',
  URL_NOT_ALLOWED: '지원하지 않는 주소이거나 인증정보가 포함된 주소입니다.',
  DOWNLOAD_ENGINE_UNAVAILABLE: '앱에서 다운로드 도구를 확인하세요.',
  CONNECTION_TEST_TIMEOUT: '30초 안에 연결 테스트를 완료하지 못했습니다. 앱에서 테스트 대기를 다시 시작하세요.',
  CANDIDATE_BUSY: '이 후보의 전달 결과를 기다리고 있습니다.',
  CONNECTION_TEST_BUSY: '이미 연결 테스트를 진행하고 있습니다.',
};
export const failure = (code: string): NativeResult => {
  const safeCode = Object.hasOwn(errors, code) ? code : 'INVALID_PAYLOAD';
  return { ok: false, code: safeCode, safeMessage: errors[safeCode] };
};
type Runtime = Pick<typeof chrome.runtime, 'connectNative'> & Partial<Pick<typeof chrome.runtime, 'lastError'>>;
type Envelope = { version: 1; type: string; requestId: string; payload: Record<string, unknown> };
export function nativeTransaction(runtime: Runtime, initial: Envelope, timeout: number,
  receive: (message: Envelope, send: (message: Envelope) => void) => NativeResult | undefined,
  timeoutCode = 'IPC_TIMEOUT'): Promise<NativeResult> {
  return new Promise(resolve => {
    let done = false;
    let port: chrome.runtime.Port;
    let timer: ReturnType<typeof setTimeout>;
    const finish = (result: NativeResult) => {
      if (done) return; done = true; clearTimeout(timer);
      try { port?.disconnect(); } catch { /* fixed error result only */ }
      resolve(result);
    };
    try {
      port = runtime.connectNative(HOST_NAME);
      port.onDisconnect.addListener(() => { void runtime.lastError; if (!done) finish(failure('HOST_START_FAILED')); });
      port.onMessage.addListener((input: unknown) => {
        if (done) return;
        try {
          if (!input || typeof input !== 'object' || Array.isArray(input) || new TextEncoder().encode(JSON.stringify(input)).byteLength > 65_536) { finish(failure('INVALID_PAYLOAD')); return; }
          const message = input as Envelope;
          if (message.version !== 1) { finish(failure('UNSUPPORTED_VERSION')); return; }
          if (message.requestId !== initial.requestId || !message.payload || typeof message.payload !== 'object' || Array.isArray(message.payload)) { finish(failure('INVALID_PAYLOAD')); return; }
          if (message.type === 'ERROR') {
            finish(failure(typeof message.payload.code === 'string' ? message.payload.code : 'INVALID_PAYLOAD')); return;
          }
          const result = receive(message, message => port.postMessage(message));
          if (result) finish(result);
        } catch { finish(failure('INVALID_PAYLOAD')); }
      });
      timer = setTimeout(() => finish(failure(timeoutCode)), timeout);
      port.postMessage(initial);
    } catch { finish(failure('HOST_START_FAILED')); }
  });
}
export function sendCandidate(runtime: Runtime, candidate: Candidate): Promise<NativeResult> {
  if (!publicUrl(candidate.pageUrl) || !publicUrl(candidate.mediaUrl)) return Promise.resolve(failure('URL_NOT_ALLOWED'));
  const requestId = crypto.randomUUID();
  const { candidateId, tabId, pageUrl, pageTitle, mediaUrl, sourceType, detectedAt } = candidate;
  return nativeTransaction(runtime, { version: 1, type: 'VIDEO_CANDIDATE', requestId,
    payload: { candidateId, tabId, pageUrl, ...(pageTitle ? { pageTitle } : {}), mediaUrl, sourceType, detectedAt } }, 15_000,
  message => message.type === 'ACK' && message.payload.accepted === true && message.payload.candidateId === candidateId
    ? { ok: true, code: 'ACK', safeMessage: '앱이 후보를 수신했습니다. 다운로드 진행 상태는 앱에서 확인하세요.' }
    : failure('INVALID_PAYLOAD'));
}
export function testConnection(runtime: Runtime): Promise<NativeResult> {
  const requestId = crypto.randomUUID();
  let stage: 'pong' | 'ack' = 'pong';
  return nativeTransaction(runtime, { version: 1, type: 'PING', requestId, payload: { purpose: 'connection_test' } }, 30_000,
  (message, send) => {
    if (stage === 'pong' && message.type === 'PONG' && message.payload.protocolVersion !== 1) return failure('UNSUPPORTED_VERSION');
    if (stage === 'pong' && message.type === 'PONG' && message.payload.appReachable === false) return failure('APP_NOT_RUNNING');
    if (stage === 'pong' && message.type === 'PONG' && message.payload.protocolVersion === 1 && message.payload.appReachable === true) {
      stage = 'ack';
      send({ version: 1, type: 'CONNECTION_TEST_RESULT', requestId, payload: { success: true } });
      return;
    }
    if (stage === 'ack' && message.type === 'ACK' && message.payload.accepted === true) {
      return { ok: true, code: 'ACK', verifiedAt: new Date().toISOString(),
        safeMessage: '왕복 확인을 완료했습니다. 테스트 연결은 종료되며 마지막 성공 시각은 현재 연결을 보장하지 않습니다.' };
    }
    return failure('INVALID_PAYLOAD');
  }, 'CONNECTION_TEST_TIMEOUT');
}
