import assert from 'node:assert/strict';
import { spawnSync } from 'node:child_process';
import { endianness } from 'node:os';
const host = process.argv[2];
assert.ok(host, 'Supply the built VideoNativeHost.exe');
const encode = value => {
  const bytes = Buffer.from(JSON.stringify(value), 'utf8');
  const prefix = Buffer.alloc(4);
  prefix[endianness() === 'LE' ? 'writeUInt32LE' : 'writeUInt32BE'](bytes.length);
  return Buffer.concat([prefix, bytes]);
};
const ping = requestId => ({ version: 1, type: 'PING', requestId, payload: { purpose: 'connection_test' } });
const processResult = spawnSync(host, [], {
  input: Buffer.concat([encode(ping('first')), encode({ ...ping('second'), version: 99 })]),
  timeout: 5000, windowsHide: true,
});
assert.equal(processResult.status, 0);
assert.equal(processResult.stderr.length, 0);
let output = processResult.stdout;
for (const [id, code] of [['first', 'APP_NOT_RUNNING'], ['second', 'UNSUPPORTED_VERSION']]) {
  const len = output[endianness() === 'LE' ? 'readUInt32LE' : 'readUInt32BE']();
  const reply = JSON.parse(output.subarray(4, len + 4).toString('utf8'));
  assert.equal(reply.requestId, id);
  assert.equal(reply.payload.code, code);
  output = output.subarray(len + 4);
}
assert.equal(output.length, 0);
const malformed = spawnSync(host, [], { input: Buffer.alloc(4), timeout: 5000, windowsHide: true });
assert.equal(malformed.status, 1);
assert.equal(malformed.stdout.length, 0);
console.log('NATIVE_FRAME_PASS: Windows EXE, consecutive frames, correlated errors and stdout isolation');
