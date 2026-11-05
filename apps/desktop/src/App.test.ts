import { createSSRApp } from 'vue';
import { renderToString } from 'vue/server-renderer';
import { expect, test, vi } from 'vitest';
import App from './App.vue';

test('the startup component renders without browser APIs or network requests', async () => {
  const fetch = vi.fn(() => { throw new Error('Unexpected network request'); });
  vi.stubGlobal('fetch', fetch);

  const html = await renderToString(createSSRApp(App));

  expect(html).toMatch(/<main\b[^>]*data-app-ready/);
  expect(html).toMatch(/<h1>[^<]+<\/h1>/);
  expect(fetch).not.toHaveBeenCalled();
  expect(html).toContain('영상 URL 직접 입력');
  expect(html).toContain('id="candidates-heading"');
  expect(html).toContain('id="jobs-heading"');
  expect(html).toContain('Chrome 연동: 확인 필요');
  expect(html).not.toContain('Chrome 연동: 연결됨');
});
