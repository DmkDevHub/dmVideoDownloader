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
  lastVerifiedId?: string | null;
  remainingSeconds?: number | null;
  errorCode?: string | null;
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
    case 'MANIFEST_MISSING': return '호스트 매니페스트 누락 · 연결 복구 필요';
    case 'EXTENSION_ID_MISMATCH': return 'ID 변경 · 재등록 필요';
    case 'HOST_PATH_MISMATCH': return '경로 불일치 · 재등록 필요';
    case 'REGISTRATION_CONFLICT': return '등록 소유권을 확인하세요';
    case 'INVALID_EXTENSION_ID': return 'Chrome의 실제 확장 ID를 확인하세요';
    case 'REGISTRATION_FAILED': return '등록 읽기 또는 쓰기 실패';
    case 'CONNECTION_TEST_BUSY': return '연결 테스트 응답을 기다리고 있습니다';
    case 'CONNECTION_TEST_TIMEOUT': return '30초 안에 왕복 응답을 받지 못했습니다. Chrome에서 확장 활성화 여부를 확인하고 팝업에 표시된 오류를 확인하세요.';
    case 'HOST_START_FAILED': return '호스트 또는 앱 연결이 종료됐습니다. 실행 상태를 확인한 뒤 다시 테스트하세요.';
    case 'UNSUPPORTED_VERSION': return '앱·호스트·확장을 같은 배포본으로 맞추고 Chrome에서 확장을 새로고침하세요.';
    default: return '설정 파일·경로·등록 권한을 확인하세요';
  }
}
