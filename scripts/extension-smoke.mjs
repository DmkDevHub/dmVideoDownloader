// Process/CDP assertions only. No screen, click, keyboard, cookie or browser profile inspection.
import { spawn } from 'node:child_process';
import { mkdtempSync, mkdirSync, readFileSync, existsSync, realpathSync } from 'node:fs';
import { resolve } from 'node:path';
import { fileURLToPath } from 'node:url';
import { createServer } from 'node:http';
import assert from 'node:assert/strict';
import { runChromeNativeSmoke } from './chrome-native-smoke.mjs';
const root = fileURLToPath(new URL('../', import.meta.url));
const binary = process.env.DM_CHROME_BINARY ?? resolve(process.env.LOCALAPPDATA ?? '',
  'Programs/dm-video-tools/chrome-for-testing-157.0.8103.38/chrome-win64/chrome.exe');
assert.ok(existsSync(binary), 'Set DM_CHROME_BINARY to a Chrome for Testing executable.');
const extensionDir = resolve(root, process.env.DM_EXTENSION_DIR ?? 'extension/dist');
assert.ok(existsSync(resolve(extensionDir, 'manifest.json')), 'Run npm run build:extension first.');
const unpacked = realpathSync(extensionDir);
mkdirSync(resolve(root, '.test-output'), { recursive: true });
const profile = mkdtempSync(resolve(root, '.test-output/extension-'));
const pause = ms => new Promise(r => setTimeout(r, ms));
async function until(fn, label) {
  const end = Date.now() + 20_000;
  while (Date.now() < end) { const result = await fn(); if (result) return result; await pause(100); }
  throw new Error(`Timed out: ${label}`);
}
const playableMp4 = process.env.DM_MEDIA_FIXTURE ? readFileSync(process.env.DM_MEDIA_FIXTURE) : undefined;
const server = createServer((req, res) => {
  if (/\/(?:master|variant-\d+)\.m3u8(?:\?|$)/u.test(req.url ?? '')) {
    res.setHeader('Content-Type', 'application/vnd.apple.mpegurl');
    res.end('#EXTM3U\n#EXT-X-STREAM-INF:BANDWIDTH=100000\nvariant.m3u8\n'); return;
  }
  if (req.url === '/manifest') {
    res.setHeader('Content-Type', 'application/dash+xml');
    res.end('<?xml version="1.0"?><MPD xmlns="urn:mpeg:dash:schema:mpd:2011"/>'); return;
  }
  if (/\.(?:mp4|m4v|webm)(?:\?|$)/u.test(req.url ?? '')) {
    res.setHeader('Content-Type', req.url.includes('.webm') ? 'video/webm' : 'video/mp4');
    if (req.url.startsWith('/sample.mp4') && playableMp4) {
      res.setHeader('Content-Length', playableMp4.length); res.end(playableMp4); return;
    }
    res.end('metadata fixture, not a playable video'); return;
  }
  res.setHeader('Content-Type', 'text/html');
  res.end('<!doctype html><title>Public fixture</title><h1>Extension process test</h1>');
});
await new Promise(r => server.listen(0, '127.0.0.1', r));
const pageUrl = `http://fixture.example.org:${server.address().port}/watch`;
const browser = spawn(binary, ['--headless=new', '--no-first-run', '--no-default-browser-check',
  '--disable-background-networking', '--no-proxy-server',
  '--host-resolver-rules=MAP fixture.example.org 127.0.0.1',
  `--user-data-dir=${profile}`, '--remote-debugging-port=0',
  `--load-extension=${unpacked}`, 'about:blank'],
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
  await evaluate(pageSession, 'Promise.all([fetch("/sample.mp4"), fetch("/sample.webm")]).then(() => true)');
  const tabId = await evaluate(workerSession, `(async () => (await chrome.tabs.query({url:${JSON.stringify(pageUrl)}}))[0].id)()`);
  const { targetId: popupTarget } = await send('Target.createTarget', { url: `chrome-extension://${extensionId}/popup.html` });
  const { sessionId: popupSession } = await send('Target.attachToTarget', { targetId: popupTarget, flatten: true });
  await until(async () => (await evaluate(popupSession, 'document.querySelector("#detection")?.textContent'))?.includes('자동 감지'), 'popup worker reply');
  assert.equal(await evaluate(popupSession, 'document.querySelector("#extension-id").textContent'), extensionId);
  assert.equal(await evaluate(popupSession, 'document.querySelector("#test-connection").disabled'), false);
  assert.ok((await evaluate(popupSession, 'document.querySelector("#last-success").textContent')).includes('현재 연결 확인 필요'));
  await evaluate(popupSession, `chrome.tabs.update(${tabId}, {active:true}).then(() => true)`);
  await until(async () => await evaluate(popupSession, 'document.querySelectorAll("#candidates li").length') === 2, 'current-tab popup rows');
  assert.equal(await evaluate(popupSession, `chrome.action.getBadgeText({tabId:${tabId}})`), '2');
  assert.ok(!(await evaluate(popupSession, 'document.querySelector("#candidates").textContent')).includes('/sample.mp4'));
  const rows = await until(async () => {
    const state = await evaluate(popupSession, `chrome.runtime.sendMessage({type:'POPUP_STATE',tabId:${tabId}})`);
    return state.candidates?.length >= 2 && state.candidates;
  }, 'MP4/WebM request metadata');
  assert.deepEqual(rows.map(r => r.sourceType).sort(), ['mp4', 'webm']);
  assert.ok(rows.every(r => r.tabId === tabId && r.status === 'detected'));
  if (playableMp4) {
    await evaluate(pageSession, `(() => { const video = document.createElement('video');
      video.id = 'playback-probe'; video.muted = true; video.src = '/sample.mp4';
      document.body.append(video); return video.play().then(() => true); })()`);
    await until(async () => await evaluate(pageSession, 'document.querySelector("#playback-probe").currentTime > 0'), 'actual MP4 playback');
  }
  await evaluate(pageSession, 'Promise.all([fetch("/master.m3u8"), fetch("/manifest")]).then(() => true)');
  await until(async () => {
    const state = await evaluate(popupSession, `chrome.runtime.sendMessage({type:'POPUP_STATE',tabId:${tabId}})`);
    return state.candidates?.some(r => r.sourceType === 'hls') && state.candidates?.some(r => r.sourceType === 'dash');
  }, 'HLS path and DASH MIME detection');
  await evaluate(pageSession, `(() => {
    const video = document.createElement('video'); video.preload = 'none';
    const source = document.createElement('source'); source.src = '/dom-only.webm'; source.type = 'video/webm';
    video.append(source); document.body.append(video); return true;
  })()`);
  await until(async () => {
    const state = await evaluate(popupSession, `chrome.runtime.sendMessage({type:'POPUP_STATE',tabId:${tabId}})`);
    return state.candidates?.some(r => r.mediaUrl.endsWith('/dom-only.webm'));
  }, 'dynamic HTML5 source observation');
  await evaluate(pageSession, `(() => { const source = document.querySelector('source'); source.src = '/changed.mp4'; source.type = 'video/mp4'; return true; })()`);
  await until(async () => {
    const state = await evaluate(popupSession, `chrome.runtime.sendMessage({type:'POPUP_STATE',tabId:${tabId}})`);
    return state.candidates?.some(r => r.mediaUrl.endsWith('/changed.mp4'));
  }, 'HTML5 source attribute change');
  const stateForTab = () => evaluate(popupSession, `chrome.runtime.sendMessage({type:'POPUP_STATE',tabId:${tabId}})`);
  await evaluate(pageSession, 'fetch("/show/variant-720.m3u8?v=1").then(() => true)');
  const variantId = await until(async () => (await stateForTab()).candidates.find(row => row.mediaUrl.includes('/show/variant-720'))?.candidateId,
    'HLS variant before master');
  await evaluate(pageSession, 'Promise.all([fetch("/show/master.m3u8?v=1"), fetch("/show/variant-480.m3u8?v=1"), fetch("/show/master.m3u8?v=2")]).then(() => true)');
  await until(async () => {
    const rows = (await stateForTab()).candidates.filter(row => row.mediaUrl.includes('/show/'));
    return rows.length === 2 && rows.some(row => row.candidateId === variantId && row.mediaUrl.endsWith('/show/master.m3u8?v=1'));
  }, 'master promotion and independent asset identity');
  const beforeRepeat = await stateForTab();
  await evaluate(pageSession, 'Promise.all([fetch("/sample.mp4?utm_source=repeat"), fetch("/chunk-1.mp4"), fetch("/segment2.mp4"), fetch("/sample.mp4?range=0-99"), fetch("/show/v-0576p-libx264-s1.mp4"), fetch("/show/track_720/track_720_1.m4v"), fetch("/show/variant-1080.m3u8?v=1")]).then(() => true)');
  await pause(300);
  assert.equal((await stateForTab()).candidates.length, beforeRepeat.candidates.length);
  const otherUrl = pageUrl.replace('/watch', '/other');
  const { targetId: otherTarget } = await send('Target.createTarget', { url: otherUrl });
  const { sessionId: otherSession } = await send('Target.attachToTarget', { targetId: otherTarget, flatten: true });
  await until(async () => (await evaluate(otherSession, 'globalThis.document?.readyState')) === 'complete', 'second tab load');
  await evaluate(otherSession, 'fetch("/other.webm").then(() => true)');
  const otherTabId = await evaluate(popupSession, `(async () => (await chrome.tabs.query({url:${JSON.stringify(otherUrl)}}))[0].id)()`);
  await until(async () => (await evaluate(popupSession, `chrome.runtime.sendMessage({type:'POPUP_STATE',tabId:${otherTabId}})`)).candidates.length === 1, 'second tab isolation');
  assert.equal((await stateForTab()).candidates.length, beforeRepeat.candidates.length);
  await send('Target.closeTarget', { targetId: otherTarget });
  await send('ServiceWorker.enable', {}, pageSession);
  const version = await until(() => events.filter(e => e.method === 'ServiceWorker.workerVersionUpdated')
    .flatMap(e => e.params.versions).find(item => item.scriptURL === worker.url && item.runningStatus === 'running'), 'extension worker version');
  await send('Target.detachFromTarget', { sessionId: workerSession });
  await send('ServiceWorker.stopWorker', { versionId: version.versionId }, pageSession);
  await until(() => events.filter(e => e.method === 'ServiceWorker.workerVersionUpdated')
    .flatMap(e => e.params.versions).some(item => item.versionId === version.versionId && item.runningStatus === 'stopped'), 'worker termination');
  const restored = await stateForTab();
  assert.deepEqual(restored.candidates.map(r => r.candidateId), beforeRepeat.candidates.map(r => r.candidateId));
  await send('Page.navigate', { url: pageUrl.replace('/watch', '/replacement') }, pageSession);
  await until(async () => await evaluate(pageSession, 'globalThis.document?.readyState === "complete" && location.pathname === "/replacement"'), 'full navigation');
  await until(async () => (await stateForTab()).candidates.length === 0, 'full navigation cleanup');
  await evaluate(pageSession, 'history.pushState({}, "", "/next"); true');
  await until(async () => (await stateForTab()).candidates.length === 0, 'SPA navigation cleanup');
  await evaluate(pageSession, 'fetch("/sample.webm").then(() => true)');
  await until(async () => (await stateForTab()).candidates.length === 1, 'new page candidate');
  assert.ok((await stateForTab()).candidates[0].pageUrl.endsWith('/next'));
  if (process.env.DM_NATIVE_APP || process.env.DM_NATIVE_HOST) {
    const candidateId = (await stateForTab()).candidates[0].candidateId;
    await runChromeNativeSmoke({ root, extensionId, candidateId,
      requestSelection: id => evaluate(popupSession, `chrome.runtime.sendMessage(${JSON.stringify({ type: 'DOWNLOAD_SELECTED', tabId, candidateId: id })})`),
      requestDiagnosis: () => evaluate(popupSession, 'chrome.runtime.sendMessage({type:"CONNECTION_TEST"})'),
      waitForSettings: () => until(async () => (await stateForTab()).autoDetect === false, 'app runtime setting'),
      refreshCandidate: async () => {
        await evaluate(popupSession, 'chrome.storage.local.set({lastRuntimeSettings:{browserAutoDetect:true}}).then(() => true)');
        await until(async () => (await stateForTab()).autoDetect, 'fixture detection reset');
        await evaluate(pageSession, 'fetch("/moved.webm").then(() => true)');
        return until(async () => (await stateForTab()).candidates.find(row => row.mediaUrl.endsWith('/moved.webm'))?.candidateId, 'moved-install candidate');
      },
    });
  }
  await evaluate(popupSession, 'chrome.storage.local.set({lastRuntimeSettings:{browserAutoDetect:false}}).then(() => true)');
  await until(async () => (await stateForTab()).autoDetect === false && (await stateForTab()).candidates.length === 0, 'last-confirmed auto-detect off');
  assert.equal(await evaluate(popupSession, `chrome.action.getBadgeText({tabId:${tabId}})`), '');
  await evaluate(pageSession, 'fetch("/disabled.mp4").then(() => true)');
  await pause(200); assert.equal((await stateForTab()).candidates.length, 0);
  await send('Target.closeTarget', { targetId });
  await until(async () => {
    const cache = await evaluate(popupSession, 'chrome.storage.session.get("candidateSession")');
    return !cache.candidateSession.tabs.some(tab => tab.tabId === tabId);
  }, 'closed tab session cleanup');
  console.log(`EXTENSION_SMOKE_PASS: MV3, network/DOM/playlists, tab/session lifecycle, popup ID/badge; MP4 playback ${playableMp4 ? 'PASS' : 'NOT RUN (set DM_MEDIA_FIXTURE)'}; no UI input`);
} finally {
  if (socket?.readyState === WebSocket.OPEN) { await send('Browser.close').catch(() => undefined); socket.close(); }
  if (browser.exitCode === null) browser.kill();
  for (const item of pending.values()) { clearTimeout(item.timer); item.reject(new Error('Browser closed')); }
  await new Promise(r => server.close(r));
}
