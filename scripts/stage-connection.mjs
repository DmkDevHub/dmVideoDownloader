import assert from 'node:assert/strict';
import { copyFileSync, mkdirSync, mkdtempSync, realpathSync } from 'node:fs';
import { isAbsolute, join, relative } from 'node:path';
import { fileURLToPath } from 'node:url';

const root = realpathSync(fileURLToPath(new URL('../', import.meta.url)));
mkdirSync(join(root, 'dist'), { recursive: true });
const destination = realpathSync(join(root, 'dist'));
const within = relative(root, destination);
assert.ok(within && !within.startsWith('..') && !isAbsolute(within), 'Build destination must remain inside the project.');
const stage = mkdtempSync(join(destination, 'connection-'));
copyFileSync(join(root, 'apps/desktop/src-tauri/target/debug/dm-video-desktop.exe'), join(stage, 'VideoDownloader.exe'));
copyFileSync(join(root, 'shared/protocol/target/debug/VideoNativeHost.exe'), join(stage, 'VideoNativeHost.exe'));
mkdirSync(join(stage, 'extension'));
for (const file of ['manifest.json', 'background.js', 'content.js', 'popup.html', 'popup.js', 'popup.css']) {
  copyFileSync(join(root, 'extension/dist', file), join(stage, 'extension', file));
}
console.log(stage);
