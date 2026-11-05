import { spawnSync } from 'node:child_process';
import { fileURLToPath } from 'node:url';
import { resolve } from 'node:path';
import { mkdirSync, mkdtempSync } from 'node:fs';

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
const result = spawnSync(resolve(desktop, 'src-tauri/target/debug/dm-video-desktop.exe'), [], {
  cwd: root, encoding: 'utf8', timeout: 45_000, windowsHide: true,
  env: { ...process.env, DM_SMOKE_ROOT: testRoot },
});
if (result.error) throw result.error;
if (result.status !== 0 || !result.stdout.includes('SMOKE_PASS:')) {
  console.error('Native smoke validation failed.', result.status, result.stderr);
  process.exit(1);
}
console.log(result.stdout.trim());
