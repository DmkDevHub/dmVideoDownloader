import { expect, test } from 'vitest';
import { CandidateStore } from '../src/media';
import { candidateLabel } from '../src/popupView';
test('popup labels show site/type without media URL queries or HTML interpretation', () => {
  const store = new CandidateStore();
  store.navigate(1, { url: 'https://example.org/watch?v=private-query', title: '<img onerror=alert(1)> https://example.org/watch?token=DO_NOT_SHOW' }, 'doc');
  const row = store.add({ tabId: 1, mediaUrl: 'https://cdn.example.org/movie.mp4?quality=1080', sourceType: 'mp4' })!;
  const label = candidateLabel(row);
  expect(label).toEqual({ title: '<img onerror=alert(1)> example.org', site: 'example.org', type: 'MP4' });
  expect(JSON.stringify(label)).not.toContain('private-query');
  expect(JSON.stringify(label)).not.toContain('quality=');
  expect(JSON.stringify(store.snapshot())).not.toContain('DO_NOT_SHOW');
});
