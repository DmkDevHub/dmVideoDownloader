import { createSSRApp } from 'vue';
import { renderToString } from 'vue/server-renderer';
import { expect, test } from 'vitest';
import BrowserConnection from './BrowserConnection.vue';
import { connectionLabel, validExtensionId, type BrowserRuntime } from './browserConnection';

test('extension IDs require precisely the lowercase Chrome ID alphabet', () => {
  expect(validExtensionId('abcdefghijklmnopabcdefghijklmnop')).toBe(true);
  for (const invalid of ['', '*', 'A'.repeat(32), 'q'.repeat(32), 'a'.repeat(31), 'a'.repeat(33)]) {
    expect(validExtensionId(invalid)).toBe(false);
  }
});

test('registration or past success cannot independently mark a current connection as connected', () => {
  expect(connectionLabel({ phase: 'connected', roundTripConfirmed: false, lastSuccessAt: '2026-11-05T00:00:00Z' })).toBe('확인 필요');
  expect(connectionLabel({ phase: 'disconnected', roundTripConfirmed: true, lastSuccessAt: '2026-11-05T00:00:00Z' })).toBe('연결 끊김');
});

test('unimplemented registration actions remain disabled even with valid ID and resources', async () => {
  const runtime: BrowserRuntime = { extensionPath: 'D:\\App\\extension', extensionResourcesAvailable: true,
    hostAvailable: true, chromeAvailable: true, connectionActionsAvailable: false };
  const html = await renderToString(createSSRApp(BrowserConnection, {
    extensionId: 'a'.repeat(32), savedId: '', canSave: true, busy: false, runtime,
  }));
  for (const name of ['호스트 등록', '연결 테스트', '연결 복구/재등록', '연결 해제']) {
    const button = [...html.matchAll(/<button\b([^>]*)>([^<]*)<\/button>/g)].find(match => match[2] === name);
    expect(button?.[1]).toContain('disabled');
  }
  expect(html).toContain('<strong>확인 필요</strong>');
  expect(html).toContain('ID 변경');
  expect(html).not.toContain('<dialog');
  expect(html).toContain('개발자 모드를 직접');
  expect(html).toContain('압축해제된 확장 프로그램');
  expect(html).toContain('chrome://extensions');
  expect(html).toContain('30초');
});
