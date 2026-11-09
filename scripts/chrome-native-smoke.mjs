import assert from 'node:assert/strict';
import { spawn, spawnSync } from 'node:child_process';
import { copyFileSync, existsSync, mkdirSync, mkdtempSync, writeFileSync } from 'node:fs';
import { join, resolve } from 'node:path';

const hostKey = String.raw`Software\Google\Chrome\NativeMessagingHosts\com.browser_video_downloader.host`;
const views = ['/reg:32', '/reg:64'];
const registry = (command, hive, ...args) => spawnSync('reg.exe', [command, hive + '\\' + hostKey, ...args],
  { encoding: 'utf8', windowsHide: true, timeout: 5000 });
const query = (hive, view) => registry('query', hive, view);
const hasDefault = result => result.status === 0 && /\bREG_[A-Z0-9_]+\b/u.test(result.stdout);
const owned = (result, path) => hasDefault(result) && result.stdout.split(/\r?\n/u).some(line =>
  line.includes('REG_SZ') && line.trimEnd().endsWith(path));
const pause = ms => new Promise(resolve => setTimeout(resolve, ms));

export async function runChromeNativeSmoke({ root, extensionId, requestSelection, requestDiagnosis, waitForSettings, candidateId }) {
  const appBinary = process.env.DM_NATIVE_APP;
  const hostBinary = process.env.DM_NATIVE_HOST;
  assert.ok(appBinary && hostBinary, 'Specify DM_NATIVE_APP and DM_NATIVE_HOST together.');
  const existingKeys = new Set(views.filter(view => query('HKCU', view).status === 0));
  for (const hive of ['HKCU', 'HKLM']) for (const view of views) {
    const result = query(hive, view);
    assert.ok(!result.error && !hasDefault(result), 'Existing Native Host registration must be preserved.');
  }
  const stage = mkdtempSync(resolve(root, '.test-output/chrome-native-'));
  const desktop = join(stage, 'VideoDownloader.exe');
  const manifest = join(stage, 'native-host-manifest.json');
  copyFileSync(appBinary, desktop);
  copyFileSync(hostBinary, join(stage, 'VideoNativeHost.exe'));
  mkdirSync(join(stage, 'data'));
  writeFileSync(join(stage, 'data/settings.json'), JSON.stringify({ schemaVersion: 1, downloadDirectory: join(stage, 'Downloads'),
    quality: 'best', format: 'auto', filename: 'title', concurrentDownloads: 2, retryCount: 3, browserAutoDetect: false,
    browserExtensionId: extensionId, completionNotification: true }));
  const app = spawn(desktop, [], { windowsHide: true, env: { ...process.env, DM_SMOKE_ROOT: stage, DM_SMOKE_PHASE: 'chrome-bridge' } });
  let stdout = '';
  app.stdout.setEncoding('utf8').on('data', chunk => { stdout += chunk; });
  app.stderr.resume();
  let exited;
  const closed = new Promise(resolve => {
    app.once('error', () => resolve(-1));
    app.once('close', code => { exited = code; resolve(code); });
  });
  try {
    const deadline = Date.now() + 9000;
    while (Date.now() < deadline && exited === undefined && !owned(query('HKCU', views[0]), manifest)) await pause(80);
    assert.ok(existsSync(manifest) && owned(query('HKCU', views[0]), manifest), 'Desktop did not register the isolated Host.');
    const result = await requestSelection();
    assert.equal(result.code, 'ACK');
    assert.equal(result.ok, true);
    const diagnosis = await requestDiagnosis();
    assert.equal(diagnosis.code, 'ACK');
    assert.ok(diagnosis.verifiedAt);
    await waitForSettings();
    writeFileSync(join(stage, 'bridge-ack'), candidateId);
    assert.equal(await closed, 0);
    assert.match(stdout, /SMOKE_PASS:/u);
    for (const view of views) assert.ok(!hasDefault(query('HKCU', view)), 'Temporary registration was not removed.');
    console.log('CHROME_NATIVE_PASS: selection, candidate row, diagnostic ACK/disconnect, and runtime settings');
  } finally {
    if (app.exitCode === null) { app.kill(); await closed; }
    for (const view of views) {
      if (owned(query('HKCU', view), manifest)) {
        const result = registry('delete', 'HKCU', '/ve', '/f', view);
        assert.equal(result.status, 0, 'Failed to remove the isolated test registration.');
      }
      const empty = query('HKCU', view);
      const entries = empty.stdout?.split(/\r?\n/u).map(line => line.trim()).filter(Boolean) ?? [];
      if (!existingKeys.has(view) && empty.status === 0 && entries.every(line => line.toLowerCase() === ('HKEY_CURRENT_USER\\' + hostKey).toLowerCase())) {
        assert.equal(registry('delete', 'HKCU', '/f', view).status, 0, 'Failed to remove the empty test key.');
      }
    }
  }
}
