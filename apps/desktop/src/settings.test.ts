import { expect, test } from 'vitest';
import { createSettingsEditor, defaultSettings } from './settings';
import { createSSRApp } from 'vue';
import { renderToString } from 'vue/server-renderer';
import SettingsDialog from './SettingsDialog.vue';

test('one settings dialog contains all specified general options', async () => {
  const html = await renderToString(createSSRApp(SettingsDialog, {
    draft: defaultSettings('D:\\Videos'), busy: false, error: '',
  }));
  expect(html.match(/<dialog\b/g)).toHaveLength(1);
  for (const label of ['다운로드 저장 위치', '기본 화질', '저장 형식', '파일 이름', '동시 다운로드', '실패 재시도', '자동 감지', '완료 알림', '기본값 복원', '취소']) {
    expect(html).toContain(label);
  }
});

test('cancel discards changes and a repeated open preserves the active draft', () => {
  const saved = { ...defaultSettings('D:\\Videos'), quality: '720' as const };
  const editor = createSettingsEditor(saved, defaultSettings('C:\\Downloads\\Videos'));
  editor.open();
  editor.state.draft.quality = '1080';
  editor.open();
  expect(editor.state.draft.quality).toBe('1080');
  editor.cancel();
  editor.open();
  expect(editor.state.draft).toEqual(saved);
  expect(saved.quality).toBe('720');
});

test('reset is a draft operation and cancel keeps the saved settings', () => {
  const saved = { ...defaultSettings('D:\\Videos'), browserAutoDetect: false };
  const defaults = defaultSettings('C:\\Downloads\\Videos');
  const editor = createSettingsEditor(saved, defaults);
  editor.open();
  editor.reset();
  expect(editor.state.draft).toEqual(defaults);
  expect(editor.state.current).toEqual(saved);
  editor.cancel();
  expect(editor.state.current).toEqual(saved);
});

test('picker cancellation and failure preserve the draft and release busy state', async () => {
  const editor = createSettingsEditor(defaultSettings('D:\\Videos'), defaultSettings('D:\\Videos'));
  editor.open();
  await editor.browse(async () => null);
  expect(editor.state.draft.downloadDirectory).toBe('D:\\Videos');
  await editor.browse(async () => { throw new Error('denied'); });
  expect(editor.state.draft.downloadDirectory).toBe('D:\\Videos');
  expect(editor.state.error).not.toBe('');
  expect(editor.state.busy).toBe(false);
  await editor.browse(async () => 'E:\\Videos');
  expect(editor.state.draft.downloadDirectory).toBe('E:\\Videos');
  expect(editor.state.current.downloadDirectory).toBe('D:\\Videos');
});

test('failed save leaves the dialog and applied values intact; success applies a copy', async () => {
  const editor = createSettingsEditor(defaultSettings('D:\\Videos'), defaultSettings('D:\\Videos'));
  editor.open();
  editor.state.draft.quality = '720';
  await editor.save(async () => { throw 'SETTINGS_CHANGED'; });
  expect(editor.state.open).toBe(true);
  expect(editor.state.current.quality).toBe('best');
  expect(editor.state.draft.quality).toBe('720');
  expect(editor.state.error).toContain('다른 앱');
  await editor.save(async (draft) => draft);
  expect(editor.state.open).toBe(false);
  expect(editor.state.current.quality).toBe('720');
  editor.open();
  expect(editor.state.draft.quality).toBe('720');
});
