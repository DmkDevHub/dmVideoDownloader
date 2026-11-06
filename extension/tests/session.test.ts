import { expect, test } from 'vitest';
import { CandidateStore, classifyMedia } from '../src/media';
const page = { url: 'https://example.org/watch?v=1', title: 'Public video' };
const observation = { tabId: 1, mediaUrl: 'https://cdn.example.org/movie.mp4?a=1&b=2', sourceType: 'mp4' as const };
test('tracking and query ordering deduplicate while format distinctions remain separate', () => {
  const store = new CandidateStore(); store.navigate(1, page, 'doc');
  const first = store.add(observation)!;
  expect(store.add({ ...observation, mediaUrl: 'https://cdn.example.org/movie.mp4?utm_source=x&b=2&a=1' })!.candidateId).toBe(first.candidateId);
  store.add({ ...observation, mediaUrl: 'https://cdn.example.org/movie.mp4?a=1&b=3' });
  expect(store.list(1)).toHaveLength(2);
});
test('segments never become rows even when the server calls them video/mp4', () => {
  for (const path of ['chunk-001.mp4', 'segment100.mp4', 'init.mp4', 'fragment_4.mp4', '1.m4s', '1.ts', 'a.m4a', 'movie.mp4?range=0-100', 'movie.mp4?sq=10']) {
    expect(classifyMedia(`https://cdn.example.org/${path}`, 'video/mp4'), path).toBeNull();
  }
  expect(classifyMedia('https://cdn.example.org/movie.mp4', 'video/mp4')).toBe('mp4');
});
test('document and frame identity reject previous documents, and navigation/removal clear rows', () => {
  const store = new CandidateStore(); store.navigate(1, page, 'doc-a');
  store.navigate(1, {}, 'iframe-a', 3);
  expect(store.accepts(1, 3, 'iframe-a')).toBe(true);
  expect(store.accepts(1, 3, 'doc-a')).toBe(false);
  store.add(observation);
  store.navigate(1, { url: 'https://example.org/next' }, 'doc-b');
  expect(store.accepts(1, 0, 'doc-a')).toBe(false);
  expect(store.accepts(1, 3, 'iframe-a')).toBe(false);
  expect(store.list(1)).toEqual([]);
  store.add(observation); store.remove(1);
  expect(store.list(1)).toEqual([]);
});
test('session restore validates data, preserves IDs and exposes copies rather than mutable rows', () => {
  const store = new CandidateStore(); store.navigate(1, page, 'doc-a');
  const original = store.add(observation)!;
  const snapshot = store.snapshot() as { tabs: { rows: unknown[] }[] };
  snapshot.tabs[0].rows.push({ ...original, mediaUrl: 'http://127.0.0.1/a.mp4' }, { ...original, mediaUrl: 'https://example.org/a.mp4?token=secret' });
  const restored = new CandidateStore(); restored.restore(snapshot);
  expect(restored.list(1)).toHaveLength(1);
  expect(restored.list(1)[0].candidateId).toBe(original.candidateId);
  restored.list(1)[0].mediaUrl = 'https://wrong.example.org/a.mp4';
  expect(restored.list(1)[0].mediaUrl).toBe(original.mediaUrl);
  expect(JSON.stringify(restored.snapshot())).not.toContain('secret');
});
test('candidate and tab bounds limit session size without dropping existing rows', () => {
  const store = new CandidateStore(); store.navigate(1, page, 'doc');
  for (let i = 0; i < 100; i++) store.add({ ...observation, mediaUrl: `https://example.org/video-${i}.mp4` });
  expect(store.list(1)).toHaveLength(CandidateStore.perTabLimit);
  for (let tabId = 2; tabId <= 15; tabId++) {
    store.navigate(tabId, page, `doc-${tabId}`);
    for (let i = 0; i < 30; i++) store.add({ ...observation, tabId, mediaUrl: `https://example.org/video-${i}.mp4` });
  }
  expect(store.tabIds().reduce((count, id) => count + store.list(id).length, 0)).toBe(CandidateStore.totalLimit);
  for (let tabId = 16; tabId <= 40; tabId++) store.navigate(tabId, page, `doc-${tabId}`);
  expect(store.tabIds()).toHaveLength(CandidateStore.tabLimit);
});
