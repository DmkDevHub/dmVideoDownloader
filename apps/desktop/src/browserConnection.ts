export interface BrowserRuntime {
  extensionPath: string;
  extensionResourcesAvailable: boolean;
  hostAvailable: boolean;
  chromeAvailable: boolean;
  connectionActionsAvailable: boolean;
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
