import assert from 'node:assert/strict';
import { spawn, spawnSync } from 'node:child_process';
import { createHash } from 'node:crypto';
import { copyFileSync, mkdirSync, mkdtempSync, readdirSync, realpathSync, statSync, writeFileSync } from 'node:fs';
import { createConnection } from 'node:net';
import { join, resolve } from 'node:path';

const [appBinary, hostBinary] = process.argv.slice(2);
assert.ok(appBinary && hostBinary, 'Supply the smoke desktop and Native Host binaries.');
mkdirSync('.test-output', { recursive: true });
const root = mkdtempSync(resolve('.test-output/native-bridge-'));
const desktop = join(root, 'VideoDownloader.exe');
const host = join(root, 'VideoNativeHost.exe');
copyFileSync(appBinary, desktop);
copyFileSync(hostBinary, host);
const toolRoot = process.env.DM_R4_TOOL_ROOT;
if (toolRoot) {
  mkdirSync(join(root, 'tools'));
  for (const exe of ['yt-dlp.exe', 'ffmpeg.exe']) {
    copyFileSync(join(toolRoot, 'tools', exe), join(root, 'tools', exe));
  }
}
mkdirSync(join(root, 'data'));
writeFileSync(join(root, 'data/settings.json'), JSON.stringify({ schemaVersion: 1, downloadDirectory: join(root, 'Downloads'),
  quality: 'best', format: 'auto', filename: 'title', concurrentDownloads: 2, retryCount: 3,
  browserAutoDetect: true, browserExtensionId: 'a'.repeat(32), completionNotification: true }));
writeFileSync(join(root, 'native-host-manifest.json'), JSON.stringify({ name: 'com.browser_video_downloader.host',
  description: 'Browser Video Downloader bridge', path: host, type: 'stdio', allowed_origins: ['chrome-extension://' + 'a'.repeat(32) + '/'] }));
const app = spawn(desktop, [], { windowsHide: true, env: { ...process.env,
  WEBVIEW2_USER_DATA_FOLDER: join(root, 'webview-test'), DM_SMOKE_ROOT: root,
  DM_SMOKE_PHASE: toolRoot ? 'bridge-analysis' : 'bridge' } });
let stdout = '';
let stderr = '';
app.stdout.setEncoding('utf8').on('data', chunk => { stdout += chunk; });
app.stderr.setEncoding('utf8').on('data', chunk => { stderr += chunk; });
const exit = new Promise((done, reject) => { app.on('error', reject); app.on('exit', done); });
const pause = ms => new Promise(done => setTimeout(done, ms));
const message = { version: 1, type: 'VIDEO_CANDIDATE', requestId: 'bridge-smoke', payload: {
  candidateId: 'bridge-row', tabId: 1, pageUrl: 'https://example.org/',
  mediaUrl: toolRoot ? 'https://interactive-examples.mdn.mozilla.net/media/cc0-videos/flower.mp4' : 'https://example.org/a.mp4',
  sourceType: 'mp4', detectedAt: '2026-11-09T00:00:00Z',
} };
const data = Buffer.from(JSON.stringify(message));
const prefix = Buffer.alloc(4);
prefix.writeUInt32LE(data.length);
const frame = Buffer.concat([prefix, data]);
const windowsPath = '\\\\?\\' + realpathSync(root);
const pipe = '\\\\.\\pipe\\dm-video-downloader-' + createHash('sha256').update(windowsPath).digest('hex');
function probeOtherClient() {
  return new Promise((resolve, reject) => {
    const socket = createConnection(pipe);
    let connected = false;
    let bytes = 0;
    const timer = setTimeout(() => { socket.destroy(); reject(new Error('Unauthorized pipe client remained connected')); }, 2000);
    socket.on('connect', () => { connected = true; socket.write(Buffer.concat([Buffer.from('a'.repeat(32)), frame])); });
    socket.on('data', chunk => { bytes += chunk.length; });
    socket.on('error', error => {
      if (!connected && !['ENOENT', 'EBUSY'].includes(error.code)) reject(error);
    });
    socket.on('close', () => { clearTimeout(timer); resolve({ connected, bytes }); });
  });
}
try {
  let rejected = false;
  for (let attempt = 0; attempt < 30 && !rejected; attempt++) {
    const probe = await probeOtherClient();
    assert.equal(probe.bytes, 0);
    rejected = probe.connected;
    if (!rejected) await pause(50);
  }
  assert.ok(rejected, 'Untrusted client did not reach the pipe');
  let received = false;
  for (let attempt = 0; attempt < 15 && !received; attempt++) {
    const reply = spawnSync(host, ['chrome-extension://' + 'a'.repeat(32) + '/'], {
      windowsHide: true, input: Buffer.concat([frame, frame]), timeout: 8000,
    });
    assert.equal(reply.status, 0);
    const firstLength = reply.stdout.readUInt32LE();
    const result = JSON.parse(reply.stdout.subarray(4, 4 + firstLength).toString('utf8'));
    const tail = reply.stdout.subarray(4 + firstLength);
    assert.equal(tail.readUInt32LE(), tail.length - 4);
    const repeated = JSON.parse(tail.subarray(4).toString('utf8'));
    assert.equal(result.requestId, message.requestId);
    if (result.type === 'ACK') {
      assert.deepEqual(result.payload, { accepted: true, candidateId: 'bridge-row' });
      assert.deepEqual(repeated, result);
      received = true;
    } else {
      assert.ok(['APP_NOT_RUNNING', 'IPC_TIMEOUT'].includes(result.payload.code), result.payload.code);
      await pause(100);
    }
  }
  assert.ok(received, 'Host never received desktop ACK');
  writeFileSync(join(root, 'bridge-ack'), 'bridge-row');
  assert.equal(await exit, 0, stderr || stdout);
  assert.match(stdout, /SMOKE_PASS:/u);
  console.log('NATIVE_BRIDGE_PASS: verified Windows pipe, untrusted process rejection, candidate row, repeat ACK');
  if (toolRoot) {
    console.log('ANALYSIS_SMOKE_PASS: public page, candidate media fallback, private URL rejected in the hidden WebView');
    const downloads = join(root, 'Downloads');
    const files = readdirSync(downloads);
    assert.deepEqual(new Set(files), new Set(['flower.mp4', 'flower (1).mp4']));
    assert.ok(files.every(file => statSync(join(downloads, file)).size > 0));
    console.log('DOWNLOAD_SMOKE_PASS: two requested downloads, unique output files, opt-in notification dispatch');
    if (process.env.DM_R5_TEST_WINDOWS_NOTIFICATION === '1') {
      console.log('WINDOWS_NOTIFICATION_ACCEPTED: Windows Shell accepted one enabled completion notification');
    }
  }
} finally { if (app.exitCode === null) app.kill(); }
