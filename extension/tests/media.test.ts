import { expect, test } from 'vitest';
import { CandidateStore, classifyMedia, domObservation, networkObservation, publicUrl } from '../src/media';
test('only public HTTP URLs without recognizable embedded credentials enter candidates', () => {
  for (const url of ['file:///a.mp4', 'blob:https://example.org/a', 'data:video/mp4;base64,AAA',
    'https://user:password@example.org/a.mp4', 'http://localhost/a', 'http://127.1/a',
    'http://2130706433/a', 'http://0x7f000001/a', 'http://10.0.0.1/a', 'http://172.16.0.1/a',
    'http://192.168.1.1/a', 'http://169.254.169.254/a', 'http://100.64.0.1/a', 'http://[::1]/a',
    'http://[::ffff:127.0.0.1]/a', 'https://example.org/a?access_token=secret',
    'https://example.org/a?X-Amz-Signature=secret', 'https://example.org/a?auth=secret']) {
    expect(publicUrl(url), url).toBeNull();
  }
  expect(publicUrl('https://cdn.example.org/a.mp4?v=1#fragment')).toBe('https://cdn.example.org/a.mp4?v=1');
});
test('direct video classification uses extension or allowlisted MIME and rejects HTML errors', () => {
  expect(classifyMedia('https://example.org/VIDEO.MP4?quality=1')).toBe('mp4');
  expect(classifyMedia('https://example.org/file', 'Video/WebM; charset=utf-8')).toBe('webm');
  expect(classifyMedia('https://example.org/a.mp4', 'text/html')).toBeNull();
  expect(classifyMedia('https://example.org/a.jpg', 'image/jpeg')).toBeNull();
});
test('network projection retains only media metadata, ignoring non-GET, errors and background requests', () => {
  const details = { tabId: 4, url: 'https://example.org/a.mp4', method: 'GET', statusCode: 206,
    type: 'media', responseHeaders: [{ name: 'Content-Type', value: 'video/mp4' },
      { name: 'Set-Cookie', value: 'DO_NOT_RETAIN' }, { name: 'Authorization', value: 'DO_NOT_RETAIN' }] };
  expect(networkObservation(details)).toEqual({ tabId: 4, mediaUrl: details.url, sourceType: 'mp4' });
  for (const change of [{ method: 'POST' }, { tabId: -1 }, { statusCode: 403 }, { type: 'script' }]) {
    expect(networkObservation({ ...details, ...change })).toBeNull();
  }
  const store = new CandidateStore(); store.navigate(4, { url: 'https://example.org/watch', title: 'Sample' }, 'doc-4'); store.add(networkObservation(details)!);
  expect(store.list(4)[0]).toMatchObject({ tabId: 4, status: 'detected', sourceType: 'mp4' });
  expect(JSON.stringify(store.list(4))).not.toContain('DO_NOT_RETAIN');
  expect(store.list(9)).toEqual([]);
});
test('HTML5 source projection and network evidence merge without changing candidate identity', () => {
  const store = new CandidateStore();
  const page = { url: 'https://example.org/watch' };
  store.navigate(3, page, 'doc-3');
  const source = domObservation(3, { mediaUrl: 'https://cdn.example.org/a.webm', mime: 'video/webm', unexpected: 'discard' })!;
  const first = store.add(source)!;
  const second = store.add({ ...source, sourceType: 'webm' })!;
  expect(second.candidateId).toBe(first.candidateId);
  expect(store.list(3)).toHaveLength(1);
  expect(domObservation(3, { mediaUrl: 'blob:https://example.org/id' })).toBeNull();
  expect(domObservation(3, { mediaUrl: 'https://example.org/a.mp4?token=x' })).toBeNull();
  expect(domObservation(3, { mediaUrl: 'https://example.org/a.mp4', mime: {} })).toBeNull();
  expect(domObservation(-1, source)).toBeNull();
});
test('HLS and DASH classify by playlist path or normalized MIME, never by query text', () => {
  for (const mime of ['Application/Vnd.Apple.Mpegurl; charset=utf-8', 'application/x-mpegurl', 'audio/mpegurl', 'audio/x-mpegurl']) {
    expect(classifyMedia('https://example.org/play', mime)).toBe('hls');
  }
  expect(classifyMedia('https://example.org/manifest', 'application/dash+xml')).toBe('dash');
  expect(classifyMedia('https://example.org/MASTER.M3U8?v=1', 'application/octet-stream')).toBe('hls');
  expect(classifyMedia('https://example.org/stream.MPD')).toBe('dash');
  expect(classifyMedia('https://example.org/page?next=master.m3u8')).toBeNull();
  expect(classifyMedia('https://example.org/master.m3u8', 'text/html')).toBeNull();
  expect(domObservation(2, { mediaUrl: 'https://example.org/master.m3u8' })?.sourceType).toBe('hls');
});
