import { copyExtensionId, renderCandidates, type PopupState } from './popupView';
import type { Candidate } from './media';
import type { NativeResult } from './native';
let pending = false;
export async function selectCandidate(candidate: Candidate): Promise<NativeResult> {
  return chrome.runtime.sendMessage({ type: 'DOWNLOAD_SELECTED', tabId: candidate.tabId, candidateId: candidate.candidateId });
}
export async function refreshPopup(): Promise<void> {
  if (pending) return;
  try {
    const state: PopupState = await chrome.runtime.sendMessage({ type: 'POPUP_STATE' });
    if (!state?.ready) throw new Error('STATE_UNAVAILABLE');
    renderCandidates(document, state, async candidate => {
      pending = true;
      try { return await selectCandidate(candidate); } finally { pending = false; }
    });
  } catch { document.querySelector('#status')!.textContent = '확장 상태를 확인하지 못했습니다. 팝업을 다시 열어주세요.'; }
}
await refreshPopup();
chrome.runtime.onMessage.addListener(message => { if (message?.type === 'CANDIDATES_CHANGED') void refreshPopup(); });
chrome.tabs.onActivated.addListener(() => { void refreshPopup(); });
document.querySelector('#extension-id')!.textContent = chrome.runtime.id;
document.querySelector('#copy-id')!.addEventListener('click', () => {
  void copyExtensionId(chrome.runtime.id, text => navigator.clipboard.writeText(text)).then(copied => {
    document.querySelector('#identity-status')!.textContent = copied ? '확장 ID를 복사했습니다.' : 'ID를 복사하지 못했습니다. 표시된 ID를 직접 복사하세요.';
  });
});
document.querySelector<HTMLButtonElement>('#test-connection')!.addEventListener('click', () => {
  const button = document.querySelector<HTMLButtonElement>('#test-connection')!;
  button.disabled = true;
  document.querySelector('#test-result')!.textContent = '연결 테스트 중 · 최대 30초 대기합니다. 앱에서 테스트 대기를 시작했는지 확인하세요.';
  void chrome.runtime.sendMessage({ type: 'CONNECTION_TEST' }).then((result: NativeResult) => {
    document.querySelector('#test-result')!.textContent = result.safeMessage;
    if (result.verifiedAt) document.querySelector('#last-success')!.textContent = `마지막 성공: ${new Date(result.verifiedAt).toLocaleString()} · 현재 연결은 확인 필요`;
  }).catch(() => { document.querySelector('#test-result')!.textContent = '테스트를 시작하지 못했습니다. 앱 설정의 연결 안내를 확인하세요.'; })
    .finally(() => { button.disabled = false; });
});
