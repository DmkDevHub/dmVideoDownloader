import { expect, test } from 'vitest';
import config from '../src-tauri/tauri.conf.json';

test('the scaffold grants no native capabilities or remote webview access', () => {
  expect(config.app.security.capabilities).toEqual([]);
  expect(config.app.windows).toHaveLength(1);
  expect(config.app.windows[0].label).toBe('main');
  expect(config.app.windows[0]).not.toHaveProperty('url');
});

test('the production content policy permits IPC without public network access or script evaluation', () => {
  const directives = new Map(config.app.security.csp.split(';').map((entry) => {
    const [name, ...sources] = entry.trim().split(/\s+/);
    return [name, sources];
  }));

  expect(directives.get('default-src')).toEqual(["'self'"]);
  expect(directives.get('connect-src')).toEqual(['ipc:', 'http://ipc.localhost']);
  expect(directives.get('script-src')).toEqual(["'self'"]);
});
