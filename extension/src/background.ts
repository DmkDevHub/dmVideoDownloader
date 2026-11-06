chrome.runtime.onMessage.addListener((message, sender, respond) => {
  if (sender.id !== chrome.runtime.id) return;
  if (message?.type === 'CONTENT_READY') respond({ ready: true });
  if (message?.type === 'POPUP_STATE' && sender.url === chrome.runtime.getURL('popup.html')) respond({ ready: true, candidates: [] });
});
