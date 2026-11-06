import { renderCandidates, type PopupState } from './popupView';
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
