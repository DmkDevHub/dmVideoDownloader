import { observeVideos, scanOpenShadowVideos } from './dom';
import { contentMessage } from './media';
const observer = observeVideos(document, source => {
  const message = contentMessage(source);
  if (message) void chrome.runtime.sendMessage(message).catch(() => undefined);
});
chrome.runtime.onMessage.addListener((message, sender, respond) => {
  if (sender.id !== chrome.runtime.id || sender.tab !== undefined) return;
  if (message?.type === 'DETECTION_POLICY' && typeof message.enabled === 'boolean') observer.setEnabled(message.enabled);
  if (message?.type === 'SCAN_OPEN_SHADOW') respond(scanOpenShadowVideos(document));
});
void chrome.runtime.sendMessage({ type: 'CONTENT_POLICY' }).then(result => observer.setEnabled(result?.enabled === true)).catch(() => observer.setEnabled(false));
