const state = await chrome.runtime.sendMessage({ type: 'POPUP_STATE' });
document.querySelector('#status')!.textContent = state?.ready ? '감지 준비됨 · 연결 확인 필요' : '확장 확인 필요';
export {};
