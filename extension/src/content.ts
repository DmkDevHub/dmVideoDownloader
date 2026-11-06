import { observeVideos } from './dom';
import { contentMessage } from './media';
const observer = observeVideos(document, source => {
  const message = contentMessage(source);
  if (message) void chrome.runtime.sendMessage(message).catch(() => undefined);
});
chrome.runtime.onMessage.addListener((message, sender) => {
  if (sender.id === chrome.runtime.id && message?.type === 'DETECTION_POLICY' && typeof message.enabled === 'boolean') observer.setEnabled(message.enabled);
});
void chrome.runtime.sendMessage({ type: 'CONTENT_POLICY' }).then(result => observer.setEnabled(result?.enabled === true)).catch(() => observer.setEnabled(false));
