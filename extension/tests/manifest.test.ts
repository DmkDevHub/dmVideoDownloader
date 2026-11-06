import { readFileSync } from 'node:fs';
import { expect, test } from 'vitest';
test('MV3 has isolated scripts and no unnecessary privileged APIs', () => {
  const manifest = JSON.parse(readFileSync(new URL('../manifest.json', import.meta.url), 'utf8'));
  expect(manifest.manifest_version).toBe(3);
  expect(manifest.background).toEqual({ service_worker: 'background.js', type: 'module' });
  expect(manifest.action.default_popup).toBe('popup.html');
  expect(manifest.content_scripts[0].matches).toEqual(['http://*/*', 'https://*/*']);
  for (const permission of ['cookies', 'debugger', 'downloads', 'tabs', 'webRequestBlocking', 'scripting']) {
    expect(manifest.permissions ?? []).not.toContain(permission);
  }
  expect(manifest.externally_connectable).toBeUndefined();
  expect(manifest.web_accessible_resources).toBeUndefined();
  expect(manifest.content_security_policy.extension_pages).toBe("script-src 'self'; object-src 'none'");
});
