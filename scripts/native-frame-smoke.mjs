import assert from 'node:assert/strict';
import { spawnSync } from 'node:child_process';
import { copyFileSync, mkdirSync, mkdtempSync, writeFileSync } from 'node:fs';
import { endianness } from 'node:os';
import { join, resolve } from 'node:path';

const binary = process.argv[2];
assert.ok(binary, 'Supply the built VideoNativeHost.exe.');
mkdirSync('.test-output', { recursive: true });
const root = mkdtempSync(resolve('.test-output/native-frame-'));
const host = join(root, 'VideoNativeHost.exe');
copyFileSync(binary, host);
const origin = character => 'chrome-extension://' + character.repeat(32) + '/';
const order = endianness() === 'LE' ? 'LE' : 'BE';
function encode(value) {
  const bytes = Buffer.from(JSON.stringify(value), 'utf8');
  const prefix = Buffer.alloc(4);
  prefix['writeUInt32' + order](bytes.length);
  return Buffer.concat([prefix, bytes]);
}
function invoke(input, extension = 'a') {
  return spawnSync(host, [origin(extension)], { input, timeout: 5000, windowsHide: true });
}
function replies(input, extension) {
  const result = invoke(input, extension);
  assert.equal(result.status, 0);
  assert.equal(result.stderr.length, 0);
  const messages = [];
  let output = result.stdout;
  while (output.length) {
    const len = output['readUInt32' + order]();
    assert.ok(len > 0 && len <= output.length - 4);
    messages.push(JSON.parse(output.subarray(4, len + 4).toString('utf8')));
    output = output.subarray(len + 4);
  }
  return messages;
}
const ping = requestId => ({ version: 1, type: 'PING', requestId, payload: { purpose: 'connection_test' } });
assert.equal(replies(encode(ping('missing')))[0].payload.code, 'HOST_NOT_REGISTERED');
writeFileSync(join(root, 'native-host-manifest.json'), JSON.stringify({ name: 'com.browser_video_downloader.host',
  description: 'Browser Video Downloader bridge', path: host, type: 'stdio', allowed_origins: [origin('a')] }));
assert.equal(replies(encode(ping('wrong-id')), 'b')[0].payload.code, 'EXTENSION_ID_MISMATCH');
const result = replies(Buffer.concat([encode(ping('first')), encode({ ...ping('second'), version: 99 })]));
assert.deepEqual(result.map(reply => [reply.requestId, reply.payload.code]), [['first', 'APP_NOT_RUNNING'], ['second', 'UNSUPPORTED_VERSION']]);
const malformed = invoke(Buffer.alloc(4));
assert.equal(malformed.status, 1);
assert.equal(malformed.stdout.length, 0);
console.log('NATIVE_FRAME_PASS: consecutive frames, missing registration, wrong ID, app closed, version error and stdout isolation');
