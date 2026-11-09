import assert from 'node:assert/strict';
import { spawn, spawnSync } from 'node:child_process';
import { copyFileSync, mkdirSync, mkdtempSync, writeFileSync } from 'node:fs';
import { join, resolve } from 'node:path';

const [appBinary, hostBinary] = process.argv.slice(2);
assert.ok(appBinary && hostBinary, 'Supply the smoke desktop and Native Host binaries.');
mkdirSync('.test-output', { recursive: true });
const root = mkdtempSync(resolve('.test-output/native-bridge-'));
const desktop = join(root, 'VideoDownloader.exe');
const host = join(root, 'VideoNativeHost.exe');
copyFileSync(appBinary, desktop);
copyFileSync(hostBinary, host);
mkdirSync(join(root, 'data'));
writeFileSync(join(root, 'data/settings.json'), JSON.stringify({ schemaVersion: 1, downloadDirectory: join(root, 'Downloads'),
  quality: 'best', format: 'auto', filename: 'title', concurrentDownloads: 2, retryCount: 3,
  browserAutoDetect: true, browserExtensionId: 'a'.repeat(32), completionNotification: true }));
writeFileSync(join(root, 'native-host-manifest.json'), JSON.stringify({ name: 'com.browser_video_downloader.host',
  description: 'Browser Video Downloader bridge', path: host, type: 'stdio', allowed_origins: ['chrome-extension://' + 'a'.repeat(32) + '/'] }));
const app = spawn(desktop, [], { windowsHide: true, env: { ...process.env, DM_SMOKE_ROOT: root, DM_SMOKE_PHASE: 'bridge' } });
let stdout = ''; app.stderr.resume();
app.stdout.setEncoding('utf8').on('data', chunk => { stdout += chunk; });
const exit = new Promise((done, reject) => { app.on('error', reject); app.on('exit', done); });
const pause = ms => new Promise(done => setTimeout(done, ms));
const message = { version: 1, type: 'VIDEO_CANDIDATE', requestId: 'bridge-smoke', payload: {
  candidateId: 'bridge-row', tabId: 1, pageUrl: 'https://example.org/watch', mediaUrl: 'https://example.org/a.mp4',
  sourceType: 'mp4', detectedAt: '2026-11-09T00:00:00Z',
} };
const data = Buffer.from(JSON.stringify(message));
const prefix = Buffer.alloc(4);
prefix.writeUInt32LE(data.length);
try {
  let received = false;
  for (let attempt = 0; attempt < 15 && !received; attempt++) {
    const reply = spawnSync(host, ['chrome-extension://' + 'a'.repeat(32) + '/'], {
      windowsHide: true, input: Buffer.concat([prefix, data]), timeout: 8000,
    });
    assert.equal(reply.status, 0);
    assert.equal(reply.stdout.readUInt32LE(), reply.stdout.length - 4);
    const result = JSON.parse(reply.stdout.subarray(4).toString('utf8'));
    assert.equal(result.requestId, message.requestId);
    if (result.type === 'ACK') {
      assert.deepEqual(result.payload, { accepted: true, candidateId: 'bridge-row' });
      received = true;
    } else {
      assert.ok(['APP_NOT_RUNNING', 'IPC_TIMEOUT'].includes(result.payload.code), result.payload.code);
      await pause(100);
    }
  }
  assert.ok(received, 'Host never received desktop ACK');
  writeFileSync(join(root, 'bridge-ack'), 'ok');
  assert.equal(await exit, 0);
  assert.match(stdout, /SMOKE_PASS:/u);
  console.log('NATIVE_BRIDGE_PASS: native host -> verified Windows pipe -> mounted desktop -> ACK');
} finally { if (app.exitCode === null) app.kill(); }
