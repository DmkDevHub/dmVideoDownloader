import assert from 'node:assert/strict';
import { spawnSync } from 'node:child_process';
import { createHash } from 'node:crypto';
import { cpSync, mkdirSync, mkdtempSync, readFileSync, realpathSync, renameSync } from 'node:fs';
import { dirname, join, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';

const root = realpathSync(fileURLToPath(new URL('../', import.meta.url)));
const [portable] = process.argv.slice(2);
assert.ok(portable, 'Supply the verified portable folder.');
assert.ok(process.env.DM_NATIVE_APP, 'Supply the hidden desktop test build.');
const release = realpathSync(resolve(portable));
const testFolder = join(root, '.test-output');
mkdirSync(testFolder, { recursive: true });
assert.equal(dirname(realpathSync(testFolder)), root);
const isolated = mkdtempSync(join(testFolder, 'portable-extension-'));
const first = join(isolated, 'extension');
const second = join(isolated, '옮긴 확장 폴더');
cpSync(join(release, 'extension'), first, { recursive: true, errorOnExist: true, force: false });
const manifest = JSON.parse(readFileSync(join(first, 'manifest.json'), 'utf8'));
assert.equal(manifest.key, undefined);
assert.equal(manifest.update_url, undefined);

function policySnapshot() {
  return ['HKCU', 'HKLM'].flatMap(hive => ['/reg:32', '/reg:64'].map(view => {
    const result = spawnSync('reg.exe', ['query', hive + '\\Software\\Policies\\Google\\Chrome', '/s', view],
      { windowsHide: true, timeout: 5000 });
    if (result.error) throw result.error;
    return createHash('sha256').update(result.stdout).update(result.stderr).digest('hex') + ':' + result.status;
  }));
}
function verify(directory, previous = '') {
  const result = spawnSync(process.execPath, [join(root, 'scripts/extension-smoke.mjs')], {
    cwd: root, encoding: 'utf8', windowsHide: true, timeout: 90_000,
    env: { ...process.env, DM_EXTENSION_DIR: directory,
      DM_NATIVE_HOST: join(release, 'VideoNativeHost.exe'), DM_PREVIOUS_EXTENSION_ID: previous },
  });
  if (result.error) throw result.error;
  assert.equal(result.status, 0, result.stdout + result.stderr);
  assert.match(result.stdout, /CHROME_NATIVE_PASS/u);
  if (previous) assert.match(result.stdout, /OLD_ALLOWLIST_DENIED/u);
  const id = result.stdout.match(/^EXTENSION_ID=([a-p]{32})$/mu)?.[1];
  assert.ok(id, 'Chrome did not report the unpacked ID.');
  process.stdout.write(result.stdout);
  return id;
}

const before = policySnapshot();
try {
  const firstId = verify(first);
  assert.equal(dirname(realpathSync(first)), isolated);
  assert.equal(dirname(second), isolated);
  renameSync(first, second);
  const secondId = verify(second, firstId);
  assert.notEqual(firstId, secondId);
} finally {
  assert.deepEqual(policySnapshot(), before, 'Chrome policy state changed.');
}
console.log('PORTABLE_EXTENSION_MOVE_PASS: new profiles, moved unpacked ID, exact re-registration, unchanged Chrome policies');
