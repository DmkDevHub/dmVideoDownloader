import { spawnSync } from 'node:child_process';
import { fileURLToPath } from 'node:url';
import { resolve } from 'node:path';
import { mkdirSync, mkdtempSync, readFileSync, writeFileSync, readdirSync } from 'node:fs';
import assert from 'node:assert/strict';

if (process.platform !== 'win32') {
  throw new Error('Native WebView smoke validation requires Windows.');
}
const root = fileURLToPath(new URL('../', import.meta.url));
const desktop = resolve(root, 'apps/desktop');
// Run the JS CLI entry point directly; no shell command interpolation.
const cli = resolve(root, 'node_modules/@tauri-apps/cli/tauri.js');
const build = spawnSync(process.execPath, [cli, 'build', '--debug', '--no-bundle',
  '--features', 'smoke-test', '--config', 'src-tauri/tauri.smoke.conf.json'],
{ cwd: desktop, stdio: 'inherit' });
if (build.error) throw build.error;
if (build.status !== 0) process.exit(build.status ?? 1);

mkdirSync(resolve(root, '.test-output'), { recursive: true });
const testRoot = mkdtempSync(resolve(root, '.test-output/smoke-'));
const settingsPath = resolve(testRoot, 'data/settings.json');
const corrupt = '{broken-json';
const future = '{"schemaVersion":99,"futureField":true}';
for (const phase of ['write', 'restart', 'corrupt', 'future']) {
  if (phase === 'corrupt') writeFileSync(settingsPath, corrupt);
  if (phase === 'future') writeFileSync(settingsPath, future);
  const result = spawnSync(resolve(desktop, 'src-tauri/target/debug/dm-video-desktop.exe'), [], {
    cwd: root, encoding: 'utf8', timeout: 45_000, windowsHide: true,
    env: { ...process.env, DM_SMOKE_ROOT: testRoot, DM_SMOKE_PHASE: phase },
  });
  if (result.error) throw result.error;
  if (result.status !== 0 || !result.stdout.includes('SMOKE_PASS:')) {
    console.error('Native smoke validation failed.', phase, result.status, result.stderr);
    process.exit(1);
  }
  if (phase === 'restart') assert.equal(JSON.parse(readFileSync(settingsPath, 'utf8')).quality, '1080');
  if (phase === 'corrupt') {
    const backup = readdirSync(resolve(testRoot, 'data')).find(name => name.startsWith('settings.backup-'));
    assert.ok(backup);
    assert.equal(readFileSync(resolve(testRoot, 'data', backup), 'utf8'), corrupt);
  }
  if (phase === 'future') assert.equal(readFileSync(settingsPath, 'utf8'), future);
  console.log(`${phase}: ${result.stdout.trim()}`);
}
