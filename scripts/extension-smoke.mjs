// Process/CDP assertions only. No screen, click, keyboard, cookie or browser profile inspection.
import { spawn } from 'node:child_process';
import { mkdtempSync, mkdirSync, readFileSync, existsSync } from 'node:fs';
import { resolve } from 'node:path';
import { fileURLToPath } from 'node:url';
import { createServer } from 'node:http';
import assert from 'node:assert/strict';
const root = fileURLToPath(new URL('../', import.meta.url));
const binary = process.env.DM_CHROME_BINARY ?? resolve(process.env.LOCALAPPDATA ?? '',
  'Programs/dm-video-tools/chrome-for-testing-157.0.8103.38/chrome-win64/chrome.exe');
assert.ok(existsSync(binary), 'Set DM_CHROME_BINARY to a Chrome for Testing executable.');
assert.ok(existsSync(resolve(root, 'extension/dist/manifest.json')), 'Run npm run build:extension first.');
mkdirSync(resolve(root, '.test-output'), { recursive: true });
const profile = mkdtempSync(resolve(root, '.test-output/extension-'));
const pause = ms => new Promise(r => setTimeout(r, ms));
async function until(fn, label) {
  const end = Date.now() + 20_000;
  while (Date.now() < end) { const result = await fn(); if (result) return result; await pause(100); }
  throw new Error(`Timed out: ${label}`);
}
const server = createServer((_req, res) => {
  res.setHeader('Content-Type', 'text/html');
  res.end('<!doctype html><title>Public fixture</title><h1>Extension process test</h1>');
});
await new Promise(r => server.listen(0, '127.0.0.1', r));
const pageUrl = `http://fixture.example.org:${server.address().port}/watch`;
const browser = spawn(binary, ['--headless=new', '--no-first-run', '--no-default-browser-check',
  '--disable-background-networking', '--no-proxy-server',
  '--host-resolver-rules=MAP fixture.example.org 127.0.0.1',
  `--user-data-dir=${profile}`, '--remote-debugging-port=0',
  `--load-extension=${resolve(root, 'extension/dist')}`, 'about:blank'],
{ windowsHide: true, stdio: 'ignore' });
let socket;
const pending = new Map(); let serial = 0;
const events = [];
const send = (method, params = {}, sessionId) => new Promise((accept, reject) => {
  const id = ++serial;
  const timer = setTimeout(() => { pending.delete(id); reject(new Error(`CDP timeout: ${method}`)); }, 15_000);
  pending.set(id, { accept, reject, timer });
  socket.send(JSON.stringify({ id, method, params, ...(sessionId ? { sessionId } : {}) }));
});
const evaluate = async (session, expression) => {
  const result = await send('Runtime.evaluate', { expression, awaitPromise: true, returnByValue: true }, session);
  assert.ok(!result.exceptionDetails, 'Browser assertion expression failed');
  return result.result.value;
};
try {
  const portFile = resolve(profile, 'DevToolsActivePort');
  const [port, endpoint] = await until(() => existsSync(portFile) && readFileSync(portFile, 'utf8').trim().split('\n'), 'debug endpoint');
  socket = new WebSocket(`ws://127.0.0.1:${port}${endpoint}`);
  await new Promise((accept, reject) => { socket.onopen = accept; socket.onerror = reject; });
  socket.onmessage = event => {
    const message = JSON.parse(event.data);
    if (!message.id) { events.push(message); return; }
    const item = pending.get(message.id); if (!item) return;
    clearTimeout(item.timer); pending.delete(message.id);
    if (message.error) item.reject(new Error(`CDP ${message.error.code}`)); else item.accept(message.result);
  };
  const worker = await until(async () => {
    const targets = (await send('Target.getTargets')).targetInfos.filter(t => t.type === 'service_worker' && t.url.endsWith('/background.js'));
    for (const target of targets) {
      const { sessionId } = await send('Target.attachToTarget', { targetId: target.targetId, flatten: true });
      const name = await evaluate(sessionId, 'globalThis.chrome?.runtime?.getManifest?.()?.name');
      if (name === 'dmVideoDownloader 영상 감지') return { ...target, sessionId };
      await send('Target.detachFromTarget', { sessionId });
    }
  }, 'MV3 worker');
  const extensionId = new URL(worker.url).hostname;
  assert.match(extensionId, /^[a-p]{32}$/);
  const workerSession = worker.sessionId;
  assert.equal(await evaluate(workerSession, 'chrome.runtime.getManifest().manifest_version'), 3);
  const { targetId } = await send('Target.createTarget', { url: pageUrl });
  const { sessionId: pageSession } = await send('Target.attachToTarget', { targetId, flatten: true });
  await send('Runtime.enable', {}, pageSession);
  await until(async () => await evaluate(pageSession, 'globalThis.document?.title') === 'Public fixture', 'fixture page load');
  await until(() => events.find(e => e.sessionId === pageSession && e.method === 'Runtime.executionContextCreated'
    && e.params.context.origin === `chrome-extension://${extensionId}`), 'content script isolated context');
  const { targetId: popupTarget } = await send('Target.createTarget', { url: `chrome-extension://${extensionId}/popup.html` });
  const { sessionId: popupSession } = await send('Target.attachToTarget', { targetId: popupTarget, flatten: true });
  await until(async () => (await evaluate(popupSession, 'document.querySelector("#status")?.textContent'))?.includes('감지 준비됨'), 'popup worker reply');
  console.log('EXTENSION_SMOKE_PASS: MV3 worker, isolated content script, popup/runtime messaging; no UI input');
} finally {
  if (socket?.readyState === WebSocket.OPEN) { await send('Browser.close').catch(() => undefined); socket.close(); }
  if (browser.exitCode === null) browser.kill();
  for (const item of pending.values()) { clearTimeout(item.timer); item.reject(new Error('Browser closed')); }
  await new Promise(r => server.close(r));
}
