import { observeVideos } from './dom';
observeVideos(document, source => {
  void chrome.runtime.sendMessage({ type: 'DOM_MEDIA', ...source }).catch(() => undefined);
});
