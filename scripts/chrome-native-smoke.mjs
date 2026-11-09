import assert from 'node:assert/strict';
import { spawn, spawnSync } from 'node:child_process';
import { copyFileSync, existsSync, mkdirSync, mkdtempSync, realpathSync, renameSync, unlinkSync, writeFileSync } from 'node:fs';
import { basename, dirname, isAbsolute, join, relative, resolve } from 'node:path';

const hostKey = String.raw`Software\Google\Chrome\NativeMessagingHosts\com.browser_video_downloader.host`;
const views = ['/reg:32', '/reg:64'];
const registry = (command, hive, ...args) => spawnSync('reg.exe', [command, hive + '\\' + hostKey, ...args],
  { encoding: 'utf8', windowsHide: true, timeout: 5000 });
const query = (hive, view) => registry('query', hive, view);
const hasValues = result => result.status === 0 && /\bREG_[A-Z0-9_]+\b/u.test(result.stdout);
const owned = (result, path) => hasValues(result) && result.stdout.split(/\r?\n/u).some(line =>
  line.includes('REG_SZ') && line.trimEnd().endsWith(path));
const pause = ms => new Promise(resolve => setTimeout(resolve, ms));

async function verifyInstall(stage, phase, { requestSelection, requestDiagnosis, waitForSettings }, candidateId) {
  const manifest = join(stage, 'native-host-manifest.json');
  const app = spawn(join(stage, 'VideoDownloader.exe'), [], { windowsHide: true, env: { ...process.env,
    WEBVIEW2_USER_DATA_FOLDER: join(dirname(stage), basename(stage) + '-webview'), DM_SMOKE_ROOT: stage, DM_SMOKE_PHASE: phase } });
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
    const result = await requestSelection(candidateId);
    assert.equal(result.code, 'ACK');
    assert.equal(result.ok, true);
    const diagnosis = await requestDiagnosis();
    assert.equal(diagnosis.code, 'ACK');
    assert.ok(diagnosis.verifiedAt);
    await waitForSettings();
    writeFileSync(join(stage, 'bridge-ack'), candidateId);
    assert.equal(await closed, 0);
    assert.match(stdout, /SMOKE_PASS:/u);
  } finally {
    if (app.exitCode === null && app.signalCode === null) { app.kill(); await closed; }
  }
}

export async function runChromeNativeSmoke(options) {
  const { root, extensionId, candidateId, refreshCandidate } = options;
  const appBinary = process.env.DM_NATIVE_APP;
  const hostBinary = process.env.DM_NATIVE_HOST;
  assert.ok(appBinary && hostBinary, 'Specify DM_NATIVE_APP and DM_NATIVE_HOST together.');
  const existingKeys = new Set(views.filter(view => query('HKCU', view).status === 0));
  for (const hive of ['HKCU', 'HKLM']) for (const view of views) {
    const result = query(hive, view);
    assert.ok(!result.error && !hasValues(result), 'Existing Native Host registration must be preserved.');
  }
  const testFolder = realpathSync(resolve(root, '.test-output'));
  const workspaceRelative = relative(realpathSync(root), testFolder);
  assert.ok(workspaceRelative && !workspaceRelative.startsWith('..') && !isAbsolute(workspaceRelative));
  const stage = mkdtempSync(join(testFolder, 'chrome-native-'));
  const moved = stage + ' moved';
  const manifestPaths = [stage, moved].map(folder => join(folder, 'native-host-manifest.json'));
  copyFileSync(appBinary, join(stage, 'VideoDownloader.exe'));
  copyFileSync(hostBinary, join(stage, 'VideoNativeHost.exe'));
  mkdirSync(join(stage, 'data'));
  writeFileSync(join(stage, 'data/settings.json'), JSON.stringify({ schemaVersion: 1, downloadDirectory: join(stage, 'Downloads'),
    quality: 'best', format: 'auto', filename: 'title', concurrentDownloads: 2, retryCount: 3, browserAutoDetect: false,
    browserExtensionId: extensionId, completionNotification: true }));
  try {
    await verifyInstall(stage, 'chrome-move-from', options, candidateId);
    assert.ok(owned(query('HKCU', views[0]), manifestPaths[0]));
    assert.ok(!existsSync(moved));
    assert.equal(realpathSync(stage), stage);
    const moveDeadline = Date.now() + 4000;
    while (true) {
      try { renameSync(stage, moved); break; }
      catch (error) {
        if (!['EPERM', 'EBUSY'].includes(error.code) || Date.now() >= moveDeadline) throw error;
        await pause(100);
      }
    }
    assert.ok(!existsSync(stage));
    unlinkSync(join(moved, 'bridge-ack'));
    const nextCandidate = await refreshCandidate();
    await verifyInstall(moved, 'chrome-move-to', options, nextCandidate);
    for (const view of views) assert.ok(!hasValues(query('HKCU', view)), 'Temporary registration was not removed.');
    console.log('CHROME_NATIVE_PASS: selection, app row, diagnostic ACK/disconnect, settings, moved-folder repair and repeated round trip');
  } finally {
    for (const view of views) {
      if (manifestPaths.some(manifest => owned(query('HKCU', view), manifest))) {
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
