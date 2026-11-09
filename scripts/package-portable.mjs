import assert from 'node:assert/strict';
import { spawnSync } from 'node:child_process';
import { createHash } from 'node:crypto';
import { copyFileSync, mkdirSync, mkdtempSync, readFileSync, realpathSync, statSync, writeFileSync } from 'node:fs';
import { isAbsolute, join, relative, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';

assert.equal(process.platform, 'win32', 'Portable packaging requires Windows.');
const [toolRoot, ...rest] = process.argv.slice(2);
assert.ok(toolRoot && !rest.length, 'Usage: npm run package:portable -- <verified-tool-root>');
const npm = process.env.npm_execpath;
assert.ok(npm, 'Run this script through npm run package:portable.');
const kit = realpathSync(resolve(toolRoot));
const root = realpathSync(fileURLToPath(new URL('../', import.meta.url)));
const kitFiles = {
  'tools/yt-dlp.exe': 'f5389d3698ecde4ef06f4e6b1ce0294fe9ac1396228c8990b8d3416d6d177a7d',
  'tools/ffmpeg.exe': '3256173f3f8bffd7df12227c68adf68025edb1832273a9530688a7bb1ed8edec',
  'notices/yt-dlp-LICENSE.txt': '7e12e5df4bae12cb21581ba157ced20e1986a0508dd10d0e8a4ab9a4cf94e85c',
  'notices/yt-dlp-THIRD_PARTY_LICENSES.txt': '472aefe951c7db35e1657c1d13fd337140511ed6f2b329205105ad441c5a02b7',
  'notices/ffmpeg-LICENSE.txt': '8ceb4b9ee5adedde47b31e975c1d90c73ad27b6b165a1dcd80c7c545eb65b903',
};
const sha256 = path => createHash('sha256').update(readFileSync(path)).digest('hex');
function checkKit(base) {
  for (const [file, expected] of Object.entries(kitFiles)) {
    assert.ok(statSync(join(base, file)).isFile(), 'Missing tool input ' + file);
    assert.equal(sha256(join(base, file)), expected, 'Unverified tool input ' + file);
  }
}
function command(program, args) {
  const result = spawnSync(program, args, { cwd: root, stdio: 'inherit', windowsHide: true });
  if (result.error) throw result.error;
  assert.equal(result.status, 0, 'Build command failed ' + program);
}
function requireX64(file) {
  const bytes = readFileSync(file);
  assert.equal(bytes.toString('ascii', 0, 2), 'MZ');
  const header = bytes.readUInt32LE(0x3c);
  assert.equal(bytes.readUInt32LE(header), 0x4550);
  assert.equal(bytes.readUInt16LE(header + 4), 0x8664, 'Expected Windows x64 executable');
}

checkKit(kit);
command(process.execPath, [npm, 'run', 'build']);
command('cargo', ['build', '--release', '--locked', '--manifest-path', 'native-host/Cargo.toml',
  '--target-dir', 'shared/protocol/target']);
command(process.execPath, [npm, 'run', 'build:extension']);

mkdirSync(join(root, 'dist'), { recursive: true });
const dist = realpathSync(join(root, 'dist'));
const inside = relative(root, dist);
assert.ok(inside && !inside.startsWith('..') && !isAbsolute(inside), 'Output must stay inside the project.');
const output = mkdtempSync(join(dist, 'portable-'));
const folder = 'VideoDownloaderPortable';
const stage = join(output, folder);
for (const name of ['tools', 'extension', 'notices']) mkdirSync(join(stage, name), { recursive: true });
const files = {
  'VideoDownloader.exe': join(root, 'apps/desktop/src-tauri/target/release/dm-video-desktop.exe'),
  'VideoNativeHost.exe': join(root, 'shared/protocol/target/release/VideoNativeHost.exe'),
  'README.txt': join(root, 'docs/PORTABLE_README.txt'),
};
for (const file of ['manifest.json', 'background.js', 'content.js', 'popup.html', 'popup.js', 'popup.css']) {
  files['extension/' + file] = join(root, 'extension/dist', file);
}
for (const file of Object.keys(kitFiles)) files[file] = join(kit, file);
for (const [file, source] of Object.entries(files)) {
  if (file.endsWith('.exe')) requireX64(source);
  copyFileSync(source, join(stage, file));
}
checkKit(stage);
const sources = 'notices/SOURCES.txt';
writeFileSync(join(stage, sources), [
  'yt-dlp Windows x64 2026.10.26',
  'https://github.com/yt-dlp/yt-dlp/releases/tag/2026.10.26',
  'https://github.com/yt-dlp/yt-dlp/tree/f264a6cbd1f630894da0f368df00f8aae50140e0',
  '',
  'FFmpeg 9.0.2 Gyan Windows x64 essentials',
  'https://www.gyan.dev/ffmpeg/builds/',
  'https://github.com/FFmpeg/FFmpeg/tree/n9.0.2',
  'Vendor archive SHA-256 60f467265b1e312373dbcd92200c2618a74850f98d3d078e94296bb3fa2047ba',
  '',
].join('\r\n'));
const payload = [...Object.keys(files), sources].sort();
writeFileSync(join(stage, 'SHA256SUMS.txt'), payload.map(file => sha256(join(stage, file)) + '  ' + file).join('\r\n') + '\r\n');
const zip = join(output, 'VideoDownloaderPortable-win-x64.zip');
command(join(process.env.SystemRoot, 'System32', 'tar.exe'), ['-a', '-cf', zip, '-C', output, folder]);
writeFileSync(zip + '.sha256', sha256(zip) + '  VideoDownloaderPortable-win-x64.zip\r\n');
console.log('PORTABLE_PACKAGE=' + zip);
