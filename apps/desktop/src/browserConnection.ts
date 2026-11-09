export interface BrowserRuntime {
  extensionPath: string;
  extensionResourcesAvailable: boolean;
  hostAvailable: boolean;
  chromeAvailable: boolean;
  connectionActionsAvailable: boolean;
  registrationCode?: string;
  registeredPath?: string | null;
  configuredId?: string;
  manifestPath?: string;
}
export interface ConnectionStatus {
  phase: 'needsCheck' | 'waiting' | 'connected' | 'disconnected' | 'failed';
  roundTripConfirmed: boolean;
  lastSuccessAt: string | null;
}
export function connectionLabel(status: ConnectionStatus): string {
  if (status.phase === 'connected') return status.roundTripConfirmed ? '연결됨' : '확인 필요';
  return { needsCheck: '확인 필요', waiting: '테스트 대기', disconnected: '연결 끊김', failed: '연결 확인 실패' }[status.phase];
}
export function validExtensionId(id: string): boolean { return /^[a-p]{32}$/.test(id); }
export function registrationLabel(code: unknown): string {
  switch (code) {
    case 'REGISTERED': return '등록됨 · 연결 테스트 필요';
    case 'HOST_NOT_REGISTERED': return '미등록';
    case 'HOST_MISSING': return 'Native Host 실행 파일 누락';
    case 'EXTENSION_ID_MISMATCH': return 'ID 변경 · 재등록 필요';
    case 'HOST_PATH_MISMATCH': return '경로 불일치 · 재등록 필요';
    case 'REGISTRATION_CONFLICT': return '등록 소유권을 확인하세요';
    case 'INVALID_EXTENSION_ID': return 'Chrome의 실제 확장 ID를 확인하세요';
    case 'REGISTRATION_FAILED': return '등록 읽기 또는 쓰기 실패';
    default: return '설정 파일·경로·등록 권한을 확인하세요';
  }
}
